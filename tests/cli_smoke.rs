#![allow(missing_docs)]

//! End-to-end CLI smoke coverage for cached target-binary packaging.

use std::fs;
use std::process::Command;

use pkg_rust::{BinaryKind, PkgFetchCache, TargetDefaults, parse_targets};

#[test]
fn cli_packages_with_cached_built_target_binary() -> Result<(), Box<dyn std::error::Error>> {
    let temp_root = std::env::temp_dir().join(format!("pkg-rust-cli-smoke-{}", std::process::id()));
    let cache_root = temp_root.join("cache");
    let output = temp_root.join("demo-bin");
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = "../test/test-50-require-resolve/test-x-index.js";
    let target = parse_targets("node18-macos-arm64", &TargetDefaults::host("node18"))?
        .targets
        .into_iter()
        .next()
        .ok_or_else(|| "target parser returned no targets".to_owned())?;
    let cache = PkgFetchCache::new(&cache_root);
    let built = cache.binary_path(&target, BinaryKind::Built)?;
    fs::create_dir_all(
        built
            .parent()
            .ok_or_else(|| "cache binary path has no parent".to_owned())?,
    )?;
    fs::write(&built, binary_with_placeholders())?;

    let output_result = Command::new(env!("CARGO_BIN_EXE_pkg"))
        .current_dir(manifest_dir)
        .env("PKG_CACHE_PATH", &cache_root)
        .arg("--target")
        .arg("node18-macos-arm64")
        .arg("--output")
        .arg(&output)
        .arg("--options")
        .arg("trace-warnings")
        .arg(input)
        .output()?;

    assert!(
        output_result.status.success(),
        "pkg CLI failed: {}{}",
        String::from_utf8_lossy(&output_result.stdout),
        String::from_utf8_lossy(&output_result.stderr)
    );

    let image = fs::read(&output)?;
    let image_text = String::from_utf8_lossy(&image);
    assert!(image_text.contains("--trace-warnings"));
    assert!(image_text.contains("test-x-index.js"));
    assert!(!image_text.contains("%VIRTUAL_FILESYSTEM%"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&output)?.permissions().mode() & 0o111;
        assert_eq!(mode, 0o111);
    }

    fs::remove_dir_all(temp_root)?;
    Ok(())
}

fn binary_with_placeholders() -> Vec<u8> {
    let mut binary = Vec::from([b'\0']);
    for _index in 0..20 {
        binary.extend_from_slice(b"// BAKERY ");
    }
    binary.extend_from_slice(b"// PAYLOAD_POSITION //");
    binary.extend_from_slice(b"// PAYLOAD_SIZE //");
    binary.extend_from_slice(b"// PRELUDE_POSITION //");
    binary.extend_from_slice(b"// PRELUDE_SIZE //");
    binary
}
