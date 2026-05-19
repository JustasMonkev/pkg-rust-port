use std::path::PathBuf;

use crate::cli::PackagePlan;
use crate::error::PkgError;
use crate::fsx::plus_x;
use crate::pack::pack;
use crate::produce::{ProducedExecutable, write_executable_image};
use crate::refine::refine_walked;
use crate::target::{NodeTarget, Platform};
use crate::walk::{WalkerParams, walk};

/// Supplies target binary bytes for packaging.
///
/// Real CLI orchestration will use a cache/fetch-backed implementation. Tests
/// can provide deterministic in-memory binaries with producer placeholders.
pub trait TargetBinaryProvider {
    /// Return target binary bytes for the requested target.
    fn binary_for(&self, target: &NodeTarget) -> Result<Vec<u8>, PkgError>;
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
///     "%VIRTUAL_FILESYSTEM% %DEFAULT_ENTRYPOINT% %SYMLINKS% %DICT% %DOCOMPRESS%",
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

    for planned in &plan.outputs {
        let binary = provider.binary_for(&planned.target)?;
        let walked = walk(
            plan.marker.clone(),
            &plan.entrypoint,
            plan.addition.clone(),
            WalkerParams::new().with_root(&plan.root),
        )?;
        let refined = refine_walked(walked, &plan.entrypoint, planned.path_style);
        let packed = pack(refined, plan.bytecode)?;
        let image = write_executable_image(
            &planned.output,
            binary,
            packed,
            prelude_template,
            plan.compression,
            planned.path_style,
            bakery_from_bakes(&plan.bakes),
        )?;
        if planned.target.platform != Platform::Win {
            plus_x(&planned.output)?;
        }
        outputs.push(ProducedOutput {
            target: planned.target.clone(),
            output: planned.output.clone(),
            image,
        });
    }

    Ok(PackageBuild { outputs })
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
