use std::sync::Arc;

use crate::config::AppConfig;
use crate::storage::ImageStorage;

/// Shared application state passed to all request handlers.
///
/// Minimal by design: only `config` and `storage` are universally required.
/// Optional services (auth store, search index, discovery store, image cache)
/// are wired through `axum::Extension<Arc<T>>` instead — typed at the call site,
/// no `Arc<dyn Any>` downcast at runtime.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub storage: Arc<dyn ImageStorage>,
}
