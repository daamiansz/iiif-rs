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
    /// Optionally filter by motivation.
    pub fn search(&self, query: &str, motivation: Option<&str>) -> Vec<IndexedAnnotation> {
        let terms = tokenize(query);
        if terms.is_empty() {
            return Vec::new();
        }

        let inverted = self.inverted.read().expect("inverted lock");
        let annotations = self.annotations.read().expect("annotations lock");

        // Find indices that contain ALL terms (AND logic)
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

        indices
            .into_iter()
            .filter_map(|i| {
                let anno = annotations.get(i)?;
                if let Some(mot) = motivation {
                    if anno.motivation != mot {
                        return None;
                    }
                }
                Some(anno.clone())
            })
            .collect()
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
    fn case_insensitive() {
        let idx = SearchIndex::new();
        idx.add(sample_annotation("a1", "The Creation of Adam", "c1"));

        assert_eq!(idx.search("creation", None).len(), 1);
        assert_eq!(idx.search("CREATION", None).len(), 1);
        assert_eq!(idx.search("Creation", None).len(), 1);
    }
}
