use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::archive;
use crate::manifest::Manifest;

/// `vibox pack`: build the Docker image for the project in `dir` and write a
/// `.vibox` archive (default `{slug}-{version}.vibox` in the current directory).
pub fn run(dir: &Path, out: Option<&Path>) -> Result<()> {
    let manifest = read_project_manifest(dir)?;
    let tag = manifest.image_tag();
    let out = out.map_or_else(|| default_out_path(&manifest), Path::to_path_buf);

    build_image(dir, &tag)?;

    // Random, exclusively-created temp dir (removed on drop, even on error);
    // a predictable path would be open to symlink/TOCTOU games.
    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    let image_tar = tmp_dir.path().join(archive::IMAGE_ENTRY);
    save_image(&tag, &image_tar)?;
    archive::create_vibox(&manifest, &image_tar, &out)?;

    let size = std::fs::metadata(&out)
        .with_context(|| format!("failed to stat {}", out.display()))?
        .len();
    crate::say(&format!(
        "wrote {} ({size} bytes, {})",
        out.display(),
        human_size(size)
    ));
    Ok(())
}

fn read_project_manifest(dir: &Path) -> Result<Manifest> {
    let path = dir.join("vibox.json");
    let bytes = std::fs::read(&path).with_context(|| {
        format!(
            "failed to read {} — does the project have a vibox.json?",
            path.display()
        )
    })?;
    let manifest: Manifest = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "{} is not a valid manifest (name, slug, version, icon, internalPort are required)",
            path.display()
        )
    })?;
    manifest.validate()?;
    Ok(manifest)
}

fn default_out_path(manifest: &Manifest) -> PathBuf {
    PathBuf::from(format!("{}-{}.vibox", manifest.slug, manifest.version))
}

/// Runs `docker build` with inherited stdio so build progress stays visible.
fn build_image(dir: &Path, tag: &str) -> Result<()> {
    crate::say(&format!(
        "building {tag} (docker build --platform linux/arm64)"
    ));
    let status = Command::new("docker")
        .args(["build", "--platform", "linux/arm64", "-t", tag])
        .arg(dir)
        .status()
        .context("failed to run docker — is Docker installed and on PATH?")?;
    if !status.success() {
        bail!("docker build failed ({status}) — see output above");
    }
    Ok(())
}

fn save_image(tag: &str, image_tar: &Path) -> Result<()> {
    let output = Command::new("docker")
        .arg("save")
        .arg("-o")
        .arg(image_tar)
        .arg(tag)
        .output()
        .context("failed to run docker — is Docker installed and on PATH?")?;
    if !output.status.success() {
        bail!(
            "docker save failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Formats a byte count as tenths of a MiB using integer math only
/// (the workspace denies `as` conversions).
fn human_size(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    let tenths = bytes.saturating_mul(10) / MIB;
    format!("{}.{} MiB", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::{default_out_path, human_size, read_project_manifest};
    use std::path::PathBuf;

    #[test]
    fn default_out_path_is_slug_dash_version() {
        let manifest = serde_json::from_str(
            r#"{ "name": "Smoke", "slug": "smoke-cli", "version": "0.0.1", "icon": "🧪", "internalPort": 8000 }"#,
        )
        .expect("manifest parses");
        assert_eq!(
            default_out_path(&manifest),
            PathBuf::from("smoke-cli-0.0.1.vibox")
        );
    }

    #[test]
    fn human_size_renders_tenths_of_mib() {
        assert_eq!(human_size(0), "0.0 MiB");
        assert_eq!(human_size(1024 * 1024), "1.0 MiB");
        assert_eq!(human_size(1024 * 1024 * 3 / 2), "1.5 MiB");
        assert_eq!(human_size(100 * 1024 * 1024), "100.0 MiB");
    }

    #[test]
    fn read_project_manifest_reports_missing_vibox_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = read_project_manifest(dir.path()).expect_err("missing vibox.json must fail");
        assert!(
            error.to_string().contains("vibox.json"),
            "error should name the file: {error}"
        );
    }

    #[test]
    fn read_project_manifest_parses_project_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("vibox.json"),
            r#"{ "name": "Smoke", "slug": "smoke-cli", "version": "0.0.1", "icon": "🧪", "internalPort": 8000, "description": "" }"#,
        )
        .expect("write vibox.json");
        let manifest = read_project_manifest(dir.path()).expect("manifest parses");
        assert_eq!(manifest.slug, "smoke-cli");
        assert_eq!(manifest.version, "0.0.1");
    }
}
