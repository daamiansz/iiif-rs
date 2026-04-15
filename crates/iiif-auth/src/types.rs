use serde::Serialize;

/// Auth service descriptor embedded in info.json / Manifest for protected resources.
/// Conforms to IIIF Authorization Flow API 2.0.
#[derive(Debug, Clone, Serialize)]
pub struct AuthService {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    pub profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<serde_json::Value>,
    #[serde(rename = "confirmLabel", skip_serializing_if = "Option::is_none")]
    pub confirm_label: Option<serde_json::Value>,
    pub service: Vec<AuthSubService>,
}

/// Sub-services within an auth service (token service, probe service, logout).
#[derive(Debug, Clone, Serialize)]
pub struct AuthSubService {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    pub profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<serde_json::Value>,
    #[serde(rename = "errorHeading", skip_serializing_if = "Option::is_none")]
    pub error_heading: Option<serde_json::Value>,
    #[serde(rename = "errorNote", skip_serializing_if = "Option::is_none")]
    pub error_note: Option<serde_json::Value>,
}

/// Token service JSON response.
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: u64,
}

/// Token service error response.
#[derive(Debug, Serialize)]
pub struct TokenError {
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Probe service response.
#[derive(Debug, Serialize)]
pub struct ProbeResult {
    pub id: String,
    #[serde(rename = "type")]
    pub result_type: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<serde_json::Value>,
}

/// Build auth service descriptors for a protected resource.
pub fn build_auth_service_descriptor(base_url: &str, resource_id: &str) -> AuthService {
    let lang_val = |s: &str| serde_json::json!({"en": [s]});

    AuthService {
        id: format!("{base_url}/auth/login"),
        service_type: "AuthAccessService2".to_string(),
        profile: "active".to_string(),
        label: Some(lang_val("Login")),
        heading: Some(lang_val("Please log in")),
        note: Some(lang_val("This resource requires authentication.")),
        confirm_label: Some(lang_val("Login")),
        service: vec![
            AuthSubService {
                id: format!("{base_url}/auth/token"),
                service_type: "AuthAccessTokenService2".to_string(),
                profile: "active".to_string(),
                label: None,
                error_heading: Some(lang_val("Authentication failed")),
                error_note: Some(lang_val("The token could not be issued.")),
            },
            AuthSubService {
                id: format!("{base_url}/auth/probe/{resource_id}"),
                service_type: "AuthProbeService2".to_string(),
                profile: "active".to_string(),
                label: None,
                error_heading: None,
                error_note: None,
            },
            AuthSubService {
                id: format!("{base_url}/auth/logout"),
                service_type: "AuthLogoutService2".to_string(),
                profile: "active".to_string(),
                label: Some(lang_val("Logout")),
                error_heading: None,
                error_note: None,
            },
        ],
    }
}

/// Check if an identifier matches any of the protected patterns.
pub fn is_protected(identifier: &str, protected_patterns: &[String]) -> bool {
    protected_patterns.iter().any(|pattern| {
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            identifier.starts_with(prefix)
        } else {
            identifier == pattern
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert!(is_protected("protected", &["protected".to_string()]));
        assert!(!is_protected("public", &["protected".to_string()]));
    }

    #[test]
    fn wildcard_match() {
        let patterns = vec!["secret_*".to_string()];
        assert!(is_protected("secret_image", &patterns));
        assert!(is_protected("secret_", &patterns));
        assert!(!is_protected("public_image", &patterns));
    }

    #[test]
    fn auth_service_serializes() {
        let svc = build_auth_service_descriptor("http://localhost:8080", "img1");
        let json = serde_json::to_string(&svc).unwrap();
        assert!(json.contains("AuthAccessService2"));
        assert!(json.contains("AuthAccessTokenService2"));
        assert!(json.contains("AuthProbeService2"));
    }
}
