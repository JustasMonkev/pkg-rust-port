#![allow(missing_docs)]

use std::fs;
use std::path::PathBuf;

use pkg_rust::{
    Compression, Marker, PackageJson, PathStyle, PkgError, PlaceholderKind, PlaceholderValues,
    StoreKind, WalkerParams, discover_placeholders, inject_placeholders, pack,
    produce_executable_image, produce_manifest, refine_walked, render_prelude, walk,
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
fn gzip_manifest_compresses_payload_accounting_and_vfs_keys() -> Result<(), PkgError> {
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
    let manifest = produce_manifest(packed, Compression::Gzip, PathStyle::Posix)?;

    assert_eq!(manifest.compression, Compression::Gzip);
    assert_eq!(manifest.entrypoint, "/snapshot/test-z-require-content.css");
    assert_eq!(
        manifest.path_dictionary.get("").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        manifest.path_dictionary.get("snapshot").map(String::as_str),
        Some("1")
    );
    assert!(manifest.vfs.keys().any(|key| key.starts_with("0/1/")));
    assert!(manifest.payload_size > 0);
    Ok(())
}

#[test]
fn brotli_manifest_compresses_payload_accounting() -> Result<(), PkgError> {
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
    let manifest = produce_manifest(packed, Compression::Brotli, PathStyle::Posix)?;

    assert_eq!(manifest.compression, Compression::Brotli);
    assert!(manifest.payload_size > 0);
    assert!(manifest.vfs.keys().any(|key| key.starts_with("0/1/")));
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

#[test]
fn renders_compressed_prelude_dictionary() -> Result<(), PkgError> {
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
    let manifest = produce_manifest(packed, Compression::Gzip, PathStyle::Posix)?;
    let rendered = render_prelude("%VIRTUAL_FILESYSTEM%\n%DICT%\n%DOCOMPRESS%", &manifest)?;

    assert!(rendered.contains(r#""":"0""#));
    assert!(rendered.contains(r#""snapshot":"1""#));
    assert!(rendered.ends_with('1'));
    Ok(())
}

#[test]
fn discovers_and_injects_binary_placeholders() -> Result<(), PkgError> {
    let mut binary = Vec::new();
    binary.extend_from_slice(b"prefix");
    binary.extend_from_slice(&bakery_placeholder());
    binary.extend_from_slice(b"// PAYLOAD_POSITION //");
    binary.extend_from_slice(b"// PAYLOAD_SIZE //");
    binary.extend_from_slice(b"// PRELUDE_POSITION //");
    binary.extend_from_slice(b"// PRELUDE_SIZE //");

    let placeholders = discover_placeholders(&binary);
    assert!(placeholders.bakery.is_some());
    assert!(placeholders.payload_position.is_some());
    assert!(placeholders.payload_size.is_some());
    assert!(placeholders.prelude_position.is_some());
    assert!(placeholders.prelude_size.is_some());

    let values = PlaceholderValues {
        bakery: b"--trace".to_vec(),
        payload_position: 123,
        payload_size: 456,
        prelude_position: 789,
        prelude_size: 10,
    };
    inject_placeholders(
        &mut binary,
        &placeholders,
        &values,
        &[
            PlaceholderKind::Bakery,
            PlaceholderKind::PayloadPosition,
            PlaceholderKind::PayloadSize,
            PlaceholderKind::PreludePosition,
            PlaceholderKind::PreludeSize,
        ],
    )?;

    let text = String::from_utf8_lossy(&binary);
    assert!(text.contains("--trace"));
    assert!(text.contains("123"));
    assert!(text.contains("456"));
    assert!(text.contains("789"));
    assert!(text.contains("10"));
    Ok(())
}

#[test]
fn injection_errors_when_placeholder_is_missing() {
    let mut binary = b"no placeholders".to_vec();
    let placeholders = discover_placeholders(&binary);
    let values = PlaceholderValues {
        bakery: Vec::new(),
        payload_position: 1,
        payload_size: 2,
        prelude_position: 3,
        prelude_size: 4,
    };
    let error = inject_placeholders(
        &mut binary,
        &placeholders,
        &values,
        &[PlaceholderKind::PayloadSize],
    )
    .err();

    assert!(matches!(error, Some(PkgError::Pack(message)) if message.contains("was not found")));
}

#[test]
fn produces_executable_image_and_injects_layout_placeholders() -> Result<(), PkgError> {
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
    let binary = binary_with_placeholders();
    let binary_len = binary.len();
    let produced = produce_executable_image(
        binary,
        packed,
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
        Compression::None,
        PathStyle::Posix,
        b"--trace".to_vec(),
    )?;

    assert_eq!(produced.payload_position, binary_len as u64);
    assert_eq!(
        produced.prelude_position,
        produced.payload_position + produced.manifest.payload_size
    );
    assert_eq!(
        produced.bytes.len() as u64,
        produced.prelude_position + produced.prelude_size
    );
    let binary_text = String::from_utf8_lossy(&produced.bytes[..binary_len]);
    assert!(binary_text.contains("--trace"));
    assert!(binary_text.contains(&produced.payload_position.to_string()));
    assert!(binary_text.contains(&produced.manifest.payload_size.to_string()));
    assert!(binary_text.contains(&produced.prelude_position.to_string()));
    assert!(binary_text.contains(&produced.prelude_size.to_string()));
    Ok(())
}

#[test]
fn produced_image_errors_when_required_placeholder_is_missing() -> Result<(), PkgError> {
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
    let error = produce_executable_image(
        b"no placeholders".to_vec(),
        packed,
        "%VIRTUAL_FILESYSTEM%",
        Compression::None,
        PathStyle::Posix,
        Vec::new(),
    )
    .err();

    assert!(matches!(error, Some(PkgError::Pack(message)) if message.contains("was not found")));
    Ok(())
}

fn binary_with_placeholders() -> Vec<u8> {
    let mut binary = Vec::from(&b"prefix"[..]);
    binary.extend_from_slice(&bakery_placeholder());
    binary.extend_from_slice(b"// PAYLOAD_POSITION //");
    binary.extend_from_slice(b"// PAYLOAD_SIZE //");
    binary.extend_from_slice(b"// PRELUDE_POSITION //");
    binary.extend_from_slice(b"// PRELUDE_SIZE //");
    binary
}

fn bakery_placeholder() -> Vec<u8> {
    let mut value = Vec::from([b'\0']);
    for _index in 0..20 {
        value.extend_from_slice(b"// BAKERY ");
    }
    value
}
