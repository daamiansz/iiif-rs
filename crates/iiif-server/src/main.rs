use std::net::SocketAddr;
use std::sync::Arc;

use axum::middleware;
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use iiif_auth::middleware::{check_access, CookieName};
use iiif_auth::AuthStore;
use iiif_core::config::AppConfig;
use iiif_core::state::AppState;
use iiif_core::storage::filesystem::FilesystemStorage;
use iiif_discovery::ActivityStore;
use iiif_search::index::IndexedAnnotation;
use iiif_search::SearchIndex;

#[tokio::main]
async fn main() {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Load configuration
    let config = load_config().unwrap_or_else(|e| {
        error!("Failed to load configuration: {e}");
        std::process::exit(1);
    });

    info!(
        host = %config.server.host,
        port = config.server.port,
        base_url = %config.server.base_url,
        storage_path = %config.storage.root_path,
        auth_enabled = config.auth.enabled,
        "Starting IIIF server"
    );

    // Initialize storage
    let storage = FilesystemStorage::new(&config.storage.root_path).unwrap_or_else(|e| {
        error!("Failed to initialize storage: {e}");
        std::process::exit(1);
    });

    // Initialize auth store (if enabled)
    let auth_store: Option<Arc<AuthStore>> = if config.auth.enabled {
        info!(
            pattern = %config.auth.pattern,
            protected_dirs = ?config.auth.protected_dirs,
            "Authorization Flow enabled"
        );
        Some(Arc::new(AuthStore::new(config.auth.token_ttl)))
    } else {
        None
    };

    // Initialize search index with sample annotations
    let search_index = Arc::new(SearchIndex::new());
    seed_search_index(&search_index, &config.server.base_url);
    info!(annotations = search_index.len(), "Search index initialized");

    // Initialize change discovery store with seed activities
    let activity_store = Arc::new(ActivityStore::new(20));
    seed_activities(&activity_store, &config.server.base_url);
    info!(
        activities = activity_store.total(),
        "Activity store initialized"
    );

    let state = AppState {
        config: Arc::new(config.clone()),
        storage: Arc::new(storage),
        auth: auth_store
            .clone()
            .map(|s| s as Arc<dyn std::any::Any + Send + Sync>),
        search: Some(search_index as Arc<dyn std::any::Any + Send + Sync>),
        discovery: Some(activity_store as Arc<dyn std::any::Any + Send + Sync>),
    };

    // Build application with middleware
    let mut app = Router::new()
        .merge(iiif_image::router())
        .merge(iiif_presentation::router())
        .merge(iiif_search::router())
        .merge(iiif_state::router())
        .merge(iiif_discovery::router());

    // Add auth routes and middleware if enabled
    if config.auth.enabled {
        app = app.merge(iiif_auth::router());

        // Apply auth middleware — inject config, cookie name, storage, and auth store
        let auth_config = config.auth.clone();
        let cookie_name = CookieName(config.auth.cookie_name.clone());
        let auth_store_mw = auth_store.clone();
        let storage_for_mw: Arc<dyn iiif_core::storage::ImageStorage> = Arc::clone(&state.storage);
        app = app.layer(middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                req.extensions_mut().insert(auth_config.clone());
                req.extensions_mut().insert(cookie_name.clone());
                req.extensions_mut().insert(Arc::clone(&storage_for_mw));
                if let Some(ref store) = auth_store_mw {
                    req.extensions_mut().insert(Arc::clone(store));
                }
                check_access(req, next)
            },
        ));
    }

    let app = app
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .unwrap_or_else(|e| {
            error!("Invalid bind address: {e}");
            std::process::exit(1);
        });

    info!(%addr, "IIIF server listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to bind to {addr}: {e}");
            std::process::exit(1);
        });

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            error!("Server error: {e}");
            std::process::exit(1);
        });

    info!("Server shut down gracefully");
}

fn load_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let config_path = std::env::var("IIIF_CONFIG").unwrap_or_else(|_| "config.toml".to_string());

    // Load base config from file (or use defaults if file missing)
    let mut config: AppConfig = match std::fs::read_to_string(&config_path) {
        Ok(contents) => toml::from_str(&contents)
            .map_err(|e| format!("Invalid config file '{config_path}': {e}"))?,
        Err(_) => {
            tracing::warn!(
                path = %config_path,
                "Config file not found, using defaults + environment variables"
            );
            AppConfig::default()
        }
    };

    // Override with environment variables (take precedence over file)
    apply_env_overrides(&mut config);

    Ok(config)
}

/// Apply environment variable overrides to the configuration.
///
/// Supported variables:
///   IIIF_HOST           — bind address (default: 127.0.0.1)
///   IIIF_PORT           — bind port (default: 8080)
///   IIIF_BASE_URL       — public base URL
///   IIIF_STORAGE_PATH   — path to images directory
///   IIIF_MAX_WIDTH      — maximum image width
///   IIIF_MAX_HEIGHT     — maximum image height
///   IIIF_MAX_AREA       — maximum pixel area (w*h)
///   IIIF_ALLOW_UPSCALING — "true" or "false"
///   IIIF_TILE_WIDTH     — tile width for info.json
///   IIIF_AUTH_ENABLED   — "true" or "false"
///   IIIF_AUTH_COOKIE    — auth cookie name
///   IIIF_AUTH_TOKEN_TTL — token time-to-live in seconds
fn apply_env_overrides(config: &mut AppConfig) {
    if let Ok(v) = std::env::var("IIIF_HOST") {
        config.server.host = v;
    }
    if let Ok(v) = std::env::var("IIIF_PORT") {
        if let Ok(port) = v.parse() {
            config.server.port = port;
        }
    }
    if let Ok(v) = std::env::var("IIIF_BASE_URL") {
        config.server.base_url = v;
    }
    if let Ok(v) = std::env::var("IIIF_STORAGE_PATH") {
        config.storage.root_path = v;
    }
    if let Ok(v) = std::env::var("IIIF_MAX_WIDTH") {
        if let Ok(n) = v.parse() {
            config.image.max_width = Some(n);
        }
    }
    if let Ok(v) = std::env::var("IIIF_MAX_HEIGHT") {
        if let Ok(n) = v.parse() {
            config.image.max_height = Some(n);
        }
    }
    if let Ok(v) = std::env::var("IIIF_MAX_AREA") {
        if let Ok(n) = v.parse() {
            config.image.max_area = Some(n);
        }
    }
    if let Ok(v) = std::env::var("IIIF_ALLOW_UPSCALING") {
        config.image.allow_upscaling = v == "true" || v == "1";
    }
    if let Ok(v) = std::env::var("IIIF_TILE_WIDTH") {
        if let Ok(n) = v.parse() {
            config.image.tile_width = n;
        }
    }
    if let Ok(v) = std::env::var("IIIF_AUTH_ENABLED") {
        config.auth.enabled = v == "true" || v == "1";
    }
    if let Ok(v) = std::env::var("IIIF_AUTH_COOKIE") {
        config.auth.cookie_name = v;
    }
    if let Ok(v) = std::env::var("IIIF_AUTH_TOKEN_TTL") {
        if let Ok(n) = v.parse() {
            config.auth.token_ttl = n;
        }
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
    info!("Received shutdown signal, finishing active requests...");
}

/// Seed the search index with sample annotations for demonstration.
fn seed_search_index(index: &SearchIndex, base_url: &str) {
    let samples = [
        ("creation", "The Creation of the World, Bible Historiale, medieval manuscript illumination depicting Genesis"),
        ("creation", "God creating heaven and earth, angels observing the divine act of creation"),
        ("creation", "Decorated initial with gold leaf, blue and red pigments, Gothic script"),
        ("test", "IIIF validator test image with colored squares in a 10x10 grid pattern"),
        ("test", "Calibration image for testing region extraction, scaling, and rotation"),
    ];

    for (i, (manifest, text)) in samples.iter().enumerate() {
        index.add(IndexedAnnotation {
            id: format!("{base_url}/annotation/content/{manifest}/{i}"),
            text: text.to_string(),
            motivation: "painting".to_string(),
            target: format!("{base_url}/canvas/{manifest}/p1"),
            manifest_id: manifest.to_string(),
        });
    }
}

/// Seed the activity store with initial Create activities for existing images.
fn seed_activities(store: &ActivityStore, base_url: &str) {
    store.record("Create", &format!("{base_url}/manifest/test"), "Manifest");
    store.record(
        "Create",
        &format!("{base_url}/manifest/creation"),
        "Manifest",
    );
}
