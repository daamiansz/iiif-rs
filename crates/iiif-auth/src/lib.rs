pub mod handlers;
pub mod middleware;
pub mod store;
pub mod types;

pub use handlers::router;
pub use store::AuthStore;
pub use types::{build_probe_service_descriptor, AUTH_CONTEXT};

// Re-export for convenience.
pub use iiif_core::services::Service;
