use std::any::Any;
use std::sync::Arc;

use crate::config::AppConfig;
use crate::storage::ImageStorage;

/// Shared application state passed to all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub storage: Arc<dyn ImageStorage>,
    /// Optional auth store (type-erased to avoid coupling iiif-core to iiif-auth).
    pub auth: Option<Arc<dyn Any + Send + Sync>>,
    /// Optional search index (type-erased).
    pub search: Option<Arc<dyn Any + Send + Sync>>,
    /// Optional activity/discovery store (type-erased).
    pub discovery: Option<Arc<dyn Any + Send + Sync>>,
    /// Optional response cache (type-erased moka::sync::Cache).
    pub cache: Option<Arc<dyn Any + Send + Sync>>,
}
