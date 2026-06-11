//! Wire types for dotapps apps (frozen contract: camelCase JSON).

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// App manifest as published to the registry (`dotapps.json` / `manifest.json`).
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
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

/// An app installed on this host, persisted in `~/.dotapps/apps.json`.
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub manifest: Manifest,
    pub host_port: Option<u16>,
    #[serde(default)]
    pub running: bool,
}

/// An app as listed by the registry's store catalog.
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StoreApp {
    pub manifest: Manifest,
}

impl Manifest {
    /// Validate every field that reaches a shell command, image tag, or path.
    ///
    /// The manifest arrives over the network from the registry, so this runs
    /// before any of its fields are interpolated into an `sh -c` string.
    pub fn validate(&self) -> Result<(), AppError> {
        validate_slug(&self.slug)?;
        validate_version(&self.version)?;
        if self.internal_port == 0 {
            return Err(AppError::InvalidInput {
                field: "internalPort".into(),
                reason: "must be non-zero".into(),
            });
        }
        Ok(())
    }

    /// Podman image reference used to run the app. The CLI builds and saves
    /// the image as `dotapps/{slug}:{version}`; `podman load` stores a
    /// registry-less image under the `localhost/` namespace, and running it by
    /// the bare name would trigger an unqualified-registry search against
    /// docker.io. The `localhost/` prefix forces a purely local lookup.
    pub fn image_ref(&self) -> String {
        format!("localhost/dotapps/{}:{}", self.slug, self.version)
    }

    /// Container name inside the VM. The `dotapps-` prefix keeps these apart
    /// from the agent-managed `opnble-*` project containers.
    pub fn container_name(&self) -> String {
        format!("dotapps-{}", self.slug)
    }

    /// Named volume mounted at `/data`. Version-independent so app data
    /// survives updates.
    pub fn volume_name(&self) -> String {
        format!("dotapps-{}-data", self.slug)
    }
}

/// Validate a slug for safe use in shell commands and filesystem paths.
///
/// Matches the registry's slug rules: lowercase alphanumerics and dashes only.
/// Slugs are interpolated into `sh -c` strings sent to the VM, so anything
/// outside this set is rejected before it reaches a command line.
pub fn validate_slug(slug: &str) -> Result<(), AppError> {
    let valid = !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(AppError::InvalidInput {
            field: "slug".into(),
            reason: format!("'{slug}' must contain only lowercase letters, digits, and dashes"),
        })
    }
}

/// Validate a version string for safe use in shell commands and image tags.
///
/// Like the slug, `version` is interpolated into the podman image reference
/// inside an `sh -c` string, so it is restricted to a conservative charset.
pub fn validate_version(version: &str) -> Result<(), AppError> {
    let valid = !version.is_empty()
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(AppError::InvalidInput {
            field: "version".into(),
            reason: format!("'{version}' must be 1-64 chars of letters, digits, '.', '_', or '-'"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            name: "Cafe Tracker".to_string(),
            slug: "cafe-tracker".to_string(),
            version: "1.0.0".to_string(),
            icon: "☕".to_string(),
            internal_port: 8000,
            description: "Log coffee bean deliveries".to_string(),
        }
    }

    #[test]
    fn manifest_round_trips_camel_case_json() {
        let json = r#"{"name":"Cafe Tracker","slug":"cafe-tracker","version":"1.0.0","icon":"☕","internalPort":8000,"description":"Log coffee bean deliveries"}"#;
        let parsed: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.name, "Cafe Tracker");
        assert_eq!(parsed.internal_port, 8000);

        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(
            value.get("internalPort").and_then(serde_json::Value::as_u64),
            Some(8000)
        );
        assert!(value.get("internal_port").is_none());
    }

    #[test]
    fn manifest_description_defaults_to_empty() {
        let json = r#"{"name":"X","slug":"x","version":"0.1.0","icon":"📦","internalPort":3000}"#;
        let parsed: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.description, "");
    }

    #[test]
    fn naming_helpers_derive_from_slug_and_version() {
        let m = manifest();
        assert_eq!(m.image_ref(), "localhost/dotapps/cafe-tracker:1.0.0");
        assert_eq!(m.container_name(), "dotapps-cafe-tracker");
        assert_eq!(m.volume_name(), "dotapps-cafe-tracker-data");
    }

    #[test]
    fn installed_app_serializes_camel_case_and_defaults_running() {
        let json = r#"{"manifest":{"name":"X","slug":"x","version":"0.1.0","icon":"📦","internalPort":3000},"hostPort":4100}"#;
        let parsed: InstalledApp = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.host_port, Some(4100));
        assert!(!parsed.running);

        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(
            value.get("hostPort").and_then(serde_json::Value::as_u64),
            Some(4100)
        );
        assert_eq!(
            value.get("running").and_then(serde_json::Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn validate_slug_accepts_kebab_case() {
        assert!(validate_slug("cafe-tracker").is_ok());
        assert!(validate_slug("app2").is_ok());
    }

    #[test]
    fn validate_slug_rejects_shell_metacharacters() {
        assert!(validate_slug("").is_err());
        assert!(validate_slug("Cafe").is_err());
        assert!(validate_slug("a b").is_err());
        assert!(validate_slug("a;rm -rf /").is_err());
        assert!(validate_slug("../escape").is_err());
        assert!(validate_slug("a$(boom)").is_err());
    }

    #[test]
    fn validate_version_accepts_semver_like() {
        assert!(validate_version("1.0.0").is_ok());
        assert!(validate_version("2.0.0-beta.1").is_ok());
    }

    #[test]
    fn validate_version_rejects_shell_metacharacters() {
        assert!(validate_version("").is_err());
        assert!(validate_version("1.0 ; rm -rf /").is_err());
        assert!(validate_version("$(boom)").is_err());
        assert!(validate_version("1/0").is_err());
        assert!(validate_version("a b").is_err());
    }

    #[test]
    fn validate_rejects_injection_in_version() {
        let mut m = manifest();
        m.version = "1.0.0; rm -rf /".to_string();
        assert!(m.validate().is_err());
    }

    #[test]
    fn validate_accepts_clean_manifest() {
        assert!(manifest().validate().is_ok());
    }

    #[test]
    fn validate_rejects_port_zero() {
        let mut m = manifest();
        m.internal_port = 0;
        assert!(m.validate().is_err());
    }
}
