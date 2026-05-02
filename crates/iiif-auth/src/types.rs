use iiif_core::services::{
    lang_map, AuthAccessService2, AuthAccessTokenService2, AuthLogoutService2, AuthProbeService2,
    Service,
};

pub const AUTH_CONTEXT: &str = "http://iiif.io/api/auth/2/context.json";

/// Interaction patterns from IIIF Authorization Flow API 2.0 §5.
#[derive(Debug, Clone, Copy)]
pub enum AuthPattern {
    /// User must take a UI action to log in (credentials, click-through).
    Active,
    /// Managed device, no UI in the opened tab; access service still has `id`.
    Kiosk,
    /// Ambient auth (IP, prior SSO); access service has no `id`/`label`.
    External,
}

impl AuthPattern {
    pub fn from_config(s: &str) -> Self {
        match s {
            "kiosk" => Self::Kiosk,
            "external" => Self::External,
            _ => Self::Active,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Kiosk => "kiosk",
            Self::External => "external",
        }
    }
}

/// Build the probe service descriptor for a protected resource.
///
/// Returns a typed `Service::AuthProbeService2` which serialises with the
/// spec-mandated hierarchy: probe → access → [token, logout]. The shape of the
/// nested `AuthAccessService2` varies with `pattern`:
///
/// - `active`  — full UI fields (label/heading/note/confirmLabel) and `id`.
/// - `kiosk`   — has `id` but no UI strings (managed device, no UI in opened tab).
/// - `external` — no `id`, no UI; ambient auth assumed (IP, prior SSO, etc.).
pub fn build_probe_service_descriptor(
    base_url: &str,
    identifier: &str,
    pattern: AuthPattern,
) -> Service {
    let token_service = Service::AuthAccessTokenService2(AuthAccessTokenService2 {
        id: format!("{base_url}/auth/token"),
        error_heading: Some(lang_map("en", "Authentication failed")),
        error_note: Some(lang_map("en", "The token could not be issued.")),
    });
    let logout_service = Service::AuthLogoutService2(AuthLogoutService2 {
        id: format!("{base_url}/auth/logout"),
        label: lang_map("en", "Logout"),
    });

    let access = match pattern {
        AuthPattern::Active => AuthAccessService2 {
            id: Some(format!("{base_url}/auth/login")),
            profile: "active".to_string(),
            label: Some(lang_map("en", "Login")),
            heading: Some(lang_map("en", "Please log in")),
            note: Some(lang_map("en", "This resource requires authentication.")),
            confirm_label: Some(lang_map("en", "Login")),
            service: vec![token_service, logout_service],
        },
        AuthPattern::Kiosk => AuthAccessService2 {
            id: Some(format!("{base_url}/auth/login")),
            profile: "kiosk".to_string(),
            label: None,
            heading: None,
            note: None,
            confirm_label: None,
            service: vec![token_service, logout_service],
        },
        AuthPattern::External => AuthAccessService2 {
            // Spec: external pattern MUST omit `id`. Label is shown only if
            // the ambient auth ultimately fails.
            id: None,
            profile: "external".to_string(),
            label: Some(lang_map("en", "Authentication required")),
            heading: None,
            note: None,
            confirm_label: None,
            // External flow has no logout — there's nothing to log out from.
            service: vec![token_service],
        },
    };

    Service::AuthProbeService2(AuthProbeService2 {
        id: format!("{base_url}/auth/probe/{identifier}"),
        error_heading: Some(lang_map("en", "Authentication required")),
        error_note: Some(lang_map("en", "This resource requires authentication.")),
        service: vec![Service::AuthAccessService2(access)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_pattern_has_full_ui_fields() {
        let v = serde_json::to_value(build_probe_service_descriptor(
            "http://localhost:8080",
            "img1",
            AuthPattern::Active,
        ))
        .unwrap();

        assert_eq!(v["type"], "AuthProbeService2");
        assert_eq!(v["id"], "http://localhost:8080/auth/probe/img1");

        let access = &v["service"][0];
        assert_eq!(access["type"], "AuthAccessService2");
        assert_eq!(access["profile"], "active");
        assert_eq!(access["id"], "http://localhost:8080/auth/login");
        assert!(access["label"].is_object());
        assert!(access["heading"].is_object());
        assert!(access["confirmLabel"].is_object());

        let sub_services = access["service"].as_array().unwrap();
        assert_eq!(sub_services.len(), 2);
        assert_eq!(sub_services[0]["type"], "AuthAccessTokenService2");
        assert_eq!(sub_services[1]["type"], "AuthLogoutService2");
        assert!(sub_services[0].get("profile").is_none());
        assert!(sub_services[1].get("profile").is_none());
    }

    #[test]
    fn kiosk_pattern_has_id_but_no_ui_strings() {
        let v = serde_json::to_value(build_probe_service_descriptor(
            "http://localhost:8080",
            "img1",
            AuthPattern::Kiosk,
        ))
        .unwrap();
        let access = &v["service"][0];
        assert_eq!(access["profile"], "kiosk");
        assert_eq!(access["id"], "http://localhost:8080/auth/login");
        // No user-facing strings on a kiosk descriptor.
        assert!(access.get("label").is_none());
        assert!(access.get("heading").is_none());
        assert!(access.get("note").is_none());
        assert!(access.get("confirmLabel").is_none());
        // Token + logout still nested.
        assert_eq!(access["service"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn external_pattern_omits_id_and_logout() {
        let v = serde_json::to_value(build_probe_service_descriptor(
            "http://localhost:8080",
            "img1",
            AuthPattern::External,
        ))
        .unwrap();
        let access = &v["service"][0];
        assert_eq!(access["profile"], "external");
        // Spec: external pattern MUST NOT carry `id`.
        assert!(access.get("id").is_none());
        // Label shown only if ambient auth fails.
        assert!(access["label"].is_object());

        // Only the token service is present — there's nothing to log out from.
        let sub_services = access["service"].as_array().unwrap();
        assert_eq!(sub_services.len(), 1);
        assert_eq!(sub_services[0]["type"], "AuthAccessTokenService2");
    }
}
