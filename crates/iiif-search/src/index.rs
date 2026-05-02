use std::collections::HashMap;
use std::sync::RwLock;

/// In-memory inverted index for full-text annotation search.
///
/// Tokens are lowercase words. Search performs AND across query terms.
pub struct SearchIndex {
    /// term → list of annotation indices
    inverted: RwLock<HashMap<String, Vec<usize>>>,
    /// all indexed annotations
    annotations: RwLock<Vec<IndexedAnnotation>>,
}

/// A single annotation stored in the index.
#[derive(Debug, Clone)]
pub struct IndexedAnnotation {
    pub id: String,
    pub text: String,
    pub motivation: String,
    pub target: String,
    pub manifest_id: String,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self {
            inverted: RwLock::new(HashMap::new()),
            annotations: RwLock::new(Vec::new()),
        }
    }

    /// Add an annotation to the index.
    pub fn add(&self, annotation: IndexedAnnotation) {
        let tokens = tokenize(&annotation.text);
        let mut annotations = self.annotations.write().expect("annotations lock");
        let idx = annotations.len();
        annotations.push(annotation);

        let mut inverted = self.inverted.write().expect("inverted lock");
        for token in tokens {
            inverted.entry(token).or_default().push(idx);
        }
    }

    /// Search for annotations matching all query terms.
    /// Optionally filter by a single motivation (back-compat helper for tests).
    pub fn search(&self, query: &str, motivation: Option<&str>) -> Vec<IndexedAnnotation> {
        let motivations = motivation.map(|m| vec![m.to_string()]);
        let (results, _total) =
            self.search_paginated(query, motivations.as_deref(), 0, usize::MAX);
        results
    }

    /// Search returning a page of results plus the total match count for paging.
    ///
    /// `motivations` (Search 2.0): if `Some(&[m1, m2, ...])`, match if the
    /// annotation's motivation is *any* of the listed values (OR-semantics).
    /// `None` or `Some(&[])` disables filtering.
    pub fn search_paginated(
        &self,
        query: &str,
        motivations: Option<&[String]>,
        offset: usize,
        limit: usize,
    ) -> (Vec<IndexedAnnotation>, usize) {
        let terms = tokenize(query);
        if terms.is_empty() {
            return (Vec::new(), 0);
        }

        let inverted = self.inverted.read().expect("inverted lock");
        let annotations = self.annotations.read().expect("annotations lock");

        let mut result_indices: Option<Vec<usize>> = None;
        for term in &terms {
            let matching = inverted.get(term).cloned().unwrap_or_default();
            result_indices = Some(match result_indices {
                Some(current) => current
                    .into_iter()
                    .filter(|i| matching.contains(i))
                    .collect(),
                None => matching,
            });
        }
        let indices = result_indices.unwrap_or_default();

        let motivation_filter = motivations.filter(|m| !m.is_empty());

        let filtered: Vec<&IndexedAnnotation> = indices
            .into_iter()
            .filter_map(|i| {
                let anno = annotations.get(i)?;
                if let Some(allowed) = motivation_filter {
                    if !allowed.iter().any(|m| m == &anno.motivation) {
                        return None;
                    }
                }
                Some(anno)
            })
            .collect();

        let total = filtered.len();
        let page: Vec<IndexedAnnotation> = filtered
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();

        (page, total)
    }

    /// Find terms matching a prefix, with occurrence counts.
    pub fn autocomplete(&self, prefix: &str, limit: usize) -> Vec<(String, usize)> {
        let prefix_lower = prefix.to_lowercase();
        let inverted = self.inverted.read().expect("inverted lock");

        let mut results: Vec<(String, usize)> = inverted
            .iter()
            .filter(|(term, _)| term.starts_with(&prefix_lower))
            .map(|(term, indices)| (term.clone(), indices.len()))
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        results.truncate(limit);
        results
    }

    /// Number of indexed annotations.
    pub fn len(&self) -> usize {
        self.annotations.read().expect("annotations lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Tokenize text into lowercase words.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// Locate every case-insensitive occurrence of `term` in `text`. Returns
/// (start, end) byte ranges into the *original* text. Used by the search
/// handler to build `TextQuoteSelector` snippets for hit augmentation.
///
/// Lowercase-folding is naive (ASCII-clean, mostly fine for Latin scripts);
/// for scripts where casefold changes byte length (German `ß` → `ss`) the
/// offsets may drift. Acceptable for v0.3.0; revisit when we add a real
/// linguistic tokenizer.
pub fn find_term_positions(text: &str, term: &str) -> Vec<(usize, usize)> {
    let lower_text = text.to_lowercase();
    let lower_term = term.to_lowercase();
    if lower_term.is_empty() || lower_term.len() != term.len() {
        // If casefolding changed length we can't safely map back to byte offsets.
        // Fall back to no positions (consumer falls back to bare `exact`).
        return Vec::new();
    }
    let mut positions = Vec::new();
    let mut start = 0;
    while start <= lower_text.len() {
        match lower_text[start..].find(&lower_term) {
            Some(idx) => {
                let abs_start = start + idx;
                let abs_end = abs_start + lower_term.len();
                // Verify abs_start lies on a char boundary in the original text;
                // tokeniser prevented mid-codepoint splits, but search input is
                // arbitrary, so be defensive.
                if text.is_char_boundary(abs_start) && text.is_char_boundary(abs_end) {
                    positions.push((abs_start, abs_end));
                }
                start = abs_end;
                if abs_end == abs_start {
                    break;
                }
            }
            None => break,
        }
    }
    positions
}

/// Take up to `max_chars` characters from one end of the slice on a UTF-8
/// boundary. `from_end = true` returns the last N chars; `false` returns the
/// first N. Used to trim `prefix`/`suffix` for `TextQuoteSelector`.
pub fn trim_to_chars(s: &str, max_chars: usize, from_end: bool) -> &str {
    if max_chars == 0 || s.is_empty() {
        return "";
    }
    if from_end {
        for (count, (idx, _)) in s.char_indices().rev().enumerate() {
            if count + 1 == max_chars {
                return &s[idx..];
            }
        }
        s
    } else {
        let mut end = s.len();
        for (count, (idx, _)) in s.char_indices().enumerate() {
            if count == max_chars {
                end = idx;
                break;
            }
        }
        &s[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_annotation(id: &str, text: &str, target: &str) -> IndexedAnnotation {
        IndexedAnnotation {
            id: id.to_string(),
            text: text.to_string(),
            motivation: "painting".to_string(),
            target: target.to_string(),
            manifest_id: "manifest1".to_string(),
        }
    }

    #[test]
    fn search_single_term() {
        let idx = SearchIndex::new();
        idx.add(sample_annotation("a1", "The quick brown fox", "canvas1"));
        idx.add(sample_annotation("a2", "A slow brown dog", "canvas2"));
        idx.add(sample_annotation("a3", "The quick red cat", "canvas3"));

        let results = idx.search("brown", None);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_multiple_terms_and_logic() {
        let idx = SearchIndex::new();
        idx.add(sample_annotation("a1", "The quick brown fox", "canvas1"));
        idx.add(sample_annotation("a2", "A slow brown dog", "canvas2"));

        let results = idx.search("quick brown", None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a1");
    }

    #[test]
    fn search_with_motivation_filter() {
        let idx = SearchIndex::new();
        idx.add(IndexedAnnotation {
            id: "a1".to_string(),
            text: "Hello world".to_string(),
            motivation: "painting".to_string(),
            target: "c1".to_string(),
            manifest_id: "m1".to_string(),
        });
        idx.add(IndexedAnnotation {
            id: "a2".to_string(),
            text: "Hello again".to_string(),
            motivation: "commenting".to_string(),
            target: "c2".to_string(),
            manifest_id: "m1".to_string(),
        });

        let results = idx.search("hello", Some("commenting"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a2");
    }

    #[test]
    fn search_empty_query() {
        let idx = SearchIndex::new();
        idx.add(sample_annotation("a1", "Some text", "c1"));
        assert!(idx.search("", None).is_empty());
    }

    #[test]
    fn autocomplete_prefix() {
        let idx = SearchIndex::new();
        idx.add(sample_annotation("a1", "bird birthday biryani", "c1"));
        idx.add(sample_annotation("a2", "bird watching", "c2"));

        let results = idx.autocomplete("bir", 10);
        assert!(results.iter().any(|(t, _)| t == "bird"));
        assert!(results.iter().any(|(t, _)| t == "birthday"));
        assert!(results.iter().any(|(t, _)| t == "biryani"));

        // "bird" appears in 2 annotations
        let bird = results.iter().find(|(t, _)| t == "bird").unwrap();
        assert_eq!(bird.1, 2);
    }

    #[test]
    fn autocomplete_no_match() {
        let idx = SearchIndex::new();
        idx.add(sample_annotation("a1", "hello world", "c1"));
        assert!(idx.autocomplete("xyz", 10).is_empty());
    }

    #[test]
    fn find_positions_basic() {
        let text = "The quick brown fox and the brown dog.";
        let positions = find_term_positions(text, "brown");
        assert_eq!(positions, vec![(10, 15), (28, 33)]);
    }

    #[test]
    fn find_positions_case_insensitive() {
        let text = "Brown bears, brown rivers, BROWN sky";
        let positions = find_term_positions(text, "brown");
        assert_eq!(positions.len(), 3);
        // Each position must point at the original (case-preserved) substring.
        assert_eq!(&text[positions[0].0..positions[0].1], "Brown");
        assert_eq!(&text[positions[1].0..positions[1].1], "brown");
        assert_eq!(&text[positions[2].0..positions[2].1], "BROWN");
    }

    #[test]
    fn find_positions_empty_query() {
        assert!(find_term_positions("anything", "").is_empty());
    }

    #[test]
    fn trim_to_chars_first_n() {
        assert_eq!(trim_to_chars("hello world", 5, false), "hello");
        assert_eq!(trim_to_chars("café shop", 4, false), "café");
        assert_eq!(trim_to_chars("short", 100, false), "short");
    }

    #[test]
    fn trim_to_chars_last_n() {
        assert_eq!(trim_to_chars("hello world", 5, true), "world");
        assert_eq!(trim_to_chars("café shop", 4, true), "shop");
    }

    #[test]
    fn case_insensitive() {
        let idx = SearchIndex::new();
        idx.add(sample_annotation("a1", "The Creation of Adam", "c1"));

        assert_eq!(idx.search("creation", None).len(), 1);
        assert_eq!(idx.search("CREATION", None).len(), 1);
        assert_eq!(idx.search("Creation", None).len(), 1);
    }
}
