use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::PackagePlan;
use crate::error::PkgError;
use crate::fsx::plus_x;
use crate::macho::{patch_macho_executable_file, sign_macho_executable};
use crate::pack::pack;
use crate::produce::{
    NativeAddonOptions, ProducedExecutable, ProducerBuildOptions,
    write_executable_image_with_fabricator_diagnostics,
};
use crate::refine::refine_walked_with_snapshot_base;
use crate::target::{Arch, NodeTarget, Platform};
use crate::walk::{PackageWarning, WalkerParams, walk};

/// Target binary data plus optional cache path metadata.
///
/// Providers that read binaries from disk should preserve the path so later
/// stages can use the same executable for target-specific bytecode generation.
/// In-memory test providers can return bytes without a path.
///
/// # Example
///
/// ```
/// let artifact = pkg_rust::TargetBinary::from_bytes(vec![1, 2, 3]);
/// assert_eq!(artifact.bytes(), &[1, 2, 3]);
/// assert!(artifact.path().is_none());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetBinary {
    bytes: Vec<u8>,
    path: Option<PathBuf>,
}

impl TargetBinary {
    /// Build an in-memory target binary artifact.
    #[must_use]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes, path: None }
    }

    /// Attach a filesystem path to this target binary artifact.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Return the executable bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the cache path when the provider read this binary from disk.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn into_parts(self) -> (Vec<u8>, Option<PathBuf>) {
        (self.bytes, self.path)
    }
}

/// Supplies target binary bytes for packaging.
///
/// Real CLI orchestration will use a cache/fetch-backed implementation. Tests
/// can provide deterministic in-memory binaries with producer placeholders.
pub trait TargetBinaryProvider {
    /// Return target binary bytes for the requested target.
    fn binary_for(&self, target: &NodeTarget) -> Result<Vec<u8>, PkgError>;

    /// Return target binary bytes with optional filesystem path metadata.
    ///
    /// Implementers that only have bytes can rely on this default method.
    fn binary_artifact_for(&self, target: &NodeTarget) -> Result<TargetBinary, PkgError> {
        self.binary_for(target).map(TargetBinary::from_bytes)
    }
}

/// One executable artifact produced from a package plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducedOutput {
    /// Target that was produced.
    pub target: NodeTarget,
    /// Output path written on disk.
    pub output: PathBuf,
    /// Produced image and layout metadata.
    pub image: ProducedExecutable,
}

/// Result of building all outputs in a package plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageBuild {
    /// Outputs in plan order.
    pub outputs: Vec<ProducedOutput>,
    /// Non-fatal warnings encountered while producing outputs.
    pub warnings: Vec<PackageWarning>,
}

/// Build package outputs using an explicit target binary provider.
///
/// # Example
///
/// ```
/// struct StubBinary;
///
/// impl pkg_rust::TargetBinaryProvider for StubBinary {
///     fn binary_for(&self, _target: &pkg_rust::NodeTarget) -> Result<Vec<u8>, pkg_rust::PkgError> {
///         let mut binary = Vec::from([b'\0']);
///         for _index in 0..20 {
///             binary.extend_from_slice(b"// BAKERY ");
///         }
///         binary.extend_from_slice(
///             b"// PAYLOAD_POSITION //// PAYLOAD_SIZE //// PRELUDE_POSITION //// PRELUDE_SIZE //",
///         );
///         Ok(binary)
///     }
/// }
///
/// let output = std::env::temp_dir().join(format!("pkg-rust-build-{}", std::process::id()));
/// let output_text = output
///     .to_str()
///     .ok_or_else(|| pkg_rust::PkgError::Cli("non-utf8 temp path".to_owned()))?;
/// let plan = pkg_rust::plan_package([
///     "--target",
///     "linux",
///     "--output",
///     output_text,
///     "../test/test-50-require-resolve/test-x-index.js",
/// ])?;
/// let build = pkg_rust::build_package_with_provider(
///     &plan,
///     &StubBinary,
///     &pkg_rust::prelude_template(false),
/// )?;
/// assert_eq!(build.outputs.len(), 1);
/// let _ = std::fs::remove_file(output);
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn build_package_with_provider(
    plan: &PackagePlan,
    provider: &impl TargetBinaryProvider,
    prelude_template: &str,
) -> Result<PackageBuild, PkgError> {
    let mut outputs = Vec::new();
    let mut warnings = Vec::new();

    for planned in &plan.outputs {
        let binary = provider.binary_artifact_for(&planned.target)?;
        let (binary_bytes, binary_path) = binary.into_parts();
        let fabricator_path = runnable_fabricator_path(&binary_bytes, binary_path.as_deref());
        let native_addons =
            native_addon_options(plan.native_build, &planned.target, binary_path.as_deref());
        let walked = walk(
            plan.marker.clone(),
            &plan.entrypoint,
            plan.addition.clone(),
            WalkerParams::new()
                .with_root(&plan.root)
                .with_public_toplevel(plan.public_toplevel)
                .with_public_packages(plan.public_packages.clone())
                .with_no_dictionary(plan.no_dictionary.clone()),
        )?;
        let output_warnings = walked.warnings.clone();
        warnings.extend(output_warnings.clone());
        let refined = refine_walked_with_snapshot_base(
            walked,
            &plan.entrypoint,
            &plan.snapshot_base,
            planned.path_style,
        );
        let packed = pack(refined, plan.bytecode)?;
        prepare_output_path(&planned.output)?;
        let produced = write_executable_image_with_fabricator_diagnostics(
            &planned.output,
            binary_bytes,
            packed,
            prelude_template,
            ProducerBuildOptions {
                compression: plan.compression,
                style: planned.path_style,
                bakery: bakery_from_bakes(&plan.bakes),
                bakes: &plan.bakes,
                fabricator_path,
                native_addons,
            },
        )?;
        let image = produced.executable;
        warnings.extend(produced.warnings);
        if planned.target.platform != Platform::Win {
            if plan.signature && planned.target.platform == Platform::Macos {
                patch_macho_executable_file(&planned.output)?;
                if let Err(error) = sign_macho_executable(&planned.output)
                    && planned.target.arch == Arch::Arm64
                {
                    warnings.push(PackageWarning::MacosSignature {
                        output: planned.output.clone(),
                        message: error.to_string(),
                    });
                }
            }
            plus_x(&planned.output)?;
        }
        copy_deploy_files(&output_warnings, &planned.output)?;
        outputs.push(ProducedOutput {
            target: planned.target.clone(),
            output: planned.output.clone(),
            image,
        });
    }

    Ok(PackageBuild { outputs, warnings })
}

fn copy_deploy_files(warnings: &[PackageWarning], output: &Path) -> Result<(), PkgError> {
    let output_dir = output.parent().unwrap_or_else(|| Path::new(""));
    for warning in warnings {
        let PackageWarning::DeployFile { source, target, .. } = warning else {
            continue;
        };
        copy_deploy_path(source, &output_dir.join(target))?;
    }
    Ok(())
}

fn copy_deploy_path(source: &Path, target: &Path) -> Result<(), PkgError> {
    let Ok(metadata) = fs::metadata(source) else {
        return Ok(());
    };

    if metadata.is_file() {
        copy_deploy_file(source, target, &metadata)?;
    } else if metadata.is_dir() {
        copy_deploy_directory(source, target)?;
    }
    Ok(())
}

fn copy_deploy_directory(source: &Path, target: &Path) -> Result<(), PkgError> {
    let mut entries = fs::read_dir(source)
        .map_err(|source_error| PkgError::Io {
            path: source.display().to_string(),
            source: source_error,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source_error| PkgError::Io {
            path: source.display().to_string(),
            source: source_error,
        })?;
    entries.sort_by_key(std::fs::DirEntry::path);

    for entry in entries {
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        copy_deploy_path(&source_path, &target_path)?;
    }
    Ok(())
}

fn copy_deploy_file(source: &Path, target: &Path, metadata: &fs::Metadata) -> Result<(), PkgError> {
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source_error| PkgError::Io {
            path: parent.display().to_string(),
            source: source_error,
        })?;
    }
    fs::copy(source, target).map_err(|source_error| PkgError::Io {
        path: target.display().to_string(),
        source: source_error,
    })?;
    fs::set_permissions(target, metadata.permissions()).map_err(|source_error| PkgError::Io {
        path: target.display().to_string(),
        source: source_error,
    })?;
    Ok(())
}

fn bakery_from_bakes(bakes: &[String]) -> Vec<u8> {
    let mut bakery = Vec::new();
    if bakes.is_empty() {
        return bakery;
    }

    for bake in bakes {
        bakery.extend_from_slice(bake.as_bytes());
        bakery.push(0);
    }
    bakery.push(0);
    bakery
}

fn runnable_fabricator_path<'a>(binary: &[u8], path: Option<&'a Path>) -> Option<&'a Path> {
    if looks_like_executable(binary) {
        path
    } else {
        None
    }
}

fn native_addon_options(
    native_build: bool,
    target: &NodeTarget,
    binary_path: Option<&Path>,
) -> NativeAddonOptions {
    if !native_build {
        return NativeAddonOptions::default();
    }

    NativeAddonOptions {
        platform: Some(target.platform.to_string()),
        install_platform: Some(prebuild_platform(target.platform).to_owned()),
        arch: Some(target.arch.to_string()),
        node_version: binary_path.and_then(node_version_from_binary_path),
        prebuild_install: prebuild_install_path(),
    }
}

fn prebuild_platform(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => "darwin",
        Platform::Win => "win32",
        Platform::Alpine | Platform::Linux | Platform::LinuxStatic => "linux",
        Platform::Freebsd => "freebsd",
    }
}

fn prebuild_install_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PKG_PREBUILD_INSTALL").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }

    source_tree_prebuild_install()
        .or_else(|| local_prebuild_install().filter(|path| path.is_file()))
        .or_else(|| find_on_path("prebuild-install"))
}

fn source_tree_prebuild_install() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir
        .parent()?
        .join("node_modules")
        .join(".bin")
        .join(executable_name("prebuild-install"));
    candidate.is_file().then_some(candidate)
}

fn local_prebuild_install() -> Option<PathBuf> {
    Some(
        PathBuf::from("node_modules")
            .join(".bin")
            .join(executable_name("prebuild-install")),
    )
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|path| path.join(executable_name(name)))
        .find(|candidate| candidate.is_file())
}

#[cfg(windows)]
fn executable_name(name: &str) -> String {
    format!("{name}.cmd")
}

#[cfg(not(windows))]
fn executable_name(name: &str) -> String {
    name.to_owned()
}

fn node_version_from_binary_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    name.split('-')
        .find(|part| node_version_token(part))
        .map(ToOwned::to_owned)
}

fn node_version_token(value: &str) -> bool {
    let Some(version) = value.strip_prefix('v') else {
        return false;
    };
    let mut parts = version.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !major.is_empty()
        && !minor.is_empty()
        && !patch.is_empty()
        && major.chars().all(|character| character.is_ascii_digit())
        && minor.chars().all(|character| character.is_ascii_digit())
        && patch.chars().all(|character| character.is_ascii_digit())
}

fn looks_like_executable(binary: &[u8]) -> bool {
    binary.starts_with(b"#!")
        || binary.starts_with(b"\x7fELF")
        || binary.starts_with(b"MZ")
        || binary.starts_with(&[0xcf, 0xfa, 0xed, 0xfe])
        || binary.starts_with(&[0xfe, 0xed, 0xfa, 0xcf])
        || binary.starts_with(&[0xca, 0xfe, 0xba, 0xbe])
        || binary.starts_with(&[0xca, 0xfe, 0xba, 0xbf])
}

fn prepare_output_path(output: &Path) -> Result<(), PkgError> {
    match fs::metadata(output) {
        Ok(metadata) if metadata.is_file() => {
            fs::remove_file(output).map_err(|source| PkgError::Io {
                path: output.display().to_string(),
                source,
            })
        }
        Ok(_) => Err(PkgError::Cli(
            "Refusing to overwrite non-file output".to_owned(),
        )),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = output.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent).map_err(|source| PkgError::Io {
                    path: parent.display().to_string(),
                    source,
                })?;
            }
            Ok(())
        }
        Err(source) => Err(PkgError::Io {
            path: output.display().to_string(),
            source,
        }),
    }
}
