#![allow(missing_docs)]

use std::path::PathBuf;

use pkg_rust::{
    Marker, PackageJson, PathStyle, PkgError, StoreKind, SymlinkMap, WalkerParams, pack, refine,
    walk,
};

fn empty_marker() -> Result<Marker, PkgError> {
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    Ok(Marker::new(package))
}

#[test]
fn packs_content_links_and_stat_stripes() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");
    let walked = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;
    let refined = refine(walked, &entrypoint, SymlinkMap::new(), PathStyle::Posix);
    let packed = pack(refined, true)?;

    assert_eq!(packed.entrypoint, "/test-x-index.js");
    assert!(packed.stripes.iter().any(|stripe| {
        stripe.snap == "/test-z-require-content.css"
            && stripe.store == StoreKind::Content
            && stripe.buffer.is_some()
    }));
    assert!(packed.stripes.iter().any(|stripe| {
        stripe.snap == "/"
            && stripe.store == StoreKind::Links
            && stripe
                .buffer
                .as_ref()
                .is_some_and(|buffer| buffer == br#"["main.js","test-x-index.js","test-y-resolve.any","test-z-require-code-1.js","test-z-require-code-2.js","test-z-require-code-3.js","test-z-require-code-4.js","test-z-require-content.css","test-z-require-json-1.json","test-z-require-json-2.json","test-z-require-json-3.json","test-z-require-json-4.json","test-z-require-json-5.json"]"#)
    }));
    assert!(packed.stripes.iter().any(|stripe| {
        stripe.snap == "/test-x-index.js"
            && stripe.store == StoreKind::Stat
            && stripe.buffer.is_some()
    }));
    Ok(())
}

#[test]
fn no_bytecode_requires_content_for_blob_records() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");
    let walked = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;
    let refined = refine(walked, &entrypoint, SymlinkMap::new(), PathStyle::Posix);
    let error = pack(refined, false).err();

    assert!(matches!(error, Some(PkgError::Pack(message)) if message.contains("--no-bytecode")));
    Ok(())
}
