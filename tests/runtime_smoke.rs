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
    package_and_compare_fixture(
        "require-resolve",
        &fixture_dir,
        "test-x-index.js",
        "test-x-index.js",
    )
}

#[test]
fn filesystem_asset_fixture_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-fs-runtime-layer");
    package_and_compare_fixture("fs-runtime", &fixture_dir, "test-x-index.js", ".")
}

#[test]
fn spawn_fixtures_run_when_real_cache_is_configured() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-spawn");
    for input in [
        "test-cluster.js",
        "test-cpfork-a-1.js",
        "test-cpfork-a-2.js",
        "test-cpfork-b-1.js",
        "test-cpfork-b-2.js",
        "test-exec-1.js",
        "test-exec-2.js",
        "test-exec-3.js",
        "test-execFile.js",
        "test-execFileSync.js",
        "test-execSync-1.js",
        "test-execSync-2.js",
        "test-execSync-3.js",
        "test-node.js",
        "test-spawn-a-1.js",
        "test-spawn-a-2.js",
        "test-spawn-a-3.js",
        "test-spawn-a-4.js",
        "test-spawn-a-5.js",
        "test-spawn-b.js",
        "test-spawn-c.js",
        "test-spawn-d.js",
        "test-spawnSync.js",
    ] {
        let name = format!("spawn-{}", input.trim_end_matches(".js"));
        package_and_compare_fixture(&name, &fixture_dir, input, input)?;
    }
    Ok(())
}

#[test]
fn package_json_files_fixtures_run_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");
    for (name, fixture, node_input, package_input) in [
        (
            "package-json",
            "test-50-package-json",
            "test-x-index.js",
            ".",
        ),
        (
            "package-json-2",
            "test-50-package-json-2",
            "sub/test-x-index.js",
            ".",
        ),
        (
            "package-json-3",
            "test-50-package-json-3",
            "test-x-index.js",
            ".",
        ),
        (
            "package-json-4",
            "test-50-package-json-4",
            "test-x-index.js",
            "test-x-index.js",
        ),
        (
            "package-json-5",
            "test-50-package-json-5",
            "node_modules/input/test-x-index.js",
            "node_modules/input/test-x-index.js",
        ),
        (
            "package-json-6",
            "test-50-package-json-6",
            "test-x-index.js",
            "test-x-index.js",
        ),
        (
            "package-json-6b",
            "test-50-package-json-6b",
            "node_modules/alpha/alpha.js",
            "node_modules/alpha/alpha.js",
        ),
        (
            "package-json-6c",
            "test-50-package-json-6c",
            "beta/alpha.js",
            "beta/alpha.js",
        ),
        (
            "package-json-6d",
            "test-50-package-json-6d",
            "test-x-index.js",
            "test-x-index.js",
        ),
        (
            "package-json-7",
            "test-50-package-json-7",
            "test-x-index.js",
            ".",
        ),
        (
            "package-json-7p",
            "test-50-package-json-7p",
            "test-x-index.js",
            ".",
        ),
        (
            "package-json-8",
            "test-50-package-json-8",
            "sub/test-x-index.js",
            ".",
        ),
        (
            "package-json-8b",
            "test-50-package-json-8b",
            "sub/test-x-index.js",
            ".",
        ),
        (
            "package-json-8p",
            "test-50-package-json-8p",
            "sub/test-x-index.js",
            ".",
        ),
        (
            "package-json-9",
            "test-50-package-json-9",
            "test-x-index.js",
            "test-x-index.js",
        ),
        (
            "package-json-9p",
            "test-50-package-json-9p",
            "test-x-index.js",
            "test-x-index.js",
        ),
        (
            "package-json-A",
            "test-50-package-json-A",
            "test-x-index.js",
            ".",
        ),
    ] {
        package_and_compare_fixture(name, &root.join(fixture), node_input, package_input)?;
    }
    Ok(())
}

fn package_and_compare_fixture(
    name: &str,
    fixture_dir: &Path,
    node_input: &str,
    package_input: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = Command::new("node")
        .current_dir(fixture_dir)
        .arg(node_input)
        .output()?;
    assert!(
        expected.status.success(),
        "node oracle failed: {}{}",
        String::from_utf8_lossy(&expected.stdout),
        String::from_utf8_lossy(&expected.stderr)
    );

    let Some(run_result) = package_and_run_real_fixture(name, fixture_dir, package_input)? else {
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
