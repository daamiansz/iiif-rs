use serde::Serialize;
use serde_json::{json, Value};

pub const AUTH_CONTEXT: &str = "http://iiif.io/api/auth/2/context.json";

/// Probe service descriptor — placed in the protected resource's `service[]`.
/// AuthAccessService2 entries are nested inside its own `service[]`.
#[derive(Debug, Clone, Serialize)]
pub struct ProbeServiceDescriptor {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    pub service: Vec<AccessServiceDescriptor>,
    #[serde(rename = "errorHeading", skip_serializing_if = "Option::is_none")]
    pub error_heading: Option<Value>,
    #[serde(rename = "errorNote", skip_serializing_if = "Option::is_none")]
    pub error_note: Option<Value>,
}

/// Access service descriptor — `id` is required for `active`/`kiosk`,
/// MUST be omitted for `external`.
#[derive(Debug, Clone, Serialize)]
pub struct AccessServiceDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub service_type: String,
    pub profile: String,
    pub service: Vec<AccessSubService>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<Value>,
    #[serde(rename = "confirmLabel", skip_serializing_if = "Option::is_none")]
    pub confirm_label: Option<Value>,
}

/// Token / logout sub-service. Spec 2.0 does NOT define `profile` on these.
#[derive(Debug, Clone, Serialize)]
pub struct AccessSubService {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Value>,
    #[serde(rename = "errorHeading", skip_serializing_if = "Option::is_none")]
    pub error_heading: Option<Value>,
    #[serde(rename = "errorNote", skip_serializing_if = "Option::is_none")]
    pub error_note: Option<Value>,
}

/// Build the probe service descriptor for a protected resource as a `serde_json::Value`,
/// ready to drop into `info.json` `service[]` or a Manifest content body's `service[]`.
///
/// Currently emits the `active` pattern only. `kiosk` and `external` will land in v0.3.0.
pub fn build_probe_service_descriptor(base_url: &str, identifier: &str) -> Value {
    let lang = |s: &str| json!({"en": [s]});

    let descriptor = ProbeServiceDescriptor {
        id: format!("{base_url}/auth/probe/{identifier}"),
        service_type: "AuthProbeService2".to_string(),
        error_heading: Some(lang("Authentication required")),
        error_note: Some(lang("This resource requires authentication.")),
        service: vec![AccessServiceDescriptor {
            id: Some(format!("{base_url}/auth/login")),
            service_type: "AuthAccessService2".to_string(),
            profile: "active".to_string(),
            label: Some(lang("Login")),
            heading: Some(lang("Please log in")),
            note: Some(lang("This resource requires authentication.")),
            confirm_label: Some(lang("Login")),
            service: vec![
                AccessSubService {
                    id: format!("{base_url}/auth/token"),
                    service_type: "AuthAccessTokenService2".to_string(),
                    label: None,
                    error_heading: Some(lang("Authentication failed")),
                    error_note: Some(lang("The token could not be issued.")),
                },
                AccessSubService {
                    id: format!("{base_url}/auth/logout"),
                    service_type: "AuthLogoutService2".to_string(),
                    label: Some(lang("Logout")),
                    error_heading: None,
                    error_note: None,
                },
            ],
        }],
    };

    serde_json::to_value(descriptor).expect("ProbeServiceDescriptor serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_descriptor_has_correct_hierarchy() {
        let v = build_probe_service_descriptor("http://localhost:8080", "img1");
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

        // Spec 2.0: sub-services do NOT carry `profile`
        assert!(sub_services[0].get("profile").is_none());
        assert!(sub_services[1].get("profile").is_none());
    }
}
