use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub image: ImageConfig,
    pub storage: StorageConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    /// Path to TLS certificate file (PEM). Enables HTTPS + HTTP/2 when set.
    #[serde(default)]
    pub tls_cert: Option<String>,
    /// Path to TLS private key file (PEM).
    #[serde(default)]
    pub tls_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageConfig {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub max_area: Option<u64>,
    pub allow_upscaling: bool,
    pub tile_width: u32,
    pub tile_scale_factors: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    /// Filesystem root for the default backend. Always present (used when
    /// `sources` is empty or omitted, and as a fallback for `scan_images`).
    pub root_path: String,
    /// Optional multi-source backends. When non-empty, requests are routed
    /// across them in declaration order, with the filesystem `root_path`
    /// appended last as the catch-all fallback.
    #[serde(default)]
    pub sources: Vec<StorageSourceConfig>,
}

/// One entry in the `[[storage.sources]]` array.
///
/// Example (S3-compatible):
/// ```toml
/// [[storage.sources]]
/// kind = "s3"
/// label = "rare-books"
/// bucket = "iiif-rare"
/// region = "eu-west-1"
/// prefix = "manuscripts/"
/// access_zone = "restricted"
/// prefix_filter = "rare-"
/// ```
///
/// Example (HTTP remote with source caching):
/// ```toml
/// [[storage.sources]]
/// kind = "http"
/// label = "wikimedia"
/// url = "https://upload.wikimedia.org/wikipedia/commons/"
/// prefix_filter = "wm-"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct StorageSourceConfig {
    /// Backend type: `"s3"`, `"azure"`, `"gcs"`, `"http"`, or `"local"`.
    pub kind: String,
    /// Human-readable label for logs.
    #[serde(default)]
    pub label: String,
    /// S3/GCS bucket name (required for `kind = "s3"` / `"gcs"`).
    #[serde(default)]
    pub bucket: String,
    /// AWS region (S3 only). Falls back to env / default chain when empty.
    #[serde(default)]
    pub region: String,
    /// Custom endpoint URL (S3-compatible like MinIO; HTTP remote root).
    #[serde(default)]
    pub url: String,
    /// Azure storage account name (Azure only).
    #[serde(default)]
    pub account: String,
    /// Azure container name (Azure only).
    #[serde(default)]
    pub container: String,
    /// Object prefix prepended to every key (e.g. `"images/"`).
    #[serde(default)]
    pub prefix: String,
    /// Access zone reported by `access_zone()` for every id this source
    /// serves. Empty = public.
    #[serde(default)]
    pub access_zone: String,
    /// Identifier-prefix routing hint. Only identifiers starting with this
    /// string are dispatched to this source. Empty = catch-all.
    #[serde(default)]
    pub prefix_filter: String,
}

/// Authentication / authorization configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    /// Enable the IIIF Authorization Flow.
    pub enabled: bool,
    /// Interaction pattern: "active", "kiosk", or "external".
    pub pattern: String,
    /// Cookie name used for the access session.
    pub cookie_name: String,
    /// Token time-to-live in seconds.
    pub token_ttl: u64,
    /// Subdirectories of the images folder that require authorization.
    /// Images in `images/restricted/` are protected when `protected_dirs = ["restricted"]`.
    pub protected_dirs: Vec<String>,
    /// Simple user/password pairs for the "active" login flow.
    pub users: Vec<UserCredential>,
    /// Whitelist of origins allowed to call the token service. Empty list =
    /// any well-formed origin is accepted (back-compat with v0.3.0b). When
    /// non-empty, the token service rejects origins not exactly listed here
    /// with `AuthAccessTokenError2 { profile: "invalidOrigin" }`.
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// Background sweep interval (seconds) to purge expired tokens. `0` disables
    /// the sweeper; the store still validates token TTL on each request.
    #[serde(default = "default_token_sweep_interval")]
    pub token_sweep_interval_secs: u64,
    /// IIIF Image API size parameter for the substitute (degraded) image
    /// served via the probe response when access is denied. Empty = no
    /// substitute (probe returns plain 401). Example: `"^200,"` for a
    /// 200-pixel-wide low-resolution preview.
    #[serde(default)]
    pub substitute_size: String,
}

fn default_token_sweep_interval() -> u64 {
    300
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserCredential {
    pub username: String,
    pub password: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pattern: "active".to_string(),
            cookie_name: "iiif_access".to_string(),
            token_ttl: 3600,
            protected_dirs: Vec::new(),
            users: Vec::new(),
            allowed_origins: Vec::new(),
            token_sweep_interval_secs: default_token_sweep_interval(),
            substitute_size: String::new(),
        }
    }
}

/// Performance and production tuning.
#[derive(Debug, Clone, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum cached processed images (LRU eviction).
    pub cache_max_entries: u64,
    /// Request timeout in seconds (0 = no timeout).
    pub request_timeout_secs: u64,
    /// Rate limit: max requests per second per IP (0 = unlimited).
    pub rate_limit_rps: u64,
    /// Enable Prometheus metrics on /metrics endpoint.
    pub metrics_enabled: bool,
    /// Path for disk tile cache. Empty = disabled.
    #[serde(default)]
    pub tile_cache_dir: Option<String>,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            cache_max_entries: 1000,
            request_timeout_secs: 30,
            rate_limit_rps: 0,
            metrics_enabled: false,
            tile_cache_dir: None,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                base_url: "http://localhost:8080".to_string(),
                tls_cert: None,
                tls_key: None,
            },
            image: ImageConfig {
                max_width: Some(4096),
                max_height: Some(4096),
                max_area: Some(16_777_216),
                allow_upscaling: true,
                tile_width: 512,
                tile_scale_factors: vec![1, 2, 4, 8, 16],
            },
            storage: StorageConfig {
                root_path: "./images".to_string(),
                sources: Vec::new(),
            },
            auth: AuthConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

/// Validate combinations of settings that would otherwise silently expose data
/// or quietly downgrade security. Returns the first reason found (if any).
///
/// Called at startup; a non-`Ok` result must terminate the process — these are
/// fail-fast conditions, not warnings.
pub fn validate_security_config(config: &AppConfig) -> Result<(), String> {
    if !config.auth.enabled && !config.auth.protected_dirs.is_empty() {
        return Err(format!(
            "auth.protected_dirs is set ({:?}) but auth.enabled = false; \
             this would silently expose protected images. \
             Either set auth.enabled = true or clear protected_dirs.",
            config.auth.protected_dirs
        ));
    }

    match (&config.server.tls_cert, &config.server.tls_key) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(
                "TLS misconfiguration: tls_cert and tls_key must both be set or both unset"
                    .to_string(),
            );
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_validates() {
        assert!(validate_security_config(&AppConfig::default()).is_ok());
    }

    #[test]
    fn protected_dirs_without_auth_enabled_fails_fast() {
        let config = AppConfig {
            auth: AuthConfig {
                enabled: false,
                protected_dirs: vec!["restricted".to_string()],
                ..AuthConfig::default()
            },
            ..AppConfig::default()
        };
        let err = validate_security_config(&config).expect_err("must reject");
        assert!(err.contains("protected_dirs"));
        assert!(err.contains("enabled"));
    }

    #[test]
    fn protected_dirs_with_auth_enabled_validates() {
        let config = AppConfig {
            auth: AuthConfig {
                enabled: true,
                protected_dirs: vec!["restricted".to_string()],
                ..AuthConfig::default()
            },
            ..AppConfig::default()
        };
        assert!(validate_security_config(&config).is_ok());
    }

    #[test]
    fn empty_protected_dirs_with_auth_disabled_validates() {
        let config = AppConfig {
            auth: AuthConfig {
                enabled: false,
                protected_dirs: Vec::new(),
                ..AuthConfig::default()
            },
            ..AppConfig::default()
        };
        assert!(validate_security_config(&config).is_ok());
    }

    #[test]
    fn cert_without_key_fails_fast() {
        let mut config = AppConfig::default();
        config.server.tls_cert = Some("cert.pem".to_string());
        let err = validate_security_config(&config).expect_err("must reject");
        assert!(err.contains("tls_cert") && err.contains("tls_key"));
    }

    #[test]
    fn key_without_cert_fails_fast() {
        let mut config = AppConfig::default();
        config.server.tls_key = Some("key.pem".to_string());
        let err = validate_security_config(&config).expect_err("must reject");
        assert!(err.contains("tls_cert") && err.contains("tls_key"));
    }

    #[test]
    fn both_tls_set_validates() {
        let mut config = AppConfig::default();
        config.server.tls_cert = Some("cert.pem".to_string());
        config.server.tls_key = Some("key.pem".to_string());
        assert!(validate_security_config(&config).is_ok());
    }
}
