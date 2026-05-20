use std::collections::HashMap;

use crate::indexer::{self, Index};

// ── Hyper-parameters ──────────────────────────────────────────────────────────

const RRF_K: f64 = 60.0;

// ── Sparse TF-IDF embedding ───────────────────────────────────────────────────

pub type SparseVec = HashMap<String, f64>;

// Lightweight semantic scoring: plain TF-IDF vectors + cosine.
pub fn embed(text: &str, index: &Index) -> SparseVec {
    let n = index.total_docs as f64;

    // Count raw term frequencies.
    let mut tf_raw: HashMap<String, f64> = HashMap::new();
    for lexeme in indexer::stemmed_retrieval_lexemes(text) {
        *tf_raw.entry(lexeme).or_insert(0.0) += 1.0;
    }

    let max_tf = tf_raw.values().cloned().fold(0.0_f64, f64::max).max(1.0);

    // Compute TF-IDF for each term.
    tf_raw.into_iter().filter_map(|(term, raw_tf)| {
        // Augmented TF normalisation: 0.5 + 0.5·(tf/max_tf)
        let tf  = 0.5 + 0.5 * (raw_tf / max_tf);
        // IDF from the index (fall back to 0 if term not indexed).
        let df  = index.postings.get(&term).map(|p| p.len() as f64).unwrap_or(0.0);
        if df == 0.0 { return None; }
        let idf = (n / df).ln();
        Some((term, tf * idf))
    }).collect()
}

pub fn cosine(a: &SparseVec, b: &SparseVec) -> f64 {
    let dot: f64 = a.iter()
        .filter_map(|(t, wa)| b.get(t).map(|wb| wa * wb))
        .sum();

    let mag_a: f64 = a.values().map(|w| w * w).sum::<f64>().sqrt();
    let mag_b: f64 = b.values().map(|w| w * w).sum::<f64>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 { 0.0 }
    else { dot / (mag_a * mag_b) }
}

// ── SemanticScorer ────────────────────────────────────────────────────────────

pub struct SemanticScorer<'a> {
    index: &'a Index,
}

impl<'a> SemanticScorer<'a> {
    pub fn new(index: &'a Index) -> Self { Self { index } }

    pub fn score(&self, query: &str) -> Vec<(u32, f64)> {
        let query_vec = embed(query, self.index);

        let mut scores: Vec<(u32, f64)> = self.index.documents.keys().filter_map(|&doc_id| {
            let doc = self.index.documents.get(&doc_id)?;
            let text = format!("{} {}", doc.title, doc.body);
            let doc_vec = embed(&text, self.index);
            let sim = cosine(&query_vec, &doc_vec);
            if sim > 0.0 { Some((doc_id, sim)) } else { None }
        }).collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores
    }
}

// ── Reciprocal Rank Fusion ────────────────────────────────────────────────────

pub type RankedList = Vec<(u32, f64)>;

pub fn merge_rrf(
    bm25_ranks:     &RankedList,
    semantic_ranks: &RankedList,
    bm25_weight:     f64,
    semantic_weight: f64,
    limit:           usize,
) -> Vec<(u32, f64)> {
    let mut rrf_scores: HashMap<u32, f64> = HashMap::new();

    for (rank, &(doc_id, _)) in bm25_ranks.iter().enumerate() {
        *rrf_scores.entry(doc_id).or_insert(0.0) +=
            bm25_weight / (RRF_K + rank as f64 + 1.0);
    }

    for (rank, &(doc_id, _)) in semantic_ranks.iter().enumerate() {
        *rrf_scores.entry(doc_id).or_insert(0.0) +=
            semantic_weight / (RRF_K + rank as f64 + 1.0);
    }

    let mut merged: Vec<(u32, f64)> = rrf_scores.into_iter().collect();
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    merged.truncate(limit);
    merged
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_identical_vectors() {
        let mut v: SparseVec = HashMap::new();
        v.insert("rust".into(), 1.0);
        v.insert("search".into(), 2.0);
        let sim = cosine(&v, &v);
        assert!((sim - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_cosine_orthogonal_vectors() {
        let mut a: SparseVec = HashMap::new();
        a.insert("rust".into(), 1.0);
        let mut b: SparseVec = HashMap::new();
        b.insert("python".into(), 1.0);
        assert_eq!(cosine(&a, &b), 0.0);
    }

    #[test]
    fn test_rrf_merge() {
        let bm25     = vec![(1, 3.0), (2, 2.0), (3, 1.0)];
        let semantic = vec![(3, 0.9), (1, 0.7), (2, 0.5)];
        let merged   = merge_rrf(&bm25, &semantic, 1.0, 1.0, 3);
        // doc 1 is in top 2 of both lists, so should rank first
        assert_eq!(merged[0].0, 1);
    }
}
