use serde::{Deserialize, Serialize};

/// App manifest carried as `manifest.json` inside a `.apps` archive and as
/// `dotapps.json` in a project directory.
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
    /// Docker image tag the packed image carries: `dotapps/{slug}:{version}`.
    pub fn image_tag(&self) -> String {
        format!("dotapps/{}:{}", self.slug, self.version)
    }

    /// Validates identifier fields before they reach URLs, file names, image
    /// tags, or container names.
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_slug(&self.slug)?;
        validate_version(&self.version)?;
        if self.internal_port == 0 {
            anyhow::bail!("manifest internalPort must be non-zero");
        }
        Ok(())
    }
}

fn validate_slug(slug: &str) -> anyhow::Result<()> {
    let starts_ok = slug
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let chars_ok = slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !(starts_ok && chars_ok && slug.len() <= 63) {
        anyhow::bail!(
            "manifest slug {slug:?} is invalid: 1-63 chars of [a-z0-9-], starting with a letter or digit"
        );
    }
    Ok(())
}

fn validate_version(version: &str) -> anyhow::Result<()> {
    let ok = !version.is_empty()
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !ok {
        anyhow::bail!("manifest version {version:?} is invalid: 1-64 chars of [A-Za-z0-9._-]");
    }
    Ok(())
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
        assert_eq!(manifest.image_tag(), "dotapps/cafe-tracker:1.0.0");
    }

    #[test]
    fn validate_accepts_frozen_manifest() {
        let manifest: Manifest = serde_json::from_str(FROZEN_JSON).expect("frozen JSON parses");
        manifest.validate().expect("frozen manifest is valid");
    }

    #[test]
    fn validate_rejects_path_breaking_slugs() {
        let mut manifest: Manifest = serde_json::from_str(FROZEN_JSON).expect("frozen JSON parses");
        for bad in ["../escape", "a/b", "UPPER", "", "-leading", "pct%41", "a b"] {
            manifest.slug = bad.to_owned();
            assert!(
                manifest.validate().is_err(),
                "slug {bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn validate_rejects_path_breaking_versions() {
        let mut manifest: Manifest = serde_json::from_str(FROZEN_JSON).expect("frozen JSON parses");
        for bad in ["1/0", "../1", "", "1.0 beta", "v%31"] {
            manifest.version = bad.to_owned();
            assert!(
                manifest.validate().is_err(),
                "version {bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn validate_rejects_port_zero() {
        let mut manifest: Manifest = serde_json::from_str(FROZEN_JSON).expect("frozen JSON parses");
        manifest.internal_port = 0;
        assert!(manifest.validate().is_err(), "port 0 must be rejected");
    }
}
