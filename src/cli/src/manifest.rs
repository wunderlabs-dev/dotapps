use serde::{Deserialize, Serialize};

/// App manifest carried as `manifest.json` inside a `.vibox` archive and as
/// `vibox.json` in a project directory.
///
/// The wire format is frozen camelCase JSON (e.g. `internalPort`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: String,
    pub slug: String,
    pub version: String,
    pub icon: String,
    pub internal_port: u16,
    #[serde(default)]
    pub description: String,
}

impl Manifest {
    /// Docker image tag the packed image carries: `vibox/{slug}:{version}`.
    pub fn image_tag(&self) -> String {
        format!("vibox/{}:{}", self.slug, self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::Manifest;

    /// The frozen wire-contract example from the implementation plan.
    const FROZEN_JSON: &str = r#"{ "name": "Cafe Tracker", "slug": "cafe-tracker", "version": "1.0.0", "icon": "☕", "internalPort": 8000, "description": "Log coffee bean deliveries" }"#;

    #[test]
    fn parses_frozen_camel_case_json() {
        let manifest: Manifest = serde_json::from_str(FROZEN_JSON).expect("frozen JSON parses");
        assert_eq!(manifest.name, "Cafe Tracker");
        assert_eq!(manifest.slug, "cafe-tracker");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.icon, "☕");
        assert_eq!(manifest.internal_port, 8000);
        assert_eq!(manifest.description, "Log coffee bean deliveries");
    }

    #[test]
    fn description_defaults_to_empty_when_absent() {
        let json =
            r#"{ "name": "X", "slug": "x", "version": "0.1.0", "icon": "🧪", "internalPort": 80 }"#;
        let manifest: Manifest =
            serde_json::from_str(json).expect("JSON without description parses");
        assert_eq!(manifest.description, "");
    }

    #[test]
    fn version_is_required() {
        let json = r#"{ "name": "X", "slug": "x", "icon": "🧪", "internalPort": 80 }"#;
        let error = serde_json::from_str::<Manifest>(json).expect_err("missing version must fail");
        assert!(
            error.to_string().contains("version"),
            "error should name the missing field: {error}"
        );
    }

    #[test]
    fn serializes_back_to_camel_case() {
        let manifest: Manifest = serde_json::from_str(FROZEN_JSON).expect("frozen JSON parses");
        let json = serde_json::to_string(&manifest).expect("manifest serializes");
        assert!(
            json.contains(r#""internalPort":8000"#),
            "serialized form must use camelCase: {json}"
        );
        assert!(
            !json.contains("internal_port"),
            "snake_case must not leak onto the wire: {json}"
        );
    }

    #[test]
    fn image_tag_follows_frozen_contract() {
        let manifest: Manifest = serde_json::from_str(FROZEN_JSON).expect("frozen JSON parses");
        assert_eq!(manifest.image_tag(), "vibox/cafe-tracker:1.0.0");
    }
}
