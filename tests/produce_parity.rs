#![allow(missing_docs)]

use std::fs;
use std::path::PathBuf;

use pkg_rust::{
    Compression, Marker, PackageJson, PathStyle, PkgError, StoreKind, WalkerParams, pack,
    produce_manifest, refine_walked, render_prelude, walk,
};

fn empty_marker() -> Result<Marker, PkgError> {
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    Ok(Marker::new(package))
}

#[test]
fn builds_uncompressed_vfs_manifest_from_packed_stripes() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");
    let walked = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;
    let refined = refine_walked(walked, &entrypoint, PathStyle::Posix);
    let packed = pack(refined, true)?;
    let manifest = produce_manifest(packed, Compression::None, PathStyle::Posix)?;

    assert_eq!(manifest.entrypoint, "/snapshot/test-x-index.js");
    assert!(manifest.payload_size > 0);
    let content_pointer = manifest
        .vfs
        .get("/snapshot/test-z-require-content.css")
        .and_then(|stores| stores.get(&StoreKind::Content.as_index()));
    assert!(content_pointer.is_some_and(|pointer| pointer.size > 0));
    let stat_pointer = manifest
        .vfs
        .get("/snapshot/test-x-index.js")
        .and_then(|stores| stores.get(&StoreKind::Stat.as_index()));
    assert!(stat_pointer.is_some_and(|pointer| pointer.size > 0));
    Ok(())
}

#[cfg(unix)]
#[test]
fn snapshotifies_symlinks_in_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = PathBuf::from("/private/tmp")
        .join(format!("pkg-rust-produce-symlink-{}", std::process::id()));
    let real_file = fixture_dir.join("real.js");
    let link_file = fixture_dir.join("link.js");
    let _ignored = fs::remove_dir_all(&fixture_dir);
    fs::create_dir_all(&fixture_dir)?;
    fs::write(&real_file, "'use strict';\nmodule.exports = 1;\n")?;
    std::os::unix::fs::symlink(&real_file, &link_file)?;

    let walked = walk(
        empty_marker()?,
        &link_file,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;
    let refined = refine_walked(walked, &link_file, PathStyle::Posix);
    let packed = pack(refined, true)?;
    let manifest = produce_manifest(packed, Compression::None, PathStyle::Posix)?;

    assert_eq!(manifest.entrypoint, "/snapshot/real.js");
    assert_eq!(
        manifest
            .symlinks
            .get("/snapshot/link.js")
            .map(String::as_str),
        Some("/snapshot/real.js")
    );

    fs::remove_dir_all(&fixture_dir)?;
    Ok(())
}

#[test]
fn compressed_manifest_is_explicitly_not_implemented_yet() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-z-require-content.css");
    let walked = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;
    let refined = refine_walked(walked, &entrypoint, PathStyle::Posix);
    let packed = pack(refined, true)?;
    let error = produce_manifest(packed, Compression::Gzip, PathStyle::Posix).err();

    assert!(
        matches!(error, Some(PkgError::NotImplemented(message)) if message.contains("compressed producer"))
    );
    Ok(())
}

#[test]
fn renders_prelude_placeholders_from_manifest() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");
    let walked = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;
    let refined = refine_walked(walked, &entrypoint, PathStyle::Posix);
    let packed = pack(refined, true)?;
    let manifest = produce_manifest(packed, Compression::None, PathStyle::Posix)?;
    let rendered = render_prelude(
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
        &manifest,
    )?;

    assert!(rendered.contains(r#""/snapshot/test-x-index.js""#));
    assert!(rendered.contains(r#""0":["#));
    assert!(rendered.contains(r#""3":["#));
    assert!(rendered.contains(r#""/snapshot/test-x-index.js""#));
    assert!(rendered.contains("{}"));
    assert!(!rendered.ends_with('\n'));
    assert!(rendered.ends_with('0'));
    Ok(())
}
