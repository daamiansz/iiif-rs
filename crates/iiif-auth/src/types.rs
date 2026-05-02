use iiif_core::services::{
    lang_map, AuthAccessService2, AuthAccessTokenService2, AuthLogoutService2, AuthProbeService2,
    Service,
};

pub const AUTH_CONTEXT: &str = "http://iiif.io/api/auth/2/context.json";

/// Build the probe service descriptor for a protected resource.
///
/// Returns a typed `Service::AuthProbeService2` which serialises with the
/// spec-mandated hierarchy: probe → access → [token, logout]. Currently emits
/// the `active` pattern only; `kiosk`/`external` land in v0.3.0c.
pub fn build_probe_service_descriptor(base_url: &str, identifier: &str) -> Service {
    Service::AuthProbeService2(AuthProbeService2 {
        id: format!("{base_url}/auth/probe/{identifier}"),
        error_heading: Some(lang_map("en", "Authentication required")),
        error_note: Some(lang_map(
            "en",
            "This resource requires authentication.",
        )),
        service: vec![Service::AuthAccessService2(AuthAccessService2 {
            id: Some(format!("{base_url}/auth/login")),
            profile: "active".to_string(),
            label: Some(lang_map("en", "Login")),
            heading: Some(lang_map("en", "Please log in")),
            note: Some(lang_map(
                "en",
                "This resource requires authentication.",
            )),
            confirm_label: Some(lang_map("en", "Login")),
            service: vec![
                Service::AuthAccessTokenService2(AuthAccessTokenService2 {
                    id: format!("{base_url}/auth/token"),
                    error_heading: Some(lang_map("en", "Authentication failed")),
                    error_note: Some(lang_map("en", "The token could not be issued.")),
                }),
                Service::AuthLogoutService2(AuthLogoutService2 {
                    id: format!("{base_url}/auth/logout"),
                    label: lang_map("en", "Logout"),
                }),
            ],
        })],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_descriptor_has_correct_hierarchy() {
        let v = serde_json::to_value(build_probe_service_descriptor(
            "http://localhost:8080",
            "img1",
        ))
        .unwrap();

        assert_eq!(v["type"], "AuthProbeService2");
        assert_eq!(v["id"], "http://localhost:8080/auth/probe/img1");

        let access = &v["service"][0];
        assert_eq!(access["type"], "AuthAccessService2");
        assert_eq!(access["profile"], "active");
        assert_eq!(access["id"], "http://localhost:8080/auth/login");

        let sub_services = access["service"].as_array().unwrap();
        assert_eq!(sub_services.len(), 2);
        assert_eq!(sub_services[0]["type"], "AuthAccessTokenService2");
        assert_eq!(sub_services[1]["type"], "AuthLogoutService2");

        assert!(sub_services[0].get("profile").is_none());
        assert!(sub_services[1].get("profile").is_none());
    }
}
