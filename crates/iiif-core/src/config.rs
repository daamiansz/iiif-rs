use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub image: ImageConfig,
    pub storage: StorageConfig,
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
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
    pub root_path: String,
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
            },
            auth: AuthConfig::default(),
        }
    }
}
