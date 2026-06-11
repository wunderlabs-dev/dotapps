use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use reqwest::blocking::{Body, Client, Request, Response};
use serde::Deserialize;

use crate::archive;
use crate::manifest::Manifest;

/// Response to `POST /v1/apps/{slug}/versions` (frozen camelCase contract).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartUploadResponse {
    upload_url: String,
    complete_url: String,
}

#[derive(Debug, Deserialize)]
struct CompleteResponse {
    ok: bool,
}

/// `dotapps publish`: upload a `.apps` archive to the registry.
///
/// The archive defaults to the newest `*.apps` in the current directory, the
/// registry to `$DOTAPPS_REGISTRY`; the publish token comes from `$DOTAPPS_TOKEN`.
pub fn run(file: Option<&Path>, registry: Option<&str>) -> Result<()> {
    let registry = resolve_registry(registry)?;
    let registry_url = ensure_safe_registry(&registry)?;
    let token = std::env::var("DOTAPPS_TOKEN")
        .context("DOTAPPS_TOKEN is not set — export the registry publish token")?;
    let file = match file {
        Some(file) => file.to_path_buf(),
        None => newest_artifact(Path::new("."))?,
    };
    let manifest = archive::read_manifest(&file)?;
    manifest.validate()?;

    // No client timeout: uploading a multi-hundred-MB image can take a while.
    let client = Client::builder()
        .timeout(None)
        .build()
        .context("failed to build HTTP client")?;

    let response = execute(
        &client,
        build_start_request(&client, &registry, &token, &manifest)?,
        "start upload",
    )?;
    let start: StartUploadResponse = response
        .json()
        .context("registry returned invalid JSON for start upload")?;

    let blob = File::open(&file).with_context(|| format!("failed to open {}", file.display()))?;
    let len = blob
        .metadata()
        .with_context(|| format!("failed to stat {}", file.display()))?
        .len();
    execute(
        &client,
        build_upload_request(&client, &registry_url, &start.upload_url, &token, blob, len)?,
        "upload",
    )?;

    let response = execute(
        &client,
        build_complete_request(&client, &registry_url, &start.complete_url, &token)?,
        "complete upload",
    )?;
    let complete: CompleteResponse = response
        .json()
        .context("registry returned invalid JSON for complete upload")?;
    if !complete.ok {
        bail!("registry did not confirm the upload (expected {{\"ok\":true}})");
    }

    crate::say(&format!("published {} {}", manifest.slug, manifest.version));
    Ok(())
}

fn resolve_registry(flag: Option<&str>) -> Result<String> {
    if let Some(registry) = flag {
        return Ok(registry.to_owned());
    }
    std::env::var("DOTAPPS_REGISTRY")
        .context("no registry specified — pass --registry or set DOTAPPS_REGISTRY")
}

/// Newest `*.apps` in `dir` by modification time.
fn newest_artifact(dir: &Path) -> Result<PathBuf> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read directory {}", dir.display()))?;
    for entry in entries {
        let entry = entry.context("failed to read directory entry")?;
        let path = entry.path();
        if path.extension().and_then(std::ffi::OsStr::to_str) != Some("apps") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .with_context(|| format!("failed to stat {}", path.display()))?;
        if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
            newest = Some((modified, path));
        }
    }
    match newest {
        Some((_, path)) => Ok(path),
        None => bail!(
            "no *.apps files in {} — run `dotapps pack` first or pass --file",
            dir.display()
        ),
    }
}

fn versions_url(registry: &str, slug: &str) -> String {
    format!("{}/v1/apps/{slug}/versions", registry.trim_end_matches('/'))
}

/// Rejects registry URLs that would expose the publish token: only `https`,
/// or plain `http` to loopback (the local `wrangler dev` fallback).
fn ensure_safe_registry(registry: &str) -> Result<reqwest::Url> {
    let url: reqwest::Url = registry
        .parse()
        .with_context(|| format!("invalid registry URL {registry:?}"))?;
    match url.scheme() {
        "https" => Ok(url),
        "http" if is_loopback(&url) => Ok(url),
        "http" => bail!(
            "refusing to send the publish token over plain http to {registry} — use https (http is allowed only for localhost)"
        ),
        other => bail!("unsupported registry URL scheme {other:?}"),
    }
}

fn is_loopback(url: &reqwest::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(domain)) => domain == "localhost",
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// The publish token is attached only to requests that target the registry's
/// own origin; presigned blob URLs on other hosts must never see it.
fn same_origin(a: &reqwest::Url, b: &reqwest::Url) -> bool {
    a.scheme() == b.scheme() && a.host() == b.host() && a.port_or_known_default() == b.port_or_known_default()
}

fn build_start_request(
    client: &Client,
    registry: &str,
    token: &str,
    manifest: &Manifest,
) -> Result<Request> {
    client
        .post(versions_url(registry, &manifest.slug))
        .bearer_auth(token)
        .json(manifest)
        .build()
        .context("failed to build start-upload request")
}

/// Bearer is required on the registry's fallback `/v1/blob/...` upload route
/// (same origin); presigned URLs on other origins authenticate via their own
/// signature and must not receive the token.
fn build_upload_request(
    client: &Client,
    registry: &reqwest::Url,
    upload_url: &str,
    token: &str,
    blob: File,
    len: u64,
) -> Result<Request> {
    let url: reqwest::Url = upload_url
        .parse()
        .with_context(|| format!("registry returned an invalid upload URL {upload_url:?}"))?;
    let mut request = client
        .put(url.clone())
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::sized(blob, len));
    if same_origin(registry, &url) {
        request = request.bearer_auth(token);
    }
    request.build().context("failed to build upload request")
}

fn build_complete_request(
    client: &Client,
    registry: &reqwest::Url,
    complete_url: &str,
    token: &str,
) -> Result<Request> {
    let url: reqwest::Url = complete_url
        .parse()
        .with_context(|| format!("registry returned an invalid complete URL {complete_url:?}"))?;
    let mut request = client.post(url.clone());
    if same_origin(registry, &url) {
        request = request.bearer_auth(token);
    }
    request.build().context("failed to build complete request")
}

/// Executes a request and turns any non-2xx response into a human-readable
/// error carrying the HTTP status and response body.
fn execute(client: &Client, request: Request, what: &str) -> Result<Response> {
    let url = request.url().clone();
    let response = client
        .execute(request)
        .with_context(|| format!("{what} request to {url} failed"))?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().unwrap_or_default();
    bail!("{what} request to {url} failed: HTTP {status}\n{body}");
}

#[cfg(test)]
mod tests {
    use super::{
        build_complete_request, build_start_request, build_upload_request, newest_artifact,
        versions_url, StartUploadResponse,
    };
    use crate::manifest::Manifest;
    use std::fs::File;
    use std::time::{Duration, SystemTime};

    fn frozen_manifest() -> Manifest {
        serde_json::from_str(
            r#"{ "name": "Cafe Tracker", "slug": "cafe-tracker", "version": "1.0.0", "icon": "☕", "internalPort": 8000, "description": "Log coffee bean deliveries" }"#,
        )
        .expect("frozen manifest JSON parses")
    }

    fn header(request: &reqwest::blocking::Request, name: &str) -> String {
        request
            .headers()
            .get(name)
            .expect("header should be present")
            .to_str()
            .expect("header is ASCII")
            .to_owned()
    }

    #[test]
    fn versions_url_joins_registry_and_slug() {
        assert_eq!(
            versions_url("https://reg.example", "smoke"),
            "https://reg.example/v1/apps/smoke/versions"
        );
        assert_eq!(
            versions_url("https://reg.example/", "smoke"),
            "https://reg.example/v1/apps/smoke/versions"
        );
    }

    #[test]
    fn start_request_posts_manifest_json_with_bearer() {
        let client = reqwest::blocking::Client::new();
        let request = build_start_request(
            &client,
            "https://reg.example",
            "tok-123",
            &frozen_manifest(),
        )
        .expect("build start request");
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(
            request.url().as_str(),
            "https://reg.example/v1/apps/cafe-tracker/versions"
        );
        assert_eq!(header(&request, "authorization"), "Bearer tok-123");
        assert_eq!(header(&request, "content-type"), "application/json");
        let body = request
            .body()
            .expect("body")
            .as_bytes()
            .expect("buffered body");
        let sent: Manifest = serde_json::from_slice(body).expect("body is manifest JSON");
        assert_eq!(sent, frozen_manifest());
    }

    fn registry_url() -> reqwest::Url {
        "https://reg.example".parse().expect("registry URL parses")
    }

    fn open_blob() -> File {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("app.apps");
        std::fs::write(&path, b"bytes").expect("write archive");
        File::open(&path).expect("open archive")
    }

    #[test]
    fn upload_to_registry_origin_carries_bearer() {
        let client = reqwest::blocking::Client::new();
        let request = build_upload_request(
            &client,
            &registry_url(),
            "https://reg.example/v1/blob/apps/x/1.0.0/app.apps",
            "tok-123",
            open_blob(),
            5,
        )
        .expect("build upload request");
        assert_eq!(request.method(), reqwest::Method::PUT);
        assert_eq!(header(&request, "authorization"), "Bearer tok-123");
        assert_eq!(header(&request, "content-type"), "application/octet-stream");
    }

    #[test]
    fn upload_to_foreign_origin_never_carries_bearer() {
        let client = reqwest::blocking::Client::new();
        let request = build_upload_request(
            &client,
            &registry_url(),
            "https://blob.example/upload?sig=abc",
            "tok-123",
            open_blob(),
            5,
        )
        .expect("build upload request");
        assert!(
            request.headers().get("authorization").is_none(),
            "publish token must not leak to a foreign origin"
        );
        assert_eq!(header(&request, "content-type"), "application/octet-stream");
    }

    #[test]
    fn complete_on_registry_origin_carries_bearer() {
        let client = reqwest::blocking::Client::new();
        let request = build_complete_request(
            &client,
            &registry_url(),
            "https://reg.example/v1/apps/x/versions/1.0.0/complete",
            "tok-123",
        )
        .expect("build complete request");
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(header(&request, "authorization"), "Bearer tok-123");
    }

    #[test]
    fn complete_on_foreign_origin_never_carries_bearer() {
        let client = reqwest::blocking::Client::new();
        let request = build_complete_request(
            &client,
            &registry_url(),
            "https://evil.example/v1/apps/x/versions/1.0.0/complete",
            "tok-123",
        )
        .expect("build complete request");
        assert!(
            request.headers().get("authorization").is_none(),
            "publish token must not leak to a foreign origin"
        );
    }

    #[test]
    fn registry_must_be_https_unless_loopback() {
        super::ensure_safe_registry("https://reg.example").expect("https accepted");
        super::ensure_safe_registry("http://localhost:8787").expect("http localhost accepted");
        super::ensure_safe_registry("http://127.0.0.1:8787").expect("http 127.0.0.1 accepted");
        assert!(
            super::ensure_safe_registry("http://reg.example").is_err(),
            "plain http to a remote host must be rejected"
        );
        assert!(
            super::ensure_safe_registry("ftp://reg.example").is_err(),
            "non-http schemes must be rejected"
        );
    }

    #[test]
    fn start_upload_response_parses_camel_case() {
        let parsed: StartUploadResponse = serde_json::from_str(
            r#"{"uploadUrl":"https://u.example/put","completeUrl":"https://c.example/done"}"#,
        )
        .expect("response parses");
        assert_eq!(parsed.upload_url, "https://u.example/put");
        assert_eq!(parsed.complete_url, "https://c.example/done");
    }

    #[test]
    fn newest_artifact_picks_most_recently_modified() {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = dir.path().join("old-1.0.0.apps");
        let new = dir.path().join("new-2.0.0.apps");
        std::fs::write(&old, b"old").expect("write old");
        std::fs::write(&new, b"new").expect("write new");
        std::fs::write(dir.path().join("notes.txt"), b"not an archive").expect("write decoy");

        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        File::options()
            .write(true)
            .open(&old)
            .expect("open old")
            .set_modified(base)
            .expect("set old mtime");
        File::options()
            .write(true)
            .open(&new)
            .expect("open new")
            .set_modified(base + Duration::from_secs(60))
            .expect("set new mtime");

        assert_eq!(newest_artifact(dir.path()).expect("newest archive"), new);
    }

    #[test]
    fn newest_artifact_errors_when_directory_has_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = newest_artifact(dir.path()).expect_err("empty dir must fail");
        assert!(
            error.to_string().contains("no *.apps"),
            "error should explain: {error}"
        );
    }
}
