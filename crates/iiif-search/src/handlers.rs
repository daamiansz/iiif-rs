use axum::extract::{Query, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use tracing::info;

use iiif_core::state::AppState;

use crate::index::SearchIndex;
use crate::types::{
    AutocompleteResponse, SearchAnnotation, SearchResponse, TermEntry, TextualBody,
};

/// Build the axum router for IIIF Content Search API 2.0 endpoints.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/search", get(search_handler))
        .route("/autocomplete", get(autocomplete_handler))
}

// ---------------------------------------------------------------------------
// Query parameters
// ---------------------------------------------------------------------------

/// Supported search query parameters per IIIF Content Search API 2.0.
#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    motivation: Option<String>,
    date: Option<String>,
    user: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AutocompleteQuery {
    q: Option<String>,
    motivation: Option<String>,
    min: Option<usize>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET `/search?q=...&motivation=...`
async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let base = &state.config.server.base_url;
    let query = params.q.unwrap_or_default();
    let motivation = params.motivation.as_deref();

    // Build ignored list for unsupported params
    let mut ignored = Vec::new();
    if params.date.is_some() {
        ignored.push("date".to_string());
    }
    if params.user.is_some() {
        ignored.push("user".to_string());
    }

    // Get search index from AppState extensions
    let search_index = state
        .search
        .as_ref()
        .and_then(|a| a.downcast_ref::<SearchIndex>());

    let items = match search_index {
        Some(index) => {
            let results = index.search(&query, motivation);
            results
                .into_iter()
                .enumerate()
                .map(|(i, anno)| SearchAnnotation {
                    id: format!("{base}/annotation/search/{i}"),
                    resource_type: "Annotation".to_string(),
                    motivation: anno.motivation,
                    body: TextualBody {
                        body_type: "TextualBody".to_string(),
                        value: anno.text,
                        format: "text/plain".to_string(),
                    },
                    target: anno.target,
                })
                .collect()
        }
        None => Vec::new(),
    };

    let request_url = format!("{base}/search?q={}", urlencoded(&query));
    let response = SearchResponse::new(&request_url, items, ignored);

    info!(query = %query, hits = response.items.len(), "Search executed");

    let json = serde_json::to_string(&response).expect("valid json");
    (StatusCode::OK, search_headers(), json).into_response()
}

/// GET `/autocomplete?q=...`
async fn autocomplete_handler(
    State(state): State<AppState>,
    Query(params): Query<AutocompleteQuery>,
) -> Response {
    let base = &state.config.server.base_url;
    let prefix = params.q.unwrap_or_default();
    let min_count = params.min.unwrap_or(1);

    let mut ignored = Vec::new();
    if params.motivation.is_some() {
        ignored.push("motivation".to_string());
    }

    let search_index = state
        .search
        .as_ref()
        .and_then(|a| a.downcast_ref::<SearchIndex>());

    let items: Vec<TermEntry> = match search_index {
        Some(index) => index
            .autocomplete(&prefix, 20)
            .into_iter()
            .filter(|(_, count)| *count >= min_count)
            .map(|(term, count)| TermEntry {
                value: term,
                total: Some(count),
            })
            .collect(),
        None => Vec::new(),
    };

    let request_url = format!("{base}/autocomplete?q={}", urlencoded(&prefix));
    let response = AutocompleteResponse::new(&request_url, items, ignored);

    info!(prefix = %prefix, terms = response.items.len(), "Autocomplete executed");

    let json = serde_json::to_string(&response).expect("valid json");
    (StatusCode::OK, search_headers(), json).into_response()
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

fn urlencoded(s: &str) -> String {
    s.replace(' ', "+").replace('&', "%26").replace('=', "%3D")
}
