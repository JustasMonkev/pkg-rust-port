use std::fs;
use std::path::{Path, PathBuf};

use crate::error::PkgError;
use crate::package::TargetBinaryProvider;
use crate::target::NodeTarget;

const PKG_FETCH_VERSION: &str = "3.5.2";
const SUPPORTED_NODE_VERSIONS: &[&str] = &[
    "8.17.0", "10.24.1", "12.22.11", "14.21.3", "16.19.1", "18.15.0", "19.8.1",
];

/// Kind of pkg-fetch cache artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryKind {
    /// Binary downloaded from the pkg-fetch GitHub release cache.
    Fetched,
    /// Binary built locally from patched Node.js source.
    Built,
}

impl BinaryKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Fetched => "fetched",
            Self::Built => "built",
        }
    }
}

/// Cache-backed provider for pkg-fetch target binaries.
///
/// # Example
///
/// ```
/// let cache = pkg_rust::PkgFetchCache::new(std::env::temp_dir());
/// let defaults = pkg_rust::TargetDefaults::host("node18");
/// let target = pkg_rust::parse_targets("linux-x64", &defaults)
///     .map_err(|error| pkg_rust::PkgError::Fetch(error.to_string()))?
///     .targets
///     .remove(0);
/// let path = cache.binary_path(&target, pkg_rust::BinaryKind::Fetched)?;
/// assert!(path.to_string_lossy().contains("fetched-v18.15.0-linux-x64"));
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PkgFetchCache {
    root: PathBuf,
}

impl PkgFetchCache {
    /// Create a cache provider rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Create a cache provider from `$PKG_CACHE_PATH` or `~/.pkg-cache`.
    ///
    /// # Example
    ///
    /// ```
    /// let cache = pkg_rust::PkgFetchCache::default_cache();
    /// assert!(cache.is_ok() || cache.is_err());
    /// ```
    pub fn default_cache() -> Result<Self, PkgError> {
        if let Some(path) = std::env::var_os("PKG_CACHE_PATH") {
            return Ok(Self::new(path));
        }
        let Some(home) = std::env::var_os("HOME") else {
            return Err(PkgError::Fetch(
                "HOME is not set and PKG_CACHE_PATH was not provided".to_owned(),
            ));
        };
        Ok(Self::new(PathBuf::from(home).join(".pkg-cache")))
    }

    /// Return the cache path for a target and cache artifact kind.
    pub fn binary_path(&self, target: &NodeTarget, kind: BinaryKind) -> Result<PathBuf, PkgError> {
        let node_version = satisfying_node_version(&target.node_range)?;
        Ok(self.cache_dir().join(format!(
            "{}-v{}-{}-{}",
            kind.prefix(),
            node_version,
            target.platform,
            target.arch
        )))
    }

    fn cache_dir(&self) -> PathBuf {
        self.root.join(tag_from_version(PKG_FETCH_VERSION))
    }
}

impl TargetBinaryProvider for PkgFetchCache {
    fn binary_for(&self, target: &NodeTarget) -> Result<Vec<u8>, PkgError> {
        for kind in [BinaryKind::Fetched, BinaryKind::Built] {
            let path = self.binary_path(target, kind)?;
            if path.is_file() {
                return read_binary(&path);
            }
        }

        // DECISION: this provider is cache-only for the first Rust slice; it
        // preserves pkg-fetch cache naming before adding network download and
        // expected-hash verification.
        Err(PkgError::Fetch(format!(
            "no cached binary for target {target}; expected {} or {}",
            self.binary_path(target, BinaryKind::Fetched)?.display(),
            self.binary_path(target, BinaryKind::Built)?.display()
        )))
    }
}

fn read_binary(path: &Path) -> Result<Vec<u8>, PkgError> {
    fs::read(path).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn tag_from_version(version: &str) -> String {
    let mut parts = version.split('.');
    let major = parts.next().filter(|part| !part.is_empty()).unwrap_or("0");
    let minor = parts.next().filter(|part| !part.is_empty()).unwrap_or("0");
    format!("v{major}.{minor}")
}

fn satisfying_node_version(node_range: &str) -> Result<&'static str, PkgError> {
    if node_range == "latest" {
        return SUPPORTED_NODE_VERSIONS
            .last()
            .copied()
            .ok_or_else(|| PkgError::Fetch("no supported Node versions configured".to_owned()));
    }

    let Some(major) = node_range.strip_prefix("node") else {
        return Err(PkgError::Fetch(format!(
            "node range must start with 'node': {node_range}"
        )));
    };
    SUPPORTED_NODE_VERSIONS
        .iter()
        .rev()
        .copied()
        .find(|version| version.split('.').next() == Some(major))
        .ok_or_else(|| {
            PkgError::Fetch(format!(
                "no available Node version satisfies '{node_range}'"
            ))
        })
}
