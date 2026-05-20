use std::collections::HashMap;
use crate::porter;

const LEVENSHTEIN_THRESHOLD: f64 = 0.78;

// ── Phrase dictionary ─────────────────────────────────────────────────────────
// Maps multi-word phrases (lowercase) → canonical compound token.
// Add entries here to teach the engine new domain phrases.

fn phrase_dict() -> Vec<(Vec<&'static str>, &'static str)> {
    vec![
        // Technology
        (vec!["machine", "learning"],            "machine~learning"),
        (vec!["Steve", "Jobs"],                  "Steve~Jobs"),
        (vec!["deep", "learning"],               "deep~learning"),
        (vec!["artificial", "intelligence"],     "artificial~intelligence"),
        (vec!["neural", "network"],              "neural~network"),
        (vec!["neural", "networks"],             "neural~network"),
        (vec!["natural", "language", "processing"], "natural~language~processing"),
        (vec!["natural", "language"],            "natural~language"),
        (vec!["large", "language", "model"],     "large~language~model"),
        (vec!["large", "language", "models"],    "large~language~model"),
        (vec!["language", "model"],              "language~model"),
        (vec!["search", "engine"],               "search~engine"),
        (vec!["search", "engines"],              "search~engine"),
        (vec!["full", "text", "search"],         "full~text~search"),
        (vec!["full", "text"],                   "full~text"),
        (vec!["inverted", "index"],              "inverted~index"),
        (vec!["data", "structure"],              "data~structure"),
        (vec!["data", "structures"],             "data~structure"),
        (vec!["binary", "tree"],                 "binary~tree"),
        (vec!["linked", "list"],                 "linked~list"),
        (vec!["hash", "map"],                    "hash~map"),
        (vec!["hash", "table"],                  "hash~table"),
        (vec!["source", "code"],                 "source~code"),
        (vec!["open", "source"],                 "open~source"),
        (vec!["version", "control"],             "version~control"),
        (vec!["software", "engineering"],        "software~engineering"),
        (vec!["computer", "science"],            "computer~science"),
        (vec!["operating", "system"],            "operating~system"),
        (vec!["operating", "systems"],           "operating~system"),
        (vec!["web", "server"],                  "web~server"),
        (vec!["web", "framework"],               "web~framework"),
        (vec!["rest", "api"],                    "rest~api"),
        (vec!["api", "endpoint"],                "api~endpoint"),
        (vec!["load", "balancer"],               "load~balancer"),
        (vec!["load", "balancing"],              "load~balancing"),
        (vec!["message", "queue"],               "message~queue"),
        (vec!["event", "loop"],                  "event~loop"),
        (vec!["async", "await"],                 "async~await"),
        (vec!["garbage", "collection"],          "garbage~collection"),
        (vec!["memory", "management"],           "memory~management"),
        (vec!["memory", "safety"],               "memory~safety"),
        (vec!["type", "system"],                 "type~system"),
        (vec!["type", "inference"],              "type~inference"),
        (vec!["compile", "time"],                "compile~time"),
        (vec!["run", "time"],                    "run~time"),
        (vec!["runtime", "error"],               "runtime~error"),
        (vec!["stack", "overflow"],              "stack~overflow"),
        (vec!["null", "pointer"],                "null~pointer"),
        (vec!["race", "condition"],              "race~condition"),
        (vec!["dead", "lock"],                   "deadlock"),
        (vec!["deadlock"],                       "deadlock"),
        (vec!["context", "switch"],              "context~switch"),
        (vec!["system", "call"],                 "system~call"),
        (vec!["bit", "map"],                     "bitmap"),
        (vec!["data", "base"],                   "database"),
        (vec!["data", "bases"],                  "database"),
        (vec!["key", "value"],                   "key~value"),
        (vec!["time", "series"],                 "time~series"),
        (vec!["time", "complexity"],             "time~complexity"),
        (vec!["space", "complexity"],            "space~complexity"),
        (vec!["big", "o"],                       "big~o"),
        (vec!["object", "oriented"],             "object~oriented"),
        (vec!["functional", "programming"],      "functional~programming"),
        (vec!["concurrency", "control"],         "concurrency~control"),
        (vec!["parallel", "computing"],          "parallel~computing"),
        (vec!["distributed", "system"],          "distributed~system"),
        (vec!["distributed", "systems"],         "distributed~system"),
        (vec!["cloud", "computing"],             "cloud~computing"),
        (vec!["edge", "computing"],              "edge~computing"),
        (vec!["micro", "services"],              "microservices"),
        (vec!["microservice"],                   "microservices"),
        (vec!["design", "pattern"],              "design~pattern"),
        (vec!["design", "patterns"],             "design~pattern"),
        (vec!["dependency", "injection"],        "dependency~injection"),
        (vec!["unit", "test"],                   "unit~test"),
        (vec!["unit", "tests"],                  "unit~test"),
        (vec!["integration", "test"],            "integration~test"),
        (vec!["continuous", "integration"],      "continuous~integration"),
        (vec!["continuous", "deployment"],       "continuous~deployment"),
        (vec!["command", "line"],                "command~line"),
        (vec!["command", "line", "interface"],   "command~line~interface"),
        (vec!["user", "interface"],              "user~interface"),
        (vec!["user", "experience"],             "user~experience"),
        (vec!["application", "programming", "interface"], "api"),
        // Rust specific
        (vec!["ownership", "model"],             "ownership~model"),
        (vec!["borrow", "checker"],              "borrow~checker"),
        (vec!["lifetime", "annotation"],         "lifetime~annotation"),
        (vec!["trait", "object"],                "trait~object"),
        (vec!["async", "runtime"],               "async~runtime"),
        (vec!["error", "handling"],              "error~handling"),
        // IR / Search specific
        (vec!["term", "frequency"],              "term~frequency"),
        (vec!["inverse", "document", "frequency"], "inverse~document~frequency"),
        (vec!["document", "frequency"],          "document~frequency"),
        (vec!["relevance", "feedback"],          "relevance~feedback"),
        (vec!["query", "expansion"],             "query~expansion"),
        (vec!["rank", "fusion"],                 "rank~fusion"),
        (vec!["reciprocal", "rank"],             "reciprocal~rank"),
        (vec!["cosine", "similarity"],           "cosine~similarity"),
        (vec!["edit", "distance"],               "edit~distance"),
        (vec!["levenshtein", "distance"],        "levenshtein~distance"),
        (vec!["prefix", "tree"],                 "prefix~tree"),
        (vec!["porter", "stemmer"],              "porter~stemmer"),
        (vec!["information", "retrieval"],       "information~retrieval"),
    ]
}

// ── Synonym dictionary ────────────────────────────────────────────────────────
// Stemmed key → stemmed synonyms.  Both directions are added automatically.

fn raw_synonyms() -> Vec<(&'static str, &'static [&'static str])> {
    vec![
        // Speed / performance
        ("fast",        &["quick","rapid","swift","speedy","performant","efficient"]),
        ("quick",       &["fast","rapid","swift","speedy"]),
        ("slow",        &["sluggish","leisurely","inefficient"]),
        ("speed",       &["performance","throughput","velocity","latency","rate"]),
        ("performance", &["speed","efficiency","throughput","benchmark"]),
        ("efficient",   &["fast","optimal","lean","lightweight"]),
        ("optimize",    &["improve","tune","enhance","accelerate","speed"]),
        // Search / retrieval
        ("search",      &["find","query","lookup","retrieve","locate","discover"]),
        ("find",        &["search","locate","discover","retrieve","lookup"]),
        ("query",       &["search","request","lookup","find"]),
        ("retrieve",    &["fetch","get","obtain","find","search"]),
        ("index",       &["catalogue","catalog","store","record","register"]),
        ("rank",        &["score","order","sort","prioritize","weight"]),
        ("score",       &["rank","weight","rate","evaluate","measure"]),
        ("relevance",   &["accuracy","pertinence","significance","importance"]),
        ("match",       &["find","locate","result","hit","correspond"]),
        // Data / storage
        ("store",       &["save","persist","write","record","keep"]),
        ("data",        &["information","content","record","entry"]),
        ("database",    &["storage","repository","datastore","db"]),
        ("file",        &["document","record","resource","entry"]),
        ("document",    &["file","record","text","content","entry"]),
        ("memory",      &["ram","heap","storage","space","allocation"]),
        ("cache",       &["buffer","store","memory","temp"]),
        // Code / engineering
        ("build",       &["compile","construct","create","make","generate"]),
        ("compile",     &["build","transpile","assemble","process"]),
        ("run",         &["execute","start","launch","invoke"]),
        ("execute",     &["run","perform","invoke","call"]),
        ("create",      &["make","build","generate","new","construct","produce"]),
        ("delete",      &["remove","erase","drop","clear","destroy"]),
        ("update",      &["edit","modify","change","patch","alter","revise"]),
        ("error",       &["bug","fault","issue","problem","exception","failure"]),
        ("fix",         &["repair","resolve","patch","correct","debug","solve"]),
        ("test",        &["check","verify","validate","assert","confirm"]),
        ("deploy",      &["release","publish","ship","launch","push"]),
        ("implement",   &["build","create","develop","code","write","make"]),
        ("library",     &["package","module","crate","dependency","framework"]),
        ("framework",   &["library","toolkit","platform","engine","system"]),
        // Algorithms / CS
        ("algorithm",   &["method","procedure","routine","approach","technique"]),
        ("sort",        &["order","arrange","rank","organize","sequence"]),
        ("parse",       &["analyze","process","read","interpret","decode"]),
        ("tokenize",    &["split","segment","parse","break","divide"]),
        ("stem",        &["normalize","reduce","truncate","root"]),
        // Systems
        ("server",      &["host","service","backend","node","instance"]),
        ("client",      &["user","consumer","caller","requester"]),
        ("network",     &["internet","web","connection","communication"]),
        ("async",       &["asynchronous","concurrent","nonblocking","parallel"]),
        ("concurrent",  &["parallel","async","simultaneous","multi"]),
        ("safe",        &["secure","sound","reliable","robust","stable"]),
        ("type",        &["kind","category","class","sort","form"]),
        ("large",       &["big","huge","massive","giant","enormous","colossal","titanic"]),
        ("small",       &["tiny","compact","lightweight","minimal","little"]),
        ("simple",      &["easy","basic","straightforward","plain"]),
        ("complex",     &["advanced","sophisticated","difficult","intricate"]),
        // General actions
        ("use",         &["apply","employ","utilize","leverage","adopt"]),
        ("support",     &["help","assist","enable","allow","facilitate"]),
        ("provide",     &["give","offer","supply","deliver","produce"]),
        ("require",     &["need","depend","demand","expect"]),
        ("include",     &["contain","add","incorporate","embed"]),
        ("handle",      &["manage","process","deal","control","treat"]),
        ("generate",    &["create","produce","make","output","emit"]),
        ("extract",     &["parse","get","pull","obtain","derive"]),
        ("compute",     &["calculate","process","evaluate","determine"]),
        ("measure",     &["evaluate","assess","benchmark","track","monitor"]),
    ]
}

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ProcessedQuery {
    pub original:    String,
    pub tokens:      Vec<String>,
    pub corrections: Vec<(String, String)>,
    pub expansions:  Vec<(String, Vec<String>)>,
    pub phrases:     Vec<String>,
}

pub struct QueryPreprocessor {
    synonyms: HashMap<String, Vec<String>>,
    phrases:  Vec<(Vec<String>, String)>,
}

impl QueryPreprocessor {
    pub fn new() -> Self {
        // Build synonym map (bidirectional, all stemmed).
        let mut synonyms: HashMap<String, Vec<String>> = HashMap::new();
        for (word, syns) in raw_synonyms() {
            let sw = porter::stem(word);
            for &s in syns.iter() {
                let ss = porter::stem(s);
                synonyms.entry(sw.clone()).or_default().push(ss.clone());
                synonyms.entry(ss).or_default().push(sw.clone());
            }
        }
        // Deduplicate
        for v in synonyms.values_mut() { v.sort(); v.dedup(); }

        // Build phrase map: compound key matches indexer postings (`stem1~stem2`, …).
        let phrases = phrase_dict()
            .into_iter()
            .map(|(words, _)| {
                let stemmed: Vec<String> = words.iter().map(|w| porter::stem(w)).collect();
                let compound = stemmed.join("~");
                (stemmed, compound)
            })
            .collect();

        Self { synonyms, phrases }
    }

    pub fn process(&self, raw: &str, corpus_vocab: &[&str]) -> ProcessedQuery {
        let words: Vec<String> = raw
            .split(|c: char| !c.is_alphabetic())
            .filter(|t| t.len() >= 1)
            .map(|t| t.to_lowercase())
            .collect();

        let mut tokens:      Vec<String>                 = Vec::new();
        let mut corrections: Vec<(String, String)>       = Vec::new();
        let mut expansions:  Vec<(String, Vec<String>)>  = Vec::new();
        let mut phrases_found: Vec<String>               = Vec::new();

        // ── Step 1: phrase detection ──────────────────────────────────────────
        // Walk the word list and greedily match the longest phrase first.
        let stemmed_words: Vec<String> = words.iter().map(|w| porter::stem(w)).collect();
        let mut used = vec![false; words.len()];

        // Try trigrams first, then bigrams.
        for window in [3usize, 2] {
            if stemmed_words.len() < window { continue; }
            for i in 0..=(stemmed_words.len() - window) {
                if used[i..i+window].iter().any(|&u| u) { continue; }
                let slice = &stemmed_words[i..i+window];
                if let Some(compound) = self.find_phrase(slice) {
                    tokens.push(compound.clone());
                    phrases_found.push(compound);
                    for j in i..i+window { used[j] = true; }
                }
            }
        }

        // ── Step 2: individual tokens (unused words) ──────────────────────────
        for (i, word) in words.iter().enumerate() {
            if used[i] { continue; }
            let stem = porter::stem(word);

            // Spell-correct against corpus vocabulary if not found.
            let (final_stem, corrected) = if corpus_vocab.contains(&stem.as_str()) {
                (stem.clone(), None)
            } else {
                match levenshtein_correct(word, &stem, corpus_vocab) {
                    Some(c) if c != stem => (c.clone(), Some((word.clone(), c))),
                    _                   => (stem.clone(), None),
                }
            };

            if let Some(pair) = corrected { corrections.push(pair); }

            // Synonym expansion (synonym must exist in the index to be useful).
            let syns = self.expand_with_vocab(&final_stem, corpus_vocab);
            if !syns.is_empty() {
                expansions.push((final_stem.clone(), syns.clone()));
                for s in syns {
                    if !tokens.contains(&s) { tokens.push(s); }
                }
            }

            if !tokens.contains(&final_stem) { tokens.push(final_stem); }
        }

        tokens.dedup();
        ProcessedQuery { original: raw.to_owned(), tokens, corrections, expansions, phrases: phrases_found }
    }

    fn find_phrase(&self, stems: &[String]) -> Option<String> {
        self.phrases.iter().find_map(|(phrase_stems, compound)| {
            if phrase_stems == stems { Some(compound.clone()) } else { None }
        })
    }

    fn expand_with_vocab(&self, stem: &str, corpus_vocab: &[&str]) -> Vec<String> {
        self.synonyms
            .get(stem)
            .map(|syns| {
                syns.iter()
                    .filter(|s| corpus_vocab.contains(&s.as_str()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for QueryPreprocessor {
    fn default() -> Self { Self::new() }
}

// ── Levenshtein correction ────────────────────────────────────────────────────

fn levenshtein_correct(word: &str, stem: &str, vocab: &[&str]) -> Option<String> {
    let mut best = 0.0_f64;
    let mut found: Option<String> = None;
    for &term in vocab {
        let s1 = strsim::normalized_levenshtein(word, term);
        let s2 = strsim::normalized_levenshtein(stem, term);
        let s  = s1.max(s2);
        if s > best && s >= LEVENSHTEIN_THRESHOLD { best = s; found = Some(term.to_owned()); }
    }
    found
}

// ── Phrase token helpers (used by indexer) ────────────────────────────────────

// Helper used by indexing and query preprocessing.
pub fn bigram_tokens(words: &[&str]) -> Vec<String> {
    words.windows(2)
        .map(|w| format!("{}~{}", porter::stem(w[0]), porter::stem(w[1])))
        .collect()
}

pub fn trigram_tokens(words: &[&str]) -> Vec<String> {
    words.windows(3)
        .map(|w| format!("{}~{}~{}", porter::stem(w[0]), porter::stem(w[1]), porter::stem(w[2])))
        .collect()
}
