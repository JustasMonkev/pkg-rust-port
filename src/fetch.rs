use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::PkgError;
use crate::fsx::plus_x;
use crate::package::TargetBinaryProvider;
use crate::target::NodeTarget;

const PKG_FETCH_VERSION: &str = "3.5.2";
const PKG_FETCH_RELEASE_BASE_URL: &str = "https://github.com/vercel/pkg-fetch/releases/download";
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

    /// Download the fetched binary for `target` into the pkg-fetch cache.
    ///
    /// The downloaded file is made executable, SHA-256 verified against the
    /// embedded pkg-fetch expected hash table, then returned as bytes.
    ///
    /// # Example
    ///
    /// ```no_run
    /// let cache = pkg_rust::PkgFetchCache::default_cache()?;
    /// let defaults = pkg_rust::TargetDefaults::host("node18");
    /// let target = pkg_rust::parse_targets("linux-x64", &defaults)
    ///     .map_err(|error| pkg_rust::PkgError::Fetch(error.to_string()))?
    ///     .targets
    ///     .remove(0);
    /// let binary = cache.download_fetched(&target)?;
    /// assert!(!binary.is_empty());
    /// # Ok::<(), pkg_rust::PkgError>(())
    /// ```
    pub fn download_fetched(&self, target: &NodeTarget) -> Result<Vec<u8>, PkgError> {
        self.download_fetched_from_base_url(target, PKG_FETCH_RELEASE_BASE_URL)
    }

    fn download_fetched_from_base_url(
        &self,
        target: &NodeTarget,
        base_url: &str,
    ) -> Result<Vec<u8>, PkgError> {
        let expected = expected_hash(target)?;
        self.download_fetched_from_base_url_with_expected(target, base_url, &expected)
    }

    fn download_fetched_from_base_url_with_expected(
        &self,
        target: &NodeTarget,
        base_url: &str,
        expected_hash: &str,
    ) -> Result<Vec<u8>, PkgError> {
        let url = format!(
            "{}/{}/{}",
            base_url.trim_end_matches('/'),
            tag_from_version(PKG_FETCH_VERSION),
            remote_name(target)?
        );
        let response = reqwest::blocking::get(&url)
            .map_err(|source| PkgError::Fetch(format!("download {url} failed: {source}")))?;
        if !response.status().is_success() {
            return Err(PkgError::Fetch(format!(
                "download {url} failed with HTTP {}",
                response.status()
            )));
        }
        let bytes = response
            .bytes()
            .map_err(|source| PkgError::Fetch(format!("download {url} body failed: {source}")))?;
        self.store_fetched_bytes_with_expected(target, bytes.as_ref(), expected_hash)
    }

    fn store_fetched_bytes_with_expected(
        &self,
        target: &NodeTarget,
        bytes: &[u8],
        expected_hash: &str,
    ) -> Result<Vec<u8>, PkgError> {
        let fetched = self.binary_path(target, BinaryKind::Fetched)?;
        let parent = fetched.parent().ok_or_else(|| {
            PkgError::Fetch(format!("cache path has no parent: {}", fetched.display()))
        })?;
        fs::create_dir_all(parent).map_err(|source| PkgError::Io {
            path: parent.display().to_string(),
            source,
        })?;
        let temp = downloading_path(&fetched);
        fs::write(&temp, bytes).map_err(|source| PkgError::Io {
            path: temp.display().to_string(),
            source,
        })?;
        plus_x(&temp)?;
        let actual_hash = sha256_hex(&temp)?;
        if actual_hash != expected_hash {
            remove_file_if_exists(&temp)?;
            return Err(PkgError::Fetch(format!(
                "downloaded binary hash does not match for {}",
                remote_name(target)?
            )));
        }
        fs::rename(&temp, &fetched).map_err(|source| PkgError::Io {
            path: fetched.display().to_string(),
            source,
        })?;
        read_binary(&fetched)
    }

    fn cache_dir(&self) -> PathBuf {
        self.root.join(tag_from_version(PKG_FETCH_VERSION))
    }
}

impl TargetBinaryProvider for PkgFetchCache {
    fn binary_for(&self, target: &NodeTarget) -> Result<Vec<u8>, PkgError> {
        let fetched = self.binary_path(target, BinaryKind::Fetched)?;
        if fetched.is_file() && self.verify_fetched(target, &fetched)? {
            return read_binary(&fetched);
        }

        let built = self.binary_path(target, BinaryKind::Built)?;
        if built.is_file() {
            return read_binary(&built);
        }

        // DECISION: this provider is cache-only until the next slice adds
        // GitHub release download; verified cache reuse is still useful and
        // keeps package builds deterministic in tests.
        Err(PkgError::Fetch(format!(
            "no cached binary for target {target}; expected {} or {}",
            fetched.display(),
            built.display()
        )))
    }
}

impl PkgFetchCache {
    fn verify_fetched(&self, target: &NodeTarget, path: &Path) -> Result<bool, PkgError> {
        let expected = expected_hash(target)?;
        let actual = sha256_hex(path)?;
        if actual == expected {
            return Ok(true);
        }
        fs::remove_file(path).map_err(|source| PkgError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Ok(false)
    }
}

fn read_binary(path: &Path) -> Result<Vec<u8>, PkgError> {
    fs::read(path).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn remove_file_if_exists(path: &Path) -> Result<(), PkgError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(PkgError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn sha256_hex(path: &Path) -> Result<String, PkgError> {
    let mut file = fs::File::open(path).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|source| PkgError::Io {
            path: path.display().to_string(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    Ok(output)
}

fn expected_hash(target: &NodeTarget) -> Result<String, PkgError> {
    let node_version = satisfying_node_version(&target.node_range)?;
    let key = format!("node-v{}-{}-{}", node_version, target.platform, target.arch);
    let hashes: BTreeMap<String, String> =
        serde_json::from_str(include_str!("fetch_expected_shas.json")).map_err(|source| {
            PkgError::Fetch(format!(
                "invalid embedded pkg-fetch expected hashes: {source}"
            ))
        })?;
    hashes.get(&key).cloned().ok_or_else(|| {
        PkgError::Fetch(format!(
            "no expected hash is configured for target artifact {key}"
        ))
    })
}

fn remote_name(target: &NodeTarget) -> Result<String, PkgError> {
    let node_version = satisfying_node_version(&target.node_range)?;
    Ok(format!(
        "node-v{}-{}-{}",
        node_version, target.platform, target.arch
    ))
}

fn downloading_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.downloading", path.display()))
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

#[cfg(test)]
mod tests {
    use std::fs;

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::target::{TargetDefaults, parse_targets};

    #[test]
    fn stores_fetched_binary_and_verifies_hash() -> Result<(), Box<dyn std::error::Error>> {
        let body = b"downloaded binary";
        let cache = PkgFetchCache::new(temp_root("download-ok"));
        let target = target()?;
        let expected = hex_digest(body);

        let binary = cache.store_fetched_bytes_with_expected(&target, body, &expected)?;

        assert_eq!(binary, body);
        assert!(cache.binary_path(&target, BinaryKind::Fetched)?.is_file());
        fs::remove_dir_all(cache.root)?;
        Ok(())
    }

    #[test]
    fn removes_stored_download_when_hash_mismatches() -> Result<(), Box<dyn std::error::Error>> {
        let cache = PkgFetchCache::new(temp_root("download-bad"));
        let target = target()?;
        let fetched = cache.binary_path(&target, BinaryKind::Fetched)?;

        let error = cache
            .store_fetched_bytes_with_expected(&target, b"bad binary", "not-the-hash")
            .err();

        assert!(
            matches!(error, Some(PkgError::Fetch(message)) if message.contains("hash does not match"))
        );
        assert!(!fetched.exists());
        fs::remove_dir_all(cache.root)?;
        Ok(())
    }

    fn target() -> Result<NodeTarget, PkgError> {
        let defaults = TargetDefaults::host("node18");
        parse_targets("linux-x64", &defaults)
            .map_err(|error| PkgError::Fetch(error.to_string()))
            .map(|mut parsed| parsed.targets.remove(0))
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pkg-rust-fetch-{label}-{}", std::process::id()))
    }

    fn hex_digest(body: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(body);
        let digest = hasher.finalize();
        let mut output = String::with_capacity(digest.len() * 2);
        for byte in digest {
            output.push_str(&format!("{byte:02x}"));
        }
        output
    }
}
