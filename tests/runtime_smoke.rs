#![allow(missing_docs)]

//! Runtime smoke tests that need a real pkg-fetch target binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn js_api_happy_path_demo_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-api");
    let Some(run_result) = package_and_run_real_fixture("api", &fixture_dir, "test-x-index.js")?
    else {
        return Ok(());
    };
    assert_eq!(String::from_utf8_lossy(&run_result.stdout), "42\n");
    Ok(())
}

#[test]
fn require_resolve_fixture_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-require-resolve");
    let expected = Command::new("node")
        .current_dir(&fixture_dir)
        .arg("test-x-index.js")
        .output()?;
    assert!(
        expected.status.success(),
        "node oracle failed: {}{}",
        String::from_utf8_lossy(&expected.stdout),
        String::from_utf8_lossy(&expected.stderr)
    );

    let Some(run_result) =
        package_and_run_real_fixture("require-resolve", &fixture_dir, "test-x-index.js")?
    else {
        return Ok(());
    };
    assert_eq!(run_result.stdout, expected.stdout);
    Ok(())
}

#[test]
fn filesystem_asset_fixture_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-fs-runtime-layer");
    let expected = Command::new("node")
        .current_dir(&fixture_dir)
        .arg("test-x-index.js")
        .output()?;
    assert!(
        expected.status.success(),
        "node oracle failed: {}{}",
        String::from_utf8_lossy(&expected.stdout),
        String::from_utf8_lossy(&expected.stderr)
    );

    let Some(run_result) = package_and_run_real_fixture("fs-runtime", &fixture_dir, ".")? else {
        return Ok(());
    };
    assert_eq!(run_result.stdout, expected.stdout);
    Ok(())
}

fn package_and_run_real_fixture(
    name: &str,
    fixture_dir: &Path,
    input: &str,
) -> Result<Option<std::process::Output>, Box<dyn std::error::Error>> {
    let Some(cache_root) = std::env::var_os("PKG_RUST_REAL_CACHE") else {
        eprintln!("skipping real runtime smoke: PKG_RUST_REAL_CACHE is not set");
        return Ok(None);
    };

    let output = real_output_path(name);
    let package_result = Command::new(env!("CARGO_BIN_EXE_pkg"))
        .current_dir(fixture_dir)
        .env("PKG_CACHE_PATH", cache_root)
        .arg("--target")
        .arg("node18-macos-x64")
        .arg("--output")
        .arg(&output)
        .arg(input)
        .output()?;
    assert!(
        package_result.status.success(),
        "pkg CLI failed: {}{}",
        String::from_utf8_lossy(&package_result.stdout),
        String::from_utf8_lossy(&package_result.stderr)
    );

    let run_result = Command::new(&output).current_dir(fixture_dir).output()?;
    fs::remove_file(output)?;
    assert!(
        run_result.status.success(),
        "produced executable failed: {}{}",
        String::from_utf8_lossy(&run_result.stdout),
        String::from_utf8_lossy(&run_result.stderr)
    );
    Ok(Some(run_result))
}

fn real_output_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pkg-rust-real-{name}-{}", std::process::id()))
}
