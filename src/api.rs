use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Query as AxumQuery, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use parking_lot::RwLock;
use sysinfo::{Pid, System};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::indexer::Index;
use crate::query::QueryPreprocessor;
use crate::ranker;
use crate::semantic::{SemanticScorer, merge_rrf};
use crate::snippet;
use crate::trie::PrefixTree;
use crate::typo::TrigramIndex;

// ── Shared State ──────────────────────────────────────────────────────────────

pub struct AppState {
    pub index:        RwLock<Index>,
    pub trie:         RwLock<PrefixTree>,
    pub trigram:      RwLock<TrigramIndex>,
    pub preprocessor: QueryPreprocessor,
    pub index_path:   String,
    pub index_tx:     mpsc::Sender<IndexTask>,
    pub latencies:    RwLock<Vec<u64>>,
}

pub type SharedState = Arc<AppState>;

pub struct IndexTask {
    pub title:  String,
    pub body:   String,
    pub result: tokio::sync::oneshot::Sender<u32>,
}

impl AppState {
    pub fn new(index: Index, index_path: impl Into<String>, index_tx: mpsc::Sender<IndexTask>) -> Self {
        let trie    = build_title_trie(&index);
        let trigram = TrigramIndex::build(index.vocabulary().into_iter());
        Self {
            trie:        RwLock::new(trie),
            trigram:     RwLock::new(trigram),
            preprocessor: QueryPreprocessor::new(),
            index:       RwLock::new(index),
            index_path:  index_path.into(),
            index_tx,
            latencies:   RwLock::new(Vec::with_capacity(50)),
        }
    }

    fn record_latency(&self, us: u64) {
        let mut l = self.latencies.write();
        if l.len() >= 50 { l.remove(0); }
        l.push(us);
    }

    fn p50_latency_us(&self) -> u64 {
        let mut l = self.latencies.read().clone();
        if l.is_empty() { return 0; }
        l.sort_unstable();
        l[l.len() / 2]
    }
}

pub fn build_title_trie(index: &Index) -> PrefixTree {
    use std::collections::HashMap;

    let mut trie = PrefixTree::new();

    // Tier 1 — entire title string (single-spaced, lowercased). Highest weight.
    const TITLE_PHRASE_WEIGHT: u64 = 10_000;
    for doc in index.documents.values() {
        let normalized: String = doc.title.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.len() < 2 {
            continue;
        }
        trie.insert(&normalized.to_lowercase(), TITLE_PHRASE_WEIGHT);
    }

    // Tier 2 — Porter stems of words that appear in titles (body ignored).
    let mut stem_counts: HashMap<String, u64> = HashMap::new();
    for doc in index.documents.values() {
        for word in doc.title.split(|c: char| !c.is_alphabetic()) {
            if word.len() >= 2 {
                let stem = crate::porter::stem(word);
                *stem_counts.entry(stem).or_insert(0) += 1;
            }
        }
    }
    const STEM_UNIT: u64 = 100;
    for (stem, c) in stem_counts {
        let w = STEM_UNIT.saturating_mul(c).max(STEM_UNIT);
        trie.insert(&stem, w);
    }

    trie
}

// ── Background Indexer ────────────────────────────────────────────────────────

pub async fn background_indexer(state: SharedState, mut rx: mpsc::Receiver<IndexTask>) {
    info!("Background indexer started");
    while let Some(task) = rx.recv().await {
        let doc_id = {
            let mut idx = state.index.write();
            let id = idx.add_document(&task.title, &task.body);
            if let Err(e) = idx.save(&state.index_path) { warn!("Save failed: {e}"); }
            id
        };
        {
            let idx = state.index.read();
            *state.trie.write()    = build_title_trie(&idx);
            *state.trigram.write() = TrigramIndex::build(idx.vocabulary().into_iter());
        }
        let _ = task.result.send(doc_id);
    }
}

// ── POST /api/search ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchRequest {
    pub query:          String,
    #[serde(default = "default_limit")]
    pub limit:          usize,
    #[serde(default = "default_true")]
    pub typo_tolerance: bool,
    #[serde(default = "default_blend")]
    pub semantic_blend: f64,
}
fn default_limit() -> usize { 10 }
fn default_true()  -> bool  { true }
fn default_blend() -> f64   { 0.4 }

#[derive(Serialize)]
pub struct SearchResult {
    pub doc_id:  u32,
    pub title:   String,
    pub score:   f64,
    pub snippet: String,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub query:           String,
    pub processed_query: Vec<String>,
    pub corrections:     Vec<(String, String)>,
    pub expansions:      Vec<(String, Vec<String>)>,
    pub results:         Vec<SearchResult>,
    pub total_results:   usize,
    pub latency_ms:      f64,
}

pub async fn search_handler(
    State(state): State<SharedState>,
    Json(req):    Json<SearchRequest>,
) -> impl IntoResponse {
    let t0 = Instant::now();

    // Full index vocabulary (title + body + phrase keys) for spell-check & synonyms.
    let corpus_vocab_owned: Vec<String> = {
        let idx = state.index.read();
        idx.vocabulary().iter().map(|s| s.to_string()).collect()
    };
    let corpus_vocab: Vec<&str> = corpus_vocab_owned.iter().map(String::as_str).collect();

    let processed = state.preprocessor.process(&req.query, &corpus_vocab);

    // Build effective query string from processed tokens (stemmed `~` compounds).
    let effective_query = processed.tokens.join(" ");

    // Typo correction pass on any remaining unrecognised tokens.
    let final_query = if req.typo_tolerance {
        let corrected = state.trigram.read().correct_query(&effective_query, 0.25);
        corrected.join(" ")
    } else {
        effective_query
    };

    let index = state.index.read();

    // BM25 ranking.
    let bm25_hits = ranker::search(&index, &final_query, req.limit * 2);
    let bm25_ranked: Vec<(u32, f64)> = bm25_hits.iter().map(|h| (h.doc_id, h.score)).collect();
    let bm25_terms: std::collections::HashMap<u32, Vec<String>> =
        bm25_hits.iter().map(|h| (h.doc_id, h.matched_terms.clone())).collect();

    // Semantic ranking.
    let semantic_ranked: Vec<(u32, f64)> = if req.semantic_blend > 0.01 {
        SemanticScorer::new(&index).score(&final_query)
    } else {
        vec![]
    };

    // RRF fusion.
    let bm25_w = 1.0 - req.semantic_blend.clamp(0.0, 1.0);
    let sem_w  = req.semantic_blend.clamp(0.0, 1.0);
    let fused  = merge_rrf(&bm25_ranked, &semantic_ranked, bm25_w, sem_w, req.limit);

    let results: Vec<SearchResult> = fused.into_iter().filter_map(|(doc_id, score)| {
        let doc   = index.documents.get(&doc_id)?;
        // Filter out compound phrase tokens for snippet matching (use word stems only).
        let terms: Vec<String> = bm25_terms.get(&doc_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|t| !t.contains('~'))
            .collect();
        let snip = snippet::extract_snippet(&doc.body, &terms);
        Some(SearchResult {
            doc_id,
            title: doc.title.clone(),
            score: (score * 100000.0).round() / 100000.0,
            snippet: snip,
        })
    }).collect();

    let elapsed_us = t0.elapsed().as_micros() as u64;
    state.record_latency(elapsed_us);
    let total = results.len();

    Json(SearchResponse {
        query:           req.query,
        processed_query: processed.tokens,
        corrections:     processed.corrections,
        expansions:      processed.expansions,
        results,
        total_results: total,
        latency_ms: (elapsed_us as f64 / 1000.0 * 1000.0).round() / 1000.0,
    })
}

// ── POST /api/index ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct IndexRequest {
    pub title: String,
    pub body:  String,
}

#[derive(Serialize)]
pub struct IndexResponse {
    pub doc_id:  u32,
    pub message: String,
}

pub async fn index_handler(
    State(state): State<SharedState>,
    Json(req):    Json<IndexRequest>,
) -> impl IntoResponse {
    let (tx, rx) = tokio::sync::oneshot::channel();
    if state.index_tx.send(IndexTask { title: req.title, body: req.body, result: tx }).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR,
                Json(IndexResponse { doc_id: 0, message: "Indexer unavailable".into() }));
    }
    match rx.await {
        Ok(doc_id) => (StatusCode::CREATED,
                       Json(IndexResponse { doc_id, message: format!("Document {doc_id} indexed") })),
        Err(_)     => (StatusCode::INTERNAL_SERVER_ERROR,
                       Json(IndexResponse { doc_id: 0, message: "Indexer task failed".into() })),
    }
}

// ── GET /api/suggest — title-only trie autocomplete ──────────────────────────

#[derive(Deserialize)]
pub struct SuggestParams {
    pub q:     String,
    #[serde(default = "default_suggest_limit")]
    pub limit: usize,
}
fn default_suggest_limit() -> usize { 6 }

#[derive(Serialize)]
pub struct SuggestResponse {
    pub prefix:      String,
    pub suggestions: Vec<SuggestItem>,
}

#[derive(Serialize)]
pub struct SuggestItem {
    pub term:   String,
    pub weight: u64,
}

fn normalize_suggest_input(q: &str) -> String {
    q.trim().split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub async fn suggest_handler(
    State(state): State<SharedState>,
    AxumQuery(p): AxumQuery<SuggestParams>,
) -> impl IntoResponse {
    let normalized = normalize_suggest_input(&p.q);
    if normalized.is_empty() {
        return Json(SuggestResponse { prefix: p.q, suggestions: vec![] });
    }

    // Multi-word prefix: match full title strings (spaces preserved).
    // Single token: match Porter stems stored from title words.
    let prefix = if normalized.contains(' ') {
        normalized
    } else {
        crate::porter::stem(&normalized)
    };

    let suggestions = state.trie.read()
        .complete(&prefix, p.limit)
        .into_iter()
        // Hide inverted-index phrase keys from UI; title tier uses spaces, not ~.
        .filter(|c| !c.term.contains('~'))
        .map(|c| SuggestItem { term: c.term, weight: c.weight })
        .collect();

    Json(SuggestResponse { prefix: p.q, suggestions })
}

// ── GET /api/stats ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_docs:       usize,
    pub vocab_size:       usize,
    pub title_vocab_size: usize,
    pub avg_title_length: f64,
    pub avg_body_length:  f64,
}

pub async fn stats_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let idx = state.index.read();
    Json(StatsResponse {
        total_docs:       idx.total_docs,
        vocab_size:       idx.postings.len(),
        title_vocab_size: idx.title_vocabulary().len(),
        avg_title_length: (idx.avg_title_length * 100.0).round() / 100.0,
        avg_body_length:  (idx.avg_body_length  * 100.0).round() / 100.0,
    })
}

// ── GET /api/health ───────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status:         &'static str,
    pub rss_mb:         f64,
    pub total_docs:     usize,
    pub p50_latency_us: u64,
    pub vocab_size:     usize,
}

pub async fn health_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let rss_mb = rss_mb();
    let idx    = state.index.read();
    Json(HealthResponse {
        status:         "ok",
        rss_mb:         (rss_mb * 100.0).round() / 100.0,
        total_docs:     idx.total_docs,
        p50_latency_us: state.p50_latency_us(),
        vocab_size:     idx.postings.len(),
    })
}

fn rss_mb() -> f64 {
    let mut sys = System::new();
    let pid = Pid::from(std::process::id() as usize);
    sys.refresh_process(pid);
    sys.process(pid).map(|p| p.memory() as f64 / 1_048_576.0).unwrap_or(0.0)
}

#[cfg(test)]
mod title_trie_tests {
    use super::build_title_trie;
    use crate::indexer::Index;

    #[test]
    fn trie_never_includes_body_only_words() {
        let mut idx = Index::default();
        idx.add_document(
            "Amazon Rainforest",
            "Something Only In Body xyzzyunique999",
        );
        let trie = build_title_trie(&idx);
        assert!(trie.complete("xyzzyunique999", 3).is_empty());
        assert!(trie.complete("something", 3).is_empty());
    }

    #[test]
    fn full_title_suggestions_rank_above_bare_stem() {
        let mut idx = Index::default();
        idx.add_document("Amazon Rainforest", "Amazon");
        idx.add_document("Amazon Forest", "");
        let trie = build_title_trie(&idx);
        let hits = trie.complete("amazon", 10);
        assert!(!hits.is_empty(), "{hits:?}");
        assert!(
            hits[0].term.contains(' '),
            "expected a full-title completion first, got {:?}",
            hits[0].term
        );
        assert_eq!(hits[0].weight, 10_000);
    }
}
