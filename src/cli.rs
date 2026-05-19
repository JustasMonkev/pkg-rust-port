use std::ffi::OsString;
use std::path::PathBuf;

use clap::Parser;

use crate::error::PkgError;

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
    let args = std::iter::once(OsString::from("pkg")).chain(argv.into_iter().map(Into::into));
    let _cli = Cli::try_parse_from(args).map_err(|error| PkgError::Cli(error.to_string()))?;

    Err(PkgError::NotImplemented(
        "CLI skeleton is wired; port logic starts after parity tests",
    ))
}
