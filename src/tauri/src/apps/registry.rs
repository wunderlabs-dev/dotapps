//! Registry client: fetch the app catalog and download `.apps` artifacts.
//!
//! The registry base URL comes from the `DOTAPPS_REGISTRY` environment
//! variable, falling back to [`DEFAULT_REGISTRY`].

use std::path::Path;

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use super::types::{Manifest, StoreApp};
use crate::error::AppError;

/// Deployed registry URL. Overridable via `DOTAPPS_REGISTRY`. Uses the branded
/// custom domain; the worker is also reachable at
/// `dotapps-registry.isopusoktoday.workers.dev` as a fallback.
pub const DEFAULT_REGISTRY: &str = "https://registry.dotapps.club";

/// The only entries a `.apps` archive may contain (frozen contract).
const DOTAPPS_ENTRIES: [&str; 2] = ["manifest.json", "image.tar"];

/// Registry base URL, overridable via `DOTAPPS_REGISTRY`.
fn registry_base() -> String {
    std::env::var("DOTAPPS_REGISTRY").unwrap_or_else(|_| DEFAULT_REGISTRY.to_string())
}

#[derive(serde::Deserialize)]
struct AppsResponse {
    apps: Vec<StoreApp>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LatestResponse {
    manifest: Manifest,
    download_url: String,
}

/// GET `url` and fail with status + body on non-2xx so registry errors stay
/// human-readable.
async fn get_success(url: &str) -> Result<reqwest::Response, AppError> {
    let response = reqwest::Client::new().get(url).send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Internal {
            reason: format!("registry request to {url} failed: HTTP {status}: {body}"),
        });
    }
    Ok(response)
}

/// `GET {base}/v1/apps` → the published app catalog.
pub async fn fetch_store_apps() -> Result<Vec<StoreApp>, AppError> {
    let url = format!("{}/v1/apps", registry_base());
    let parsed: AppsResponse = get_success(&url).await?.json().await?;
    Ok(parsed.apps)
}

/// `GET {base}/v1/apps/{slug}/latest` → (manifest, download URL).
pub async fn fetch_latest(slug: &str) -> Result<(Manifest, String), AppError> {
    let url = format!("{}/v1/apps/{slug}/latest", registry_base());
    let parsed: LatestResponse = get_success(&url).await?.json().await?;
    Ok((parsed.manifest, parsed.download_url))
}

/// `GET {base}/v1/apps/{slug}/versions/{version}` → (manifest, download URL).
///
/// Resolves an exact version, backing `dotapps://slug@version` deep links.
pub async fn fetch_version(slug: &str, version: &str) -> Result<(Manifest, String), AppError> {
    let url = format!("{}/v1/apps/{slug}/versions/{version}", registry_base());
    let parsed: LatestResponse = get_success(&url).await?.json().await?;
    Ok((parsed.manifest, parsed.download_url))
}

/// Stream a `.apps` artifact from `url` to `dest`.
pub async fn download_artifact(url: &str, dest: &Path) -> Result<(), AppError> {
    let response = get_success(url).await?;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(dest).await?;
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?).await?;
    }
    file.flush().await?;
    Ok(())
}

/// Unpack a `.apps` file (zstd-compressed tar of exactly `manifest.json` +
/// `image.tar` at the root) into `dest_dir` and return the parsed manifest.
///
/// Rejects archives containing any other entry so a malicious artifact can
/// never write outside the app's repo directory.
pub fn unpack_artifact(file: &Path, dest_dir: &Path) -> Result<Manifest, AppError> {
    std::fs::create_dir_all(dest_dir)?;

    let decoder = zstd::stream::Decoder::new(std::fs::File::open(file)?)?;
    let mut archive = tar::Archive::new(decoder);
    let mut seen: Vec<String> = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry_file_name(entry.path()?.as_ref())?;
        entry.unpack(dest_dir.join(&name))?;
        seen.push(name);
    }

    for required in DOTAPPS_ENTRIES {
        if !seen.iter().any(|s| s == required) {
            return Err(invalid_artifact(format!(
                "archive missing required entry: {required}"
            )));
        }
    }

    let manifest = std::fs::read(dest_dir.join("manifest.json"))?;
    Ok(serde_json::from_slice(&manifest)?)
}

/// Reduce an archive entry path to one of the allowed root file names,
/// tolerating a leading `./`. Anything else (subpaths, `..`, unknown names)
/// is rejected.
fn entry_file_name(path: &Path) -> Result<String, AppError> {
    let mut components = path
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir));
    let (first, rest) = (components.next(), components.next());
    if let (Some(std::path::Component::Normal(name)), None) = (first, rest) {
        let name = name.to_string_lossy().into_owned();
        if DOTAPPS_ENTRIES.contains(&name.as_str()) {
            return Ok(name);
        }
    }
    Err(invalid_artifact(format!(
        "unexpected archive entry: {}",
        path.display()
    )))
}

fn invalid_artifact(reason: String) -> AppError {
    AppError::InvalidInput {
        field: "dotapps".into(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    const MANIFEST_JSON: &str = r#"{"name":"Smoke","slug":"smoke","version":"0.0.1","icon":"🧪","internalPort":8000,"description":""}"#;

    /// Build a `.apps` with the same crates the CLI uses (tar + zstd).
    fn write_artifact(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let encoder = zstd::stream::Encoder::new(file, 3).unwrap();
        let mut builder = tar::Builder::new(encoder);
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(u64::try_from(data.len()).unwrap());
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, name, *data).unwrap();
        }
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap().flush().unwrap();
    }

    #[test]
    fn unpack_artifact_round_trips_manifest_and_image() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("app.apps");
        write_artifact(
            &archive,
            &[
                ("manifest.json", MANIFEST_JSON.as_bytes()),
                ("image.tar", b"dummy image bytes"),
            ],
        );

        let dest = dir.path().join("unpacked");
        let manifest = unpack_artifact(&archive, &dest).unwrap();

        assert_eq!(manifest.slug, "smoke");
        assert_eq!(manifest.internal_port, 8000);
        assert_eq!(
            std::fs::read(dest.join("image.tar")).unwrap(),
            b"dummy image bytes"
        );
    }

    #[test]
    fn unpack_artifact_accepts_dot_slash_prefixed_entries() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("app.apps");
        write_artifact(
            &archive,
            &[
                ("./manifest.json", MANIFEST_JSON.as_bytes()),
                ("./image.tar", b"img"),
            ],
        );

        let dest = dir.path().join("unpacked");
        let manifest = unpack_artifact(&archive, &dest).unwrap();
        assert_eq!(manifest.slug, "smoke");
    }

    #[test]
    fn unpack_artifact_rejects_unknown_entries() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("app.apps");
        write_artifact(
            &archive,
            &[
                ("manifest.json", MANIFEST_JSON.as_bytes()),
                ("image.tar", b"img"),
                ("evil.sh", b"#!/bin/sh\nrm -rf /\n"),
            ],
        );

        let dest = dir.path().join("unpacked");
        let err = unpack_artifact(&archive, &dest).unwrap_err();
        assert!(
            matches!(&err, AppError::InvalidInput { field, .. } if field == "dotapps"),
            "expected InvalidInput, got {err:?}"
        );
        assert!(!dest.join("evil.sh").exists());
    }

    #[test]
    fn unpack_artifact_rejects_nested_paths() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("app.apps");
        write_artifact(
            &archive,
            &[("nested/manifest.json", MANIFEST_JSON.as_bytes())],
        );

        let err = unpack_artifact(&archive, &dir.path().join("unpacked")).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput { .. }));
    }

    #[test]
    fn unpack_artifact_rejects_missing_image_tar() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("app.apps");
        write_artifact(&archive, &[("manifest.json", MANIFEST_JSON.as_bytes())]);

        let err = unpack_artifact(&archive, &dir.path().join("unpacked")).unwrap_err();
        assert!(
            matches!(&err, AppError::InvalidInput { reason, .. } if reason.contains("image.tar")),
            "expected missing image.tar error, got {err:?}"
        );
    }

    #[test]
    fn default_registry_is_used_without_env_override() {
        // DOTAPPS_REGISTRY is unset in tests; std::env::set_var is disallowed
        // (not thread-safe), so only the fallback path is exercised here.
        assert_eq!(registry_base(), DEFAULT_REGISTRY);
    }
}
