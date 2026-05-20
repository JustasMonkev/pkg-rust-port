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
fn filesystem_write_guard_fixture_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-fs-runtime-layer-3");
    let Some(run_result) =
        package_and_run_real_fixture("fs-runtime-layer-3", &fixture_dir, "test-x-index.js")?
    else {
        return Ok(());
    };

    assert_eq!(
        String::from_utf8_lossy(&run_result.stdout),
        "true\nfalse\nCannot write to packaged file\ntrue\nclosed\nfalse\nCannot write to packaged file\nCannot write to packaged file\nundefined\nCannot write to packaged file\nundefined\n"
    );
    Ok(())
}

#[test]
fn filesystem_runtime_layer_2_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-fs-runtime-layer-2");
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
        package_and_run_real_fixture("fs-runtime-layer-2", &fixture_dir, "test-x-index.js")?
    else {
        return Ok(());
    };
    assert_stdout_lines_match_with_range_normalization(&expected.stdout, &run_result.stdout)?;
    Ok(())
}

#[test]
fn arguments_fixture_runs_when_real_cache_is_configured() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-arguments");
    for (name, arg, expected) in [
        ("arguments-number", "42", "42\n"),
        ("arguments-short-flag", "-ft", "-ft\n"),
        ("arguments-long-flag", "--fourty-two", "--fourty-two\n"),
    ] {
        let Some(run_result) =
            package_and_run_real_fixture_with_args(name, &fixture_dir, "test-x-index.js", &[arg])?
        else {
            return Ok(());
        };
        assert_eq!(String::from_utf8_lossy(&run_result.stdout), expected);
    }
    Ok(())
}

#[test]
fn modern_js_runtime_fixtures_run_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");
    for (name, fixture) in [
        ("class-to-string", "test-50-class-to-string"),
        ("object-spread", "test-50-object-spread"),
        ("for-await-of", "test-50-for-await-of"),
        ("non-ascii", "test-50-non-ascii"),
    ] {
        package_and_compare_fixture(
            name,
            &root.join(fixture),
            "test-x-index.js",
            "test-x-index.js",
        )?;
    }
    Ok(())
}

#[test]
fn path_and_resolution_runtime_fixtures_run_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");
    for (name, fixture, package_input) in [
        (
            "path-as-buffer",
            "test-50-path-as-buffer",
            "test-x-index.js",
        ),
        ("path-separators", "test-50-path-separators", "."),
        ("module-parent", "test-50-module-parent", "test-x-index.js"),
        (
            "resolve-and-nearby",
            "test-50-resolve-and-nearby",
            "test-x-index.js",
        ),
    ] {
        package_and_compare_fixture(name, &root.join(fixture), "test-x-index.js", package_input)?;
    }
    Ok(())
}

#[test]
fn require_edge_cases_fixture_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-require-edge-cases");
    package_and_compare_fixture(
        "require-edge-cases",
        &fixture_dir,
        "test-x-index.js",
        "test-x-index.js",
    )
}

#[test]
fn require_with_config_fixture_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/test-50-require-with-config");
    package_and_compare_fixture("require-with-config", &fixture_dir, "test-x-index.js", ".")
}

#[test]
fn chdir_env_var_fixture_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-chdir-env-var");
    let Some(run_result) = package_and_run_real_fixture_with_args_and_package_env(
        "chdir-env-var",
        &fixture_dir,
        "test-x-index.js",
        &[],
        &[("CHDIR", "source")],
    )?
    else {
        return Ok(());
    };
    assert_eq!(String::from_utf8_lossy(&run_result.stdout), "ok\n");
    Ok(())
}

#[test]
fn console_trace_fixture_reports_packaged_stack_paths() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-console-trace");
    let Some(run_result) =
        package_and_run_real_fixture("console-trace", &fixture_dir, "test-x-index.js")?
    else {
        return Ok(());
    };

    let stderr = String::from_utf8_lossy(&run_result.stderr);
    let lines = stderr.split('\n').collect::<Vec<_>>();
    let first_line = lines
        .first()
        .ok_or_else(|| "console trace stderr was empty".to_owned())?;
    let frame_file = extract_stack_file_name(
        lines
            .get(2)
            .ok_or_else(|| format!("missing console trace frame line: {stderr}"))?,
    )
    .ok_or_else(|| format!("could not parse console trace frame line: {stderr}"))?;
    let prelude_file = extract_stack_file_name(
        lines
            .get(3)
            .ok_or_else(|| format!("missing console trace prelude frame line: {stderr}"))?,
    )
    .ok_or_else(|| format!("could not parse console trace prelude frame line: {stderr}"))?;
    assert_eq!(*first_line, frame_file);
    assert_eq!(prelude_file, "pkg/prelude/bootstrap.js");
    Ok(())
}

#[test]
fn error_source_position_fixture_reports_original_pointer() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/test-50-error-source-position");
    let Some(run_result) = package_and_run_real_fixture_with_options(
        "error-source-position",
        &fixture_dir,
        "test-x-index.js",
        RealFixtureOptions {
            package_args: &["--public"],
            run_expectation: RunExpectation::Failure,
            ..RealFixtureOptions::success()
        },
    )?
    else {
        return Ok(());
    };

    let stderr = String::from_utf8_lossy(&run_result.stderr);
    assert!(
        stderr.contains("x.parse is not a function"),
        "missing source error message: {stderr}"
    );
    let error_pointer = format!("x.parse();{}  ^", if cfg!(windows) { "\r\n" } else { "\n" });
    assert!(
        stderr.contains(&error_pointer),
        "missing source error pointer: {stderr}"
    );
    Ok(())
}

#[test]
fn may_exclude_fixture_runs_when_real_cache_is_configured() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-may-exclude");
    package_and_compare_fixture(
        "may-exclude",
        &fixture_dir,
        "test-x-index.js",
        "test-x-index.js",
    )
}

#[test]
fn not_found_wording_fixtures_run_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");

    let first_fixture = root.join("test-50-not-found-wording");
    let Some(run_result) =
        package_and_run_real_fixture("not-found-wording", &first_fixture, "test-x-index.js")?
    else {
        return Ok(());
    };
    let stdout = String::from_utf8_lossy(&run_result.stdout);
    let mut parts = stdout.split("*****");
    let fs_message = parts
        .next()
        .ok_or_else(|| "missing filesystem error section".to_owned())?;
    let require_message = parts
        .next()
        .ok_or_else(|| "missing require error section".to_owned())?;
    assert!(fs_message.contains("was not included into executable at compilation stage"));
    assert!(require_message.contains("you want to compile the package"));

    let second_fixture = root.join("test-50-not-found-wording-2");
    package_and_compare_fixture(
        "not-found-wording-2",
        &second_fixture,
        "test-x-index.js",
        "test-x-index.js",
    )?;
    Ok(())
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
fn native_addon_fixtures_run_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");
    for (name, fixture, input) in [
        ("native-addon", "test-50-native-addon", "test-x-index.js"),
        (
            "native-addon-2",
            "test-50-native-addon-2",
            "test-x-index.js",
        ),
        (
            "native-addon-3-x",
            "test-50-native-addon-3",
            "lib/test-x-index.js",
        ),
        (
            "native-addon-3-y",
            "test-50-native-addon-3",
            "lib/community/test-y-index.js",
        ),
        (
            "native-addon-3-z",
            "test-50-native-addon-3",
            "lib/enterprise/test-z-index.js",
        ),
        (
            "native-addon-4",
            "test-50-native-addon-4",
            "test-x-index.js",
        ),
    ] {
        package_and_compare_fixture(name, &root.join(fixture), input, input)?;
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

fn assert_stdout_lines_match_with_range_normalization(
    expected: &[u8],
    actual: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = String::from_utf8_lossy(expected);
    let actual = String::from_utf8_lossy(actual);
    let expected_lines = expected.split('\n').collect::<Vec<_>>();
    let actual_lines = actual.split('\n').collect::<Vec<_>>();

    for (index, expected_line) in expected_lines.iter().enumerate() {
        let actual_line = actual_lines
            .get(index)
            .ok_or_else(|| format!("actual stdout ended before line {index}: {actual}"))?;
        let expected_line = normalize_out_of_range_line(expected_line, Some(actual_line));
        let actual_line = normalize_out_of_range_line(actual_line, expected_line.as_deref());
        assert_eq!(
            expected_line.as_deref().unwrap_or(expected_lines[index]),
            actual_line.as_deref().unwrap_or(actual_lines[index]),
            "stdout mismatch at line {index}"
        );
    }
    Ok(())
}

fn normalize_out_of_range_line(line: &str, other_line: Option<&str>) -> Option<String> {
    if !line.contains("is out of range")
        || !other_line.is_some_and(|other| other.contains("is out of range"))
    {
        return None;
    }

    let start = line.find(" It must be ")?;
    let end = line[start..].find(". ")? + start + 2;
    let mut normalized = String::new();
    normalized.push_str(&line[..start]);
    normalized.push(' ');
    normalized.push_str(&line[end..]);
    Some(normalized)
}

fn extract_stack_file_name(line: &str) -> Option<&str> {
    let mut end = line.rfind(')')?;
    let mut start = line[..end].rfind('(')? + 1;
    if let Some(line_end) = line[start..end].rfind(':') {
        end = start + line_end;
        if let Some(column_end) = line[start..end].rfind(':') {
            end = start + column_end;
        }
    }
    while line
        .as_bytes()
        .get(start)
        .is_some_and(u8::is_ascii_whitespace)
    {
        start += 1;
    }
    Some(&line[start..end])
}

fn package_and_run_real_fixture(
    name: &str,
    fixture_dir: &Path,
    input: &str,
) -> Result<Option<std::process::Output>, Box<dyn std::error::Error>> {
    package_and_run_real_fixture_with_options(
        name,
        fixture_dir,
        input,
        RealFixtureOptions::success(),
    )
}

fn package_and_run_real_fixture_with_args(
    name: &str,
    fixture_dir: &Path,
    input: &str,
    run_args: &[&str],
) -> Result<Option<std::process::Output>, Box<dyn std::error::Error>> {
    package_and_run_real_fixture_with_options(
        name,
        fixture_dir,
        input,
        RealFixtureOptions {
            run_args,
            ..RealFixtureOptions::success()
        },
    )
}

fn package_and_run_real_fixture_with_args_and_package_env(
    name: &str,
    fixture_dir: &Path,
    input: &str,
    run_args: &[&str],
    package_env: &[(&str, &str)],
) -> Result<Option<std::process::Output>, Box<dyn std::error::Error>> {
    package_and_run_real_fixture_with_options(
        name,
        fixture_dir,
        input,
        RealFixtureOptions {
            run_args,
            package_env,
            ..RealFixtureOptions::success()
        },
    )
}

fn package_and_run_real_fixture_with_options(
    name: &str,
    fixture_dir: &Path,
    input: &str,
    options: RealFixtureOptions<'_>,
) -> Result<Option<std::process::Output>, Box<dyn std::error::Error>> {
    let Some(cache_root) = std::env::var_os("PKG_RUST_REAL_CACHE") else {
        eprintln!("skipping real runtime smoke: PKG_RUST_REAL_CACHE is not set");
        return Ok(None);
    };

    let output = real_output_path(name);
    let package_result = Command::new(env!("CARGO_BIN_EXE_pkg"))
        .current_dir(fixture_dir)
        .env("PKG_CACHE_PATH", cache_root)
        .envs(options.package_env.iter().copied())
        .args(options.package_args)
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

    let run_result = Command::new(&output)
        .current_dir(fixture_dir)
        .args(options.run_args)
        .output()?;
    fs::remove_file(output)?;
    match options.run_expectation {
        RunExpectation::Success => assert!(
            run_result.status.success(),
            "produced executable failed: {}{}",
            String::from_utf8_lossy(&run_result.stdout),
            String::from_utf8_lossy(&run_result.stderr)
        ),
        RunExpectation::Failure => assert!(
            !run_result.status.success(),
            "produced executable succeeded unexpectedly: {}{}",
            String::from_utf8_lossy(&run_result.stdout),
            String::from_utf8_lossy(&run_result.stderr)
        ),
    }
    Ok(Some(run_result))
}

struct RealFixtureOptions<'a> {
    package_args: &'a [&'a str],
    package_env: &'a [(&'a str, &'a str)],
    run_args: &'a [&'a str],
    run_expectation: RunExpectation,
}

impl RealFixtureOptions<'_> {
    fn success() -> Self {
        Self {
            package_args: &[],
            package_env: &[],
            run_args: &[],
            run_expectation: RunExpectation::Success,
        }
    }
}

enum RunExpectation {
    Success,
    Failure,
}

fn real_output_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pkg-rust-real-{name}-{}", std::process::id()))
}
