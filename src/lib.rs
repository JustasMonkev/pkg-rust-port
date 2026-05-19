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
mod config;
mod detect;
mod dictionary;
mod error;
mod refine;
mod resolve;
mod target;
mod walk;

pub use crate::cli::exec;
pub use crate::common::{
    AliasKind, PathStyle, StoreKind, inside_snapshot, normalize_path_text, remove_uplevels,
    retrieve_denominator, snapshotify, strip_snapshot, substitute_denominator,
};
pub use crate::compress::{Compression, CompressionParseError};
pub use crate::config::{BinField, PackageJson, PackageJsonError, PkgConfig};
pub use crate::detect::{
    Derivative, DetectedUse, DetectionKind, detect, non_literal_and_cwd_debug_lines,
    successful_debug_lines,
};
pub use crate::dictionary::{
    DictionaryDependency, DictionaryEntry, active_dependencies, apply_dictionary_entry,
    lookup_dictionary,
};
pub use crate::error::PkgError;
pub use crate::refine::{RefinedOutput, SymlinkMap, refine};
pub use crate::resolve::{ResolveOptions, resolve_module};
pub use crate::target::{
    Arch, NodeTarget, ParsedTargets, Platform, TargetDefaults, TargetParseError, output_names,
    parse_targets,
};
pub use crate::walk::{
    FileRecord, FileStat, Marker, WalkOutput, WalkTaskRecord, WalkerParams, walk,
};
