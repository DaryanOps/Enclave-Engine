use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct TrieNode {
    pub children:    HashMap<char, Box<TrieNode>>,
    pub is_terminal: bool,
    pub weight:      u64,
    pub subtree_max: u64,
}

pub struct PrefixTree {
    root: TrieNode,
}

impl PrefixTree {
    pub fn new() -> Self {
        Self { root: TrieNode::default() }
    }

    pub fn insert(&mut self, term: &str, weight: u64) {
        let mut node = &mut self.root;
        if weight > node.subtree_max { node.subtree_max = weight; }

        for ch in term.chars() {
            node = node.children.entry(ch).or_default();
            if weight > node.subtree_max { node.subtree_max = weight; }
        }

        // Update terminal weight — keep the highest seen.
        node.is_terminal = true;
        if weight > node.weight { node.weight = weight; }
    }

    // Keep completion ranking stable: by weight first, then alphabetically.
    pub fn complete(&self, prefix: &str, limit: usize) -> Vec<Completion> {
        // Walk to the prefix node.
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(child) => node = child,
                None        => return vec![],
            }
        }

        // DFS to collect all terminals in the subtree.
        let mut results: Vec<Completion> = Vec::new();
        dfs(node, &prefix.to_owned(), &mut results);

        // Sort by weight descending, then alphabetically as tiebreaker.
        results.sort_by(|a, b| {
            b.weight.cmp(&a.weight).then(a.term.cmp(&b.term))
        });
        results.truncate(limit);
        results
    }

    pub fn build<'a>(items: impl IntoIterator<Item = (&'a str, u64)>) -> Self {
        let mut trie = Self::new();
        for (term, weight) in items { trie.insert(term, weight); }
        trie
    }

    pub fn len(&self) -> usize {
        count_terminals(&self.root)
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

impl Default for PrefixTree {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub term:   String,
    pub weight: u64,
}

// ── DFS helper ────────────────────────────────────────────────────────────────

fn dfs(node: &TrieNode, current: &str, out: &mut Vec<Completion>) {
    if node.is_terminal {
        out.push(Completion { term: current.to_owned(), weight: node.weight });
    }
    // Sort children by subtree_max descending to visit promising branches first.
    let mut children: Vec<(char, &TrieNode)> =
        node.children.iter().map(|(&c, n)| (c, n.as_ref())).collect();
    children.sort_by(|a, b| b.1.subtree_max.cmp(&a.1.subtree_max));

    for (ch, child) in children {
        let mut next = current.to_owned();
        next.push(ch);
        dfs(child, &next, out);
    }
}

fn count_terminals(node: &TrieNode) -> usize {
    let self_count = if node.is_terminal { 1 } else { 0 };
    self_count + node.children.values().map(|c| count_terminals(c)).sum::<usize>()
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completions_sorted_by_weight() {
        let mut trie = PrefixTree::new();
        trie.insert("search",   100);
        trie.insert("searcher",  30);
        trie.insert("searching", 80);
        trie.insert("sea",       10);

        let results = trie.complete("sea", 5);
        assert_eq!(results[0].term, "search");    // weight 100
        assert_eq!(results[1].term, "searching"); // weight 80
        assert_eq!(results[2].term, "searcher");  // weight 30
        assert_eq!(results[3].term, "sea");       // weight 10
    }

    #[test]
    fn test_no_match_returns_empty() {
        let mut trie = PrefixTree::new();
        trie.insert("rust", 50);
        assert!(trie.complete("xyz", 5).is_empty());
    }

    #[test]
    fn test_exact_term_is_returned() {
        let mut trie = PrefixTree::new();
        trie.insert("index", 20);
        let r = trie.complete("index", 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].term, "index");
    }
}
