use std::fs;
use std::path::Path;

use crate::error::PkgError;

/// Add executable bits for owner, group, and others.
///
/// This mirrors the JavaScript `plusx` helper, which ORs `0o111` into the
/// existing mode instead of replacing other permission bits.
///
/// # Example
///
/// ```
/// let path = std::env::temp_dir().join(format!("pkg-rust-plus-x-{}", std::process::id()));
/// std::fs::write(&path, b"demo").map_err(|source| pkg_rust::PkgError::Io {
///     path: path.display().to_string(),
///     source,
/// })?;
/// pkg_rust::plus_x(&path)?;
/// let _ = std::fs::remove_file(path);
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
#[cfg(unix)]
pub fn plus_x(path: impl AsRef<Path>) -> Result<(), PkgError> {
    use std::os::unix::fs::PermissionsExt;

    let path = path.as_ref();
    let metadata = fs::metadata(path).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let mode = metadata.permissions().mode();
    let new_mode = mode | 0o111;
    if mode == new_mode {
        return Ok(());
    }

    let mut permissions = metadata.permissions();
    permissions.set_mode(new_mode);
    fs::set_permissions(path, permissions).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })
}

/// Add executable bits for owner, group, and others.
///
/// Non-Unix platforms do not expose POSIX executable bits through `std::fs`, so
/// this is a no-op. The JS CLI only calls `plusx` for non-Windows targets.
///
/// # Example
///
/// ```
/// let path = std::env::temp_dir().join(format!("pkg-rust-plus-x-{}", std::process::id()));
/// std::fs::write(&path, b"demo").map_err(|source| pkg_rust::PkgError::Io {
///     path: path.display().to_string(),
///     source,
/// })?;
/// pkg_rust::plus_x(&path)?;
/// let _ = std::fs::remove_file(path);
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
#[cfg(not(unix))]
pub fn plus_x(path: impl AsRef<Path>) -> Result<(), PkgError> {
    let path = path.as_ref();
    fs::metadata(path).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}
