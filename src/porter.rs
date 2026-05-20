pub fn stem(word: &str) -> String {
    let word = word.to_lowercase();
    let chars: Vec<char> = word.chars().collect();

    if chars.len() <= 2 {
        return word;
    }

    let mut s: Vec<char> = chars;
    s = step1a(s);
    s = step1b(s);
    s = step1c(s);
    s = step2(s);
    s = step3(s);
    s = step4(s);
    s = step5a(s);
    s = step5b(s);
    s.iter().collect()
}

// ── helpers ────────────────────────────────────────────────────────────────

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

fn is_vowel_in_stem(s: &[char], i: usize) -> bool {
    if is_vowel(s[i]) {
        return true;
    }
    if s[i] == 'y' && i > 0 {
        return !is_vowel(s[i - 1]);
    }
    false
}

fn measure(s: &[char], end: usize) -> usize {
    let mut m = 0;
    let mut i = 0;
    // skip leading vowels
    while i < end && is_vowel_in_stem(s, i) {
        i += 1;
    }
    while i < end {
        // skip consonants
        while i < end && !is_vowel_in_stem(s, i) {
            i += 1;
        }
        // skip vowels
        while i < end && is_vowel_in_stem(s, i) {
            i += 1;
        }
        m += 1;
    }
    m
}

fn contains_vowel(s: &[char], end: usize) -> bool {
    (0..end).any(|i| is_vowel_in_stem(s, i))
}

fn ends_double_consonant(s: &[char]) -> bool {
    let n = s.len();
    n >= 2 && s[n - 1] == s[n - 2] && !is_vowel(s[n - 1])
}

fn ends_cvc(s: &[char]) -> bool {
    let n = s.len();
    if n < 3 {
        return false;
    }
    let c1 = s[n - 3];
    let c2 = s[n - 2];
    let c3 = s[n - 1];
    !is_vowel_in_stem(s, n - 3)
        && is_vowel_in_stem(s, n - 2)
        && !is_vowel_in_stem(s, n - 1)
        && !matches!(c3, 'w' | 'x' | 'y')
        && c1 != c2 // avoid "ll" type patterns — actually not in spec, removing
        // actually spec just says *o = CVC, let's keep simple
        && true
}

fn word_ends_with(s: &[char], suffix: &str) -> bool {
    let suf: Vec<char> = suffix.chars().collect();
    if s.len() < suf.len() {
        return false;
    }
    s[s.len() - suf.len()..] == suf[..]
}

fn replace_suffix(s: &[char], old_len: usize, replacement: &str) -> Vec<char> {
    let mut result = s[..s.len() - old_len].to_vec();
    result.extend(replacement.chars());
    result
}

// ── Step 1a ────────────────────────────────────────────────────────────────
// sses → ss  |  ies → i  |  ss → ss  |  s → (delete)
fn step1a(s: Vec<char>) -> Vec<char> {
    if word_ends_with(&s, "sses") {
        replace_suffix(&s, 4, "ss")
    } else if word_ends_with(&s, "ies") {
        replace_suffix(&s, 3, "i")
    } else if word_ends_with(&s, "ss") {
        s
    } else if word_ends_with(&s, "s") {
        replace_suffix(&s, 1, "")
    } else {
        s
    }
}

// ── Step 1b ────────────────────────────────────────────────────────────────
fn step1b(s: Vec<char>) -> Vec<char> {
    if word_ends_with(&s, "eed") {
        let stem_end = s.len() - 3;
        if measure(&s, stem_end) > 0 {
            replace_suffix(&s, 3, "ee")
        } else {
            s
        }
    } else if word_ends_with(&s, "ed") {
        let stem_end = s.len() - 2;
        if contains_vowel(&s, stem_end) {
            step1b_post(replace_suffix(&s, 2, ""))
        } else {
            s
        }
    } else if word_ends_with(&s, "ing") {
        let stem_end = s.len() - 3;
        if contains_vowel(&s, stem_end) {
            step1b_post(replace_suffix(&s, 3, ""))
        } else {
            s
        }
    } else {
        s
    }
}

fn step1b_post(s: Vec<char>) -> Vec<char> {
    if word_ends_with(&s, "at") {
        replace_suffix(&s, 2, "ate")
    } else if word_ends_with(&s, "bl") {
        replace_suffix(&s, 2, "ble")
    } else if word_ends_with(&s, "iz") {
        replace_suffix(&s, 2, "ize")
    } else if ends_double_consonant(&s)
        && !matches!(s.last(), Some('l') | Some('s') | Some('z'))
    {
        replace_suffix(&s, 1, "")
    } else if measure(&s, s.len()) == 1 && ends_cvc(&s) {
        let mut r = s.clone();
        r.push('e');
        r
    } else {
        s
    }
}

// ── Step 1c ────────────────────────────────────────────────────────────────
// *v* y → i
fn step1c(s: Vec<char>) -> Vec<char> {
    if word_ends_with(&s, "y") && contains_vowel(&s, s.len() - 1) {
        replace_suffix(&s, 1, "i")
    } else {
        s
    }
}

// ── Step 2 ────────────────────────────────────────────────────────────────
fn step2(s: Vec<char>) -> Vec<char> {
    let rules: &[(&str, &str)] = &[
        ("ational", "ate"),
        ("tional", "tion"),
        ("enci", "ence"),
        ("anci", "ance"),
        ("izer", "ize"),
        ("abli", "able"),
        ("alli", "al"),
        ("entli", "ent"),
        ("eli", "e"),
        ("ousli", "ous"),
        ("ization", "ize"),
        ("ation", "ate"),
        ("ator", "ate"),
        ("alism", "al"),
        ("iveness", "ive"),
        ("fulness", "ful"),
        ("ousness", "ous"),
        ("aliti", "al"),
        ("iviti", "ive"),
        ("biliti", "ble"),
    ];
    apply_suffix_rules(s, rules, 1)
}

// ── Step 3 ────────────────────────────────────────────────────────────────
fn step3(s: Vec<char>) -> Vec<char> {
    let rules: &[(&str, &str)] = &[
        ("icate", "ic"),
        ("ative", ""),
        ("alize", "al"),
        ("iciti", "ic"),
        ("ical", "ic"),
        ("ful", ""),
        ("ness", ""),
    ];
    apply_suffix_rules(s, rules, 1)
}

// ── Step 4 ────────────────────────────────────────────────────────────────
fn step4(s: Vec<char>) -> Vec<char> {
    let rules: &[(&str, &str)] = &[
        ("al", ""),
        ("ance", ""),
        ("ence", ""),
        ("er", ""),
        ("ic", ""),
        ("able", ""),
        ("ible", ""),
        ("ant", ""),
        ("ement", ""),
        ("ment", ""),
        ("ent", ""),
        ("ou", ""),
        ("ism", ""),
        ("ate", ""),
        ("iti", ""),
        ("ous", ""),
        ("ive", ""),
        ("ize", ""),
    ];
    // Special case: (m>1 and (*S or *T)) ion → (delete)
    if word_ends_with(&s, "ion") {
        let stem_end = s.len() - 3;
        if measure(&s, stem_end) > 1 {
            if let Some(&last) = s.get(stem_end.wrapping_sub(1)) {
                if last == 's' || last == 't' {
                    return replace_suffix(&s, 3, "");
                }
            }
        }
    }
    apply_suffix_rules(s, rules, 2)
}

// ── Step 5a ────────────────────────────────────────────────────────────────
fn step5a(s: Vec<char>) -> Vec<char> {
    if word_ends_with(&s, "e") {
        let m = measure(&s, s.len() - 1);
        if m > 1 || (m == 1 && !ends_cvc(&s[..s.len() - 1])) {
            return replace_suffix(&s, 1, "");
        }
    }
    s
}

// ── Step 5b ────────────────────────────────────────────────────────────────
fn step5b(s: Vec<char>) -> Vec<char> {
    if measure(&s, s.len()) > 1 && ends_double_consonant(&s) && word_ends_with(&s, "l") {
        replace_suffix(&s, 1, "")
    } else {
        s
    }
}

// ── utility ────────────────────────────────────────────────────────────────
fn apply_suffix_rules(s: Vec<char>, rules: &[(&str, &str)], min_m: usize) -> Vec<char> {
    for (suffix, replacement) in rules {
        if word_ends_with(&s, suffix) {
            let stem_end = s.len() - suffix.len();
            if measure(&s, stem_end) >= min_m {
                return replace_suffix(&s, suffix.len(), replacement);
            }
        }
    }
    s
}

// ── tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_stems() {
        assert_eq!(stem("caresses"), "caress");
        assert_eq!(stem("ponies"), "poni");
        assert_eq!(stem("troubles"), "troubl");
        assert_eq!(stem("hopping"), "hop");
        assert_eq!(stem("tanned"), "tan");
        assert_eq!(stem("motional"), "motion");
        assert_eq!(stem("running"), "run");
        assert_eq!(stem("generalization"), "general");
        assert_eq!(stem("electricity"), "electr");
    }
}
