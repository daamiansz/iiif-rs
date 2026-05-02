use axum::extract::{Query, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use tracing::info;

use iiif_core::state::AppState;

use crate::index::SearchIndex;
use crate::types::{
    AutocompleteResponse, SearchAnnotation, SearchResponse, TermEntry, TextualBody,
};

const PAGE_SIZE: usize = 50;
const AUTOCOMPLETE_LIMIT: usize = 20;

/// Build the axum router for IIIF Content Search API 2.0 endpoints.
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

/// GET `/search?q=...&motivation=...&page=N`
async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let base = &state.config.server.base_url;
    let query = params.q.unwrap_or_default();
    let motivation = params.motivation.as_deref();
    let page = params.page.unwrap_or(0);

    // `date` and `user` aren't yet parsed (deferred to v0.3.0); silently drop
    // them, but report via `ignored[]` per spec §4.1.1.
    let mut ignored = Vec::new();
    if params.date.is_some() {
        ignored.push("date".to_string());
    }
    if params.user.is_some() {
        ignored.push("user".to_string());
    }

    let search_index = state
        .search
        .as_ref()
        .and_then(|a| a.downcast_ref::<SearchIndex>());

    let (matches, total) = match search_index {
        Some(index) => {
            index.search_paginated(&query, motivation, page * PAGE_SIZE, PAGE_SIZE)
        }
        None => (Vec::new(), 0),
    };

    let items: Vec<SearchAnnotation> = matches
        .into_iter()
        .map(|anno| SearchAnnotation {
            id: anno.id,
            resource_type: "Annotation".to_string(),
            motivation: anno.motivation,
            body: TextualBody {
                body_type: "TextualBody".to_string(),
                value: anno.text,
                format: "text/plain".to_string(),
            },
            target: anno.target,
        })
        .collect();

    let collection_id = build_collection_id(base, &query, motivation);
    let collection_id_for_pages = collection_id.clone();
    let response = SearchResponse::paginated(
        &collection_id,
        items,
        ignored,
        total,
        page,
        PAGE_SIZE,
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
    (StatusCode::OK, search_headers(), json).into_response()
}

/// GET `/autocomplete?q=...&motivation=...&min=N`
async fn autocomplete_handler(
    State(state): State<AppState>,
    Query(params): Query<AutocompleteQuery>,
) -> Response {
    let base = &state.config.server.base_url;
    let prefix = params.q.unwrap_or_default();
    let min_count = params.min.unwrap_or(1);

    // `motivation` is a valid v2 parameter — it's just not honoured by the
    // current term-frequency-only autocomplete. Don't lie about it as ignored.
    let _ = &params.motivation;

    let search_index = state
        .search
        .as_ref()
        .and_then(|a| a.downcast_ref::<SearchIndex>());

    let items: Vec<TermEntry> = match search_index {
        Some(index) => index
            .autocomplete(&prefix, AUTOCOMPLETE_LIMIT)
            .into_iter()
            .filter(|(_, count)| *count >= min_count)
            .map(|(term, count)| TermEntry {
                value: term,
                total: Some(count),
            })
            .collect(),
        None => Vec::new(),
    };

    let request_url = format!("{base}/autocomplete?q={}", url_encode(&prefix));
    let response = AutocompleteResponse::new(&request_url, items, Vec::new());

    info!(prefix = %prefix, terms = response.items.len(), "Autocomplete executed");

    let json = serde_json::to_string(&response).expect("valid json");
    (StatusCode::OK, search_headers(), json).into_response()
}

fn build_collection_id(base: &str, query: &str, motivation: Option<&str>) -> String {
    let mut url = format!("{base}/search?q={}", url_encode(query));
    if let Some(m) = motivation {
        url.push_str("&motivation=");
        url.push_str(&url_encode(m));
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
