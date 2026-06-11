//! APFS copy-on-write disk image cloning
//!
//! Creates instant copy-on-write clones of the base VM image on macOS.
//! Falls back to regular copy on non-APFS filesystems.

use std::path::Path;

use super::VmError;

/// Create a copy-on-write clone of `src` at `dst`.
///
/// On macOS APFS, uses `clonefile(2)` for instant (<1ms) cloning.
/// Falls back to `std::fs::copy` if cloning is unavailable.
/// Removes `dst` first if it already exists.
pub fn clone_file_or_copy(src: &Path, dst: &Path) -> Result<(), VmError> {
    if !src.exists() {
        return Err(VmError::Io {
            reason: format!("source image not found: {}", src.display()),
        });
    }

    // Remove existing destination (clonefile requires dst not exist)
    if dst.exists() {
        std::fs::remove_file(dst).map_err(|e| VmError::Io {
            reason: format!("cannot remove existing clone {}: {e}", dst.display()),
        })?;
    }

    #[cfg(target_os = "macos")]
    {
        match apfs_clone(src, dst) {
            Ok(()) => {
                make_writable(dst)?;
                tracing::info!(
                    "APFS clone: {} -> {} (instant CoW)",
                    src.display(),
                    dst.display()
                );
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("APFS clone failed, falling back to copy: {e}");
            }
        }
    }

    // Fallback: regular copy
    std::fs::copy(src, dst).map_err(|e| VmError::Io {
        reason: format!("cannot copy {} to {}: {e}", src.display(), dst.display()),
    })?;
    make_writable(dst)?;
    tracing::info!(
        "copied {} -> {} (regular copy)",
        src.display(),
        dst.display()
    );
    Ok(())
}

/// Make a file writable (clone inherits read-only permissions from base).
fn make_writable(path: &Path) -> Result<(), VmError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o644);
    std::fs::set_permissions(path, perms).map_err(|e| VmError::Io {
        reason: format!("cannot set permissions on {}: {e}", path.display()),
    })
}

/// Attempt APFS `clonefile(2)` syscall.
#[cfg(target_os = "macos")]
#[expect(
    unsafe_code,
    reason = "clonefile(2) is a POSIX syscall with no memory-safety invariants; \
              CString args guarantee null-termination"
)]
fn apfs_clone(src: &Path, dst: &Path) -> Result<(), VmError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let src_cstr = CString::new(src.as_os_str().as_bytes()).map_err(|e| VmError::Io {
        reason: format!("invalid source path: {e}"),
    })?;
    let dst_cstr = CString::new(dst.as_os_str().as_bytes()).map_err(|e| VmError::Io {
        reason: format!("invalid destination path: {e}"),
    })?;

    // CLONE_NOFOLLOW = 0x0001 (don't follow symlinks)
    let result = unsafe { libc::clonefile(src_cstr.as_ptr(), dst_cstr.as_ptr(), 0x0001) };

    if result < 0 {
        Err(VmError::Io {
            reason: format!("clonefile failed: {}", std::io::Error::last_os_error()),
        })
    } else {
        Ok(())
    }
}

/// Clone a directory tree CoW-style on APFS, fall back to recursive copy.
///
/// On macOS, Apple's `clonefile(2)` accepts directories and recurses internally.
/// The man page suggests `copyfile(3)` with `COPYFILE_CLONE`, but the underlying
/// primitive is identical for our purposes and `clonefile` keeps the FFI surface
/// to one function we already bind.
///
/// `dst` must not exist; the function removes it first if it does.
pub fn clone_tree_or_copy(src: &Path, dst: &Path) -> Result<(), VmError> {
    if !src.exists() {
        return Err(VmError::Io {
            reason: format!("source tree not found: {}", src.display()),
        });
    }
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| VmError::Io {
            reason: format!("cannot remove existing dst tree {}: {e}", dst.display()),
        })?;
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| VmError::Io {
            reason: format!("cannot create dst parent {}: {e}", parent.display()),
        })?;
    }
    #[cfg(target_os = "macos")]
    {
        match apfs_clone(src, dst) {
            Ok(()) => {
                tracing::info!("APFS tree clone: {} -> {}", src.display(), dst.display());
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("APFS tree clone failed, falling back to copy: {e}");
            }
        }
    }
    copy_tree_recursive_impl(src, dst)?;
    tracing::info!("recursive copy: {} -> {}", src.display(), dst.display());
    Ok(())
}

fn copy_tree_recursive_impl(src: &Path, dst: &Path) -> Result<(), VmError> {
    std::fs::create_dir_all(dst).map_err(|e| VmError::Io {
        reason: format!("cannot create {}: {e}", dst.display()),
    })?;
    for entry in std::fs::read_dir(src).map_err(|e| VmError::Io {
        reason: format!("cannot read {}: {e}", src.display()),
    })? {
        let entry = entry.map_err(|e| VmError::Io {
            reason: format!("read entry: {e}"),
        })?;
        let file_type = entry.file_type().map_err(|e| VmError::Io {
            reason: format!("stat: {e}"),
        })?;
        let child_dst = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree_recursive_impl(&entry.path(), &child_dst)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &child_dst).map_err(|e| VmError::Io {
                reason: format!("copy {}: {e}", entry.path().display()),
            })?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let target = std::fs::read_link(entry.path()).map_err(|e| VmError::Io {
                    reason: format!("readlink {}: {e}", entry.path().display()),
                })?;
                std::os::unix::fs::symlink(&target, &child_dst).map_err(|e| VmError::Io {
                    reason: format!(
                        "symlink {} -> {}: {e}",
                        child_dst.display(),
                        target.display()
                    ),
                })?;
            }
            #[cfg(not(unix))]
            {
                tracing::warn!(
                    "skipping symlink {} on non-unix platform",
                    entry.path().display()
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_clone_creates_cow_copy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("source.img");
        let dst = dir.path().join("clone.img");

        fs::write(&src, b"base image content").expect("write source");

        clone_file_or_copy(&src, &dst).expect("clone");

        assert!(dst.exists(), "clone should exist");
        assert_eq!(fs::read(&dst).expect("read clone"), b"base image content");

        // Modify clone, verify source unchanged
        fs::write(&dst, b"modified clone").expect("write clone");
        assert_eq!(
            fs::read(&src).expect("read source"),
            b"base image content",
            "source must not be affected by clone modification"
        );
    }

    #[test]
    fn test_clone_replaces_existing_destination() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("source.img");
        let dst = dir.path().join("clone.img");

        fs::write(&src, b"base image").expect("write source");
        fs::write(&dst, b"stale clone").expect("write stale");

        clone_file_or_copy(&src, &dst).expect("clone should replace existing");

        assert_eq!(fs::read(&dst).expect("read"), b"base image");
    }

    #[test]
    fn test_clone_fails_if_source_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("nonexistent.img");
        let dst = dir.path().join("clone.img");

        let result = clone_file_or_copy(&src, &dst);
        assert!(result.is_err());
    }

    #[test]
    fn clone_tree_replicates_directory_with_nested_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::create_dir_all(src.join("nested/deep")).unwrap();
        std::fs::write(src.join("a.txt"), b"alpha").unwrap();
        std::fs::write(src.join("nested/b.txt"), b"beta").unwrap();
        std::fs::write(src.join("nested/deep/c.txt"), b"gamma").unwrap();
        clone_tree_or_copy(&src, &dst).expect("clone tree");
        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"alpha");
        assert_eq!(std::fs::read(dst.join("nested/b.txt")).unwrap(), b"beta");
        assert_eq!(
            std::fs::read(dst.join("nested/deep/c.txt")).unwrap(),
            b"gamma"
        );
    }

    #[test]
    fn clone_tree_modifications_dont_propagate_to_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("file.txt"), b"original").unwrap();
        clone_tree_or_copy(&src, &dst).expect("clone");
        std::fs::write(dst.join("file.txt"), b"modified").unwrap();
        assert_eq!(std::fs::read(src.join("file.txt")).unwrap(), b"original");
    }

    #[test]
    #[cfg(unix)]
    fn clone_tree_preserves_directory_execute_bit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::create_dir_all(src.join("subdir")).unwrap();
        std::fs::write(src.join("subdir/inner.txt"), b"x").unwrap();
        clone_tree_or_copy(&src, &dst).expect("clone");
        // Cloned subdir must remain traversable (execute bit set on owner).
        let mode = std::fs::metadata(dst.join("subdir"))
            .unwrap()
            .permissions()
            .mode();
        assert!(
            mode & 0o100 != 0,
            "dst/subdir mode {mode:o} has no owner-execute bit"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clone_tree_preserves_symlinks_in_fallback() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("target.txt"), b"real").unwrap();
        symlink("target.txt", src.join("link.txt")).expect("create symlink");

        // Force the fallback path by using a destination on a different volume.
        // tempdir() is APFS on macOS, so we can't easily force non-APFS. Instead,
        // exercise the recursive helper directly which is always the fallback impl.
        let dst = dir.path().join("dst");
        super::copy_tree_recursive_impl(&src, &dst).expect("copy");

        let link_meta = std::fs::symlink_metadata(dst.join("link.txt")).unwrap();
        assert!(
            link_meta.file_type().is_symlink(),
            "link.txt was not preserved as a symlink"
        );
        assert_eq!(
            std::fs::read_link(dst.join("link.txt")).unwrap(),
            std::path::PathBuf::from("target.txt")
        );
    }

    #[test]
    fn clone_tree_replaces_existing_dst() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(src.join("new.txt"), b"new").unwrap();
        std::fs::write(dst.join("stale.txt"), b"stale").unwrap();
        clone_tree_or_copy(&src, &dst).expect("clone replaces");
        assert!(dst.join("new.txt").exists());
        assert!(!dst.join("stale.txt").exists());
    }
}
