//! Node Single Executable Application (SEA) pipeline.
//!
//! Ports yao-pkg/pkg 6.19.0 `lib/sea.ts`. Unlike the classic pkg flow (which
//! appends a virtual-filesystem payload to a pkg-fetch base binary), SEA:
//!
//!   1. downloads an official Node binary from `nodejs.org/dist` (checksum
//!      verified, extracted and cached under `~/.pkg-cache/sea`),
//!   2. generates a SEA preparation blob by shelling out to a host-compatible
//!      Node with `--experimental-sea-config`,
//!   3. bakes the blob into a copy of the downloaded Node binary by natively
//!      injecting the `NODE_SEA_BLOB` resource and flipping the SEA fuse (see
//!      [`crate::sea_inject`]),
//!   4. ad-hoc signs the result on macOS.
//!
//! This slice implements **simple SEA mode** (`pkg --sea entry.js` with no
//! `package.json`), where the entry file is the SEA `main`. Enhanced SEA mode
//! (walker + per-file archive + VFS bootstrap, used when a `package.json`/config
//! is present) is the next increment and fails closed with a precise error.
//!
//! Native blob injection is implemented and verified end to end for Linux ELF
//! targets; macOS/Windows injection fails closed in [`crate::sea_inject`].

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::cli::PackagePlan;
use crate::compress::Compression;
use crate::error::PkgError;
use crate::fsx::plus_x;
use crate::macho::sign_macho_executable;
use crate::sea_inject::inject_sea_blob;
use crate::target::{Arch, NodeTarget, Platform};

/// nodejs.org archive arch segments, per yao-pkg `types.ts` `NODE_ARCHS`.
const NODE_ARCHS: &[&str] = &[
    "x64", "arm64", "armv7l", "ppc64", "s390x", "riscv64", "loong64",
];

/// Minimum host Node major that supports the SEA pipeline.
const MIN_SEA_NODE_MAJOR: u32 = 22;

// ---------------------------------------------------------------------------
// Deterministic mapping (offline-testable)
// ---------------------------------------------------------------------------

/// Map a pkg platform to the nodejs.org OS segment (`getNodeOs`).
///
/// `macos` maps to `darwin`; only `darwin`/`linux`/`win` are valid SEA OSes.
pub fn sea_node_os(platform: Platform) -> Result<&'static str, PkgError> {
    match platform {
        Platform::Macos => Ok("darwin"),
        Platform::Linux => Ok("linux"),
        Platform::Win => Ok("win"),
        Platform::Alpine | Platform::LinuxStatic | Platform::Freebsd => {
            Err(PkgError::Sea(format!("Unsupported OS: {platform}")))
        }
    }
}

/// Map a pkg arch to the nodejs.org arch segment (`getNodeArch`).
///
/// Mirrors yao-pkg exactly: the arch's canonical string must be one of
/// `NODE_ARCHS`. `armv7` and `x86` are not nodejs.org SEA arch segments and are
/// rejected with the upstream wording.
pub fn sea_node_arch(arch: Arch) -> Result<&'static str, PkgError> {
    let token = match arch {
        Arch::X64 => "x64",
        Arch::Arm64 => "arm64",
        Arch::Armv7 => "armv7",
        Arch::X86 => "x86",
        Arch::Ppc64 => "ppc64",
        Arch::S390x => "s390x",
        Arch::Riscv64 => "riscv64",
        Arch::Loong64 => "loong64",
    };
    NODE_ARCHS
        .iter()
        .copied()
        .find(|known| *known == token)
        .ok_or_else(|| PkgError::Sea(format!("Unsupported architecture: {arch}")))
}

/// Whether an arch routes through unofficial-builds.nodejs.org (`riscv64`/`loong64`).
fn is_unofficial_arch(arch: &str) -> bool {
    matches!(arch, "riscv64" | "loong64")
}

/// nodejs.org archive filename: `node-<version>-<os>-<arch>.<zip|tar.gz>`.
pub fn sea_node_archive_filename(version: &str, os: &str, arch: &str) -> String {
    let ext = if os == "win" { "zip" } else { "tar.gz" };
    format!("node-{version}-{os}-{arch}.{ext}")
}

/// Resolve the dist + checksum URLs for a Node archive (official vs unofficial).
pub fn sea_node_dist_urls(version: &str, os: &str, arch: &str) -> (String, String) {
    let filename = sea_node_archive_filename(version, os, arch);
    let base = if is_unofficial_arch(arch) {
        "https://unofficial-builds.nodejs.org/download/release"
    } else {
        "https://nodejs.org/dist"
    };
    (
        format!("{base}/{version}/{filename}"),
        format!("{base}/{version}/SHASUMS256.txt"),
    )
}

/// Validate a bare Node version against yao-pkg's `getNodeVersion` regex:
/// `^\d{1,2}(\.\d{1,2}){0,2}$` (e.g. `16`, `16.0`, `16.0.0`).
pub fn sea_validate_node_version_format(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return false;
    }
    parts.iter().all(|part| {
        !part.is_empty() && part.len() <= 2 && part.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// Parse the major from a `node<major>` range, falling back to `host_major`
/// when the range is not a plain `node<digits>` form (e.g. `latest`).
fn target_major(node_range: &str, host_major: u32) -> u32 {
    node_range
        .strip_prefix("node")
        .and_then(|rest| rest.parse::<u32>().ok())
        .unwrap_or(host_major)
}

/// Smallest target Node major across a list (`resolveMinTargetMajor`).
pub fn sea_resolve_min_target_major(targets: &[NodeTarget], host_major: u32) -> u32 {
    if targets.is_empty() {
        return host_major;
    }
    targets
        .iter()
        .map(|target| target_major(&target.node_range, host_major))
        .min()
        .unwrap_or(host_major)
}

/// Reject a target list that mixes Node majors (`assertSingleTargetMajor`).
pub fn sea_assert_single_target_major(
    targets: &[NodeTarget],
    host_major: u32,
) -> Result<(), PkgError> {
    let mut majors: Vec<u32> = targets
        .iter()
        .map(|target| target_major(&target.node_range, host_major))
        .collect();
    majors.sort_unstable();
    majors.dedup();
    if majors.len() > 1 {
        let listed = majors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(PkgError::Sea(format!(
            "SEA mode cannot mix Node.js majors in a single run (got {listed}). \
             Run pkg once per Node major."
        )));
    }
    Ok(())
}

/// Parse a leading integer major from a `vX.Y.Z` / `X.Y.Z` version string.
fn version_major(version: &str) -> Option<u32> {
    version
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|major| major.parse::<u32>().ok())
}

/// Validate that the host Node running pkg supports SEA (`>= 22`).
///
/// Mirrors `assertHostSeaNodeVersion`; returns the host major on success.
pub fn sea_assert_host_node_version(host_version: &str) -> Result<u32, PkgError> {
    let major = version_major(host_version).unwrap_or(0);
    if major < MIN_SEA_NODE_MAJOR {
        return Err(PkgError::Sea(format!(
            "SEA support requires at least node v22.0.0, actual node version is {host_version}"
        )));
    }
    Ok(major)
}

/// Index of the first target whose platform+arch match the host
/// (`pickMatchingHostTargetIndex`).
pub fn sea_pick_matching_host_target_index(
    host_platform: Platform,
    host_arch: Arch,
    targets: &[NodeTarget],
) -> Option<usize> {
    targets
        .iter()
        .position(|target| target.platform == host_platform && target.arch == host_arch)
}

// ---------------------------------------------------------------------------
// Host Node interaction
// ---------------------------------------------------------------------------

/// Return the host `node --version` string (e.g. `v22.22.2`), if Node is present.
fn host_node_version() -> Option<String> {
    let output = Command::new("node").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8(output.stdout).ok()?;
    let trimmed = version.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

// ---------------------------------------------------------------------------
// Download / extract / cache
// ---------------------------------------------------------------------------

/// SEA download/extraction cache directory (`~/.pkg-cache/sea`).
///
/// Honors `PKG_CACHE_PATH` like the pkg-fetch cache (a testable superset of
/// yao-pkg, which hardcodes `homedir()/.pkg-cache`); falls back to `$HOME`.
fn sea_cache_dir() -> Result<PathBuf, PkgError> {
    let base = if let Some(path) = std::env::var_os("PKG_CACHE_PATH") {
        PathBuf::from(path)
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".pkg-cache")
    } else {
        return Err(PkgError::Sea(
            "HOME is not set and PKG_CACHE_PATH was not provided".to_owned(),
        ));
    };
    Ok(base.join("sea"))
}

/// Resolve the concrete Node version pkg will use for `target`
/// (`resolveTargetNodeVersion`), querying the dist index for a partial range.
fn resolve_target_node_version(target: &NodeTarget) -> Result<String, PkgError> {
    let os = sea_node_os(target.platform)?;
    let arch = sea_node_arch(target.arch)?;
    let bare = target.node_range.trim_start_matches("node");
    get_node_version(os, arch, bare)
}

/// Resolve the latest `vX.Y.Z` covering a partial range (`getNodeVersion`).
fn get_node_version(os: &str, arch: &str, node_version: &str) -> Result<String, PkgError> {
    if !sea_validate_node_version_format(node_version) {
        return Err(PkgError::Sea("Invalid node version format".to_owned()));
    }
    let parts: Vec<&str> = node_version.split('.').collect();
    if parts.len() == 3 {
        return Ok(format!("v{node_version}"));
    }

    let index_url = if is_unofficial_arch(arch) {
        "https://unofficial-builds.nodejs.org/download/release/index.json"
    } else {
        "https://nodejs.org/dist/index.json"
    };
    let response = reqwest::blocking::get(index_url)
        .map_err(|source| PkgError::Sea(format!("Failed to fetch node versions: {source}")))?;
    if !response.status().is_success() {
        return Err(PkgError::Sea("Failed to fetch node versions".to_owned()));
    }
    let versions: Vec<NodeIndexEntry> = response
        .json()
        .map_err(|source| PkgError::Sea(format!("Failed to fetch node versions: {source}")))?;

    let node_os = if os == "darwin" { "osx" } else { os };
    let prefix = format!("v{node_version}");
    let file_prefix = format!("{node_os}-{arch}");
    versions
        .iter()
        .find(|entry| {
            entry.version.starts_with(&prefix)
                && entry
                    .files
                    .iter()
                    .any(|file| file.starts_with(&file_prefix))
        })
        .map(|entry| entry.version.clone())
        .ok_or_else(|| PkgError::Sea(format!("Node version {node_version} not found")))
}

#[derive(serde::Deserialize)]
struct NodeIndexEntry {
    version: String,
    files: Vec<String>,
}

/// Download `url` to `path`, streaming the body to disk.
fn download_file(url: &str, path: &Path) -> Result<(), PkgError> {
    let response = reqwest::blocking::get(url)
        .map_err(|source| PkgError::Sea(format!("Failed to download file from {url}: {source}")))?;
    if !response.status().is_success() {
        return Err(PkgError::Sea(format!("Failed to download file from {url}")));
    }
    let bytes = response
        .bytes()
        .map_err(|source| PkgError::Sea(format!("Failed to download file from {url}: {source}")))?;
    fs::write(path, &bytes).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })
}

/// Verify the SHA-256 of `path` against the `SHASUMS256.txt` at `checksum_url`.
fn verify_checksum(path: &Path, checksum_url: &str, filename: &str) -> Result<(), PkgError> {
    let response = reqwest::blocking::get(checksum_url).map_err(|source| {
        PkgError::Sea(format!(
            "Failed to download checksum file from {checksum_url}: {source}"
        ))
    })?;
    if !response.status().is_success() {
        return Err(PkgError::Sea(format!(
            "Failed to download checksum file from {checksum_url}"
        )));
    }
    let checksums = response
        .text()
        .map_err(|source| PkgError::Sea(format!("Failed to read checksum file: {source}")))?;
    let expected = checksums
        .lines()
        .find(|line| line.contains(filename))
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(|| PkgError::Sea(format!("Checksum for {filename} not found")))?;

    let bytes = fs::read(path).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex_lower(&hasher.finalize());
    if actual != expected {
        return Err(PkgError::Sea(format!(
            "Checksum verification failed for {filename}"
        )));
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Extract the Node executable from a downloaded archive (`extract`).
///
/// Returns the path to the extracted binary, using the same `.ok` sentinel
/// scheme as yao-pkg so an interrupted extract is re-run rather than trusted.
fn extract(os: &str, archive_path: &Path) -> Result<PathBuf, PkgError> {
    let archive_dir = archive_path.parent().ok_or_else(|| {
        PkgError::Sea(format!(
            "archive path has no parent: {}",
            archive_path.display()
        ))
    })?;
    let suffix = if os == "win" { ".zip" } else { ".tar.gz" };
    let file_name = archive_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let node_dir = file_name
        .strip_suffix(suffix)
        .unwrap_or(&file_name)
        .to_owned();

    let node_path = if os == "win" {
        archive_dir.join(format!("{node_dir}.exe"))
    } else {
        archive_dir.join(&node_dir).join("bin").join("node")
    };
    let sentinel = sentinel_path(&node_path);

    if sentinel.exists() && node_path.exists() {
        return Ok(node_path);
    }
    let _ = fs::remove_file(&node_path);
    let _ = fs::remove_file(&sentinel);

    if os == "win" {
        extract_zip_member(archive_path, &format!("{node_dir}/node.exe"), &node_path)?;
    } else {
        extract_tar_gz_member(archive_path, &format!("{node_dir}/bin/node"), &node_path)?;
    }

    if !node_path.exists() {
        return Err(PkgError::Sea(
            "Node executable not found in the archive".to_owned(),
        ));
    }
    fs::write(&sentinel, b"").map_err(|source| PkgError::Io {
        path: sentinel.display().to_string(),
        source,
    })?;
    Ok(node_path)
}

fn sentinel_path(path: &Path) -> PathBuf {
    let mut sentinel = path.as_os_str().to_os_string();
    sentinel.push(".ok");
    PathBuf::from(sentinel)
}

/// Extract one member from a `.tar.gz` archive to `dest`.
fn extract_tar_gz_member(archive: &Path, member: &str, dest: &Path) -> Result<(), PkgError> {
    let file = fs::File::open(archive).map_err(|source| PkgError::Io {
        path: archive.display().to_string(),
        source,
    })?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    let entries = tar
        .entries()
        .map_err(|source| PkgError::Sea(format!("invalid tar archive: {source}")))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|source| PkgError::Sea(format!("invalid tar entry: {source}")))?;
        let path = entry
            .path()
            .map_err(|source| PkgError::Sea(format!("invalid tar entry path: {source}")))?;
        if path.to_string_lossy() == member {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|source| PkgError::Io {
                    path: parent.display().to_string(),
                    source,
                })?;
            }
            entry.unpack(dest).map_err(|source| PkgError::Io {
                path: dest.display().to_string(),
                source,
            })?;
            return Ok(());
        }
    }
    Err(PkgError::Sea(
        "Node executable not found in the archive".to_owned(),
    ))
}

/// Extract one member from a `.zip` archive to `dest`.
fn extract_zip_member(archive: &Path, member: &str, dest: &Path) -> Result<(), PkgError> {
    let file = fs::File::open(archive).map_err(|source| PkgError::Io {
        path: archive.display().to_string(),
        source,
    })?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|source| PkgError::Sea(format!("invalid zip archive: {source}")))?;
    let mut entry = zip
        .by_name(member)
        .map_err(|_| PkgError::Sea("Node executable not found in the archive".to_owned()))?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|source| PkgError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let mut out = fs::File::create(dest).map_err(|source| PkgError::Io {
        path: dest.display().to_string(),
        source,
    })?;
    std::io::copy(&mut entry, &mut out)
        .map_err(|source| PkgError::Io {
            path: dest.display().to_string(),
            source,
        })
        .map(|_| ())
}

/// Fetch, verify, and extract a Node binary for `target` (`getNodejsExecutable`).
fn get_nodejs_executable(target: &NodeTarget, log: &dyn Fn(&str)) -> Result<PathBuf, PkgError> {
    let os = sea_node_os(target.platform)?;
    let arch = sea_node_arch(target.arch)?;
    let version = resolve_target_node_version(target)?;
    let filename = sea_node_archive_filename(&version, os, arch);
    let (url, checksum_url) = sea_node_dist_urls(&version, os, arch);

    let download_dir = sea_cache_dir()?;
    fs::create_dir_all(&download_dir).map_err(|source| PkgError::Io {
        path: download_dir.display().to_string(),
        source,
    })?;
    let archive_path = download_dir.join(&filename);
    let archive_sentinel = sentinel_path(&archive_path);

    if !(archive_sentinel.exists() && archive_path.exists()) {
        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_file(&archive_sentinel);
        log(&format!("Downloading nodejs executable from {url}..."));
        download_file(&url, &archive_path)?;
        log(&format!("Verifying checksum of {filename}"));
        verify_checksum(&archive_path, &checksum_url, &filename)?;
        fs::write(&archive_sentinel, b"").map_err(|source| PkgError::Io {
            path: archive_sentinel.display().to_string(),
            source,
        })?;
    }

    log(&format!("Extracting node binary from {filename}"));
    extract(os, &archive_path)
}

// ---------------------------------------------------------------------------
// Blob generation + baking
// ---------------------------------------------------------------------------

/// Generate the SEA prep blob via `node --experimental-sea-config`.
fn generate_sea_blob(
    config_path: &Path,
    generator: &Path,
    log: &dyn Fn(&str),
) -> Result<(), PkgError> {
    log("Generating the blob...");
    let output = Command::new(generator)
        .arg("--experimental-sea-config")
        .arg(config_path)
        .output()
        .map_err(|source| PkgError::Io {
            path: generator.display().to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(PkgError::Sea(format!(
            "Failed to generate SEA blob: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// Pick the Node binary used to generate the blob (`pickBlobGeneratorBinary`).
///
/// 1. Prefer a downloaded target binary whose platform+arch match the host (it is
///    already version-matched).
/// 2. Otherwise, if the host `node` on PATH is exactly the resolved target
///    version, use it (JS uses `process.execPath`).
/// 3. Otherwise download a host-platform/arch Node binary pinned to the exact
///    target version and use it purely as the generator.
///
/// The generator must match the target's Node version: per yao-pkg discussion
/// #236 the SEA blob layout is patch-version specific, so a skewed generator
/// crashes the produced binary at startup in
/// `node::sea::FindSingleExecutableResource`.
fn pick_blob_generator_binary(
    targets: &[NodeTarget],
    node_paths: &[PathBuf],
    log: &dyn Fn(&str),
) -> Result<PathBuf, PkgError> {
    if let Some(index) =
        sea_pick_matching_host_target_index(Platform::host(), Arch::host(), targets)
        && let Some(path) = node_paths.get(index)
    {
        return Ok(path.clone());
    }

    let target = targets
        .first()
        .ok_or_else(|| PkgError::Sea("SEA mode requires at least one target".to_owned()))?;
    let target_version = resolve_target_node_version(target)?;
    let host_version = host_node_version();
    if host_version.as_deref() == Some(target_version.as_str())
        && let Ok(path) = which_node()
    {
        // JS uses process.execPath (the Node running pkg). pkg-rust is not Node,
        // so the version-matched host `node` on PATH is the generator.
        return Ok(path);
    }

    // No host-matching target and no version-matched host node: download a
    // host-platform Node pinned to the exact target version. Passing the full
    // `node<major.minor.patch>` range makes version resolution short-circuit the
    // dist index, so only the archive itself is fetched.
    log(&format!(
        "No target matches host {}-{}; downloading a host-platform node {} to \
         generate the SEA blob (avoids SEA header version skew, see yao-pkg \
         discussion #236).",
        Platform::host(),
        Arch::host(),
        target_version,
    ));
    let generator_target = NodeTarget {
        node_range: format!("node{}", target_version.trim_start_matches('v')),
        platform: Platform::host(),
        arch: Arch::host(),
        force_build: false,
    };
    get_nodejs_executable(&generator_target, log).map_err(|error| {
        PkgError::Sea(format!(
            "Cannot generate SEA blob: host node {} differs from target {} and the \
             host-platform download failed ({}). Running the generator with a skewed \
             node would crash the final binary at startup in \
             node::sea::FindSingleExecutableResource (see yao-pkg discussion #236). \
             Install node {} locally (e.g. via nvm) or pass a host-runnable node of \
             that version.",
            host_version.as_deref().unwrap_or("(absent)"),
            target_version,
            error,
            target_version,
        ))
    })
}

/// Resolve a `node` executable from `PATH`.
fn which_node() -> Result<PathBuf, PkgError> {
    let paths = std::env::var_os("PATH")
        .ok_or_else(|| PkgError::Sea("PATH is not set; cannot locate node".to_owned()))?;
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::split_paths(&paths)
        .map(|dir| dir.join(exe))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| PkgError::Sea("node was not found on PATH".to_owned()))
}

/// Bake the blob into a copy of the Node binary at `output` (`bake`).
fn bake(
    node_path: &Path,
    output: &Path,
    blob: &[u8],
    target: &NodeTarget,
    log: &dyn Fn(&str),
) -> Result<(), PkgError> {
    log(&format!(
        "Creating executable for {}-{}-{}....",
        target.node_range, target.platform, target.arch
    ));
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| PkgError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    if output.exists() {
        log(&format!(
            "Executable {} already exists, will be overwritten",
            output.display()
        ));
        let _ = fs::remove_file(output);
    }

    let image = fs::read(node_path).map_err(|source| PkgError::Io {
        path: node_path.display().to_string(),
        source,
    })?;
    log(&format!("Injecting the blob into {}...", output.display()));
    let injected = inject_sea_blob(image, blob, target.platform)?;

    let mut file = fs::File::create(output).map_err(|source| PkgError::Io {
        path: output.display().to_string(),
        source,
    })?;
    file.write_all(&injected).map_err(|source| PkgError::Io {
        path: output.display().to_string(),
        source,
    })?;
    Ok(())
}

/// Ad-hoc sign a macOS SEA binary when signing is requested (`signMacOSIfNeeded`,
/// `isSea: true` — no `__LINKEDIT` patch; postject/native injection already lays
/// out the Mach-O so `codesign --sign -` suffices).
fn sign_macos_if_needed(
    output: &Path,
    target: &NodeTarget,
    signature: bool,
    log: &dyn Fn(&str),
) -> Result<(), PkgError> {
    if !signature || target.platform != Platform::Macos {
        return Ok(());
    }
    if let Err(error) = sign_macho_executable(output) {
        if target.arch == Arch::Arm64 {
            log(&format!(
                "Warning Unable to sign the macOS executable: {error}"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

/// Run the SEA pipeline for a package plan with `plan.sea` set.
///
/// Validates the host/target Node versions, then routes to simple SEA mode
/// (bare entry file). Enhanced SEA mode (package.json/config projects) fails
/// closed pending the walker + sea-assets + VFS-bootstrap slice.
pub(crate) fn run_sea(plan: &PackagePlan, log: &dyn Fn(&str)) -> Result<(), PkgError> {
    // Enhanced SEA mode is the next slice; fail closed before doing any work.
    if plan.sea_enhanced {
        return Err(PkgError::Sea(
            "Enhanced SEA mode (--sea with a package.json/config) is not implemented \
             yet; use simple --sea with a bare entry file. Enhanced mode (walker + \
             per-file archive + VFS bootstrap) is the next slice."
                .to_owned(),
        ));
    }

    // index.ts rejects --compress for simple mode before sea()'s host check.
    if plan.compression != Compression::None {
        return Err(PkgError::Sea(
            "Simple SEA mode (--sea without a package.json) does not support --compress. \
             Add a package.json with a \"pkg\" / \"bin\" entry to use the enhanced SEA \
             pipeline, which supports compression."
                .to_owned(),
        ));
    }

    let host_version = host_node_version().ok_or_else(|| {
        PkgError::Sea(
            "SEA support requires Node.js >= 22 on the build host, but `node` was not \
             found on PATH"
                .to_owned(),
        )
    })?;
    let host_major = sea_assert_host_node_version(&host_version)?;

    let targets: Vec<NodeTarget> = plan
        .outputs
        .iter()
        .map(|output| output.target.clone())
        .collect();
    sea_assert_single_target_major(&targets, host_major)?;
    // Reject unsupported targets up front so the documented default
    // (`pkg --sea index.js` -> linux,macos,win) fails fast instead of
    // downloading, generating a blob, and only then erroring on the macOS/Windows
    // injectors. (PR #6 review P1.)
    validate_sea_targets(&targets)?;

    run_simple_sea(plan, &targets, log)
}

/// Validate every SEA target before any download/blob work happens.
///
/// Checks the nodejs.org os/arch mapping (cheap, no network) and that native
/// `NODE_SEA_BLOB` injection is implemented for each target platform, failing
/// closed with one actionable error for the unsupported set.
fn validate_sea_targets(targets: &[NodeTarget]) -> Result<(), PkgError> {
    for target in targets {
        sea_node_os(target.platform)?;
        sea_node_arch(target.arch)?;
    }
    let unsupported: Vec<String> = targets
        .iter()
        .filter(|target| !crate::sea_inject::injection_supported(target.platform))
        .map(ToString::to_string)
        .collect();
    if !unsupported.is_empty() {
        return Err(PkgError::Sea(format!(
            "native SEA injection currently supports Linux (ELF) targets only; \
             unsupported target(s): {}. Use -t with node<major>-linux targets \
             (macOS and Windows SEA injection is the next slice).",
            unsupported.join(", ")
        )));
    }
    Ok(())
}

/// Simple SEA mode: the bare entry file becomes the SEA `main` (`sea()`).
fn run_simple_sea(
    plan: &PackagePlan,
    targets: &[NodeTarget],
    log: &dyn Fn(&str),
) -> Result<(), PkgError> {
    let entrypoint = &plan.entrypoint;
    if !entrypoint.exists() {
        return Err(PkgError::Sea(format!(
            "Entrypoint path \"{}\" does not exist",
            entrypoint.display()
        )));
    }

    // Download/extract every target's Node binary up front.
    let node_paths = targets
        .iter()
        .map(|target| get_nodejs_executable(target, log))
        .collect::<Result<Vec<_>, _>>()?;

    let tmp_dir = sea_tmp_dir()?;
    let result = (|| {
        let blob_path = tmp_dir.join("sea-prep.blob");
        let config_path = tmp_dir.join("sea-config.json");
        let sea_config = serde_json::json!({
            "main": entrypoint,
            "output": blob_path,
            "disableExperimentalSEAWarning": true,
            "useSnapshot": false,
            "useCodeCache": false,
        });
        log("Creating sea-config.json file...");
        let config_bytes = serde_json::to_vec(&sea_config).map_err(|source| {
            PkgError::Sea(format!("failed to encode sea-config.json: {source}"))
        })?;
        fs::write(&config_path, config_bytes).map_err(|source| PkgError::Io {
            path: config_path.display().to_string(),
            source,
        })?;

        let generator = pick_blob_generator_binary(targets, &node_paths, log)?;
        generate_sea_blob(&config_path, &generator, log)?;

        let blob = fs::read(&blob_path).map_err(|source| PkgError::Io {
            path: blob_path.display().to_string(),
            source,
        })?;

        for (index, target) in targets.iter().enumerate() {
            let output = &plan.outputs[index].output;
            let node_path = &node_paths[index];
            bake(node_path, output, &blob, target, log)?;
            sign_macos_if_needed(output, target, plan.signature, log)?;
            if target.platform != Platform::Win {
                plus_x(output)?;
            }
        }
        Ok(())
    })();

    let _ = fs::remove_dir_all(&tmp_dir);
    result
}

/// Create a unique SEA temp directory (`withSeaTmpDir`, without the `chdir`).
fn sea_tmp_dir() -> Result<PathBuf, PkgError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("pkg-sea-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|source| PkgError::Io {
        path: dir.display().to_string(),
        source,
    })?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::{TargetDefaults, TargetParseError, parse_targets};

    fn targets(spec: &str) -> Result<Vec<NodeTarget>, TargetParseError> {
        let defaults = TargetDefaults::host("node22");
        Ok(parse_targets(spec, &defaults)?.targets)
    }

    #[test]
    fn maps_node_os_and_arch() -> Result<(), PkgError> {
        assert_eq!(sea_node_os(Platform::Macos)?, "darwin");
        assert_eq!(sea_node_os(Platform::Linux)?, "linux");
        assert_eq!(sea_node_os(Platform::Win)?, "win");
        assert!(matches!(
            sea_node_os(Platform::Alpine),
            Err(PkgError::Sea(message)) if message == "Unsupported OS: alpine"
        ));
        assert_eq!(sea_node_arch(Arch::X64)?, "x64");
        assert_eq!(sea_node_arch(Arch::Arm64)?, "arm64");
        assert!(matches!(
            sea_node_arch(Arch::Armv7),
            Err(PkgError::Sea(message)) if message == "Unsupported architecture: armv7"
        ));
        assert!(matches!(
            sea_node_arch(Arch::X86),
            Err(PkgError::Sea(message)) if message == "Unsupported architecture: x86"
        ));
        Ok(())
    }

    #[test]
    fn builds_archive_filenames_and_urls() {
        assert_eq!(
            sea_node_archive_filename("v22.22.2", "linux", "x64"),
            "node-v22.22.2-linux-x64.tar.gz"
        );
        assert_eq!(
            sea_node_archive_filename("v22.22.2", "win", "x64"),
            "node-v22.22.2-win-x64.zip"
        );
        let (url, sums) = sea_node_dist_urls("v22.22.2", "linux", "x64");
        assert_eq!(
            url,
            "https://nodejs.org/dist/v22.22.2/node-v22.22.2-linux-x64.tar.gz"
        );
        assert_eq!(sums, "https://nodejs.org/dist/v22.22.2/SHASUMS256.txt");
        let (url, sums) = sea_node_dist_urls("v22.0.0", "linux", "riscv64");
        assert_eq!(
            url,
            "https://unofficial-builds.nodejs.org/download/release/v22.0.0/node-v22.0.0-linux-riscv64.tar.gz"
        );
        assert!(sums.starts_with("https://unofficial-builds.nodejs.org/"));
    }

    #[test]
    fn validates_node_version_format() {
        assert!(sea_validate_node_version_format("16"));
        assert!(sea_validate_node_version_format("16.0"));
        assert!(sea_validate_node_version_format("16.0.0"));
        assert!(!sea_validate_node_version_format("v16"));
        assert!(!sea_validate_node_version_format("16.0.0.0"));
        assert!(!sea_validate_node_version_format("160"));
        assert!(!sea_validate_node_version_format(""));
        assert!(!sea_validate_node_version_format("16."));
    }

    #[test]
    fn host_version_assertion_matches_wording() -> Result<(), PkgError> {
        assert_eq!(sea_assert_host_node_version("v22.22.2")?, 22);
        assert_eq!(sea_assert_host_node_version("v25.0.0")?, 25);
        assert!(matches!(
            sea_assert_host_node_version("v20.11.0"),
            Err(PkgError::Sea(message))
                if message == "SEA support requires at least node v22.0.0, actual node version is v20.11.0"
        ));
        Ok(())
    }

    #[test]
    fn min_target_major_and_single_major() -> Result<(), TargetParseError> {
        assert_eq!(sea_resolve_min_target_major(&[], 25), 25);
        assert_eq!(
            sea_resolve_min_target_major(&targets("node22-linux,node24-linux")?, 25),
            22
        );
        // Unparseable range falls back to host major.
        assert_eq!(
            sea_resolve_min_target_major(&targets("latest-linux")?, 25),
            25
        );

        assert!(sea_assert_single_target_major(&targets("node22-linux,node22-macos")?, 22).is_ok());
        assert!(matches!(
            sea_assert_single_target_major(&targets("node22-linux,node24-linux")?, 22),
            Err(PkgError::Sea(message))
                if message == "SEA mode cannot mix Node.js majors in a single run (got 22, 24). Run pkg once per Node major."
        ));
        Ok(())
    }

    #[test]
    fn picks_matching_host_target_index() -> Result<(), TargetParseError> {
        let list = targets("node22-linux-x64,node22-macos-arm64")?;
        assert_eq!(
            sea_pick_matching_host_target_index(Platform::Linux, Arch::X64, &list),
            Some(0)
        );
        assert_eq!(
            sea_pick_matching_host_target_index(Platform::Macos, Arch::Arm64, &list),
            Some(1)
        );
        assert_eq!(
            sea_pick_matching_host_target_index(Platform::Win, Arch::X64, &list),
            None
        );
        Ok(())
    }

    #[test]
    fn validate_targets_rejects_unsupported_platforms() -> Result<(), TargetParseError> {
        assert!(validate_sea_targets(&targets("node22-linux-x64")?).is_ok());
        // macOS (Mach-O) and Windows (PE) injection are not implemented yet.
        assert!(matches!(
            validate_sea_targets(&targets("node22-macos-arm64")?),
            Err(PkgError::Sea(message))
                if message.contains("unsupported target(s)") && message.contains("node22-macos-arm64")
        ));
        assert!(matches!(
            validate_sea_targets(&targets("node22-win-x64")?),
            Err(PkgError::Sea(message)) if message.contains("node22-win-x64")
        ));
        // alpine fails the nodejs.org OS mapping before the injection check.
        assert!(matches!(
            validate_sea_targets(&targets("node22-alpine-x64")?),
            Err(PkgError::Sea(message)) if message == "Unsupported OS: alpine"
        ));
        Ok(())
    }

    #[test]
    fn generator_reuses_host_matching_target_without_network() {
        // A host-matching target short-circuits generator selection to the
        // already-downloaded binary, so no network/version resolution happens.
        let target = NodeTarget {
            node_range: "node22".to_owned(),
            platform: Platform::host(),
            arch: Arch::host(),
            force_build: false,
        };
        let paths = vec![PathBuf::from("/cache/host-node")];
        let log = |_: &str| {};
        assert!(matches!(
            pick_blob_generator_binary(std::slice::from_ref(&target), &paths, &log),
            Ok(path) if path == Path::new("/cache/host-node")
        ));
    }

    fn unique_tmp(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "pkg-rust-sea-test-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn extract_pulls_node_from_tar_gz() -> Result<(), Box<dyn std::error::Error>> {
        use flate2::Compression;
        use flate2::write::GzEncoder;

        let dir = unique_tmp("targz");
        fs::create_dir_all(&dir)?;
        let node_dir = "node-v22.0.0-linux-x64";
        let archive = dir.join(format!("{node_dir}.tar.gz"));

        let file = fs::File::create(&archive)?;
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let payload = b"#!fake-node-binary\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, format!("{node_dir}/bin/node"), &payload[..])?;
        builder.into_inner()?.finish()?;

        let node_path = extract("linux", &archive)?;
        assert_eq!(node_path, dir.join(node_dir).join("bin").join("node"));
        assert_eq!(fs::read(&node_path)?, payload);
        assert!(
            sentinel_path(&node_path).exists(),
            "extract sentinel written"
        );

        // Second call is a no-op short-circuit on the sentinel.
        let again = extract("linux", &archive)?;
        assert_eq!(again, node_path);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn extract_pulls_node_exe_from_zip() -> Result<(), Box<dyn std::error::Error>> {
        use zip::write::SimpleFileOptions;

        let dir = unique_tmp("zip");
        fs::create_dir_all(&dir)?;
        let node_dir = "node-v22.0.0-win-x64";
        let archive = dir.join(format!("{node_dir}.zip"));

        let file = fs::File::create(&archive)?;
        let mut writer = zip::ZipWriter::new(file);
        writer.start_file(format!("{node_dir}/node.exe"), SimpleFileOptions::default())?;
        writer.write_all(b"MZ-fake-windows-node")?;
        writer.finish()?;

        let node_path = extract("win", &archive)?;
        assert_eq!(node_path, dir.join(format!("{node_dir}.exe")));
        assert_eq!(fs::read(&node_path)?, b"MZ-fake-windows-node");
        assert!(sentinel_path(&node_path).exists());

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn extract_missing_member_reports_clearly() -> Result<(), Box<dyn std::error::Error>> {
        use flate2::Compression;
        use flate2::write::GzEncoder;

        let dir = unique_tmp("targz-missing");
        fs::create_dir_all(&dir)?;
        let archive = dir.join("node-v22.0.0-linux-x64.tar.gz");
        let file = fs::File::create(&archive)?;
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(3);
        header.set_cksum();
        builder.append_data(&mut header, "node-v22.0.0-linux-x64/README", &b"hi\n"[..])?;
        builder.into_inner()?.finish()?;

        assert!(matches!(
            extract("linux", &archive),
            Err(PkgError::Sea(message)) if message == "Node executable not found in the archive"
        ));

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
