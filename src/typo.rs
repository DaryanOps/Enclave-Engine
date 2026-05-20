use std::collections::{HashMap, HashSet};
use crate::indexer::stem_query_lexeme;

type Trigram = [char; 3];

pub struct TrigramIndex {
    index: HashMap<Trigram, Vec<u32>>,
    vocab: Vec<(String, usize)>,
}

impl TrigramIndex {
    pub fn build<'a>(terms: impl IntoIterator<Item = &'a str>) -> Self {
        let mut vocab: Vec<(String, usize)> = Vec::new();
        let mut index: HashMap<Trigram, Vec<u32>> = HashMap::new();

        for term in terms {
            let id     = vocab.len() as u32;
            let tgrams = trigrams(term);
            let len    = tgrams.len();
            vocab.push((term.to_owned(), len));
            for tg in tgrams { index.entry(tg).or_default().push(id); }
        }

        Self { index, vocab }
    }

    pub fn correct_query(&self, query: &str, threshold: f64) -> Vec<String> {
        query
            .split(|c: char| !c.is_alphabetic() && c != '~')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .filter(|t| t.len() >= 2 || t.contains('~'))
            .map(|token| {
                let stemmed_key = stem_query_lexeme(token);
                if stemmed_key.is_empty() {
                    return stemmed_key;
                }
                if self.vocab.iter().any(|(t, _)| t == &stemmed_key) {
                    return stemmed_key;
                }
                self.best_match(&stemmed_key, threshold).unwrap_or(stemmed_key)
            })
            .collect()
    }

    pub fn best_match(&self, token: &str, threshold: f64) -> Option<String> {
        let q_tgrams: HashSet<Trigram> = trigrams(token).into_iter().collect();
        if q_tgrams.is_empty() { return None; }

        let mut hits: HashMap<u32, usize> = HashMap::new();
        for tg in &q_tgrams {
            if let Some(ids) = self.index.get(tg) {
                for &id in ids { *hits.entry(id).or_insert(0) += 1; }
            }
        }

        let q_len = q_tgrams.len();
        let mut best: Option<(f64, &str)> = None;

        for (id, intersection) in hits {
            let (term, cand_len) = &self.vocab[id as usize];
            let union   = q_len + cand_len - intersection;
            let jaccard = intersection as f64 / union as f64;
            if jaccard >= threshold && best.map_or(true, |(s,_)| jaccard > s) {
                best = Some((jaccard, term));
            }
        }

        best.map(|(_, t)| t.to_owned())
    }

    pub fn suggestions(&self, token: &str, threshold: f64, limit: usize) -> Vec<(String, f64)> {
        let q_tgrams: HashSet<Trigram> = trigrams(token).into_iter().collect();
        if q_tgrams.is_empty() { return vec![]; }

        let mut hits: HashMap<u32, usize> = HashMap::new();
        for tg in &q_tgrams {
            if let Some(ids) = self.index.get(tg) {
                for &id in ids { *hits.entry(id).or_insert(0) += 1; }
            }
        }

        let q_len = q_tgrams.len();
        let mut results: Vec<(String, f64)> = hits.into_iter().filter_map(|(id, intersection)| {
            let (term, cand_len) = &self.vocab[id as usize];
            let union   = q_len + cand_len - intersection;
            let jaccard = intersection as f64 / union as f64;
            if jaccard >= threshold { Some((term.clone(), jaccard)) } else { None }
        }).collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(limit);
        results
    }
}

pub fn trigrams(s: &str) -> Vec<Trigram> {
    let padded: Vec<char> = format!("${}$", s).chars().collect();
    padded.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}
