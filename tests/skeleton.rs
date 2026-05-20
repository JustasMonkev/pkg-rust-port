#![allow(missing_docs)]

use std::str::FromStr;

use pkg_rust::{AliasKind, Compression, StoreKind};

#[test]
fn store_kind_indices_match_js_constants() {
    assert_eq!(StoreKind::Blob.as_index(), 0);
    assert_eq!(StoreKind::Content.as_index(), 1);
    assert_eq!(StoreKind::Links.as_index(), 2);
    assert_eq!(StoreKind::Stat.as_index(), 3);
}

#[test]
fn alias_kind_indices_match_js_constants() {
    assert_eq!(AliasKind::Relative.as_index(), 0);
    assert_eq!(AliasKind::Resolvable.as_index(), 1);
}

#[test]
fn compression_accepts_original_cli_aliases() {
    assert_eq!(Compression::from_str("None"), Ok(Compression::None));
    assert_eq!(Compression::from_str("gzip"), Ok(Compression::Gzip));
    assert_eq!(Compression::from_str("gz"), Ok(Compression::Gzip));
    assert_eq!(Compression::from_str("brotli"), Ok(Compression::Brotli));
    assert_eq!(Compression::from_str("br"), Ok(Compression::Brotli));
    assert_eq!(Compression::Gzip.cli_label(), "GZip");
    assert_eq!(Compression::Brotli.cli_label(), "Brotli");
}
