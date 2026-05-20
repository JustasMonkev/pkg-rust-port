#![allow(missing_docs)]

//! End-to-end CLI smoke coverage for cached target-binary packaging.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use pkg_rust::{BinaryKind, PkgFetchCache, TargetDefaults, parse_targets};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cli_packages_with_cached_built_target_binary() -> TestResult {
    let temp_root = temp_root("cached-built-binary")?;
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
        .arg("--no-signature")
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

#[test]
fn cli_reports_missing_dependency_main_warning_like_js_invalid_fixture() -> TestResult {
    let temp_root = temp_root("missing-dependency-main-warning")?;
    let cache_root = temp_root.join("cache");
    seed_cached_binary(&cache_root, "node18-macos-arm64")?;
    let output_path = temp_root.join("test-output.exe");
    let output_text = output_path
        .to_str()
        .ok_or_else(|| "temp output path is not valid utf-8".to_owned())?;
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/test-50-invalid-package-json-2");
    let output = run_cli_with_env(
        &fixture,
        [
            "--target",
            "node18-macos-arm64",
            "--no-signature",
            "--output",
            output_text,
            "./test-x-index.js",
        ],
        [("PKG_CACHE_PATH", cache_root.as_os_str())],
    )?;

    assert!(
        output.status.success(),
        "pkg CLI failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.stderr.is_empty(),
        "expected warning on stdout, got stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !stdout.as_bytes().contains(&0x1b),
        "stdout contains ANSI escape bytes: {stdout}"
    );
    assert!(stdout.contains("Warning"));
    assert!(stdout.contains("Entry 'main' not found"));
    assert!(stdout.contains("crusader/package.json"));

    fs::remove_dir_all(temp_root)?;
    Ok(())
}

#[test]
fn cli_reports_dictionary_config_log_like_js_fixture() -> TestResult {
    let temp_root = temp_root("dictionary-config-log")?;
    let cache_root = temp_root.join("cache");
    seed_cached_binary(&cache_root, "node18-macos-arm64")?;
    let output_path = temp_root.join("test-output.exe");
    let output_text = output_path
        .to_str()
        .ok_or_else(|| "temp output path is not valid utf-8".to_owned())?;
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-config-log");
    let output = run_cli_with_env(
        &fixture,
        [
            "--target",
            "node18-macos-arm64",
            "--no-signature",
            "--output",
            output_text,
            "./test-x-index.js",
        ],
        [("PKG_CACHE_PATH", cache_root.as_os_str())],
    )?;

    assert!(
        output.status.success(),
        "pkg CLI failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.stderr.is_empty(),
        "expected warning on stdout, got stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !stdout.as_bytes().contains(&0x1b),
        "stdout contains ANSI escape bytes: {stdout}"
    );
    assert!(stdout.contains("stylus options to resolve imports"));

    fs::remove_dir_all(temp_root)?;
    Ok(())
}

#[test]
fn cli_reports_may_exclude_debug_diagnostics_like_js_fixture() -> TestResult {
    let temp_root = temp_root("may-exclude-debug-diagnostics")?;
    let cache_root = temp_root.join("cache");
    seed_cached_binary(&cache_root, "node18-macos-arm64")?;
    let output_path = temp_root.join("test-output.exe");
    let output_text = output_path
        .to_str()
        .ok_or_else(|| "temp output path is not valid utf-8".to_owned())?;
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/test-50-may-exclude-must-exclude");
    let output = run_cli_with_env(
        &fixture,
        [
            "--debug",
            "--target",
            "node18-macos-arm64",
            "--no-signature",
            "--output",
            output_text,
            "./test-x-index.js",
        ],
        [("PKG_CACHE_PATH", cache_root.as_os_str())],
    )?;

    assert!(
        output.status.success(),
        "pkg CLI failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.as_bytes().contains(&0x1b),
        "stdout contains ANSI escape bytes: {stdout}"
    );
    let diagnostics = diagnostic_lines(&stdout);
    let first_diagnostics = diagnostics.iter().take(16).copied().collect::<Vec<_>>();
    assert_eq!(
        first_diagnostics,
        vec![
            "> Warning Cannot resolve 'reqResSomeVar'",
            "> [debug] Cannot resolve 'reqResSomeVarMay'",
            "> Warning Malformed requirement for 'reqResSomeVar'",
            "> Warning Malformed requirement for 'reqResSomeVar'",
            "> Warning Cannot resolve 'reqSomeVar'",
            "> [debug] Cannot resolve 'reqSomeVarMay'",
            "> Warning Malformed requirement for 'reqSomeVar'",
            "> Warning Malformed requirement for 'reqSomeVar'",
            "> [debug] Cannot resolve 'tryReqResSomeVar'",
            "> [debug] Cannot resolve 'tryReqResSomeVarMay'",
            "> [debug] Cannot resolve 'tryReqSomeVar'",
            "> [debug] Cannot resolve 'tryReqSomeVarMay'",
            "> [debug] Cannot find module 'reqResSomeLit'",
            "> [debug] Cannot find module 'reqResSomeLitMay'",
            "> [debug] Cannot find module 'reqSomeLit'",
            "> [debug] Cannot find module 'reqSomeLitMay'",
        ]
    );

    fs::remove_dir_all(temp_root)?;
    Ok(())
}

#[test]
fn cli_reports_missing_input_like_js_invalid_fixture() -> TestResult {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = run_cli(
        manifest_dir,
        [
            "--target",
            "node18-macos-arm64",
            "--output",
            "no-output",
            "12345",
        ],
    )?;

    assert_cli_error(&output, ["Error!", "does not exist", "12345"]);
    Ok(())
}

#[test]
fn cli_reports_missing_package_json_like_js_invalid_fixture() -> TestResult {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/test-50-invalid-package-json");
    let output = run_cli(
        &fixture,
        [
            "--target",
            "node18-macos-arm64",
            "--output",
            "no-output",
            ".",
        ],
    )?;

    assert_cli_error(&output, ["Error!", "does not exist", "package.json"]);
    Ok(())
}

#[test]
fn cli_reports_missing_package_bin_like_js_invalid_fixture() -> TestResult {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/test-50-invalid-package-json-bin");
    let output = run_cli(
        &fixture,
        [
            "--target",
            "node18-macos-arm64",
            "--output",
            "no-output",
            ".",
        ],
    )?;

    assert_cli_error(
        &output,
        ["Error!", "Property 'bin' does not exist", "package.json"],
    );
    Ok(())
}

#[test]
fn cli_reports_missing_package_bin_file_like_js_invalid_fixture() -> TestResult {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/test-50-invalid-package-json-bin-2");
    let output = run_cli(
        &fixture,
        [
            "--target",
            "node18-macos-arm64",
            "--output",
            "no-output",
            ".",
        ],
    )?;

    assert_cli_error(&output, ["Error!", "does not exist", "package.json"]);
    Ok(())
}

#[test]
fn cli_reports_unknown_target_token_like_js_invalid_fixture() -> TestResult {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/test-50-invalid-unknown-token");
    let output = run_cli(
        &fixture,
        [
            "--target",
            "node7-x6",
            "--output",
            "no-output",
            "test-x-index.js",
        ],
    )?;

    assert_cli_error(&output, ["Error!", "Unknown token", "node7-x6"]);
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

fn seed_cached_binary(cache_root: &Path, target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let target = parse_targets(target, &TargetDefaults::host("node18"))?
        .targets
        .into_iter()
        .next()
        .ok_or_else(|| "target parser returned no targets".to_owned())?;
    let cache = PkgFetchCache::new(cache_root);
    let built = cache.binary_path(&target, BinaryKind::Built)?;
    fs::create_dir_all(
        built
            .parent()
            .ok_or_else(|| "cache binary path has no parent".to_owned())?,
    )?;
    fs::write(&built, binary_with_placeholders())?;
    Ok(())
}

fn diagnostic_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.contains(" [debug] ") || line.contains(" Warning "))
        .collect()
}

fn temp_root(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "pkg-rust-cli-smoke-{name}-{}-{nanos}",
        std::process::id()
    )))
}

fn run_cli<I, S>(
    current_dir: &Path,
    args: I,
) -> Result<std::process::Output, Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Ok(Command::new(env!("CARGO_BIN_EXE_pkg"))
        .current_dir(current_dir)
        .args(args)
        .output()?)
}

fn run_cli_with_env<I, S, E, K, V>(
    current_dir: &Path,
    args: I,
    envs: E,
) -> Result<std::process::Output, Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
    E: IntoIterator<Item = (K, V)>,
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
{
    Ok(Command::new(env!("CARGO_BIN_EXE_pkg"))
        .current_dir(current_dir)
        .args(args)
        .envs(envs)
        .output()?)
}

fn assert_cli_error<const N: usize>(output: &std::process::Output, needles: [&str; N]) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "pkg CLI unexpectedly succeeded: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.stderr.is_empty(),
        "expected JS-style errors on stdout, got stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !stdout.as_bytes().contains(&0x1b),
        "stdout contains ANSI escape bytes: {stdout}"
    );
    for needle in needles {
        assert!(
            stdout.contains(needle),
            "stdout did not contain {needle:?}: {stdout}"
        );
    }
}
