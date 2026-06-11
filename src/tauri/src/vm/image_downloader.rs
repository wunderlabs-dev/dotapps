//! First-run VM image downloader.
//!
//! On macOS the host crate needs a ~4 GiB Alpine VM disk image at runtime.
//! It is too large for GitHub release assets so it lives on Cloudflare R2.
//! At app startup the frontend calls `status()` to see whether the local
//! image matches the published manifest; if not, it calls `download()`
//! which streams the compressed image, verifies SHA256 against the
//! manifest, decompresses with zstd, atomically swaps the result into
//! place, and records the new version stamp.
//!
//! The download is cancellable via `cancel()`. Partial work lives in
//! `<vm_dir>/.alpine-base.partial*` files and is cleaned up on success or
//! cancellation.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::constants::{events, paths, VM_IMAGE_MANIFEST_URL};
use crate::error::AppError;

use super::VmError;

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(250);
const PARTIAL_COMPRESSED: &str = ".alpine-base.partial.zst";
const PARTIAL_DECOMPRESSED: &str = ".alpine-base.partial.img";

#[derive(Debug, Deserialize)]
struct Manifest {
    image_version: String,
    url: String,
    compression: String,
    sha256: String,
    uncompressed_sha256: String,
    compressed_size: u64,
    uncompressed_size: u64,
}

pub use super::image_types::{VmImageProgress, VmImageStatus};

#[derive(Debug, Deserialize, Serialize)]
struct InstalledStamp {
    image_version: String,
}

/// Owns the in-flight download task so a Cancel button can abort it.
#[derive(Default)]
pub struct VmImageDownloader {
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl VmImageDownloader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve whether the locally installed image matches the published
    /// manifest. Returns `needed = true` when either the manifest cannot
    /// be fetched (treat that as "we don't know, ask the user") or when
    /// the version stamp differs from the manifest.
    pub async fn status(&self) -> Result<VmImageStatus, AppError> {
        let manifest = fetch_manifest().await.ok();
        let installed = read_installed_version().await.ok().flatten();
        let target = manifest.as_ref().map(|m| m.image_version.clone());
        let on_disk = base_image_path()?;
        let needed = match (manifest.as_ref(), installed.as_ref()) {
            (Some(m), Some(v)) => v != &m.image_version || !on_disk.exists(),
            (Some(_), None) => true,
            (None, _) => false,
        };
        Ok(VmImageStatus {
            needed,
            installed_version: installed,
            target_version: target,
            compressed_size_bytes: manifest.as_ref().map(|m| m.compressed_size),
            uncompressed_size_bytes: manifest.as_ref().map(|m| m.uncompressed_size),
        })
    }

    /// Stream the image to disk, verify SHA256, decompress, atomic swap.
    /// Runs in a spawned task so cancellation works via `cancel()`.
    pub async fn download(self: Arc<Self>, app: AppHandle) -> Result<(), AppError> {
        let mut guard = self.handle.lock().await;
        if guard.as_ref().is_some_and(|h| !h.is_finished()) {
            return Err(VmError::ImageDownloadFailed {
                reason: "download already in progress".to_string(),
            }
            .into());
        }
        let app_clone = app.clone();
        let task = tokio::spawn(async move {
            if let Err(err) = run_download(&app_clone).await {
                tracing::error!(error = %err, "vm image download failed");
                let _ = app_clone.emit(events::VM_IMAGE_PROGRESS, &progress_failed(&err));
            }
        });
        *guard = Some(task);
        Ok(())
    }

    /// Abort the in-flight download (if any). Idempotent.
    pub async fn cancel(&self) {
        let mut guard = self.handle.lock().await;
        if let Some(handle) = guard.take() {
            handle.abort();
        }
    }
}

fn progress_failed(err: &AppError) -> serde_json::Value {
    serde_json::json!({
        "phase": "failed",
        "reason": err.to_string(),
    })
}

async fn run_download(app: &AppHandle) -> Result<(), AppError> {
    let manifest = fetch_manifest().await?;
    require_compression(&manifest)?;

    let target = base_image_path()?;
    let vm_dir = target
        .parent()
        .ok_or_else(|| VmError::ImageDownloadFailed {
            reason: "vm dir has no parent".to_string(),
        })?;
    tokio::fs::create_dir_all(vm_dir).await.map_err(io_err)?;

    let partial_zst = vm_dir.join(PARTIAL_COMPRESSED);
    let partial_img = vm_dir.join(PARTIAL_DECOMPRESSED);

    app.emit(
        events::VM_IMAGE_PROGRESS,
        &VmImageProgress::Started {
            total: manifest.compressed_size,
        },
    )?;

    stream_to_file(app, &manifest, &partial_zst).await?;
    verify_sha(app, &partial_zst, &manifest.sha256).await?;
    decompress_zstd(app, &partial_zst, &partial_img).await?;
    verify_sha(app, &partial_img, &manifest.uncompressed_sha256).await?;
    finalize(&partial_zst, &partial_img, &target).await?;
    write_installed_version(&manifest.image_version).await?;

    app.emit(events::VM_IMAGE_PROGRESS, &VmImageProgress::Finished)?;
    tracing::info!(
        image_version = %manifest.image_version,
        "vm image installed"
    );
    Ok(())
}

fn require_compression(manifest: &Manifest) -> Result<(), AppError> {
    if manifest.compression == "zstd" {
        Ok(())
    } else {
        Err(VmError::ImageDownloadFailed {
            reason: format!("unsupported compression '{}'", manifest.compression),
        }
        .into())
    }
}

async fn fetch_manifest() -> Result<Manifest, AppError> {
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| VmError::ImageDownloadFailed {
            reason: format!("cannot build http client: {e}"),
        })?;
    let res = client
        .get(VM_IMAGE_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| VmError::ImageDownloadFailed {
            reason: format!("manifest fetch failed: {e}"),
        })?;
    if !res.status().is_success() {
        return Err(VmError::ImageDownloadFailed {
            reason: format!("manifest endpoint returned {}", res.status()),
        }
        .into());
    }
    res.json::<Manifest>()
        .await
        .map_err(|e| VmError::ImageDownloadFailed {
            reason: format!("manifest parse failed: {e}"),
        })
        .map_err(AppError::from)
}

async fn stream_to_file(
    app: &AppHandle,
    manifest: &Manifest,
    dest: &std::path::Path,
) -> Result<(), AppError> {
    let existing = tokio::fs::metadata(dest).await.ok().map_or(0, |m| m.len());
    let mut request = reqwest::Client::new().get(&manifest.url);
    if existing > 0 && existing < manifest.compressed_size {
        request = request.header("Range", format!("bytes={existing}-"));
    } else if existing >= manifest.compressed_size {
        tokio::fs::remove_file(dest).await.ok();
    }

    let response = request
        .send()
        .await
        .map_err(|e| VmError::ImageDownloadFailed {
            reason: format!("download request failed: {e}"),
        })?;
    if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(VmError::ImageDownloadFailed {
            reason: format!("download returned status {}", response.status()),
        }
        .into());
    }

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dest)
        .await
        .map_err(io_err)?;
    let mut downloaded = existing;
    let mut last_emit = Instant::now();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| VmError::ImageDownloadFailed {
            reason: format!("stream chunk failed: {e}"),
        })?;
        file.write_all(&chunk).await.map_err(io_err)?;
        downloaded = downloaded.saturating_add(u64::try_from(chunk.len()).unwrap_or(0));
        if last_emit.elapsed() >= PROGRESS_EMIT_INTERVAL {
            let _ = app.emit(
                events::VM_IMAGE_PROGRESS,
                &VmImageProgress::Downloading {
                    downloaded,
                    total: manifest.compressed_size,
                },
            );
            last_emit = Instant::now();
        }
    }
    file.flush().await.map_err(io_err)?;
    Ok(())
}

async fn verify_sha(
    app: &AppHandle,
    path: &std::path::Path,
    expected: &str,
) -> Result<(), AppError> {
    app.emit(events::VM_IMAGE_PROGRESS, &VmImageProgress::Verifying)?;
    let path = path.to_path_buf();
    let actual = tokio::task::spawn_blocking(move || sha256_file(&path))
        .await
        .map_err(|e| VmError::ImageDownloadFailed {
            reason: format!("hash task join failed: {e}"),
        })??;
    if actual != expected {
        return Err(VmError::ImageDownloadFailed {
            reason: format!("sha256 mismatch (expected {expected}, got {actual})"),
        }
        .into());
    }
    Ok(())
}

fn sha256_file(path: &std::path::Path) -> Result<String, VmError> {
    let mut file = std::fs::File::open(path).map_err(|e| VmError::ImageDownloadFailed {
        reason: format!("cannot open {} for hashing: {e}", path.display()),
    })?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| VmError::ImageDownloadFailed {
        reason: format!("hash read failed: {e}"),
    })?;
    Ok(format!("{:x}", hasher.finalize()))
}

async fn decompress_zstd(
    app: &AppHandle,
    src: &std::path::Path,
    dst: &std::path::Path,
) -> Result<(), AppError> {
    app.emit(events::VM_IMAGE_PROGRESS, &VmImageProgress::Decompressing)?;
    let src_path = src.to_path_buf();
    let dst_path = dst.to_path_buf();
    tokio::task::spawn_blocking(move || decompress_blocking(&src_path, &dst_path))
        .await
        .map_err(|e| VmError::ImageDownloadFailed {
            reason: format!("decompress task join failed: {e}"),
        })??;
    Ok(())
}

fn decompress_blocking(src: &std::path::Path, dst: &std::path::Path) -> Result<(), VmError> {
    let input = std::fs::File::open(src).map_err(|e| VmError::ImageDownloadFailed {
        reason: format!("cannot open compressed image: {e}"),
    })?;
    let mut decoder = zstd::Decoder::new(input).map_err(|e| VmError::ImageDownloadFailed {
        reason: format!("zstd init failed: {e}"),
    })?;
    let mut output = std::fs::File::create(dst).map_err(|e| VmError::ImageDownloadFailed {
        reason: format!("cannot create decompressed image: {e}"),
    })?;
    std::io::copy(&mut decoder, &mut output).map_err(|e| VmError::ImageDownloadFailed {
        reason: format!("zstd decode failed: {e}"),
    })?;
    Ok(())
}

async fn finalize(
    partial_zst: &std::path::Path,
    partial_img: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), AppError> {
    tokio::fs::rename(partial_img, target)
        .await
        .map_err(io_err)?;
    tokio::fs::remove_file(partial_zst).await.ok();
    let mut perms = tokio::fs::metadata(target)
        .await
        .map_err(io_err)?
        .permissions();
    perms.set_readonly(true);
    tokio::fs::set_permissions(target, perms)
        .await
        .map_err(io_err)?;
    Ok(())
}

async fn read_installed_version() -> Result<Option<String>, AppError> {
    let path = paths::vm_image_version_file()?;
    if !path.exists() {
        return Ok(None);
    }
    let body = tokio::fs::read_to_string(&path).await.map_err(io_err)?;
    let stamp: InstalledStamp =
        serde_json::from_str(&body).map_err(|e| AppError::StorageFailed {
            reason: format!("cannot parse image-version.json: {e}"),
        })?;
    Ok(Some(stamp.image_version))
}

async fn write_installed_version(version: &str) -> Result<(), AppError> {
    let path = paths::vm_image_version_file()?;
    let stamp = InstalledStamp {
        image_version: version.to_string(),
    };
    let body = serde_json::to_string(&stamp).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot serialize image-version.json: {e}"),
    })?;
    tokio::fs::write(&path, body).await.map_err(io_err)?;
    Ok(())
}

fn base_image_path() -> Result<PathBuf, AppError> {
    paths::vm_base_image()
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "called via .map_err which passes the owned error"
)]
fn io_err(err: std::io::Error) -> AppError {
    AppError::StorageFailed {
        reason: format!("{err} (kind: {:?})", err.kind()),
    }
}
