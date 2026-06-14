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
use crate::prelude::prelude_template;
use crate::target::{NodeTarget, Platform, TargetDefaults, output_names, parse_targets};
use crate::walk::Marker;

#[derive(Debug, Parser)]
#[command(
    name = "pkg",
    version = crate::prelude::PKG_VERSION,
    disable_version_flag = true,
    about = "Package your Node.js project into an executable",
    after_help = CLI_EXAMPLES,
)]
struct Cli {
    #[arg(value_name = "input")]
    input: Option<PathBuf>,

    /// output pkg version
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// comma-separated list of targets (see examples)
    #[arg(
        short = 't',
        long = "targets",
        alias = "target",
        value_name = "targets"
    )]
    targets: Option<String>,

    /// package.json or a .json, .js, .cjs, or .mjs file with top-level config (auto-discovered as .pkgrc, .pkgrc.json, pkg.config.js, pkg.config.cjs, or pkg.config.mjs)
    #[arg(short = 'c', long = "config", value_name = "config")]
    config: Option<PathBuf>,

    /// output file name or template for several files
    #[arg(short = 'o', long = "output", value_name = "output")]
    output: Option<PathBuf>,

    /// path to save output one or more executables
    #[arg(
        long = "out-path",
        alias = "out-dir",
        alias = "outdir",
        value_name = "out-path"
    )]
    out_path: Option<PathBuf>,

    /// bake v8 options into executable to run with them on
    #[arg(long = "options", value_name = "options")]
    options: Option<String>,

    /// show more information during packaging process [off]
    #[arg(short = 'd', long = "debug", overrides_with = "no_debug")]
    debug: bool,

    #[arg(long = "no-debug", hide = true, overrides_with = "debug")]
    no_debug: bool,

    /// don't download prebuilt base binaries, build them
    #[arg(short = 'b', long = "build")]
    build: bool,

    /// speed up and disclose the sources of top-level project
    #[arg(long = "public", overrides_with = "no_public")]
    public: bool,

    #[arg(long = "no-public", hide = true, overrides_with = "public")]
    no_public: bool,

    /// force specified packages to be considered public
    #[arg(long = "public-packages", value_name = "public-packages")]
    public_packages: Option<String>,

    /// skip bytecode generation and include source files as plain js
    #[arg(long = "no-bytecode", overrides_with = "bytecode")]
    no_bytecode: bool,

    #[arg(long = "bytecode", hide = true, overrides_with = "no_bytecode")]
    bytecode: bool,

    /// skip native addons build
    #[arg(long = "no-native-build", overrides_with = "native_build")]
    no_native_build: bool,

    #[arg(long = "native-build", hide = true, overrides_with = "no_native_build")]
    native_build: bool,

    /// skip macOS binary signing [default: sign]
    #[arg(
        long = "no-signature",
        default_value_t = false,
        overrides_with = "signature"
    )]
    no_signature: bool,

    /// enable macOS binary signing (default; use to override signature:false in config)
    #[arg(
        long = "signature",
        default_value_t = false,
        overrides_with = "no_signature"
    )]
    signature: bool,

    /// if bytecode generation fails for a file, ship it as plain source instead of skipping it
    #[arg(long = "fallback-to-source", overrides_with = "no_fallback_to_source")]
    fallback_to_source: bool,

    #[arg(
        long = "no-fallback-to-source",
        hide = true,
        overrides_with = "fallback_to_source"
    )]
    no_fallback_to_source: bool,

    /// comma-separated list of packages names to ignore dictionaries. Use --no-dict * to disable all dictionaries
    #[arg(long = "no-dict", value_name = "no-dict")]
    no_dict: Option<String>,

    /// [default=None] compression algorithm = Brotli, GZip, or Zstd (Zstd requires Node.js >= 22.15 in the produced executable)
    #[arg(short = 'C', long = "compress", value_name = "compress")]
    compress: Option<String>,

    /// (Experimental) compile given file using node's SEA feature. Requires node v22.0.0 or higher on the build host
    #[arg(long = "sea", overrides_with = "no_sea")]
    sea: bool,

    #[arg(long = "no-sea", hide = true, overrides_with = "sea")]
    no_sea: bool,
}

/// Usage examples appended to the CLI help, mirroring the JS `help.ts` output.
const CLI_EXAMPLES: &str = "\
All build-shaping flags above (compress, fallback-to-source, public, public-packages,
options, bytecode, native-build, no-dict, debug, signature, sea) can also be set in
the pkg config file (camelCase keys). CLI flags override config values.

Examples:

– Makes executables for Linux, macOS and Windows
  $ pkg index.js
– Takes package.json from cwd and follows 'bin' entry
  $ pkg .
– Makes executable for particular target machine
  $ pkg -t node14-win-arm64 index.js
– Makes executables for target machines of your choice
  $ pkg -t node22-linux,node24-linux,node24-win index.js
– Bakes '--expose-gc' and '--max-heap-size=34' into executable
  $ pkg --options \"expose-gc,max-heap-size=34\" index.js
– Consider packageA and packageB to be public
  $ pkg --public-packages \"packageA,packageB\" index.js
– Consider all packages to be public
  $ pkg --public-packages \"*\" index.js
– Bakes '--expose-gc' into executable
  $ pkg --options expose-gc index.js
– reduce size of the data packed inside the executable with GZip
  $ pkg --compress GZip index.js
– reduce size further with Zstd (Node.js >= 22.15 required at runtime)
  $ pkg --compress Zstd index.js
– compile the file using node's SEA feature. Creates executables for Linux, macOS and Windows
  $ pkg --sea index.js";

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
    /// Host directory stripped from snapshot paths during refinement.
    pub snapshot_base: PathBuf,
    /// Compression algorithm requested for payload stripes.
    pub compression: Compression,
    /// Whether debug diagnostics are enabled (CLI flag or config).
    pub debug: bool,
    /// Whether bytecode generation is enabled.
    pub bytecode: bool,
    /// Whether native addon prebuild selection/building is enabled.
    pub native_build: bool,
    /// Whether macOS outputs should be ad-hoc signed.
    pub signature: bool,
    /// Whether failed bytecode fabrication ships plain source instead of
    /// skipping the file.
    pub fallback_to_source: bool,
    /// Whether to build a Single Executable Application via Node's SEA feature.
    pub sea: bool,
    /// Whether the SEA build uses enhanced mode (input is a package.json or a
    /// resolved config file) rather than simple mode (a bare entry file).
    pub sea_enhanced: bool,
    /// Whether JavaScript source should be disclosed for the top-level package.
    pub public_toplevel: bool,
    /// Dependency package names whose JavaScript source should be disclosed.
    pub public_packages: Vec<String>,
    /// Built-in dictionary module filenames disabled for this package build.
    pub no_dictionary: Vec<String>,
    /// Top-level config `ignore` glob patterns.
    pub ignore: Vec<String>,
    /// Command-line options baked into the executable.
    pub bakes: Vec<String>,
    /// Output artifacts in target order.
    pub outputs: Vec<PlannedOutput>,
    /// Informational/warning lines produced while planning (config discovery),
    /// already formatted with their `> `/`> Warning ` prefixes.
    pub notices: Vec<String>,
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
///     "test/test-46-input-package-json",
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
    if cli.version {
        // JS pkg prints the bare version (`console.log(version)`).
        println!("{}", crate::prelude::PKG_VERSION);
        return Ok(());
    }
    // JS pkg logs `pkg@<version>` once arguments are accepted, before option,
    // target, and input processing.
    println!("> pkg@{}", crate::prelude::PKG_VERSION);
    let plan = plan_from_cli(cli)?;
    let debug = plan.debug;
    for notice in &plan.notices {
        println!("{notice}");
    }
    if plan.compression != Compression::None {
        println!("compression:  {}", plan.compression.cli_label());
    }
    if plan.sea {
        // SEA downloads from nodejs.org and shells out to the host node via
        // blocking reqwest/Command, so run it on a dedicated OS thread (like the
        // classic build) instead of inside the Tokio runtime.
        let plan_for_sea = plan.clone();
        let sea_thread = std::thread::Builder::new()
            .name("pkg-rust-sea".to_owned())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let log = |message: &str| println!("> {message}");
                crate::sea::run_sea(&plan_for_sea, &log)
            })
            .map_err(|source| PkgError::Io {
                path: "pkg-rust-sea thread".to_owned(),
                source,
            })?;
        return tokio::task::spawn_blocking(move || {
            sea_thread
                .join()
                .map_err(|_payload| PkgError::Cli("SEA build task panicked".to_owned()))?
        })
        .await
        .map_err(|error| PkgError::Cli(format!("SEA build join task failed: {error}")))?;
    }
    let cache = PkgFetchCache::default_cache()?;
    let prelude = prelude_template(debug);
    // DECISION: larger fixtures overflow Tokio's default blocking-worker stack
    // during synchronous packaging. A dedicated 8 MiB OS thread keeps reqwest's
    // blocking client outside the async runtime and gives the pack/produce path
    // enough stack without requiring callers to set RUST_MIN_STACK.
    let build_thread = std::thread::Builder::new()
        .name("pkg-rust-build".to_owned())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || build_package_with_provider(&plan, &cache, &prelude))
        .map_err(|source| PkgError::Io {
            path: "pkg-rust-build thread".to_owned(),
            source,
        })?;
    let build_result = tokio::task::spawn_blocking(move || {
        build_thread
            .join()
            .map_err(|_payload| PkgError::Cli("package build task panicked".to_owned()))?
    })
    .await
    .map_err(|error| PkgError::Cli(format!("package build join task failed: {error}")))?;
    let build = build_result?;
    for warning in build.warnings {
        if warning.is_debug() {
            if debug {
                println!("> [debug] {}", warning.to_cli_message());
            }
        } else {
            println!("> Warning {}", warning.to_cli_message());
        }
    }
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
        Err(error) if error.kind() == ErrorKind::DisplayHelp => {
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
    let input_arg = cli
        .input
        .as_ref()
        .ok_or_else(|| PkgError::Cli("Entry file/directory is expected".to_owned()))?;
    let input = normalize_input_path(input_arg)?;
    let input_package = if is_configuration(&input) {
        Some(read_package_json(&input)?)
    } else {
        None
    };

    if input_package.is_some() && cli.config.is_some() {
        return Err(PkgError::Cli(
            "Specify either 'package.json' or config. Not both".to_owned(),
        ));
    }

    let mut notices = Vec::new();
    let explicit_config = cli.config.is_some();
    let config_path = match cli.config.as_ref() {
        Some(config) => Some(absolute_path(config)?),
        None => {
            let discovered = input.parent().and_then(find_pkgrc);
            if let Some(found) = discovered.as_ref() {
                notices.push(format!("> Using config {}", relative_to_cwd(found)));
            }
            discovered
        }
    };
    let config = match config_path.as_ref() {
        Some(config) => Some(load_pkgrc(config)?),
        None => None,
    };
    if !explicit_config
        && let Some(config_path) = config_path.as_ref()
        && input_package
            .as_ref()
            .is_some_and(|package| package.pkg.is_some())
    {
        let basename = config_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        notices.push(format!(
            "> Warning Both {basename} and \"pkg\" field in package.json were found. The {basename} file takes precedence."
        ));
    }

    // JS resolveConfig: an external config file's `pkg` options take
    // precedence over the input package.json `pkg` field; build-shaping
    // flags resolve CLI > config > default.
    let flag_config = config
        .as_ref()
        .and_then(|package| package.pkg.as_ref())
        .or_else(|| {
            if config.is_some() {
                None
            } else {
                input_package
                    .as_ref()
                    .and_then(|package| package.pkg.as_ref())
            }
        });
    let compression = cli
        .compress
        .as_deref()
        .map(ToOwned::to_owned)
        .or_else(|| flag_config.and_then(|pkg| pkg.compress.clone()))
        .unwrap_or_else(|| "none".to_owned())
        .parse::<Compression>()
        .map_err(|error| PkgError::Cli(error.to_string()))?;
    let debug = resolve_bool(
        cli_bool(cli.debug, cli.no_debug),
        flag_config.and_then(|pkg| pkg.debug),
        false,
    );
    let bytecode = resolve_bool(
        cli_bool(cli.bytecode, cli.no_bytecode),
        flag_config.and_then(|pkg| pkg.bytecode),
        true,
    );
    let native_build = resolve_bool(
        cli_bool(cli.native_build, cli.no_native_build),
        flag_config.and_then(|pkg| pkg.native_build),
        true,
    );
    let signature = resolve_bool(
        cli_bool(cli.signature, cli.no_signature),
        flag_config.and_then(|pkg| pkg.signature),
        true,
    );
    let fallback_to_source = resolve_bool(
        cli_bool(cli.fallback_to_source, cli.no_fallback_to_source),
        flag_config.and_then(|pkg| pkg.fallback_to_source),
        false,
    );
    let public_toplevel = resolve_bool(
        cli_bool(cli.public, cli.no_public),
        flag_config.and_then(|pkg| pkg.public),
        false,
    );
    let sea = resolve_bool(
        cli_bool(cli.sea, cli.no_sea),
        flag_config.and_then(|pkg| pkg.sea),
        false,
    );
    // Simple SEA can still use flag-only config for a bare entry file (for
    // example `{ "sea": true, "targets": [...] }`). Only package input or
    // config that shapes bundled package contents needs the enhanced pipeline.
    let sea_enhanced =
        input_package.is_some() || config.as_ref().is_some_and(package_requires_enhanced_sea);
    let public_packages_raw = cli.public_packages.clone().or_else(|| {
        flag_config
            .and_then(|pkg| pkg.public_packages.as_ref())
            .map(crate::config::StringOrList::to_comma_joined)
    });
    let no_dict_raw = cli.no_dict.clone().or_else(|| {
        flag_config
            .and_then(|pkg| pkg.no_dictionary.as_ref())
            .map(crate::config::StringOrList::to_comma_joined)
    });
    let options_raw = cli.options.clone().or_else(|| {
        flag_config
            .and_then(|pkg| pkg.options.as_ref())
            .map(crate::config::StringOrList::to_comma_joined)
    });
    let entrypoint = resolve_entrypoint(&input, input_package.as_ref())?;
    let marker = build_marker(
        &input,
        input_package.as_ref(),
        config_path.as_deref(),
        config.as_ref(),
    )?;
    let addition = if is_configuration(&input) {
        Some(input.clone())
    } else {
        None
    };
    // DECISION: Treat only the immediate parent package as the snapshot package
    // for file inputs; ancestor packages would accidentally make repo roots part
    // of unrelated fixture packages. Package files below node_modules keep the
    // first node_modules segment so bare self-requires still resolve at runtime.
    // Plain file inputs still preserve their entry directory basename under
    // /snapshot, matching pkg's /snapshot/<dir>/<entry> layout without widening
    // the walk root to sibling fixture directories.
    let package_dir = if input_package.is_some() {
        input.parent().map(Path::to_path_buf)
    } else {
        immediate_package_dir(&entrypoint)
    };
    let root = package_dir
        .as_ref()
        .filter(|_| input_package.is_some())
        .cloned()
        .or_else(|| entrypoint.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let snapshot_base = if let Some(package_dir) = package_dir {
        package_snapshot_base(&package_dir, &root)
    } else {
        file_input_snapshot_base(&root)
    };
    let auto_output = cli.output.is_none();
    let output_base = output_base(
        &cli,
        &entrypoint,
        input_package.as_ref(),
        config.as_ref(),
        flag_config,
    )?;
    let target_defaults = TargetDefaults::host(host_node_range());
    let mut targets = resolve_targets(&cli, flag_config, &target_defaults)?;
    for target in &mut targets {
        target.force_build = cli.build;
    }
    if compression == Compression::Zstd {
        reject_zstd_incapable_targets(&targets)?;
    }
    let outputs = plan_outputs(&output_base, &targets, auto_output, &entrypoint)?;
    let bakes = options_raw
        .unwrap_or_default()
        .split(',')
        .filter(|option| !option.is_empty())
        .map(|option| format!("--{option}"))
        .collect();
    let public_packages = parse_public_packages(public_packages_raw.as_deref());
    let no_dictionary = parse_dictionary_modules(no_dict_raw.as_deref());
    let ignore = flag_config
        .map(|pkg| config_value_strings(&pkg.ignore))
        .unwrap_or_default();

    Ok(PackagePlan {
        input,
        entrypoint,
        marker,
        addition,
        root,
        snapshot_base,
        compression,
        debug,
        bytecode,
        native_build,
        signature,
        fallback_to_source,
        sea,
        sea_enhanced,
        public_toplevel,
        public_packages,
        no_dictionary,
        ignore,
        bakes,
        outputs,
        notices,
    })
}

/// Flatten a string-or-array config value into a string list.
fn config_value_strings(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(text) => vec![text.clone()],
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(ToOwned::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn package_requires_enhanced_sea(package: &PackageJson) -> bool {
    package.main.is_some()
        || package.bin.is_some()
        || !package.dependencies.is_empty()
        || !package.files.is_empty()
        || package
            .pkg
            .as_ref()
            .is_some_and(pkg_config_requires_enhanced_sea)
}

fn pkg_config_requires_enhanced_sea(pkg: &crate::config::PkgConfig) -> bool {
    config_value_has_entries(&pkg.scripts)
        || config_value_has_entries(&pkg.assets)
        || config_value_has_entries(&pkg.deploy_files)
        || config_value_has_entries(&pkg.ignore)
        || !pkg.patches.is_empty()
        || !pkg.dictionary.is_empty()
}

fn config_value_has_entries(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
        serde_json::Value::String(text) => !text.is_empty(),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(entries) => !entries.is_empty(),
    }
}

/// Collapse a positive/negative CLI flag pair into an explicit tri-state.
const fn cli_bool(positive: bool, negative: bool) -> Option<bool> {
    if positive {
        Some(true)
    } else if negative {
        Some(false)
    } else {
        None
    }
}

/// JS `resolveFlags` precedence: CLI > pkg config > default.
const fn resolve_bool(cli: Option<bool>, config: Option<bool>, default: bool) -> bool {
    match (cli, config) {
        (Some(value), _) => value,
        (None, Some(value)) => value,
        (None, None) => default,
    }
}

/// Auto-discovered config filenames, in JS `PKGRC_FILENAMES` precedence order.
const PKGRC_FILENAMES: &[&str] = &[
    ".pkgrc",
    ".pkgrc.json",
    "pkg.config.js",
    "pkg.config.cjs",
    "pkg.config.mjs",
];

fn find_pkgrc(base_dir: &Path) -> Option<PathBuf> {
    PKGRC_FILENAMES
        .iter()
        .map(|name| base_dir.join(name))
        .find(|candidate| candidate.exists())
}

fn relative_to_cwd(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Load a pkgrc / pkg.config file the way JS `loadPkgrc` does: `.pkgrc` and
/// `*.json` parse as JSON; `.js`/`.cjs`/`.mjs` are evaluated through the host
/// `node` (dynamic import, default export preferred). A bare pkg config (no
/// package-like keys) is wrapped as `{ "pkg": ... }`.
fn load_pkgrc(path: &Path) -> Result<PackageJson, PkgError> {
    if !path.exists() {
        return Err(PkgError::Cli(format!(
            "Config file does not exist: {}",
            path.display()
        )));
    }
    let basename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let is_json = basename == ".pkgrc" || basename.ends_with(".json");
    let raw = if is_json {
        fs::read_to_string(path).map_err(|source| PkgError::Io {
            path: path.display().to_string(),
            source,
        })?
    } else {
        load_js_config_source(path)?
    };
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| PkgError::Cli(format!("invalid config file {basename}: {error}")))?;
    let wrapped = wrap_bare_pkg_config(value);
    serde_json::from_value(wrapped)
        .map_err(|error| PkgError::Cli(format!("invalid config file {basename}: {error}")))
}

/// Evaluate a `.js`/`.cjs`/`.mjs` config through the host `node` and return
/// its default export serialized as JSON.
///
/// DECISION: the JS implementation dynamically imports config modules
/// in-process. The Rust port keeps config-module execution behind the same
/// external-`node` boundary already used for bytecode fabrication instead of
/// embedding a JavaScript engine.
fn load_js_config_source(path: &Path) -> Result<String, PkgError> {
    let script = "const { pathToFileURL } = require('url');\n\
        import(pathToFileURL(process.argv[1]).href)\n\
          .then((mod) => process.stdout.write(JSON.stringify(mod.default ?? mod)))\n\
          .catch((error) => { console.error(error && error.message ? error.message : String(error)); process.exit(1); });";
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(path)
        .output()
        .map_err(|source| PkgError::Io {
            path: "node".to_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(PkgError::Cli(format!(
            "Failed to load config file {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| PkgError::Cli(format!("config file produced non-UTF8 JSON: {error}")))
}

/// JS `resolveConfigFile`: a config without package-like keys is a bare pkg
/// config and gets wrapped as `{ "pkg": ... }`.
fn wrap_bare_pkg_config(value: serde_json::Value) -> serde_json::Value {
    let is_bare = value.as_object().is_some_and(|object| {
        !object.contains_key("name")
            && !object.contains_key("files")
            && !object.contains_key("dependencies")
            && !object.contains_key("pkg")
    });
    if is_bare {
        serde_json::json!({ "pkg": value })
    } else {
        value
    }
}

fn parse_public_packages(packages: Option<&str>) -> Vec<String> {
    let Some(packages) = packages else {
        return Vec::new();
    };
    let parsed = packages
        .split(',')
        .map(str::trim)
        .filter(|package| !package.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if parsed.iter().any(|package| package == "*") {
        vec!["*".to_owned()]
    } else {
        parsed
    }
}

fn parse_dictionary_modules(modules: Option<&str>) -> Vec<String> {
    let Some(modules) = modules else {
        return Vec::new();
    };
    let parsed = modules
        .split(',')
        .map(str::trim)
        .filter(|module| !module.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if parsed.iter().any(|module| module == "*") {
        vec!["*".to_owned()]
    } else {
        parsed
    }
}

fn build_marker(
    input: &Path,
    input_package: Option<&PackageJson>,
    config_path: Option<&Path>,
    config: Option<&PackageJson>,
) -> Result<Marker, PkgError> {
    if let Some(package) = input_package {
        let mut package = package.clone();
        if let Some(config) = config {
            // A discovered config file replaces the package.json `pkg` field
            // for walker options (scripts, assets, patches, deployFiles,
            // dictionary), matching the precedence flag/target resolution
            // already applies. `find_pkgrc` only looks in the package
            // directory, so relative globs still resolve against the same base.
            package.pkg = config.pkg.clone();
        }
        return Ok(Marker::with_package_path(package, input));
    }
    if let (Some(config_path), Some(config)) = (config_path, config) {
        return Ok(Marker::with_package_path(config.clone(), config_path));
    }

    let package = PackageJson::parse("{}").map_err(|error| PkgError::Cli(error.to_string()))?;
    Ok(Marker::new(package))
}

fn normalize_input_path(input: &Path) -> Result<PathBuf, PkgError> {
    let input = absolute_path(input)?;
    // DECISION: JS `pkg` collapses missing and inaccessible CLI inputs into a
    // user-facing "does not exist" error; keep that wording at the CLI planning
    // boundary while lower-level file operations still return structured IO.
    let metadata = fs::metadata(&input).map_err(|_source| {
        PkgError::Cli(format!("Input file does not exist: {}", input.display()))
    })?;
    if metadata.is_dir() {
        let package = input.join("package.json");
        fs::metadata(&package).map_err(|_source| {
            PkgError::Cli(format!("Input file does not exist: {}", package.display()))
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
        // DECISION: keep the package.json provenance in the missing-bin error
        // because the JS suite asserts that the path came from the `bin` field.
        fs::metadata(&entrypoint).map_err(|_source| {
            PkgError::Cli(format!(
                "Bin file does not exist (taken from package.json 'bin' property): {}",
                entrypoint.display()
            ))
        })?;
        return Ok(entrypoint);
    }
    Ok(input.to_path_buf())
}

fn immediate_package_dir(file: &Path) -> Option<PathBuf> {
    file.parent()
        .filter(|directory| directory.join("package.json").is_file())
        .map(Path::to_path_buf)
}

fn package_snapshot_base(package_dir: &Path, root: &Path) -> PathBuf {
    let mut base = PathBuf::new();
    for component in package_dir.components() {
        if component.as_os_str() == "node_modules" {
            return if base.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                base
            };
        }
        base.push(component.as_os_str());
    }

    package_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.to_path_buf())
}

fn file_input_snapshot_base(root: &Path) -> PathBuf {
    root.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.to_path_buf())
}

fn output_base(
    cli: &Cli,
    entrypoint: &Path,
    input_package: Option<&PackageJson>,
    config: Option<&PackageJson>,
    pkg_options: Option<&crate::config::PkgConfig>,
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
    // `pkg_options` already carries the JS resolveConfig precedence: an
    // external/discovered config's `pkg` wins over the package.json `pkg`.
    let configured_out_path = cli
        .out_path
        .clone()
        .or_else(|| {
            pkg_options
                .and_then(|pkg| pkg.output_path.as_ref())
                .map(PathBuf::from)
        })
        .unwrap_or_default();
    absolute_path(&configured_out_path.join(stem))
}

fn resolve_targets(
    cli: &Cli,
    pkg_options: Option<&crate::config::PkgConfig>,
    defaults: &TargetDefaults,
) -> Result<Vec<NodeTarget>, PkgError> {
    if let Some(targets) = cli.targets.as_deref()
        && !targets.is_empty()
    {
        return parse_targets(targets, defaults)
            .map(|parsed| parsed.targets)
            .map_err(|error| PkgError::Cli(error.to_string()));
    }

    // `pkg_options` already carries the JS resolveConfig precedence: an
    // external/discovered config's `pkg` wins over the package.json `pkg`.
    let json_targets = pkg_options.map(|pkg| &pkg.targets);
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

/// Fail planning when a Zstd build selects a target whose embedded Node cannot
/// decompress it.
///
/// The runtime prelude decompresses Zstd payloads through `zlib.zstdDecompress*`,
/// which Node.js added in 22.15. The packaged executable embeds its Node, so an
/// older target cannot be repaired after the fact -- it would always throw the
/// prelude's "requires Node.js >= 22.15" error at startup. Reject such plans
/// here instead of producing an unusable executable.
fn reject_zstd_incapable_targets(targets: &[NodeTarget]) -> Result<(), PkgError> {
    let unsupported = targets
        .iter()
        .filter(|target| !node_range_supports_zstd(&target.node_range))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(PkgError::Cli(format!(
            "Zstd compression requires Node.js >= 22.15 in the produced executable (zlib.zstdDecompress); unsupported target(s): {}. Use node22 or newer targets, or --compress Brotli/GZip.",
            unsupported.join(", ")
        )))
    }
}

/// Whether the pkg-fetch binary satisfying this node range ships
/// `zlib.zstdDecompress*` (Node.js >= 22.15). Ranges that resolve to no
/// supported version fail closed: such a target cannot be fetched anyway.
fn node_range_supports_zstd(node_range: &str) -> bool {
    const ZSTD_MIN_NODE: (u32, u32) = (22, 15);
    crate::fetch::satisfying_node_version(node_range).is_ok_and(|version| {
        let mut parts = version
            .split('.')
            .map(|part| part.parse::<u32>().unwrap_or(0));
        let major = parts.next().unwrap_or(0);
        let minor = parts.next().unwrap_or(0);
        (major, minor) >= ZSTD_MIN_NODE
    })
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

fn is_configuration(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "package.json")
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".config.json"))
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
