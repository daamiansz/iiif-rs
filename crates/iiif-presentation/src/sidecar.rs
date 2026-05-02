//! Optional per-image sidecar metadata loaded from `<images>/<id>.toml`.
//!
//! Format (all fields optional):
//! ```toml
//! label = "The Creation of the World"
//! language = "en"  # default lang for label/summary; defaults to "none"
//! summary = "Detailed depiction of Genesis"
//! rights = "https://creativecommons.org/licenses/by/4.0/"
//!
//! [[metadata]]
//! label = "Date"
//! value = "13th century"
//!
//! [[metadata]]
//! label = "Source"
//! value = "Bibliothèque nationale de France"
//!
//! [provider]
//! id = "http://example.org/bnf"
//! label = "Bibliothèque nationale de France"
//! homepage = "https://www.bnf.fr/"
//! ```

use serde::Deserialize;
use tracing::warn;

use crate::types::{
    lang, ExternalResource, LanguageMap, MetadataEntry, Provider as ManifestProvider,
};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Sidecar {
    pub label: Option<String>,
    /// BCP47 language tag for the bare-string label/summary. Defaults to `none`.
    pub language: Option<String>,
    pub summary: Option<String>,
    pub rights: Option<String>,
    #[serde(default)]
    pub metadata: Vec<MetadataPair>,
    pub provider: Option<SidecarProvider>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetadataPair {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SidecarProvider {
    pub id: String,
    pub label: String,
    pub homepage: Option<String>,
}

impl Sidecar {
    pub fn from_toml_bytes(bytes: &[u8]) -> Option<Self> {
        let s = std::str::from_utf8(bytes).ok()?;
        match toml::from_str::<Self>(s) {
            Ok(sc) => Some(sc),
            Err(e) => {
                warn!(error = %e, "Failed to parse sidecar TOML — ignoring");
                None
            }
        }
    }

    fn default_lang(&self) -> &str {
        self.language.as_deref().unwrap_or("none")
    }

    pub fn label_map(&self) -> Option<LanguageMap> {
        self.label
            .as_ref()
            .map(|s| lang(self.default_lang(), s))
    }

    pub fn summary_map(&self) -> Option<LanguageMap> {
        self.summary
            .as_ref()
            .map(|s| lang(self.default_lang(), s))
    }

    pub fn metadata_entries(&self) -> Option<Vec<MetadataEntry>> {
        if self.metadata.is_empty() {
            return None;
        }
        Some(
            self.metadata
                .iter()
                .map(|m| MetadataEntry {
                    label: lang(self.default_lang(), &m.label),
                    value: lang(self.default_lang(), &m.value),
                })
                .collect(),
        )
    }

    pub fn provider_entries(&self) -> Option<Vec<ManifestProvider>> {
        let p = self.provider.as_ref()?;
        let homepage = p.homepage.as_ref().map(|h| {
            vec![ExternalResource {
                id: h.clone(),
                resource_type: "Text".to_string(),
                label: None,
                format: Some("text/html".to_string()),
                profile: None,
            }]
        });
        Some(vec![ManifestProvider {
            id: p.id.clone(),
            resource_type: "Agent".to_string(),
            label: lang(self.default_lang(), &p.label),
            homepage,
            logo: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_sidecar() {
        let toml = r#"
            label = "Test Image"
            summary = "A test"
        "#;
        let sc = Sidecar::from_toml_bytes(toml.as_bytes()).unwrap();
        assert_eq!(sc.label.as_deref(), Some("Test Image"));
        assert_eq!(sc.summary.as_deref(), Some("A test"));
    }

    #[test]
    fn lang_defaults_to_none() {
        let toml = r#"label = "X""#;
        let sc = Sidecar::from_toml_bytes(toml.as_bytes()).unwrap();
        let map = sc.label_map().unwrap();
        assert_eq!(map["none"], vec!["X".to_string()]);
    }

    #[test]
    fn explicit_language_used() {
        let toml = r#"
            label = "X"
            language = "pl"
        "#;
        let sc = Sidecar::from_toml_bytes(toml.as_bytes()).unwrap();
        let map = sc.label_map().unwrap();
        assert_eq!(map["pl"], vec!["X".to_string()]);
    }

    #[test]
    fn metadata_pairs_become_entries() {
        let toml = r#"
            [[metadata]]
            label = "Date"
            value = "1300"
            [[metadata]]
            label = "Source"
            value = "BnF"
        "#;
        let sc = Sidecar::from_toml_bytes(toml.as_bytes()).unwrap();
        let entries = sc.metadata_entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label["none"], vec!["Date".to_string()]);
        assert_eq!(entries[1].value["none"], vec!["BnF".to_string()]);
    }

    #[test]
    fn provider_section_becomes_agent() {
        let toml = r#"
            [provider]
            id = "http://example.org/bnf"
            label = "BnF"
            homepage = "https://www.bnf.fr/"
        "#;
        let sc = Sidecar::from_toml_bytes(toml.as_bytes()).unwrap();
        let providers = sc.provider_entries().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "http://example.org/bnf");
        assert_eq!(providers[0].resource_type, "Agent");
        assert!(providers[0].homepage.is_some());
    }

    #[test]
    fn malformed_toml_returns_none() {
        assert!(Sidecar::from_toml_bytes(b"definitely not toml = =").is_none());
    }
}
