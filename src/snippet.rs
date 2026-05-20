use crate::porter;

const PRE_CONTEXT_WORDS: usize = 10;

pub fn extract_snippet(body: &str, matched_stems: &[String]) -> String {
    if body.is_empty() { return String::new(); }
    if matched_stems.is_empty() {
        return html_escape(&first_two_sentences(body));
    }

    // Find the byte position of the first matched term.
    let match_pos = first_match_pos(body, matched_stems);

    // Determine snippet start.
    let start = snippet_start(body, match_pos);

    // Determine snippet end (2 periods after start).
    let end = snippet_end(body, start);

    let raw = &body[start..end];
    highlight(raw, matched_stems)
        + if end < body.len() { "…" } else { "" }
}

// ── internals ─────────────────────────────────────────────────────────────────

fn first_match_pos(text: &str, stems: &[String]) -> usize {
    let spans = word_spans(text);
    for (s, e) in &spans {
        let stem = porter::stem(&text[*s..*e]);
        if stems.contains(&stem) { return *s; }
    }
    0
}

fn snippet_start(text: &str, pos: usize) -> usize {
    let before = &text[..pos];

    // Look for the last period (sentence boundary) before the match.
    if let Some(idx) = before.rfind(|c| c == '.' || c == '!' || c == '?') {
        // Skip the period and any following whitespace.
        let after_period = idx + 1;
        let trimmed = text[after_period..pos].trim_start();
        let skip = pos - after_period - trimmed.len();
        // Only use this if it's a reasonable distance (not the very start).
        if after_period + skip > 0 {
            return after_period + skip;
        }
    }

    // Fallback: go back PRE_CONTEXT_WORDS words.
    let spans = word_spans(before);
    if spans.len() <= PRE_CONTEXT_WORDS {
        return 0;
    }
    let start_idx = spans.len() - PRE_CONTEXT_WORDS;
    spans[start_idx].0
}

fn snippet_end(text: &str, start: usize) -> usize {
    let mut periods = 0usize;
    let slice = &text[start..];
    let mut last = slice.len(); // default: full remaining text

    for (i, c) in slice.char_indices() {
        if c == '.' || c == '!' || c == '?' {
            periods += 1;
            if periods >= 2 {
                last = i + c.len_utf8();
                break;
            }
        }
    }

    (start + last).min(text.len())
}

fn first_two_sentences(text: &str) -> String {
    let end = snippet_end(text, 0);
    text[..end].to_owned()
}

// ── Word spans + highlight ─────────────────────────────────────────────────────

fn word_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if c.is_alphabetic() {
            if start.is_none() { start = Some(i); }
        } else if let Some(s) = start.take() {
            spans.push((s, i));
        }
    }
    if let Some(s) = start { spans.push((s, text.len())); }
    spans
}

fn highlight(text: &str, stems: &[String]) -> String {
    let spans = word_spans(text);
    let mut out  = String::with_capacity(text.len() * 2);
    let mut last = 0usize;
    for (start, end) in spans {
        let word = &text[start..end];
        let stem = porter::stem(word);
        out.push_str(&html_escape(&text[last..start]));
        if stems.contains(&stem) {
            out.push_str("<mark>");
            out.push_str(&html_escape(word));
            out.push_str("</mark>");
        } else {
            out.push_str(&html_escape(word));
        }
        last = end;
    }
    out.push_str(&html_escape(&text[last..]));
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}
