//! Build a diagnostics zip the tester can attach to a bug report.
//!
//! Layout inside the zip:
//!
//! ```text
//! manifest.json                       version, platform, arch, build channel
//! logs/opnble.log.YYYY-MM-DD          rolled host logs
//! logs/panic.log                      (when present)
//! vm/gvproxy.log                      (macOS only, produced by the vfkit gvproxy child)
//! vm/gvproxy-stderr.log
//! vm/console.log
//! state.json                          token-redacted copy
//! ```
//!
//! Tokens are stripped via a conservative key-name match so we never ship a
//! GitHub OAuth token to a tester clipboard.

use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use crate::constants::paths;
use crate::error::AppError;

const VM_PCAP_MAX_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsExport {
    pub zip_path: String,
}

pub fn export(app_version: &str) -> Result<DiagnosticsExport, AppError> {
    let exports_dir = paths::logs_dir()?.join("exports");
    std::fs::create_dir_all(&exports_dir).map_err(|e| AppError::StorageFailed {
        reason: format!(
            "cannot create exports directory {}: {e}",
            exports_dir.display()
        ),
    })?;

    let zip_path = exports_dir.join(format!("opnble-diagnostics-{}.zip", file_stamp()));
    let file = std::fs::File::create(&zip_path).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot create {}: {e}", zip_path.display()),
    })?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    write_manifest(&mut writer, options, app_version)?;
    add_log_files(&mut writer, options)?;
    add_vm_files(&mut writer, options)?;
    add_redacted_state(&mut writer, options)?;

    writer.finish().map_err(|e| AppError::StorageFailed {
        reason: format!("cannot finalize diagnostics zip: {e}"),
    })?;

    Ok(DiagnosticsExport {
        zip_path: zip_path.to_string_lossy().into_owned(),
    })
}

fn write_manifest(
    writer: &mut zip::ZipWriter<std::fs::File>,
    options: SimpleFileOptions,
    app_version: &str,
) -> Result<(), AppError> {
    let manifest = serde_json::json!({
        "appVersion": app_version,
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "exportedAt": file_stamp(),
        "schemaVersion": 1u32,
    });
    writer
        .start_file("manifest.json", options)
        .map_err(zip_err)?;
    writer
        .write_all(manifest.to_string().as_bytes())
        .map_err(|e| AppError::StorageFailed {
            reason: format!("cannot write manifest: {e}"),
        })
}

fn add_log_files(
    writer: &mut zip::ZipWriter<std::fs::File>,
    options: SimpleFileOptions,
) -> Result<(), AppError> {
    let logs_dir = paths::logs_dir()?;
    if !logs_dir.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(&logs_dir).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot read logs directory {}: {e}", logs_dir.display()),
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let is_log = file_name.starts_with("opnble.log") || file_name == "panic.log";
        if !is_log || !path.is_file() {
            continue;
        }
        copy_into_zip(writer, options, &path, &format!("logs/{file_name}"))?;
    }
    Ok(())
}

fn add_vm_files(
    writer: &mut zip::ZipWriter<std::fs::File>,
    options: SimpleFileOptions,
) -> Result<(), AppError> {
    let vm_dir = paths::vm_dir()?;
    if !vm_dir.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(&vm_dir).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot read VM directory {}: {e}", vm_dir.display()),
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let is_log = ext.as_deref() == Some("log");
        let is_pcap = ext.as_deref() == Some("pcap");
        if !(is_log || is_pcap) || !path.is_file() {
            continue;
        }
        let size = path.metadata().map_or(0, |m| m.len());
        if is_pcap && size > VM_PCAP_MAX_BYTES {
            continue;
        }
        copy_into_zip(writer, options, &path, &format!("vm/{file_name}"))?;
    }
    Ok(())
}

fn add_redacted_state(
    writer: &mut zip::ZipWriter<std::fs::File>,
    options: SimpleFileOptions,
) -> Result<(), AppError> {
    let state_path = paths::state_file()?;
    if !state_path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&state_path).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot read state file {}: {e}", state_path.display()),
    })?;
    let redacted = redact_secrets(&raw);
    writer.start_file("state.json", options).map_err(zip_err)?;
    writer
        .write_all(redacted.as_bytes())
        .map_err(|e| AppError::StorageFailed {
            reason: format!("cannot write state: {e}"),
        })
}

fn copy_into_zip(
    writer: &mut zip::ZipWriter<std::fs::File>,
    options: SimpleFileOptions,
    src: &Path,
    dst_name: &str,
) -> Result<(), AppError> {
    let bytes = std::fs::read(src).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot read {}: {e}", src.display()),
    })?;
    writer.start_file(dst_name, options).map_err(zip_err)?;
    writer
        .write_all(&bytes)
        .map_err(|e| AppError::StorageFailed {
            reason: format!("cannot write {dst_name}: {e}"),
        })
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "called via .map_err which passes the owned error"
)]
fn zip_err(e: zip::result::ZipError) -> AppError {
    AppError::StorageFailed {
        reason: format!("zip write failed: {e}"),
    }
}

/// Replace string values for any JSON key whose name looks like a secret.
/// Conservative on purpose: better to redact a non-secret label than to leak
/// a token. Non-string values (numbers, bools) pass through untouched so we
/// don't break parsing of redacted state for tester triage.
fn redact_secrets(json_text: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(json_text) else {
        return json_text.to_string();
    };
    walk_redact(&mut value);
    value.to_string()
}

fn walk_redact(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if is_secret_key(key) {
                    if let serde_json::Value::String(_) = child {
                        *child = serde_json::Value::String("[REDACTED]".to_string());
                    }
                } else {
                    walk_redact(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                walk_redact(item);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("token")
        || lower.contains("password")
        || lower.contains("secret")
        || lower.contains("api_key")
        || lower.contains("apikey")
}

fn file_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("{secs}")
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "tests assert via direct serde_json::Value indexing"
)]
mod tests {
    use super::*;

    #[test]
    fn redacts_token_fields() {
        let input = r#"{"accessToken":"abc","refresh_token":"xyz","name":"opnble"}"#;
        let out = redact_secrets(input);
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(value["accessToken"], "[REDACTED]");
        assert_eq!(value["refresh_token"], "[REDACTED]");
        assert_eq!(value["name"], "opnble");
    }

    #[test]
    fn leaves_non_secret_keys_intact() {
        let input = r#"{"port":3000,"running":true,"label":"hi"}"#;
        let out = redact_secrets(input);
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(value["port"], 3000);
        assert_eq!(value["running"], true);
        assert_eq!(value["label"], "hi");
    }

    #[test]
    fn handles_nested_objects_and_arrays() {
        let input = r#"{"projects":[{"name":"a","secret":"s"}, {"name":"b","token":"t"}]}"#;
        let out = redact_secrets(input);
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(value["projects"][0]["name"], "a");
        assert_eq!(value["projects"][0]["secret"], "[REDACTED]");
        assert_eq!(value["projects"][1]["name"], "b");
        assert_eq!(value["projects"][1]["token"], "[REDACTED]");
    }

    #[test]
    fn falls_back_to_raw_on_invalid_json() {
        let input = "not json at all";
        let out = redact_secrets(input);
        assert_eq!(out, input);
    }

    #[test]
    fn does_not_replace_non_string_secret_values() {
        let input = r#"{"tokenCount":42}"#;
        let out = redact_secrets(input);
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(value["tokenCount"], 42);
    }
}
