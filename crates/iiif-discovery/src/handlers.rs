use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tracing::info;

use iiif_core::error::IiifError;
use iiif_core::state::AppState;

use crate::store::ActivityStore;
use crate::types::{OrderedCollection, OrderedCollectionPage, PageRef, DISCOVERY_CONTEXT};

/// Build the axum router for IIIF Change Discovery API 1.0.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/activity/all-changes", get(collection_handler))
        .route("/activity/page/{page}", get(page_handler))
}

/// GET `/activity/all-changes` — return the OrderedCollection.
async fn collection_handler(
    State(state): State<AppState>,
    Extension(store): Extension<Arc<ActivityStore>>,
) -> Result<Response, IiifError> {
    let base = &state.config.server.base_url;

    let total = store.total();
    let page_count = store.page_count();

    let collection = OrderedCollection::new(base, total, page_count);
    let json = serde_json::to_string(&collection)
        .map_err(|e| IiifError::Internal(format!("JSON error: {e}")))?;

    info!(
        total_items = total,
        pages = page_count,
        "Served activity collection"
    );

    Ok((StatusCode::OK, discovery_headers(), json).into_response())
}

/// GET `/activity/page/{page}` — return an OrderedCollectionPage.
async fn page_handler(
    State(state): State<AppState>,
    Extension(store): Extension<Arc<ActivityStore>>,
    Path(page): Path<usize>,
) -> Result<Response, IiifError> {
    let base = &state.config.server.base_url;

    let page_count = store.page_count();

    if page >= page_count {
        return Err(IiifError::NotFound(format!("Page {page} not found")));
    }

    let items = store.get_page(page);
    let start_index = page * store.page_size();

    let prev = if page > 0 {
        Some(PageRef {
            id: format!("{base}/activity/page/{}", page - 1),
            resource_type: "OrderedCollectionPage".to_string(),
        })
    } else {
        None
    };

    let next = if page + 1 < page_count {
        Some(PageRef {
            id: format!("{base}/activity/page/{}", page + 1),
            resource_type: "OrderedCollectionPage".to_string(),
        })
    } else {
        None
    };

    let collection_page = OrderedCollectionPage {
        context: DISCOVERY_CONTEXT.to_string(),
        id: format!("{base}/activity/page/{page}"),
        resource_type: "OrderedCollectionPage".to_string(),
        start_index,
        part_of: PageRef {
            id: format!("{base}/activity/all-changes"),
            resource_type: "OrderedCollection".to_string(),
        },
        ordered_items: items,
        prev,
        next,
    };

    let json = serde_json::to_string(&collection_page)
        .map_err(|e| IiifError::Internal(format!("JSON error: {e}")))?;

    info!(
        page,
        items = collection_page.ordered_items.len(),
        "Served activity page"
    );

    Ok((StatusCode::OK, discovery_headers(), json).into_response())
}

fn discovery_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "application/ld+json;profile=\"http://iiif.io/api/discovery/1/context.json\""
            .parse()
            .expect("valid"),
    );
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().expect("valid"));
    headers
}

