use thiserror::Error;

/// Error type for library operations in the Rust port.
#[derive(Debug, Error)]
pub enum PkgError {
    /// Command-line parsing failed.
    #[error("{0}")]
    Cli(String),

    /// JavaScript parsing failed.
    #[error("javascript parse failed: {0}")]
    JavaScriptParse(String),

    /// Module resolution failed.
    #[error("module resolution failed: {0}")]
    Resolve(String),

    /// A requested behavior has not been ported yet.
    #[error("{0}")]
    NotImplemented(&'static str),
}
