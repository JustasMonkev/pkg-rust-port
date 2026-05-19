use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Parser, error::ErrorKind};

use crate::common::PathStyle;
use crate::compress::Compression;
use crate::config::PackageJson;
use crate::error::PkgError;
use crate::fetch::PkgFetchCache;
use crate::package::build_package_with_provider;
use crate::target::{NodeTarget, Platform, TargetDefaults, output_names, parse_targets};
use crate::walk::Marker;

const DEFAULT_PRELUDE_TEMPLATE: &str =
    "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%";

#[derive(Debug, Parser)]
#[command(
    name = "pkg",
    version,
    about = "Package your Node.js project into an executable"
)]
struct Cli {
    #[arg(value_name = "input")]
    input: Option<PathBuf>,

    #[arg(short = 't', long = "targets", alias = "target")]
    targets: Option<String>,

    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,

    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    #[arg(long = "out-path", alias = "out-dir", alias = "outdir")]
    out_path: Option<PathBuf>,

    #[arg(long = "options")]
    options: Option<String>,

    #[arg(short = 'd', long = "debug")]
    debug: bool,

    #[arg(short = 'b', long = "build")]
    build: bool,

    #[arg(long = "public")]
    public: bool,

    #[arg(long = "public-packages")]
    public_packages: Option<String>,

    #[arg(long = "no-bytecode", default_value_t = false)]
    no_bytecode: bool,

    #[arg(long = "no-native-build", default_value_t = false)]
    no_native_build: bool,

    #[arg(long = "no-signature", default_value_t = false)]
    no_signature: bool,

    #[arg(long = "no-dict")]
    no_dict: Option<String>,

    #[arg(short = 'C', long = "compress")]
    compress: Option<String>,
}

/// Planned output artifact for one target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedOutput {
    /// Parsed Node/platform/architecture target.
    pub target: NodeTarget,
    /// Filesystem output path for this target.
    pub output: PathBuf,
    /// Path style used inside the virtual snapshot filesystem.
    pub path_style: PathStyle,
}

/// Parsed package command plan before binary fetch and production.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePlan {
    /// Input path after directory-to-`package.json` normalization.
    pub input: PathBuf,
    /// Entrypoint file that will be walked and packed.
    pub entrypoint: PathBuf,
    /// Package marker used by the dependency walker.
    pub marker: Marker,
    /// Optional extra config file to include in the package.
    pub addition: Option<PathBuf>,
    /// Root directory that bounds directory-link walking.
    pub root: PathBuf,
    /// Compression algorithm requested for payload stripes.
    pub compression: Compression,
    /// Whether bytecode generation is enabled.
    pub bytecode: bool,
    /// Command-line options baked into the executable.
    pub bakes: Vec<String>,
    /// Output artifacts in target order.
    pub outputs: Vec<PlannedOutput>,
}

/// Parse command arguments into a package plan.
///
/// This mirrors the JavaScript CLI's input, output, target, compression, and
/// bake-option planning. Fetching base Node binaries and writing executable
/// bytes happens in later orchestration.
///
/// # Example
///
/// ```
/// let output = std::env::temp_dir().join("pkg-rust-plan-demo");
/// let plan = pkg_rust::plan_package([
///     "--targets",
///     "linux,win",
///     "--output",
///     output.to_str().ok_or_else(|| pkg_rust::PkgError::Cli("non-utf8 temp path".to_owned()))?,
///     "../test/test-46-input-package-json",
/// ])?;
/// assert_eq!(plan.outputs.len(), 2);
/// assert!(plan.outputs[1].output.ends_with("pkg-rust-plan-demo-win.exe"));
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn plan_package<I, S>(argv: I) -> Result<PackagePlan, PkgError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let cli = parse_cli(argv)?;
    plan_from_cli(cli)
}

/// Execute the `pkg` command with already-split arguments.
///
/// The argument iterator should not include the program name, matching the
/// JavaScript `exec(process.argv.slice(2))` API.
///
/// # Example
///
/// ```
/// # async fn example() -> Result<(), pkg_rust::PkgError> {
/// let result = pkg_rust::exec(["--version"]).await;
/// assert!(result.is_ok() || result.is_err());
/// # Ok(())
/// # }
/// ```
pub async fn exec<I, S>(argv: I) -> Result<(), PkgError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let Some(cli) = parse_cli_or_display(argv)? else {
        return Ok(());
    };
    let plan = plan_from_cli(cli)?;
    let cache = PkgFetchCache::default_cache()?;
    build_package_with_provider(&plan, &cache, DEFAULT_PRELUDE_TEMPLATE)?;
    Ok(())
}

fn parse_cli<I, S>(argv: I) -> Result<Cli, PkgError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = std::iter::once(OsString::from("pkg")).chain(argv.into_iter().map(Into::into));
    Cli::try_parse_from(args).map_err(|error| PkgError::Cli(error.to_string()))
}

fn parse_cli_or_display<I, S>(argv: I) -> Result<Option<Cli>, PkgError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = std::iter::once(OsString::from("pkg")).chain(argv.into_iter().map(Into::into));
    match Cli::try_parse_from(args) {
        Ok(cli) => Ok(Some(cli)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().map_err(|source| PkgError::Io {
                path: "stdout".to_owned(),
                source,
            })?;
            Ok(None)
        }
        Err(error) => Err(PkgError::Cli(error.to_string())),
    }
}

fn plan_from_cli(cli: Cli) -> Result<PackagePlan, PkgError> {
    let compression = cli
        .compress
        .as_deref()
        .unwrap_or("none")
        .parse::<Compression>()
        .map_err(|error| PkgError::Cli(error.to_string()))?;
    let input_arg = cli
        .input
        .as_ref()
        .ok_or_else(|| PkgError::Cli("Entry file/directory is expected".to_owned()))?;
    let input = normalize_input_path(input_arg)?;
    let input_package = if is_package_json(&input) {
        Some(read_package_json(&input)?)
    } else {
        None
    };

    if input_package.is_some() && cli.config.is_some() {
        return Err(PkgError::Cli(
            "Specify either 'package.json' or config. Not both".to_owned(),
        ));
    }

    let config_path = cli
        .config
        .as_ref()
        .map(|config| absolute_path(config))
        .transpose()?;
    let config = match config_path.as_ref() {
        Some(config) => Some(read_package_json(config)?),
        None => None,
    };
    let entrypoint = resolve_entrypoint(&input, input_package.as_ref())?;
    let marker = build_marker(
        &input,
        input_package.as_ref(),
        config_path.as_deref(),
        config.as_ref(),
    )?;
    let addition = if is_package_json(&input) {
        Some(input.clone())
    } else {
        None
    };
    let root = entrypoint
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let auto_output = cli.output.is_none();
    let output_base = output_base(&cli, &entrypoint, input_package.as_ref(), config.as_ref())?;
    let target_defaults = TargetDefaults::host(host_node_range());
    let targets = resolve_targets(
        &cli,
        input_package.as_ref(),
        config.as_ref(),
        &target_defaults,
    )?;
    let outputs = plan_outputs(&output_base, &targets, auto_output, &entrypoint)?;
    let bakes = cli
        .options
        .unwrap_or_default()
        .split(',')
        .filter(|option| !option.is_empty())
        .map(|option| format!("--{option}"))
        .collect();

    Ok(PackagePlan {
        input,
        entrypoint,
        marker,
        addition,
        root,
        compression,
        bytecode: !cli.no_bytecode,
        bakes,
        outputs,
    })
}

fn build_marker(
    input: &Path,
    input_package: Option<&PackageJson>,
    config_path: Option<&Path>,
    config: Option<&PackageJson>,
) -> Result<Marker, PkgError> {
    if let Some(package) = input_package {
        return Ok(Marker::with_package_path(package.clone(), input));
    }
    if let (Some(config_path), Some(config)) = (config_path, config) {
        return Ok(Marker::with_package_path(config.clone(), config_path));
    }

    let package = PackageJson::parse("{}").map_err(|error| PkgError::Cli(error.to_string()))?;
    Ok(Marker::new(package))
}

fn normalize_input_path(input: &Path) -> Result<PathBuf, PkgError> {
    let input = absolute_path(input)?;
    let metadata = fs::metadata(&input).map_err(|source| PkgError::Io {
        path: input.display().to_string(),
        source,
    })?;
    if metadata.is_dir() {
        let package = input.join("package.json");
        fs::metadata(&package).map_err(|source| PkgError::Io {
            path: package.display().to_string(),
            source,
        })?;
        return Ok(package);
    }
    Ok(input)
}

fn resolve_entrypoint(input: &Path, package: Option<&PackageJson>) -> Result<PathBuf, PkgError> {
    if let Some(package) = package {
        let Some(entrypoint) = package.resolve_selected_bin(input) else {
            return Err(PkgError::Cli(
                "Property 'bin' does not exist in package.json".to_owned(),
            ));
        };
        fs::metadata(&entrypoint).map_err(|source| PkgError::Io {
            path: entrypoint.display().to_string(),
            source,
        })?;
        return Ok(entrypoint);
    }
    Ok(input.to_path_buf())
}

fn output_base(
    cli: &Cli,
    entrypoint: &Path,
    input_package: Option<&PackageJson>,
    config: Option<&PackageJson>,
) -> Result<PathBuf, PkgError> {
    if cli.output.is_some() && cli.out_path.is_some() {
        return Err(PkgError::Cli(
            "Specify either 'output' or 'out-path'. Not both".to_owned(),
        ));
    }

    if let Some(output) = cli.output.as_ref() {
        return absolute_path(output);
    }

    let output_name = if let Some(package) = input_package {
        package.package_basename().ok_or_else(|| {
            PkgError::Cli("Property 'name' does not exist in package.json".to_owned())
        })?
    } else if let Some(package) = config {
        package
            .package_basename()
            .or_else(|| {
                entrypoint
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .ok_or_else(|| PkgError::Cli("Unable to derive output name".to_owned()))?
    } else {
        entrypoint
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| PkgError::Cli("Unable to derive output name".to_owned()))?
    };

    let stem = Path::new(&output_name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or(output_name);
    let configured_out_path = cli
        .out_path
        .clone()
        .or_else(|| {
            input_package
                .and_then(|package| package.pkg.as_ref())
                .and_then(|pkg| pkg.output_path.as_ref())
                .map(PathBuf::from)
        })
        .or_else(|| {
            config
                .and_then(|package| package.pkg.as_ref())
                .and_then(|pkg| pkg.output_path.as_ref())
                .map(PathBuf::from)
        })
        .unwrap_or_default();
    absolute_path(&configured_out_path.join(stem))
}

fn resolve_targets(
    cli: &Cli,
    input_package: Option<&PackageJson>,
    config: Option<&PackageJson>,
    defaults: &TargetDefaults,
) -> Result<Vec<NodeTarget>, PkgError> {
    if let Some(targets) = cli.targets.as_deref()
        && !targets.is_empty()
    {
        return parse_targets(targets, defaults)
            .map(|parsed| parsed.targets)
            .map_err(|error| PkgError::Cli(error.to_string()));
    }

    let json_targets = input_package
        .and_then(|package| package.pkg.as_ref())
        .map(|pkg| &pkg.targets)
        .or_else(|| {
            config
                .and_then(|package| package.pkg.as_ref())
                .map(|pkg| &pkg.targets)
        });
    if let Some(targets) = json_targets
        && !targets.is_empty()
    {
        return parse_targets(&targets.join(","), defaults)
            .map(|parsed| parsed.targets)
            .map_err(|error| PkgError::Cli(error.to_string()));
    }

    let fallback = if cli.output.is_some() {
        "host"
    } else {
        "linux,macos,win"
    };
    parse_targets(fallback, defaults)
        .map(|parsed| parsed.targets)
        .map_err(|error| PkgError::Cli(error.to_string()))
}

fn plan_outputs(
    output_base: &Path,
    targets: &[NodeTarget],
    auto_output: bool,
    entrypoint: &Path,
) -> Result<Vec<PlannedOutput>, PkgError> {
    let output_base = output_base.to_string_lossy();
    let names = output_names(&output_base, targets);
    let mut outputs = Vec::new();

    for (target, name) in targets.iter().zip(names) {
        let mut output = PathBuf::from(name);
        if output == entrypoint {
            if auto_output {
                output = PathBuf::from(format!("{}-{}", output.display(), target.platform));
            } else {
                return Err(PkgError::Cli("Refusing to overwrite input file".to_owned()));
            }
        }
        outputs.push(PlannedOutput {
            target: target.clone(),
            output,
            path_style: match target.platform {
                Platform::Win => PathStyle::Windows,
                _ => PathStyle::Posix,
            },
        });
    }

    Ok(outputs)
}

fn read_package_json(path: &Path) -> Result<PackageJson, PkgError> {
    let content = fs::read_to_string(path).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })?;
    PackageJson::parse(&content).map_err(|error| PkgError::Cli(error.to_string()))
}

fn is_package_json(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "package.json")
}

fn absolute_path(path: &Path) -> Result<PathBuf, PkgError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let cwd = std::env::current_dir().map_err(|source| PkgError::Io {
        path: ".".to_owned(),
        source,
    })?;
    Ok(cwd.join(path))
}

fn host_node_range() -> String {
    if let Ok(output) = Command::new("node").arg("--version").output()
        && output.status.success()
        && let Ok(version) = String::from_utf8(output.stdout)
        && let Some(major) = version
            .trim()
            .strip_prefix('v')
            .and_then(|value| value.split('.').next())
        && !major.is_empty()
        && major.chars().all(|ch| ch.is_ascii_digit())
    {
        return format!("node{major}");
    }

    // DECISION: target parsing needs a default even on machines without Node;
    // node18 is the oldest actively tested range in this source tree.
    "node18".to_owned()
}
