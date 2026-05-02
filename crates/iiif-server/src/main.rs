use std::net::SocketAddr;
use std::sync::Arc;

use axum::{middleware, Extension, Router};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use std::time::Duration;

use axum::http::StatusCode;
use iiif_auth::middleware::{check_access, CookieName};
use iiif_auth::AuthStore;
use iiif_core::config::AppConfig;
use iiif_core::state::AppState;
use iiif_core::storage::filesystem::FilesystemStorage;
use iiif_discovery::ActivityStore;
use iiif_image::handlers::ImageCache;
use iiif_search::SearchIndex;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::timeout::TimeoutLayer;

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

    if let Err(e) = iiif_core::config::validate_security_config(&config) {
        error!("Configuration error: {e}");
        std::process::exit(1);
    }

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

    let auth_store: Option<Arc<AuthStore>> = if config.auth.enabled {
        info!(
            pattern = %config.auth.pattern,
            protected_dirs = ?config.auth.protected_dirs,
            "Authorization Flow enabled"
        );
        let store = Arc::new(AuthStore::new(config.auth.token_ttl));

        // Periodic sweep of expired tokens. The validator checks TTL on read,
        // so the only purpose here is to bound memory for stores with high
        // token-issuance rates. Disabled with sweep interval = 0.
        let interval_secs = config.auth.token_sweep_interval_secs;
        if interval_secs > 0 {
            let sweeper = Arc::clone(&store);
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(interval_secs));
                tick.tick().await; // skip the immediate first tick
                loop {
                    tick.tick().await;
                    sweeper.cleanup();
                }
            });
            info!(interval_secs, "Token sweeper running");
        }

        Some(store)
    } else {
        None
    };

    let search_index = Arc::new(SearchIndex::new());
    info!(annotations = search_index.len(), "Search index initialized");

    let activity_store = Arc::new(ActivityStore::new(20, &config.server.base_url));
    info!(
        activities = activity_store.total(),
        "Activity store initialized"
    );

    let image_cache: Option<Arc<ImageCache>> = if config.performance.cache_max_entries > 0 {
        let cache: ImageCache = moka::sync::Cache::new(config.performance.cache_max_entries);
        info!(
            max_entries = config.performance.cache_max_entries,
            "Image response cache enabled"
        );
        Some(Arc::new(cache))
    } else {
        None
    };

    let state = AppState {
        config: Arc::new(config.clone()),
        storage: Arc::new(storage),
    };

    // Build application — each crate's router is `Router<AppState>`. Optional
    // services are wired through typed `Extension<Arc<T>>` layers.
    let mut app = Router::new()
        .merge(iiif_image::router())
        .merge(iiif_presentation::router())
        .merge(iiif_search::router())
        .merge(iiif_state::router())
        .merge(iiif_discovery::router())
        .layer(Extension(Arc::clone(&search_index)))
        .layer(Extension(Arc::clone(&activity_store)));

    if let Some(cache) = &image_cache {
        app = app.layer(Extension(Arc::clone(cache)));
    }

    // Add auth routes and middleware if enabled
    if let Some(auth_store) = auth_store.clone() {
        app = app
            .merge(iiif_auth::router())
            .layer(Extension(Arc::clone(&auth_store)));

        // Apply directory-protection middleware — only injects what the
        // middleware itself needs (config, cookie name, storage). The auth
        // store reaches the middleware via the Extension layer above.
        let auth_config = config.auth.clone();
        let cookie_name = CookieName(config.auth.cookie_name.clone());
        let storage_for_mw: Arc<dyn iiif_core::storage::ImageStorage> = Arc::clone(&state.storage);
        app = app.layer(middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                req.extensions_mut().insert(auth_config.clone());
                req.extensions_mut().insert(cookie_name.clone());
                req.extensions_mut().insert(Arc::clone(&storage_for_mw));
                check_access(req, next)
            },
        ));
    }

    // Metrics endpoint
    if config.performance.metrics_enabled {
        let metrics_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
            .install_recorder()
            .expect("Failed to install Prometheus recorder");
        app = app.route(
            "/metrics",
            axum::routing::get(move || {
                let h = metrics_handle.clone();
                async move { h.render() }
            }),
        );
    }

    // Rate limiting
    let rate_limit_rps = config.performance.rate_limit_rps;
    if rate_limit_rps > 0 {
        let governor_conf = GovernorConfigBuilder::default()
            .per_second(rate_limit_rps)
            .burst_size(rate_limit_rps as u32 * 2)
            .finish()
            .expect("valid governor config");
        app = app.layer(GovernorLayer {
            config: governor_conf.into(),
        });
        info!(rps = rate_limit_rps, "Rate limiting enabled");
    }

    // Request timeout
    let timeout_secs = config.performance.request_timeout_secs;

    let mut final_app = app
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    if timeout_secs > 0 {
        final_app = final_app.layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(timeout_secs),
        ));
        info!(timeout_secs, "Request timeout enabled");
    }

    let app = final_app.with_state(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .unwrap_or_else(|e| {
            error!("Invalid bind address: {e}");
            std::process::exit(1);
        });

    // TLS / HTTP/2 or plain HTTP
    if let (Some(cert_path), Some(key_path)) = (&config.server.tls_cert, &config.server.tls_key) {
        info!(%addr, cert = %cert_path, "IIIF server listening (HTTPS + HTTP/2)");
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .unwrap_or_else(|e| {
                error!("Failed to load TLS certificates: {e}");
                std::process::exit(1);
            });

        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            shutdown_handle.graceful_shutdown(Some(Duration::from_secs(30)));
        });

        axum_server::bind_rustls(addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap_or_else(|e| {
                error!("Server error: {e}");
                std::process::exit(1);
            });
    } else {
        info!(%addr, "IIIF server listening (HTTP)");
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .unwrap_or_else(|e| {
                error!("Failed to bind to {addr}: {e}");
                std::process::exit(1);
            });
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            error!("Server error: {e}");
            std::process::exit(1);
        });
    }

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
    if let Ok(v) = std::env::var("IIIF_TLS_CERT") {
        config.server.tls_cert = Some(v);
    }
    if let Ok(v) = std::env::var("IIIF_TLS_KEY") {
        config.server.tls_key = Some(v);
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
    // Performance
    if let Ok(v) = std::env::var("IIIF_CACHE_MAX_ENTRIES") {
        if let Ok(n) = v.parse() {
            config.performance.cache_max_entries = n;
        }
    }
    if let Ok(v) = std::env::var("IIIF_REQUEST_TIMEOUT") {
        if let Ok(n) = v.parse() {
            config.performance.request_timeout_secs = n;
        }
    }
    if let Ok(v) = std::env::var("IIIF_METRICS_ENABLED") {
        config.performance.metrics_enabled = v == "true" || v == "1";
    }
    if let Ok(v) = std::env::var("IIIF_TILE_CACHE_DIR") {
        config.performance.tile_cache_dir = if v.is_empty() { None } else { Some(v) };
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
    info!("Received shutdown signal, finishing active requests...");
}
