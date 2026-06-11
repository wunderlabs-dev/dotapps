use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::manifest::Manifest;

/// Frozen `.apps` entry names: the archive is a zstd-compressed tar holding
/// exactly these two files at its root.
pub const MANIFEST_ENTRY: &str = "manifest.json";
pub const IMAGE_ENTRY: &str = "image.tar";

const ZSTD_LEVEL: i32 = 3;

/// Writes `out` as a `.apps` archive containing `manifest.json` (serialized
/// from `manifest`) and `image.tar` (copied from `image_tar`).
pub fn create_artifact(manifest: &Manifest, image_tar: &Path, out: &Path) -> Result<()> {
    let out_file =
        File::create(out).with_context(|| format!("failed to create {}", out.display()))?;
    let encoder =
        zstd::stream::Encoder::new(out_file, ZSTD_LEVEL).context("failed to start zstd encoder")?;
    let mut builder = tar::Builder::new(encoder);

    let manifest_bytes =
        serde_json::to_vec_pretty(manifest).context("failed to serialize manifest")?;
    let mut header = tar::Header::new_gnu();
    header.set_size(u64::try_from(manifest_bytes.len()).context("manifest too large for tar")?);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, MANIFEST_ENTRY, manifest_bytes.as_slice())
        .with_context(|| format!("failed to add {MANIFEST_ENTRY} to archive"))?;

    let mut image_file =
        File::open(image_tar).with_context(|| format!("failed to open {}", image_tar.display()))?;
    builder
        .append_file(IMAGE_ENTRY, &mut image_file)
        .with_context(|| format!("failed to add {IMAGE_ENTRY} to archive"))?;

    let encoder = builder
        .into_inner()
        .context("failed to finish tar archive")?;
    encoder.finish().context("failed to finish zstd stream")?;
    Ok(())
}

/// Reads `manifest.json` out of a `.apps` archive without unpacking it to disk.
pub fn read_manifest(artifact: &Path) -> Result<Manifest> {
    let file =
        File::open(artifact).with_context(|| format!("failed to open {}", artifact.display()))?;
    let decoder = zstd::stream::Decoder::new(file)
        .with_context(|| format!("{} is not a zstd archive", artifact.display()))?;
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .with_context(|| format!("{} is not a tar archive", artifact.display()))?;
    for entry in entries {
        let mut entry = entry.context("failed to read archive entry")?;
        let path = entry
            .path()
            .context("archive entry has an invalid path")?
            .into_owned();
        if path == Path::new(MANIFEST_ENTRY) {
            let mut json = String::new();
            entry
                .read_to_string(&mut json)
                .with_context(|| format!("failed to read {MANIFEST_ENTRY}"))?;
            return serde_json::from_str(&json)
                .with_context(|| format!("{MANIFEST_ENTRY} is not a valid manifest"));
        }
    }
    bail!("{} contains no {MANIFEST_ENTRY}", artifact.display());
}

#[cfg(test)]
mod tests {
    use super::{create_artifact, read_manifest, IMAGE_ENTRY, MANIFEST_ENTRY};
    use crate::manifest::Manifest;
    use std::collections::BTreeMap;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    fn frozen_manifest() -> Manifest {
        serde_json::from_str(
            r#"{ "name": "Cafe Tracker", "slug": "cafe-tracker", "version": "1.0.0", "icon": "☕", "internalPort": 8000, "description": "Log coffee bean deliveries" }"#,
        )
        .expect("frozen manifest JSON parses")
    }

    /// Decodes a `.apps` (zstd tar) and returns entry name -> contents.
    fn archive_entries(artifact: &Path) -> BTreeMap<String, Vec<u8>> {
        let file = File::open(artifact).expect("open archive");
        let decoder = zstd::stream::Decoder::new(file).expect("zstd decoder");
        let mut archive = tar::Archive::new(decoder);
        let mut entries = BTreeMap::new();
        for entry in archive.entries().expect("archive entries") {
            let mut entry = entry.expect("archive entry");
            let name = entry.path().expect("entry path").display().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).expect("read entry");
            entries.insert(name, data);
        }
        entries
    }

    #[test]
    fn create_artifact_round_trips_through_extraction() {
        let dir = tempfile::tempdir().expect("tempdir");
        let image_tar = dir.path().join("image.tar");
        std::fs::write(&image_tar, b"pretend docker image").expect("write fake image.tar");
        let out = dir.path().join("cafe-tracker-1.0.0.apps");

        create_artifact(&frozen_manifest(), &image_tar, &out).expect("create_artifact");

        let entries = archive_entries(&out);
        let names: Vec<&str> = entries.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            vec![IMAGE_ENTRY, MANIFEST_ENTRY],
            "archive must contain exactly the two frozen entries"
        );
        assert_eq!(
            entries.get(IMAGE_ENTRY).expect("image entry").as_slice(),
            b"pretend docker image".as_slice()
        );
        let manifest: Manifest =
            serde_json::from_slice(entries.get(MANIFEST_ENTRY).expect("manifest entry"))
                .expect("manifest.json inside archive parses");
        assert_eq!(manifest, frozen_manifest());
    }

    #[test]
    fn read_manifest_pulls_manifest_out_of_archive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let image_tar = dir.path().join("image.tar");
        std::fs::write(&image_tar, b"bytes").expect("write fake image.tar");
        let out = dir.path().join("app.apps");
        create_artifact(&frozen_manifest(), &image_tar, &out).expect("create_artifact");

        let manifest = read_manifest(&out).expect("read_manifest");
        assert_eq!(manifest, frozen_manifest());
    }

    #[test]
    fn read_manifest_rejects_archive_without_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("broken.apps");
        let file = File::create(&out).expect("create archive file");
        let encoder = zstd::stream::Encoder::new(file, 3).expect("zstd encoder");
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(5);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, IMAGE_ENTRY, b"bytes".as_slice())
            .expect("append image.tar");
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish zstd");

        let error = read_manifest(&out).expect_err("archive without manifest.json must fail");
        assert!(
            error.to_string().contains("manifest.json"),
            "error should mention the missing entry: {error}"
        );
    }
}
