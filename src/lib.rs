//! Rust port of `pkg`.
//!
//! This crate is being ported from the JavaScript implementation in small,
//! parity-tested slices. The public API starts with typed equivalents of the
//! original CLI seams and grows only as each JS behavior is covered by Rust
//! tests.
//!
//! # Example
//!
//! ```
//! use pkg_rust::{Compression, StoreKind};
//!
//! # fn main() -> Result<(), pkg_rust::CompressionParseError> {
//! let compression: Compression = "gzip".parse()?;
//! assert_eq!(compression, Compression::Gzip);
//! assert_eq!(StoreKind::Blob.as_index(), 0);
//! # Ok(())
//! # }
//! ```

mod cli;
mod common;
mod compress;
mod error;

pub use crate::cli::exec;
pub use crate::common::{AliasKind, StoreKind};
pub use crate::compress::{Compression, CompressionParseError};
pub use crate::error::PkgError;
