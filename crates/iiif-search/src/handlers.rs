use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use chrono::{DateTime, Utc};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::info;

use iiif_core::annotation::{AnnotationTarget, Selector, SpecificResource};
use iiif_core::error::IiifError;
use iiif_core::state::AppState;

use crate::index::{find_term_positions, trim_to_chars, SearchIndex};
use crate::types::{
    AutocompleteResponse, HitAnnotation, HitAnnotationPage, SearchAnnotation, SearchResponse,
    TermEntry, TextualBody,
};

const PAGE_SIZE: usize = 50;
const AUTOCOMPLETE_LIMIT: usize = 20;
const SNIPPET_CONTEXT_CHARS: usize = 30;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/search", get(search_handler))
        .route("/autocomplete", get(autocomplete_handler))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    motivation: Option<String>,
    date: Option<String>,
    user: Option<String>,
    page: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AutocompleteQuery {
    q: Option<String>,
    motivation: Option<String>,
    min: Option<usize>,
}

/// GET `/search?q=...&motivation=...&date=...&user=...&page=N`
async fn search_handler(
    State(state): State<AppState>,
    Extension(index): Extension<Arc<SearchIndex>>,
    Query(params): Query<SearchQuery>,
) -> Result<Response, IiifError> {
    let base = &state.config.server.base_url;
    let query = params.q.unwrap_or_default();
    let page = params.page.unwrap_or(0);

    // motivation: space-separated, OR-semantics per spec §4.1.1.
    let motivations: Option<Vec<String>> = params
        .motivation
        .as_deref()
        .map(split_space_separated)
        .filter(|v: &Vec<String>| !v.is_empty());

    // date: ISO 8601 range `start/end` with mandatory `Z` suffix. Reject
    // malformed dates with 400; otherwise pass through (the in-memory index
    // does not yet filter by date — we just validate syntax).
    let mut ignored = Vec::new();
    if let Some(d) = params.date.as_deref() {
        if !is_valid_date_range(d) {
            return Err(IiifError::BadRequest(format!(
                "Invalid `date` parameter: expected `YYYY-MM-DDThh:mm:ssZ/YYYY-MM-DDThh:mm:ssZ`, got `{d}`"
            )));
        }
        // Recognised but not honoured by the in-memory backend.
        ignored.push("date".to_string());
    }
    // user: space-separated URIs, OR-semantics — recognised parameter but the
    // in-memory backend has no user metadata, so all values are pass-through.
    if params.user.is_some() {
        ignored.push("user".to_string());
    }

    let (matches, total) = index.search_paginated(
        &query,
        motivations.as_deref(),
        page * PAGE_SIZE,
        PAGE_SIZE,
    );

    let query_terms: Vec<String> = query
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let mut items: Vec<SearchAnnotation> = Vec::with_capacity(matches.len());
    let mut hits: Vec<HitAnnotation> = Vec::new();

    for anno in matches.iter() {
        let item_id = anno.id.clone();
        items.push(SearchAnnotation {
            id: item_id.clone(),
            resource_type: "Annotation".to_string(),
            motivation: anno.motivation.clone(),
            body: TextualBody {
                body_type: "TextualBody".to_string(),
                value: anno.text.clone(),
                format: "text/plain".to_string(),
            },
            target: AnnotationTarget::Id(anno.target.clone()),
        });

        // Hit augmentation: one Annotation per (term, position) pair, with a
        // TextQuoteSelector pinning prefix/exact/suffix into the matched body.
        for term in &query_terms {
            for (start, end) in find_term_positions(&anno.text, term) {
                let prefix = trim_to_chars(&anno.text[..start], SNIPPET_CONTEXT_CHARS, true);
                let suffix = trim_to_chars(&anno.text[end..], SNIPPET_CONTEXT_CHARS, false);
                let exact = anno.text[start..end].to_string();
                // Stable hit ID — derived from (source-annotation, term, position).
                // Same hit converges on the same URI regardless of which query
                // surfaced it, so client caches stay consistent.
                let hit_id = format!(
                    "{base}/annotation/hit/{}",
                    short_hash(&format!("{item_id}|{term}|{start}"))
                );
                hits.push(HitAnnotation {
                    id: hit_id,
                    resource_type: "Annotation".to_string(),
                    motivation: "contextualizing".to_string(),
                    target: AnnotationTarget::Specific(SpecificResource::new(
                        item_id.clone(),
                        Selector::TextQuoteSelector {
                            prefix: if prefix.is_empty() {
                                None
                            } else {
                                Some(prefix.to_string())
                            },
                            exact,
                            suffix: if suffix.is_empty() {
                                None
                            } else {
                                Some(suffix.to_string())
                            },
                        },
                    )),
                });
            }
        }
    }

    let collection_id = build_collection_id(base, &query, motivations.as_deref());
    let collection_id_for_pages = collection_id.clone();
    let hit_pages = if hits.is_empty() {
        None
    } else {
        Some(vec![HitAnnotationPage {
            id: format!("{collection_id}#hits-page-{page}"),
            resource_type: "AnnotationPage".to_string(),
            items: hits,
        }])
    };

    let response = SearchResponse::paginated(
        &collection_id,
        items,
        ignored,
        total,
        page,
        PAGE_SIZE,
        hit_pages,
        move |p| {
            if p == 0 {
                collection_id_for_pages.clone()
            } else {
                format!("{collection_id_for_pages}&page={p}")
            }
        },
    );

    info!(query = %query, page, hits = response.items.len(), total, "Search executed");

    let json = serde_json::to_string(&response).expect("valid json");
    Ok((StatusCode::OK, search_headers(), json).into_response())
}

/// GET `/autocomplete?q=...&motivation=...&min=N`
async fn autocomplete_handler(
    State(state): State<AppState>,
    Extension(index): Extension<Arc<SearchIndex>>,
    Query(params): Query<AutocompleteQuery>,
) -> Response {
    let base = &state.config.server.base_url;
    let prefix = params.q.unwrap_or_default();
    let min_count = params.min.unwrap_or(1);

    let _ = &params.motivation;

    let items: Vec<TermEntry> = index
        .autocomplete(&prefix, AUTOCOMPLETE_LIMIT)
        .into_iter()
        .filter(|(_, count)| *count >= min_count)
        .map(|(term, count)| TermEntry {
            value: term,
            total: Some(count),
        })
        .collect();

    let request_url = format!("{base}/autocomplete?q={}", url_encode(&prefix));
    let response = AutocompleteResponse::new(&request_url, items, Vec::new());

    info!(prefix = %prefix, terms = response.items.len(), "Autocomplete executed");

    let json = serde_json::to_string(&response).expect("valid json");
    (StatusCode::OK, search_headers(), json).into_response()
}

/// Short stable hex hash for hit-annotation IDs (8 bytes / 16 hex chars).
fn short_hash(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn split_space_separated(s: &str) -> Vec<String> {
    s.split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Validate ISO 8601 date range: `start/end` where each side parses as a
/// UTC `YYYY-MM-DDThh:mm:ssZ` instant per Content Search 2.0 §4.1.1.
fn is_valid_date_range(value: &str) -> bool {
    let Some((start, end)) = value.split_once('/') else {
        return false;
    };
    parse_iso8601_utc(start).is_some() && parse_iso8601_utc(end).is_some()
}

fn parse_iso8601_utc(s: &str) -> Option<DateTime<Utc>> {
    if !s.ends_with('Z') {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn build_collection_id(base: &str, query: &str, motivations: Option<&[String]>) -> String {
    let mut url = format!("{base}/search?q={}", url_encode(query));
    if let Some(m) = motivations {
        if !m.is_empty() {
            url.push_str("&motivation=");
            url.push_str(&url_encode(&m.join(" ")));
        }
    }
    url
}

fn url_encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn search_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "application/ld+json;profile=\"http://iiif.io/api/search/2/context.json\""
            .parse()
            .expect("valid"),
    );
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().expect("valid"));
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_range_validation() {
        assert!(is_valid_date_range(
            "2026-01-01T00:00:00Z/2026-12-31T23:59:59Z"
        ));
        // Missing Z suffix
        assert!(!is_valid_date_range(
            "2026-01-01T00:00:00/2026-12-31T23:59:59Z"
        ));
        // Missing slash
        assert!(!is_valid_date_range("2026-01-01T00:00:00Z"));
        // Empty side
        assert!(!is_valid_date_range("/2026-12-31T23:59:59Z"));
        // Garbage
        assert!(!is_valid_date_range("yesterday/today"));
    }

    #[test]
    fn split_space_separated_produces_or_terms() {
        assert_eq!(
            split_space_separated("painting commenting"),
            vec!["painting".to_string(), "commenting".to_string()]
        );
        assert!(split_space_separated("   ").is_empty());
    }

    #[test]
    fn hit_id_is_stable_across_queries() {
        // Same source annotation, term, and position must hash to the same URI
        // regardless of which query surfaced the hit.
        let id1 = short_hash("anno-42|creation|17");
        let id2 = short_hash("anno-42|creation|17");
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);

        // Different inputs produce different hashes.
        assert_ne!(short_hash("anno-42|creation|17"), short_hash("anno-42|world|17"));
        assert_ne!(short_hash("anno-42|creation|17"), short_hash("anno-43|creation|17"));
    }
}
