#![allow(missing_docs)]

//! Runtime smoke tests that need a real pkg-fetch target binary.

use std::fs;
use std::process::Command;

#[test]
fn js_api_happy_path_demo_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(cache_root) = std::env::var_os("PKG_RUST_REAL_CACHE") else {
        eprintln!("skipping real runtime smoke: PKG_RUST_REAL_CACHE is not set");
        return Ok(());
    };

    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-api");
    let output = std::env::temp_dir().join(format!(
        "pkg-rust-real-runtime-smoke-{}",
        std::process::id()
    ));
    let package_result = Command::new(env!("CARGO_BIN_EXE_pkg"))
        .current_dir(&fixture_dir)
        .env("PKG_CACHE_PATH", cache_root)
        .arg("--target")
        .arg("node18-macos-x64")
        .arg("--output")
        .arg(&output)
        .arg("test-x-index.js")
        .output()?;
    assert!(
        package_result.status.success(),
        "pkg CLI failed: {}{}",
        String::from_utf8_lossy(&package_result.stdout),
        String::from_utf8_lossy(&package_result.stderr)
    );

    let run_result = Command::new(&output).current_dir(&fixture_dir).output()?;
    assert!(
        run_result.status.success(),
        "produced executable failed: {}{}",
        String::from_utf8_lossy(&run_result.stdout),
        String::from_utf8_lossy(&run_result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_result.stdout), "42\n");

    fs::remove_file(output)?;
    Ok(())
}
