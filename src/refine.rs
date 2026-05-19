use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::common::{PathStyle, retrieve_denominator, substitute_denominator};
use crate::walk::{FileRecord, WalkOutput};

/// Symbolic link map keyed by link path and valued by real path.
pub type SymlinkMap = BTreeMap<PathBuf, PathBuf>;

/// Records after host path prefixes have been replaced with snapshot-relative paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefinedOutput {
    /// Refined file records keyed by snapshot-relative path text.
    pub records: BTreeMap<String, FileRecord>,
    /// Refined entrypoint path.
    pub entrypoint: String,
    /// Refined symlink map.
    pub symlinks: BTreeMap<String, String>,
}

/// Refine walker records into the path shape consumed by packing.
///
/// # Example
///
/// ```
/// let package = pkg_rust::PackageJson::parse("{}")
///     .map_err(|error| pkg_rust::PkgError::Resolve(error.to_string()))?;
/// let marker = pkg_rust::Marker::new(package);
/// let entrypoint = "../test/test-50-require-resolve/test-z-require-code-1.js";
/// let output = pkg_rust::walk(marker, entrypoint, None, pkg_rust::WalkerParams::new())?;
/// let refined = pkg_rust::refine(output, entrypoint, pkg_rust::SymlinkMap::new(), pkg_rust::PathStyle::Posix);
/// assert_eq!(refined.entrypoint, "/test-z-require-code-1.js");
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
#[must_use]
pub fn refine(
    output: WalkOutput,
    entrypoint: impl AsRef<Path>,
    symlinks: SymlinkMap,
    style: PathStyle,
) -> RefinedOutput {
    let mut records = output.records;
    purge_top_directories(&mut records);
    let entrypoint = canonicalize_or_self(entrypoint.as_ref());

    let files: Vec<String> = records.keys().map(|path| path_to_string(path)).collect();
    let file_refs: Vec<&str> = files.iter().map(String::as_str).collect();
    let denominator = retrieve_denominator(&file_refs, style);

    let records = records
        .into_iter()
        .map(|(file, record)| {
            (
                make_snap(&path_to_string(&file), denominator, style),
                record,
            )
        })
        .collect();
    let symlinks = symlinks
        .into_iter()
        .map(|(link, real)| {
            let link = canonicalize_link_path(&link);
            let real = canonicalize_or_self(&real);
            (
                make_snap(&path_to_string(&link), denominator, style),
                make_snap(&path_to_string(&real), denominator, style),
            )
        })
        .collect();

    RefinedOutput {
        records,
        entrypoint: make_snap(&path_to_string(&entrypoint), denominator, style),
        symlinks,
    }
}

/// Refine walker records using the symlink map collected by the walker.
///
/// # Example
///
/// ```
/// let package = pkg_rust::PackageJson::parse("{}")
///     .map_err(|error| pkg_rust::PkgError::Resolve(error.to_string()))?;
/// let marker = pkg_rust::Marker::new(package);
/// let entrypoint = "../test/test-50-require-resolve/test-z-require-code-1.js";
/// let output = pkg_rust::walk(marker, entrypoint, None, pkg_rust::WalkerParams::new())?;
/// let refined = pkg_rust::refine_walked(output, entrypoint, pkg_rust::PathStyle::Posix);
/// assert_eq!(refined.entrypoint, "/test-z-require-code-1.js");
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
#[must_use]
pub fn refine_walked(
    output: WalkOutput,
    entrypoint: impl AsRef<Path>,
    style: PathStyle,
) -> RefinedOutput {
    let symlinks = output.symlinks.clone();
    refine(output, entrypoint, symlinks, style)
}

fn purge_top_directories(records: &mut BTreeMap<PathBuf, FileRecord>) {
    loop {
        let Some(file) = records
            .iter()
            .find_map(|(file, record)| should_purge(file, record, records).then(|| file.clone()))
        else {
            break;
        };
        records.remove(&file);
    }
}

fn should_purge(file: &Path, record: &FileRecord, records: &BTreeMap<PathBuf, FileRecord>) -> bool {
    if !record.links || record.children.len() != 1 || has_parent(file, records) {
        return false;
    }

    let file2 = file.join(&record.children[0]);
    let Some(record2) = records.get(&file2) else {
        return false;
    };
    if !record2.links || record2.children.len() != 1 {
        return false;
    }

    let file3 = file2.join(&record2.children[0]);
    records.get(&file3).is_some_and(|record3| record3.links)
}

fn has_parent(file: &Path, records: &BTreeMap<PathBuf, FileRecord>) -> bool {
    let Some(parent) = file.parent() else {
        return false;
    };
    parent != file && records.contains_key(parent)
}

fn make_snap(file: &str, denominator: usize, style: PathStyle) -> String {
    let mut snap = substitute_denominator(file, denominator, style);
    match style {
        PathStyle::Posix => {
            if snap.is_empty() {
                snap = "/".to_owned();
            }
        }
        PathStyle::Windows => {
            if snap.get(1..) == Some(":") {
                snap.push('\\');
            }
        }
    }
    snap
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_error| path.to_path_buf())
}

fn canonicalize_link_path(path: &Path) -> PathBuf {
    let Some(parent) = path.parent() else {
        return path.to_path_buf();
    };
    let Some(name) = path.file_name() else {
        return path.to_path_buf();
    };

    parent
        .canonicalize()
        .map(|parent| parent.join(name))
        .unwrap_or_else(|_error| path.to_path_buf())
}
