use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::porter;
use crate::query::{bigram_tokens, trigram_tokens};

// ── Data Structures ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Posting {
    pub doc_id:   u32,
    pub title_tf: u32,
    pub body_tf:  u32,
    pub positions: Vec<usize>,
}

impl Posting {
    pub fn total_tf(&self) -> u32 { self.title_tf + self.body_tf }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Document {
    pub id:         u32,
    pub title:      String,
    pub body:       String,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Index {
    pub postings:         HashMap<String, Vec<Posting>>,
    pub documents:        HashMap<u32, Document>,

    pub title_lengths:    HashMap<u32, u32>,
    pub body_lengths:     HashMap<u32, u32>,
    pub avg_title_length: f64,
    pub avg_body_length:  f64,

    pub total_docs: usize,
    next_id:        u32,
}

// ── Tokenisation ──────────────────────────────────────────────────────────────

pub fn tokenise(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphabetic())
        .filter(|t| t.len() >= 2)
        .map(|t| porter::stem(t))
        .collect()
}

fn split_query_runs(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphabetic() && c != '~')
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn keep_retrieval_lexeme(raw: &str) -> bool {
    raw.contains('~') || raw.len() >= 2
}

// Keep this in sync with index-time stemming so query terms line up with postings.
pub fn stem_query_lexeme(raw: &str) -> String {
    if raw.contains('~') {
        raw.split('~')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| porter::stem(s))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("~")
    } else {
        porter::stem(raw)
    }
}

pub fn tokenise_query(text: &str) -> Vec<String> {
    stemmed_retrieval_lexemes(text).collect()
}

pub fn stemmed_retrieval_lexemes(text: &str) -> impl Iterator<Item = String> + '_ {
    split_query_runs(text)
        .filter(|t| keep_retrieval_lexeme(t))
        .map(stem_query_lexeme)
        .filter(|t| !t.is_empty())
}

pub fn tokenise_with_originals(text: &str) -> Vec<(String, String)> {
    text.split(|c: char| !c.is_alphabetic())
        .filter(|t| t.len() >= 2)
        .map(|t| { let l = t.to_lowercase(); let s = porter::stem(&l); (l, s) })
        .collect()
}

fn raw_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphabetic())
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_lowercase())
        .collect()
}

// ── Index Builder ─────────────────────────────────────────────────────────────

impl Index {
    pub fn add_document(&mut self, title: impl Into<String>, body: impl Into<String>) -> u32 {
        self.add_document_at(title, body, now_unix_secs())
    }

    pub fn add_document_at(
        &mut self,
        title:      impl Into<String>,
        body:       impl Into<String>,
        created_at: u64,
    ) -> u32 {
        let id    = self.next_id;
        self.next_id += 1;

        let title = title.into();
        let body  = body.into();

        // ── Title field ───────────────────────────────────────────────────────
        let mut tf_title: HashMap<String, (u32, Vec<usize>)> = HashMap::new();
        let mut offset = 0usize;
        for word in title.split(|c: char| !c.is_alphabetic()) {
            if word.len() >= 2 {
                let stem = porter::stem(word);
                let e    = tf_title.entry(stem).or_default();
                e.0 += 1;
                if e.1.len() < 8 { e.1.push(offset); }
            }
            offset += word.len() + 1;
        }

        // ── Title phrase tokens (bigrams + trigrams) ──────────────────────────
        let title_words = raw_words(&title);
        let title_word_refs: Vec<&str> = title_words.iter().map(String::as_str).collect();

        for tok in bigram_tokens(&title_word_refs) {
            let e = tf_title.entry(tok).or_default();
            e.0 += 3; // phrase matches get a 3× bonus in the title field
        }
        for tok in trigram_tokens(&title_word_refs) {
            let e = tf_title.entry(tok).or_default();
            e.0 += 5; // trigram phrases get a 5× bonus
        }

        // ── Body field ────────────────────────────────────────────────────────
        let mut tf_body: HashMap<String, (u32, Vec<usize>)> = HashMap::new();
        offset = title.len() + 1;
        for word in body.split(|c: char| !c.is_alphabetic()) {
            if word.len() >= 2 {
                let stem = porter::stem(word);
                let e    = tf_body.entry(stem).or_default();
                e.0 += 1;
                if e.1.len() < 8 { e.1.push(offset); }
            }
            offset += word.len() + 1;
        }

        // ── Body phrase tokens ────────────────────────────────────────────────
        let body_words = raw_words(&body);
        let body_word_refs: Vec<&str> = body_words.iter().map(String::as_str).collect();

        for tok in bigram_tokens(&body_word_refs) {
            tf_body.entry(tok).or_default().0 += 1;
        }
        for tok in trigram_tokens(&body_word_refs) {
            tf_body.entry(tok).or_default().0 += 2;
        }

        // ── Merge into postings ───────────────────────────────────────────────
        let all_terms: std::collections::HashSet<String> =
            tf_title.keys().chain(tf_body.keys()).cloned().collect();

        for term in &all_terms {
            let (title_tf, mut positions) = tf_title.get(term).cloned().unwrap_or_default();
            let (body_tf,  body_pos)      = tf_body.get(term).cloned().unwrap_or_default();
            positions.extend(body_pos);
            positions.truncate(8);
            self.postings.entry(term.clone()).or_default()
                .push(Posting { doc_id: id, title_tf, body_tf, positions });
        }

        let title_len: u32 = tf_title.values().map(|(f,_)| f).sum();
        let body_len:  u32 = tf_body.values().map(|(f,_)| f).sum();
        self.title_lengths.insert(id, title_len);
        self.body_lengths.insert(id,  body_len);
        self.documents.insert(id, Document { id, title, body, created_at });
        self.total_docs += 1;

        let n = self.total_docs as f64;
        let st: u32 = self.title_lengths.values().sum();
        let sb: u32 = self.body_lengths.values().sum();
        self.avg_title_length = st as f64 / n;
        self.avg_body_length  = sb as f64 / n;

        info!(doc_id = id, title_len, body_len, "Indexed document");
        id
    }

    pub fn add_documents(&mut self, docs: impl IntoIterator<Item = (String, String)>) {
        for (t, b) in docs { self.add_document(t, b); }
    }

    // ── Title-only vocabulary (for autocomplete + synonym expansion) ───────────

    pub fn title_vocabulary(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.postings
            .iter()
            .filter(|(_, postings)| postings.iter().any(|p| p.title_tf > 0))
            .map(|(term, _)| term.as_str())
            .collect();
        v.sort_unstable();
        v
    }

    pub fn vocabulary(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.postings.keys().map(String::as_str).collect();
        v.sort_unstable();
        v
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, bincode::serialize(self)?)?;
        info!("Index saved ({} docs)", self.total_docs);
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let index: Self = bincode::deserialize(&std::fs::read(path)?)?;
        info!("Index loaded ({} docs)", index.total_docs);
        Ok(index)
    }

    pub fn load_or_new(path: impl AsRef<Path>) -> Self {
        Self::load(&path).unwrap_or_else(|_| { info!("No index — starting fresh"); Self::default() })
    }
}

// ── Timing ────────────────────────────────────────────────────────────────────

pub fn now_unix_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

// ── Sample Corpus ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod query_token_tests {
    use super::*;

    #[test]
    fn phrase_compound_is_single_term() {
        let q = "machine~learning search";
        let t = tokenise_query(q);
        assert!(t.contains(&"machin~learn".to_string()), "got {t:?}");
        assert!(t.contains(&"search".to_string()));
    }

    #[test]
    fn plain_query_splits_words_and_stems() {
        let t = tokenise_query("inverted  index!");
        assert_eq!(t, vec![
            porter::stem("inverted"),
            porter::stem("index"),
        ]);
    }
}

pub fn sample_corpus() -> Vec<(String, String, u64)> {
    let now = now_unix_secs();
    let day = 86_400u64;
    vec![
        (
            "Rust Programming Language".into(),
            "Rust is a systems programming language focused on memory safety, speed, and \
             concurrency. It achieves memory safety without a garbage collector via ownership \
             and borrowing. Rust is used for WebAssembly, operating systems, game engines, and \
             embedded systems programming.".into(),
            now - day * 2,
        ),
        (
            "BM25 Ranking Algorithm".into(),
            "BM25 is a bag-of-words retrieval function used by search engines to rank matching \
             documents by relevance. Based on the probabilistic retrieval framework by Robertson \
             and Jones, it models term frequency saturation and document-length normalisation \
             for information retrieval systems.".into(),
            now - day * 10,
        ),
        (
            "Porter Stemming Algorithm".into(),
            "The Porter stemmer removes morphological and inflexional endings from English words. \
             It normalises terms before indexing in information retrieval systems. Invented by \
             Martin Porter in 1980, it remains the most widely used English-language stemmer for \
             search engine applications.".into(),
            now - day * 30,
        ),
        (
            "Inverted Index Data Structure".into(),
            "An inverted index maps content words to their locations in a document collection. \
             It enables fast full-text search at the cost of additional indexing time. This data \
             structure is the foundation behind Lucene, Elasticsearch, and Tantivy search \
             engines.".into(),
            now - day * 45,
        ),
        (
            "Typo Tolerance Search Engine".into(),
            "Typo tolerance returns relevant results even when users make spelling mistakes. \
             Approaches include Levenshtein edit distance, BK-trees, and trigram indexing. \
             For high-performance systems a trigram index offers amortised constant-time lookup, \
             the technique used by PostgreSQL pg_trgm.".into(),
            now - day * 7,
        ),
        (
            "Axum Web Framework".into(),
            "Axum is an ergonomic and modular web framework built with Tokio, Tower, and Hyper. \
             It features macro-free routing, powerful extractors, and seamless Tower middleware. \
             Axum is the standard choice for high-performance async HTTP APIs in Rust.".into(),
            now - day * 3,
        ),
        (
            "Information Retrieval Systems".into(),
            "Information retrieval obtains resources relevant to an information need from a \
             collection. IR systems combine inverted indices, ranking algorithms, query \
             processing, and relevance feedback. The field spans computer science, mathematics, \
             and linguistics disciplines.".into(),
            now - day * 60,
        ),
        (
            "Levenshtein Distance Algorithm".into(),
            "Levenshtein distance measures the minimum single-character edits required to \
             transform one string into another. Edits are insertions, deletions, and \
             substitutions. It is used in spell-checkers, DNA sequence alignment, and fuzzy \
             search implementations.".into(),
            now - day * 20,
        ),
        (
            "Tokio Async Runtime".into(),
            "Tokio is an asynchronous runtime for Rust providing building blocks for network \
             applications. It offers a multi-threaded work-stealing scheduler, async IO, timers, \
             and synchronisation primitives powering the Rust async ecosystem.".into(),
            now - day * 5,
        ),
        (
            "Full Text Search Architecture".into(),
            "Full-text search indexes complete document content for fast retrieval. Modern \
             engines combine inverted indices with BM25 ranking, vector embeddings for semantic \
             search, and autocomplete tries. Sub-millisecond latency is achieved through \
             sharding, caching, and SIMD-accelerated scoring.".into(),
            now - day * 1,
        ),
        (
            "Machine Learning Fundamentals".into(),
            "Machine learning is a subset of artificial intelligence that enables systems to \
             learn from data without being explicitly programmed. Deep learning uses neural \
             networks with many layers to model complex patterns. Natural language processing \
             applies machine learning to understand and generate human text.".into(),
            now - day * 4,
        ),
        (
            "Distributed Systems Design".into(),
            "Distributed systems coordinate multiple computers to achieve a common goal. Key \
             challenges include consensus, fault tolerance, and network partitions. The CAP \
             theorem states that distributed systems can guarantee at most two of consistency, \
             availability, and partition tolerance simultaneously.".into(),
            now - day * 8,
        ),
    ]
}
