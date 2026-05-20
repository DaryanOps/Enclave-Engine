use std::collections::HashMap;
use crate::indexer::{Index, now_unix_secs, tokenise_query};

const K1:             f64 = 1.5;
const B:              f64 = 0.75;
// Title matches should pop above body-only hits in most cases.
const TITLE_WEIGHT:   f64 = 8.0;
const HALF_LIFE_DAYS: f64 = 30.0;
const BASE_SCORE:     f64 = 0.3;

#[derive(Debug, Clone)]
pub struct ScoredDoc {
    pub doc_id:        u32,
    pub score:         f64,
    pub matched_terms: Vec<String>,
}

pub fn search(index: &Index, query: &str, k: usize) -> Vec<ScoredDoc> {
    if index.total_docs == 0 { return vec![]; }
    let terms = tokenise_query(query);
    if terms.is_empty() { return vec![]; }
    let n   = index.total_docs as f64;
    let now = now_unix_secs();
    let mut scores: HashMap<u32, (f64, Vec<String>)> = HashMap::new();

    for term in &terms {
        let postings = match index.postings.get(term) { Some(p) => p, None => continue };
        let df  = postings.len() as f64;
        let idf = smooth_idf(n, df);

        for p in postings {
            let t_len = *index.title_lengths.get(&p.doc_id).unwrap_or(&1) as f64;
            let b_len = *index.body_lengths.get(&p.doc_id).unwrap_or(&1)  as f64;

            let t_score = TITLE_WEIGHT * bm25_tf(p.title_tf as f64, t_len, index.avg_title_length);
            let b_score = bm25_tf(p.body_tf as f64, b_len, index.avg_body_length);
            let raw     = idf * (t_score + b_score);

            let e = scores.entry(p.doc_id).or_insert((0.0, Vec::new()));
            e.0 += raw;
            if !e.1.contains(term) { e.1.push(term.clone()); }
        }
    }

    let mut results: Vec<ScoredDoc> = scores
        .into_iter()
        .map(|(doc_id, (raw, matched_terms))| {
            let created_at = index.documents.get(&doc_id).map(|d| d.created_at).unwrap_or(now);
            ScoredDoc { doc_id, score: raw * recency_mult(now, created_at), matched_terms }
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    results.truncate(k);
    results
}

#[inline]
fn smooth_idf(n: f64, df: f64) -> f64 {
    ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
}

#[inline]
fn bm25_tf(tf: f64, field_len: f64, avg: f64) -> f64 {
    if tf == 0.0 { return 0.0; }
    let norm = 1.0 - B + B * (field_len / avg.max(1.0));
    (tf * (K1 + 1.0)) / (tf + K1 * norm)
}

fn recency_mult(now: u64, created_at: u64) -> f64 {
    let days   = now.saturating_sub(created_at) as f64 / 86_400.0;
    let lambda = std::f64::consts::LN_2 / HALF_LIFE_DAYS;
    BASE_SCORE + (1.0 - BASE_SCORE) * (-lambda * days).exp()
}
