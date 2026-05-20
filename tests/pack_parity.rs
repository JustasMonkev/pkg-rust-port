#![allow(missing_docs)]

use std::fs;
use std::path::PathBuf;

use pkg_rust::{
    Marker, PackageJson, PathStyle, PkgError, StoreKind, WalkerParams, pack, refine_walked, walk,
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
    let refined = refine_walked(walked, &entrypoint, PathStyle::Posix);
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
    let stat_text = packed
        .stripes
        .iter()
        .find(|stripe| stripe.snap == "/test-x-index.js" && stripe.store == StoreKind::Stat)
        .and_then(|stripe| stripe.buffer.as_ref())
        .and_then(|buffer| std::str::from_utf8(buffer).ok())
        .ok_or_else(|| PkgError::Pack("test stat stripe missing".to_owned()))?;
    assert!(stat_text.contains(r#""isFileValue":true"#));
    assert!(stat_text.contains(r#""isDirectoryValue":false"#));
    assert!(stat_text.contains(r#""isSocketValue":false"#));
    assert!(stat_text.contains(r#""isSymbolicLinkValue":false"#));
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
    let refined = refine_walked(walked, &entrypoint, PathStyle::Posix);
    let error = pack(refined, false).err();

    assert!(matches!(error, Some(PkgError::Pack(message)) if message.contains("--no-bytecode")));
    Ok(())
}

#[test]
fn blob_and_content_records_keep_shebang_stripped_body() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = PathBuf::from("/private/tmp").join(format!(
        "pkg-rust-pack-shebang-public-{}",
        std::process::id()
    ));
    let package_dir = fixture_dir.join("node_modules/dep");
    let bin_dir = package_dir.join("bin");
    let _ignored = fs::remove_dir_all(&fixture_dir);
    fs::create_dir_all(&bin_dir)?;
    fs::write(fixture_dir.join("app.js"), "require('dep/bin/cmd.js');\n")?;
    fs::write(
        package_dir.join("package.json"),
        r#"{"name":"dep","main":"bin/cmd.js"}"#,
    )?;
    fs::write(
        bin_dir.join("cmd.js"),
        "#!/usr/bin/env node\nmodule.exports = 42;\n",
    )?;

    let entrypoint = fixture_dir.join("app.js");
    let walked = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new()
            .with_root(&fixture_dir)
            .with_public_packages(["dep"]),
    )?;
    let refined = refine_walked(walked, &entrypoint, PathStyle::Posix);
    let packed = pack(refined, true)?;
    let blob = packed
        .stripes
        .iter()
        .find(|stripe| {
            stripe.snap == "/node_modules/dep/bin/cmd.js" && stripe.store == StoreKind::Blob
        })
        .and_then(|stripe| stripe.buffer.as_ref())
        .ok_or_else(|| PkgError::Pack("test blob stripe missing".to_owned()))?;

    assert_eq!(blob, b"module.exports = 42;\n");

    fs::remove_dir_all(&fixture_dir)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn carries_walker_symlinks_into_packed_output() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        PathBuf::from("/private/tmp").join(format!("pkg-rust-pack-symlink-{}", std::process::id()));
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

    assert_eq!(packed.entrypoint, "/real.js");
    assert_eq!(
        packed.symlinks.get("/link.js").map(String::as_str),
        Some("/real.js")
    );

    fs::remove_dir_all(&fixture_dir)?;
    Ok(())
}
