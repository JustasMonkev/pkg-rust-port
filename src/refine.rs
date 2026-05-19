use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::{PathStyle, retrieve_denominator, substitute_denominator};
use crate::walk::{FileRecord, FileStat, WalkOutput};

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
    let WalkOutput {
        mut records,
        warnings: _,
        ..
    } = output;
    purge_top_directories(&mut records);
    let entrypoint = canonicalize_or_self(entrypoint.as_ref());

    let files: Vec<String> = records.keys().map(|path| path_to_string(path)).collect();
    let file_refs: Vec<&str> = files.iter().map(String::as_str).collect();
    let denominator = retrieve_denominator(&file_refs, style);

    let mut records = records
        .into_iter()
        .map(|(file, record)| {
            (
                make_snap(&path_to_string(&file), denominator, style),
                record,
            )
        })
        .collect();
    ensure_parent_directory_records(&mut records, style);
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

fn ensure_snapshot_base_records(
    records: &mut BTreeMap<String, FileRecord>,
    entrypoint: &Path,
    snapshot_base: &Path,
    style: PathStyle,
) {
    let root = match style {
        PathStyle::Posix => "/",
        PathStyle::Windows => {
            // The current package-directory runtime parity smoke is POSIX-only.
            // Windows keeps the natural refined records until that fixture is
            // ported with platform-specific expectations.
            return;
        }
    };

    let Ok(relative) = entrypoint.strip_prefix(snapshot_base) else {
        return;
    };
    let components: Vec<String> = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str().map(ToOwned::to_owned))
        .collect();

    if components.len() < 2 {
        return;
    }

    let mut host_dir = snapshot_base.to_path_buf();
    let mut snapshot_dir = root.to_owned();
    for child in components.iter().take(components.len() - 1) {
        ensure_directory_record(records, &snapshot_dir, host_dir.clone(), child.clone());
        host_dir.push(child);
        if snapshot_dir == root {
            snapshot_dir.push_str(child);
        } else {
            snapshot_dir.push('/');
            snapshot_dir.push_str(child);
        }
    }
}

fn ensure_directory_record(
    records: &mut BTreeMap<String, FileRecord>,
    key: &str,
    file: PathBuf,
    child: String,
) {
    if let Some(record) = records.get_mut(key) {
        if !record.children.iter().any(|existing| existing == &child) {
            record.children.push(child);
            record.children.sort();
        }
        record.links = true;
        record.stat = true;
        return;
    }

    records.insert(key.to_owned(), synthetic_directory_record(file, child));
}

fn ensure_parent_directory_records(records: &mut BTreeMap<String, FileRecord>, style: PathStyle) {
    let PathStyle::Posix = style else {
        // The current runtime fixtures that require synthetic parent chains are
        // POSIX-only. Keep Windows unchanged until platform-specific parity
        // expectations are ported.
        return;
    };

    let keys = records.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        if key == "/" {
            continue;
        }
        let parts = key
            .trim_start_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }

        let mut parent = "/".to_owned();
        for child in parts.iter().take(parts.len() - 1) {
            let child = (*child).to_owned();
            ensure_directory_record(records, &parent, PathBuf::from(&parent), child.clone());
            if parent == "/" {
                parent.push_str(&child);
            } else {
                parent.push('/');
                parent.push_str(&child);
            }
        }
    }
}

fn synthetic_directory_record(file: PathBuf, child: String) -> FileRecord {
    FileRecord {
        file: file.clone(),
        blob: false,
        content: false,
        links: true,
        stat: true,
        body: None,
        children: vec![child],
        metadata: Some(directory_stat(&file)),
    }
}

fn directory_stat(file: &Path) -> FileStat {
    match fs::metadata(file) {
        Ok(metadata) => FileStat {
            is_file: false,
            is_directory: true,
            is_socket: false,
            is_symbolic_link: false,
            size: metadata.len(),
            mode: file_mode(&metadata),
        },
        Err(_error) => FileStat {
            is_file: false,
            is_directory: true,
            is_socket: false,
            is_symbolic_link: false,
            size: 0,
            mode: default_directory_mode(),
        },
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

pub(crate) fn refine_walked_with_snapshot_base(
    output: WalkOutput,
    entrypoint: impl AsRef<Path>,
    snapshot_base: impl AsRef<Path>,
    style: PathStyle,
) -> RefinedOutput {
    let symlinks = output.symlinks.clone();
    refine_with_snapshot_base(output, entrypoint, symlinks, snapshot_base, style)
}

fn refine_with_snapshot_base(
    output: WalkOutput,
    entrypoint: impl AsRef<Path>,
    symlinks: SymlinkMap,
    snapshot_base: impl AsRef<Path>,
    style: PathStyle,
) -> RefinedOutput {
    let WalkOutput {
        mut records,
        warnings,
        ..
    } = output;
    purge_top_directories(&mut records);
    let entrypoint = canonicalize_or_self(entrypoint.as_ref());
    let snapshot_base = canonicalize_or_self(snapshot_base.as_ref());
    if records
        .keys()
        .any(|file| !inside_root(&snapshot_base, file))
    {
        // DECISION: A forced snapshot base is only valid while every walked
        // record stays under it. Some file-entry fixtures resolve dependencies
        // through sibling node_modules directories, so fall back to the JS-style
        // common denominator instead of slicing those paths into invalid keys.
        return refine(
            WalkOutput {
                records,
                symlinks: BTreeMap::new(),
                task_log: Vec::new(),
                warnings,
            },
            entrypoint,
            symlinks,
            style,
        );
    }
    let denominator = path_to_string(&snapshot_base).len();

    let mut records: BTreeMap<String, FileRecord> = records
        .into_iter()
        .map(|(file, record)| {
            (
                make_snap(&path_to_string(&file), denominator, style),
                record,
            )
        })
        .collect();
    ensure_snapshot_base_records(&mut records, &entrypoint, &snapshot_base, style);
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

fn inside_root(root: &Path, path: &Path) -> bool {
    let path = canonicalize_or_self(path);
    path == root || path.starts_with(root)
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

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;

    metadata.mode()
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> u32 {
    default_directory_mode()
}

#[cfg(unix)]
fn default_directory_mode() -> u32 {
    0o40755
}

#[cfg(not(unix))]
fn default_directory_mode() -> u32 {
    0
}
