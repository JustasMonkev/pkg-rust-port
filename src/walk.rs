use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::{AliasKind, StoreKind};
use crate::config::{PackageJson, PackageJsonError};
use crate::detect::{DetectionKind, detect};
use crate::dictionary::{active_dependencies, apply_dictionary_entry, lookup_dictionary};
use crate::error::PkgError;
use crate::resolve::{ResolveOptions, resolve_module};

/// Package metadata carried while the dependency walker expands files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Marker {
    package: PackageJson,
    package_path: Option<PathBuf>,
    activated: bool,
}

impl Marker {
    /// Create a marker from already parsed package metadata.
    ///
    /// # Example
    ///
    /// ```
    /// let package = pkg_rust::PackageJson::parse(r#"{"name":"demo"}"#)?;
    /// let marker = pkg_rust::Marker::new(package);
    /// assert_eq!(marker.package().name.as_deref(), Some("demo"));
    /// # Ok::<(), pkg_rust::PackageJsonError>(())
    /// ```
    #[must_use]
    pub fn new(package: PackageJson) -> Self {
        Self {
            package,
            package_path: None,
            activated: false,
        }
    }

    /// Create a marker from metadata and its source `package.json` path.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::path::PathBuf;
    /// let package = pkg_rust::PackageJson::parse(r#"{"name":"demo"}"#)?;
    /// let marker = pkg_rust::Marker::with_package_path(package, "package.json");
    /// assert_eq!(marker.package_path(), Some(PathBuf::from("package.json").as_path()));
    /// # Ok::<(), pkg_rust::PackageJsonError>(())
    /// ```
    #[must_use]
    pub fn with_package_path(package: PackageJson, package_path: impl Into<PathBuf>) -> Self {
        Self {
            package,
            package_path: Some(package_path.into()),
            activated: false,
        }
    }

    /// Read and parse a package file into a marker.
    ///
    /// # Example
    ///
    /// ```
    /// let marker = pkg_rust::Marker::from_package_path("../test/test-46-input-package-json/package.json")?;
    /// assert!(marker.package().name.is_some());
    /// # Ok::<(), pkg_rust::PkgError>(())
    /// ```
    pub fn from_package_path(path: impl AsRef<Path>) -> Result<Self, PkgError> {
        let path = path.as_ref();
        let body = read_to_string(path)?;
        let package = PackageJson::parse(&body).map_err(package_error)?;
        Ok(Self::with_package_path(package, path.to_path_buf()))
    }

    /// Package metadata after dictionary activation.
    #[must_use]
    pub fn package(&self) -> &PackageJson {
        &self.package
    }

    /// Source `package.json` path when known.
    #[must_use]
    pub fn package_path(&self) -> Option<&Path> {
        self.package_path.as_deref()
    }
}

/// Options controlling dependency walking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WalkerParams {
    /// Optional root that bounds directory-link expansion.
    pub root: Option<PathBuf>,
}

impl WalkerParams {
    /// Create default walker parameters.
    ///
    /// # Example
    ///
    /// ```
    /// let params = pkg_rust::WalkerParams::new();
    /// assert!(params.root.is_none());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bound directory-link expansion to a root path.
    ///
    /// # Example
    ///
    /// ```
    /// let params = pkg_rust::WalkerParams::new().with_root("/project");
    /// assert!(params.root.is_some());
    /// ```
    #[must_use]
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = Some(root.into());
        self
    }
}

/// Filesystem metadata captured for a record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileStat {
    /// Whether the path is a regular file.
    pub is_file: bool,
    /// Whether the path is a directory.
    pub is_directory: bool,
    /// File size in bytes.
    pub size: u64,
    /// Unix mode when available; zero on platforms without Unix metadata.
    pub mode: u32,
}

/// One file record collected by the walker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileRecord {
    /// Host path for this record.
    pub file: PathBuf,
    /// Whether the blob store has processed this file.
    pub blob: bool,
    /// Whether the content store has processed this file.
    pub content: bool,
    /// Whether directory links have been read.
    pub links: bool,
    /// Whether file metadata has been read.
    pub stat: bool,
    /// Raw content bytes when the content store read the file.
    pub body: Option<Vec<u8>>,
    /// Sorted directory child names when the links store read a directory.
    pub children: Vec<String>,
    /// Filesystem metadata when available.
    pub metadata: Option<FileStat>,
}

impl FileRecord {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            blob: false,
            content: false,
            links: false,
            stat: false,
            body: None,
            children: Vec::new(),
            metadata: None,
        }
    }

    /// Return whether the record has processed a specific store.
    ///
    /// # Example
    ///
    /// ```
    /// let package = pkg_rust::PackageJson::parse("{}")
    ///     .map_err(|error| pkg_rust::PkgError::Resolve(error.to_string()))?;
    /// let marker = pkg_rust::Marker::new(package);
    /// let output = pkg_rust::walk(marker, "../test/test-50-require-resolve/test-z-require-code-1.js", None, pkg_rust::WalkerParams::new())?;
    /// let record = output.record("../test/test-50-require-resolve/test-z-require-code-1.js");
    /// assert!(record.is_some_and(|record| record.has_store(pkg_rust::StoreKind::Blob)));
    /// # Ok::<(), pkg_rust::PkgError>(())
    /// ```
    #[must_use]
    pub fn has_store(&self, store: StoreKind) -> bool {
        match store {
            StoreKind::Blob => self.blob,
            StoreKind::Content => self.content,
            StoreKind::Links => self.links,
            StoreKind::Stat => self.stat,
        }
    }

    fn set_store(&mut self, store: StoreKind) {
        match store {
            StoreKind::Blob => self.blob = true,
            StoreKind::Content => self.content = true,
            StoreKind::Links => self.links = true,
            StoreKind::Stat => self.stat = true,
        }
    }
}

/// One processed FIFO task, useful for parity tests that assert deterministic order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalkTaskRecord {
    /// File processed by the task.
    pub file: PathBuf,
    /// Store processed by the task.
    pub store: StoreKind,
}

/// Records returned after walking an entrypoint.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WalkOutput {
    /// Records keyed by path.
    pub records: BTreeMap<PathBuf, FileRecord>,
    /// FIFO task processing log.
    pub task_log: Vec<WalkTaskRecord>,
}

impl WalkOutput {
    /// Return the record for a path, accepting canonical or relative input.
    ///
    /// # Example
    ///
    /// ```
    /// let package = pkg_rust::PackageJson::parse("{}")
    ///     .map_err(|error| pkg_rust::PkgError::Resolve(error.to_string()))?;
    /// let marker = pkg_rust::Marker::new(package);
    /// let output = pkg_rust::walk(marker, "../test/test-50-require-resolve/test-z-require-code-1.js", None, pkg_rust::WalkerParams::new())?;
    /// assert!(output.record("../test/test-50-require-resolve/test-z-require-code-1.js").is_some());
    /// # Ok::<(), pkg_rust::PkgError>(())
    /// ```
    #[must_use]
    pub fn record(&self, path: impl AsRef<Path>) -> Option<&FileRecord> {
        let path = path.as_ref();
        self.records
            .get(path)
            .or_else(|| canonicalize(path).and_then(|path| self.records.get(&path)))
    }

    /// Return whether a path has processed a store.
    ///
    /// # Example
    ///
    /// ```
    /// let package = pkg_rust::PackageJson::parse("{}")
    ///     .map_err(|error| pkg_rust::PkgError::Resolve(error.to_string()))?;
    /// let marker = pkg_rust::Marker::new(package);
    /// let output = pkg_rust::walk(marker, "../test/test-50-require-resolve/test-z-require-code-1.js", None, pkg_rust::WalkerParams::new())?;
    /// assert!(output.contains_store("../test/test-50-require-resolve/test-z-require-code-1.js", pkg_rust::StoreKind::Blob));
    /// # Ok::<(), pkg_rust::PkgError>(())
    /// ```
    #[must_use]
    pub fn contains_store(&self, path: impl AsRef<Path>, store: StoreKind) -> bool {
        self.record(path)
            .is_some_and(|record| record.has_store(store))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Task {
    file: PathBuf,
    store: StoreKind,
    marker: Marker,
}

struct WalkerState {
    output: WalkOutput,
    tasks: VecDeque<Task>,
    root: PathBuf,
}

impl WalkerState {
    fn new(root: PathBuf) -> Self {
        Self {
            output: WalkOutput::default(),
            tasks: VecDeque::new(),
            root,
        }
    }

    fn walk(mut self) -> Result<WalkOutput, PkgError> {
        while let Some(task) = self.tasks.pop_front() {
            if self
                .output
                .records
                .get(&task.file)
                .is_some_and(|record| record.has_store(task.store))
            {
                continue;
            }

            self.output.task_log.push(WalkTaskRecord {
                file: task.file.clone(),
                store: task.store,
            });
            self.step(task)?;
        }
        Ok(self.output)
    }

    fn step(&mut self, mut task: Task) -> Result<(), PkgError> {
        self.ensure_record(task.file.clone());
        self.append(task.file.clone(), StoreKind::Stat, task.marker.clone());

        if !task.marker.activated {
            self.activate_marker(&mut task.marker)?;
        }

        match task.store {
            StoreKind::Blob => self.step_blob(&task.file, &task.marker)?,
            StoreKind::Content => self.step_content(&task.file)?,
            StoreKind::Links => self.step_links(&task.file, &task.marker)?,
            StoreKind::Stat => self.step_stat(&task.file, &task.marker)?,
        }

        self.record_mut(&task.file).set_store(task.store);
        Ok(())
    }

    fn activate_marker(&mut self, marker: &mut Marker) -> Result<(), PkgError> {
        if let Some(name) = marker.package.name.as_deref()
            && let Some(entry) = lookup_dictionary(name)
        {
            apply_dictionary_entry(&mut marker.package, &entry);
        }

        let dependencies = active_dependencies(&marker.package);
        marker.activated = true;

        let Some(base_dir) = marker.package_path.as_deref().and_then(Path::parent) else {
            return Ok(());
        };

        for dependency in dependencies {
            self.append_resolvable(base_dir, &dependency, marker.clone(), false)?;
            let package_json = format!("{dependency}/package.json");
            self.append_resolvable(base_dir, &package_json, marker.clone(), false)?;
        }
        Ok(())
    }

    fn step_blob(&mut self, file: &Path, marker: &Marker) -> Result<(), PkgError> {
        if should_retag_blob_as_content(file) {
            self.append(file.to_path_buf(), StoreKind::Content, marker.clone());
            return Ok(());
        }

        if !is_javascript_file(file) {
            return Ok(());
        }

        let body = read_to_string(file)?;
        let body = strip_bom_and_shebang(&body);
        for detected in detect(&body)? {
            let DetectionKind::Successful(derivative) = detected.kind else {
                continue;
            };
            if derivative.must_exclude {
                continue;
            }

            match derivative.alias_kind {
                AliasKind::Relative => {
                    let Some(parent) = file.parent() else {
                        continue;
                    };
                    let target = canonicalize_or_join(parent, &derivative.alias);
                    if target.is_file() {
                        self.append(target, StoreKind::Content, marker.clone());
                    }
                }
                AliasKind::Resolvable => {
                    if is_node_builtin(&derivative.alias) {
                        continue;
                    }
                    let Some(parent) = file.parent() else {
                        continue;
                    };
                    self.append_resolvable(
                        parent,
                        &derivative.alias,
                        marker.clone(),
                        derivative.may_exclude || detected.trying,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn step_content(&mut self, file: &Path) -> Result<(), PkgError> {
        let body = fs::read(file).map_err(|source| io_error(file, source))?;
        self.record_mut(file).body = Some(body);
        Ok(())
    }

    fn step_links(&mut self, directory: &Path, marker: &Marker) -> Result<(), PkgError> {
        if !directory.is_dir() {
            return Ok(());
        }

        let mut children = Vec::new();
        for entry in fs::read_dir(directory).map_err(|source| io_error(directory, source))? {
            let entry = entry.map_err(|source| io_error(directory, source))?;
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                children.push(name.to_owned());
            }
            if inside_root(&self.root, &path) {
                self.append(path, StoreKind::Stat, marker.clone());
            }
        }
        children.sort();
        self.record_mut(directory).children = children;
        Ok(())
    }

    fn step_stat(&mut self, file: &Path, marker: &Marker) -> Result<(), PkgError> {
        if let Ok(metadata) = fs::symlink_metadata(file) {
            self.record_mut(file).metadata = Some(FileStat {
                is_file: metadata.is_file(),
                is_directory: metadata.is_dir(),
                size: metadata.len(),
                mode: file_mode(&metadata),
            });

            if metadata.is_dir() {
                self.append(file.to_path_buf(), StoreKind::Links, marker.clone());
            } else if let Some(parent) = file.parent()
                && inside_root(&self.root, parent)
            {
                // DECISION: JS walks parent directory links all the way to the host root.
                // The Rust port bounds this to the package root for deterministic,
                // machine-independent records while preserving in-project directory links.
                self.append(parent.to_path_buf(), StoreKind::Links, marker.clone());
            }
        }
        Ok(())
    }

    fn append_resolvable(
        &mut self,
        basedir: &Path,
        alias: &str,
        marker: Marker,
        optional: bool,
    ) -> Result<(), PkgError> {
        let options = ResolveOptions::new(basedir);
        match resolve_module(alias, &options) {
            Ok(file) => {
                self.append(file, StoreKind::Blob, marker);
                Ok(())
            }
            Err(error) if optional => {
                tracing::debug!(alias, error = %error, "skipping optional dependency");
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn append(&mut self, file: PathBuf, store: StoreKind, marker: Marker) {
        let normalized = canonicalize(&file).unwrap_or(file);
        self.ensure_record(normalized.clone());
        self.tasks.push_back(Task {
            file: normalized,
            store,
            marker,
        });
    }

    fn ensure_record(&mut self, file: PathBuf) {
        self.output
            .records
            .entry(file.clone())
            .or_insert_with(|| FileRecord::new(file));
    }

    fn record_mut(&mut self, file: &Path) -> &mut FileRecord {
        self.output
            .records
            .entry(file.to_path_buf())
            .or_insert_with(|| FileRecord::new(file.to_path_buf()))
    }
}

/// Walk an entrypoint and collect virtual filesystem records.
///
/// The queue is FIFO to match the JavaScript walker's deterministic output
/// ordering.
///
/// # Example
///
/// ```
/// let package = pkg_rust::PackageJson::parse("{}")
///     .map_err(|error| pkg_rust::PkgError::Resolve(error.to_string()))?;
/// let marker = pkg_rust::Marker::new(package);
/// let output = pkg_rust::walk(marker, "../test/test-50-require-resolve/test-x-index.js", None, pkg_rust::WalkerParams::new())?;
/// assert!(output.contains_store("../test/test-50-require-resolve/test-x-index.js", pkg_rust::StoreKind::Blob));
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn walk(
    marker: Marker,
    entrypoint: impl AsRef<Path>,
    addition: Option<PathBuf>,
    params: WalkerParams,
) -> Result<WalkOutput, PkgError> {
    let entrypoint = canonicalize_or_self(entrypoint.as_ref());
    let root = params
        .root
        .as_deref()
        .map(canonicalize_or_self)
        .or_else(|| entrypoint.parent().map(canonicalize_or_self))
        .unwrap_or_else(|| PathBuf::from("."));

    let mut state = WalkerState::new(root);
    state.append(entrypoint, StoreKind::Blob, marker.clone());
    if let Some(addition) = addition {
        state.append(canonicalize_or_self(&addition), StoreKind::Content, marker);
    }
    state.walk()
}

fn should_retag_blob_as_content(path: &Path) -> bool {
    !is_javascript_file(path)
        && path.extension().and_then(|extension| extension.to_str()) != Some("node")
}

fn is_javascript_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("js" | "mjs" | "cjs")
    )
}

fn strip_bom_and_shebang(source: &str) -> String {
    let without_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
    if !without_bom.starts_with("#!") {
        return without_bom.to_owned();
    }

    match without_bom.find('\n') {
        Some(index) => without_bom
            .get(index + 1..)
            .map(ToOwned::to_owned)
            .unwrap_or_default(),
        None => String::new(),
    }
}

fn canonicalize(path: &Path) -> Option<PathBuf> {
    path.canonicalize().ok()
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    canonicalize(path).unwrap_or_else(|| path.to_path_buf())
}

fn canonicalize_or_join(parent: &Path, alias: &str) -> PathBuf {
    canonicalize(&parent.join(alias)).unwrap_or_else(|| parent.join(alias))
}

fn read_to_string(path: &Path) -> Result<String, PkgError> {
    fs::read_to_string(path).map_err(|source| io_error(path, source))
}

fn io_error(path: &Path, source: std::io::Error) -> PkgError {
    PkgError::Io {
        path: path.display().to_string(),
        source,
    }
}

fn package_error(error: PackageJsonError) -> PkgError {
    PkgError::Resolve(format!("package marker parse failed: {error}"))
}

fn inside_root(root: &Path, path: &Path) -> bool {
    let path = canonicalize(path).unwrap_or_else(|| path.to_path_buf());
    path == root || path.starts_with(root)
}

fn is_node_builtin(alias: &str) -> bool {
    let alias = alias.strip_prefix("node:").unwrap_or(alias);
    let root = alias.split('/').next().unwrap_or(alias);
    matches!(
        root,
        "assert"
            | "async_hooks"
            | "buffer"
            | "child_process"
            | "cluster"
            | "console"
            | "constants"
            | "crypto"
            | "dgram"
            | "dns"
            | "domain"
            | "events"
            | "fs"
            | "http"
            | "http2"
            | "https"
            | "module"
            | "net"
            | "os"
            | "path"
            | "perf_hooks"
            | "process"
            | "punycode"
            | "querystring"
            | "readline"
            | "repl"
            | "stream"
            | "string_decoder"
            | "timers"
            | "tls"
            | "tty"
            | "url"
            | "util"
            | "v8"
            | "vm"
            | "worker_threads"
            | "zlib"
    )
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;

    metadata.mode()
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> u32 {
    0
}
