#![allow(missing_docs)]

//! Runtime smoke tests that need a real pkg-fetch target binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use pkg_rust::{PkgFetchCache, TargetBinaryProvider, TargetDefaults, parse_targets};

const DEFAULT_REAL_TARGET: &str = "node18-macos-x64";
const TARGET_NODE_ORACLE_WRAPPER: &str = "const input = process.env.PKG_RUST_ORACLE_INPUT; process.argv = [process.execPath, input]; require(input);";

#[test]
fn js_api_happy_path_demo_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-api");
    let Some(run_result) = package_and_run_real_fixture("api", &fixture_dir, "test-x-index.js")?
    else {
        return Ok(());
    };
    assert_eq!(String::from_utf8_lossy(&run_result.run.stdout), "42\n");
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
        String::from_utf8_lossy(&run_result.run.stdout),
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
    assert_stdout_lines_match_with_range_normalization(&expected.stdout, &run_result.run.stdout)?;
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
        assert_eq!(String::from_utf8_lossy(&run_result.run.stdout), expected);
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
fn global_object_fixture_runs_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-global-object");
    package_and_compare_fixture("global-object", &fixture_dir, "test-x-index.js", ".")
}

#[test]
fn promisify_fixture_runs_when_real_cache_is_configured() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-promisify");
    package_and_compare_fixture(
        "promisify",
        &fixture_dir,
        "test-x-index.js",
        "test-x-index.js",
    )
}

#[test]
fn compression_fixture_runs_when_real_cache_is_configured() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-80-compression");
    for (name, algorithm, cli_label) in [
        ("compression-none", "None", None),
        ("compression-gzip", "GZip", Some("compression:  GZip")),
        ("compression-brotli", "Brotli", Some("compression:  Brotli")),
    ] {
        let Some(package_run) = package_and_run_real_fixture_with_options(
            name,
            &fixture_dir,
            "test-x.js",
            RealFixtureOptions {
                package_args: &["--compress", algorithm],
                ..RealFixtureOptions::success()
            },
        )?
        else {
            return Ok(());
        };
        assert_eq!(String::from_utf8_lossy(&package_run.run.stdout), "42\n");
        if let Some(label) = cli_label {
            assert!(
                String::from_utf8_lossy(&package_run.package.stdout).contains(label),
                "package stdout did not contain {label:?}: {}",
                String::from_utf8_lossy(&package_run.package.stdout)
            );
        }
    }
    Ok(())
}

#[test]
fn mountpoint_fixtures_run_when_real_cache_is_configured() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");

    let Some(mountpoints) = package_and_run_real_fixture_with_options(
        "mountpoints",
        &root.join("test-50-mountpoints"),
        "test-x-index.js",
        RealFixtureOptions {
            run_from_output_dir: true,
            prepare_output_dir: Some(copy_plugins_d_ext),
            ..RealFixtureOptions::success()
        },
    )?
    else {
        return Ok(());
    };
    assert_eq!(
        String::from_utf8_lossy(&mountpoints.run.stdout),
        "I am C\nI am D\ntest-x-index.js\ntest-y-common.js\nplugins-C-int\nplugins-D-ext\n"
    );

    let Some(mkdir_mountpoints) = package_and_run_real_fixture_with_options(
        "mkdir-mountpoints",
        &root.join("test-99-#1120-mkdir-mountpoints"),
        "test-x-index.js",
        RealFixtureOptions {
            run_from_output_dir: true,
            ..RealFixtureOptions::success()
        },
    )?
    else {
        return Ok(());
    };
    assert_eq!(
        String::from_utf8_lossy(&mkdir_mountpoints.run.stdout),
        "hello.txt\n"
    );

    let Some(regexp_mountpoints) = package_and_run_real_fixture_with_options(
        "regexp-mountpoints",
        &root.join("test-99-#1121-regexp-mountpoints"),
        "test-x-index.js",
        RealFixtureOptions {
            run_from_output_dir: true,
            prepare_output_dir: Some(copy_plugins_d_ext),
            ..RealFixtureOptions::success()
        },
    )?
    else {
        return Ok(());
    };
    assert_eq!(
        String::from_utf8_lossy(&regexp_mountpoints.run.stdout),
        "I am D\ntest-x-index.js\n"
    );

    Ok(())
}

#[test]
fn issue_regression_fixtures_run_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");

    let copy_fixture = root.join("test-99-#420-copy-from-snapshot");
    let Some(copy_from_snapshot) = package_and_run_real_fixture_with_options(
        "copy-from-snapshot",
        &copy_fixture,
        ".",
        RealFixtureOptions {
            run_from_output_dir: true,
            prepare_output_dir: Some(create_output_subdir),
            ..RealFixtureOptions::success()
        },
    )?
    else {
        return Ok(());
    };
    let copied_payload = fs::read_to_string(copy_fixture.join("input/test.json"))?;
    assert_eq!(
        String::from_utf8_lossy(&copy_from_snapshot.run.stdout),
        format!("{copied_payload}\n{copied_payload}\n{copied_payload}\n")
    );

    let Some(with_file_types) = package_and_run_real_fixture(
        "with-file-types-root",
        &root.join("test-99-#938-withfiletypes"),
        ".",
    )?
    else {
        return Ok(());
    };
    assert_eq!(String::from_utf8_lossy(&with_file_types.run.stdout), "ok\n");

    package_and_compare_fixture(
        "with-file-types-files",
        &root.join("test-99-#1130"),
        "read.js",
        ".",
    )?;
    package_and_compare_fixture(
        "stat-file-metadata",
        &root.join("test-99-#1505"),
        "stat.js",
        ".",
    )?;
    package_and_compare_fixture(
        "worker-threads-package",
        &root.join("test-99-#775"),
        "a.js",
        ".",
    )?;
    package_and_compare_fixture(
        "worker-threads-file",
        &root.join("test-99-#775"),
        "a.js",
        "a.js",
    )?;

    Ok(())
}

#[test]
fn windows_issue_regression_fixtures_run_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(windows) {
        eprintln!("skipping Windows issue smoke: host platform is not Windows");
        return Ok(());
    }
    let Some(cache_root) = std::env::var_os("PKG_RUST_REAL_CACHE") else {
        eprintln!("skipping Windows issue smoke: PKG_RUST_REAL_CACHE is not set");
        return Ok(());
    };

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");
    run_windows_issue_1861(&root, &cache_root)?;
    run_windows_issue_1207(&root, &cache_root)?;
    Ok(())
}

#[test]
fn npm_issue_fixtures_run_when_install_is_enabled() -> Result<(), Box<dyn std::error::Error>> {
    if !npm_fixture_installs_enabled() {
        eprintln!("skipping npm fixture smoke: PKG_RUST_INSTALL_NPM_FIXTURES is not enabled");
        return Ok(());
    }
    if std::env::var_os("PKG_RUST_REAL_CACHE").is_none() {
        eprintln!("skipping npm fixture smoke: PKG_RUST_REAL_CACHE is not set");
        return Ok(());
    }

    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-99-#1192");
    let fixture_dir = copied_fixture("issue-1192-express-pug-work", &source)?;
    let install = Command::new("npm")
        .current_dir(&fixture_dir)
        .args(["install", "--ignore-scripts", "--no-audit", "--no-fund"])
        .output()?;
    assert!(
        install.status.success(),
        "npm install failed: {}{}",
        String::from_utf8_lossy(&install.stdout),
        String::from_utf8_lossy(&install.stderr)
    );

    let expected = Command::new("node")
        .current_dir(&fixture_dir)
        .arg("src/index.js")
        .output()?;
    assert!(
        expected.status.success(),
        "node oracle failed: {}{}",
        String::from_utf8_lossy(&expected.stdout),
        String::from_utf8_lossy(&expected.stderr)
    );

    for (name, package_args) in [
        ("issue-1192-express-pug", &[][..]),
        ("issue-1192-express-pug-gzip", &["--compress", "GZip"][..]),
        (
            "issue-1192-express-pug-brotli",
            &["--compress", "Brotli"][..],
        ),
    ] {
        let Some(package_run) = package_and_run_real_fixture_with_options(
            name,
            &fixture_dir,
            ".",
            RealFixtureOptions {
                package_args,
                ..RealFixtureOptions::success()
            },
        )?
        else {
            return Ok(());
        };
        assert_eq!(package_run.run.stdout, expected.stdout);
        assert_eq!(package_run.run.stderr, expected.stderr);
    }

    fs::remove_dir_all(fixture_dir)?;
    Ok(())
}

#[test]
fn public_npm_dictionary_fixtures_run_when_install_is_enabled()
-> Result<(), Box<dyn std::error::Error>> {
    if !npm_fixture_installs_enabled() {
        eprintln!(
            "skipping public npm fixture smoke: PKG_RUST_INSTALL_NPM_FIXTURES is not enabled"
        );
        return Ok(());
    }
    if std::env::var_os("PKG_RUST_REAL_CACHE").is_none() {
        eprintln!("skipping public npm fixture smoke: PKG_RUST_REAL_CACHE is not set");
        return Ok(());
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-79-npm");
    for fixture in [
        PublicNpmFixture {
            name: "npm-connect",
            fixture_subdir: "connect",
            package_spec: "connect",
            node_input: "connect.js",
            package_input: "connect.js",
        },
        PublicNpmFixture {
            name: "npm-connect-2-3-9",
            fixture_subdir: "connect",
            package_spec: "connect@2.3.9",
            node_input: "connect@2.3.9.js",
            package_input: "connect@2.3.9.js",
        },
        PublicNpmFixture {
            name: "npm-cookie",
            fixture_subdir: "cookie",
            package_spec: "cookie",
            node_input: "cookie.js",
            package_input: "cookie.js",
        },
        PublicNpmFixture {
            name: "npm-rc",
            fixture_subdir: "rc",
            package_spec: "rc",
            node_input: "rc.js",
            package_input: "rc.config.json",
        },
        PublicNpmFixture {
            name: "npm-socket-io-client-1-7-0",
            fixture_subdir: "socket.io-client",
            package_spec: "socket.io-client@1.7.0",
            node_input: "socket.io-client@1.7.0.js",
            package_input: "socket.io-client@1.7.0.js",
        },
        PublicNpmFixture {
            name: "npm-moment",
            fixture_subdir: "moment",
            package_spec: "moment",
            node_input: "moment.js",
            package_input: "moment.js",
        },
        PublicNpmFixture {
            name: "npm-any-promise",
            fixture_subdir: "any-promise",
            package_spec: "any-promise",
            node_input: "any-promise.js",
            package_input: "any-promise.js",
        },
        PublicNpmFixture {
            name: "npm-hoek",
            fixture_subdir: "hoek",
            package_spec: "hoek",
            node_input: "hoek.js",
            package_input: "hoek.js",
        },
        PublicNpmFixture {
            name: "npm-lodash",
            fixture_subdir: "lodash",
            package_spec: "lodash",
            node_input: "lodash.js",
            package_input: "lodash.js",
        },
        PublicNpmFixture {
            name: "npm-semver",
            fixture_subdir: "semver",
            package_spec: "semver",
            node_input: "semver.js",
            package_input: "semver.js",
        },
        PublicNpmFixture {
            name: "npm-verror",
            fixture_subdir: "verror",
            package_spec: "verror",
            node_input: "verror.js",
            package_input: "verror.js",
        },
        PublicNpmFixture {
            name: "npm-uglify-js-2-7-5",
            fixture_subdir: "uglify-js",
            package_spec: "uglify-js@2.7.5",
            node_input: "uglify-js@2.7.5.js",
            package_input: "uglify-js@2.7.5.js",
        },
        PublicNpmFixture {
            name: "npm-uglify-js",
            fixture_subdir: "uglify-js",
            package_spec: "uglify-js",
            node_input: "uglify-js.js",
            package_input: "uglify-js.js",
        },
        PublicNpmFixture {
            name: "npm-logform",
            fixture_subdir: "logform",
            package_spec: "logform",
            node_input: "logform.js",
            package_input: "logform.js",
        },
        PublicNpmFixture {
            name: "npm-body-parser",
            fixture_subdir: "body-parser",
            package_spec: "body-parser",
            node_input: "body-parser.js",
            package_input: "body-parser.js",
        },
        PublicNpmFixture {
            name: "npm-body-parser-1-10-2",
            fixture_subdir: "body-parser",
            package_spec: "body-parser@1.10.2",
            node_input: "body-parser@1.10.2.js",
            package_input: "body-parser@1.10.2.js",
        },
        PublicNpmFixture {
            name: "npm-express-with-jade",
            fixture_subdir: "express-with-jade",
            package_spec: "express",
            node_input: "express.js",
            package_input: "express.config.json",
        },
        PublicNpmFixture {
            name: "npm-browserify",
            fixture_subdir: "browserify",
            package_spec: "browserify",
            node_input: "browserify.js",
            package_input: "browserify.js",
        },
        PublicNpmFixture {
            name: "npm-log4js-0-5-8",
            fixture_subdir: "log4js",
            package_spec: "log4js@0.5.8",
            node_input: "log4js@0.5.8.js",
            package_input: "log4js@0.5.8.js",
        },
        PublicNpmFixture {
            name: "npm-log4js-0-6-34",
            fixture_subdir: "log4js",
            package_spec: "log4js@0.6.34",
            node_input: "log4js@0.6.34.js",
            package_input: "log4js@0.6.34.js",
        },
        PublicNpmFixture {
            name: "npm-log4js-1-1-1",
            fixture_subdir: "log4js",
            package_spec: "log4js@1.1.1",
            node_input: "log4js@1.1.1.js",
            package_input: "log4js@1.1.1.js",
        },
        PublicNpmFixture {
            name: "npm-negotiator",
            fixture_subdir: "negotiator",
            package_spec: "negotiator",
            node_input: "negotiator.js",
            package_input: "negotiator.js",
        },
        PublicNpmFixture {
            name: "npm-negotiator-0-4-9",
            fixture_subdir: "negotiator",
            package_spec: "negotiator@0.4.9",
            node_input: "negotiator@0.4.9.js",
            package_input: "negotiator@0.4.9.js",
        },
        PublicNpmFixture {
            name: "npm-machinepack-urls",
            fixture_subdir: "machinepack-urls",
            package_spec: "machinepack-urls",
            node_input: "machinepack-urls.js",
            package_input: "machinepack-urls.js",
        },
        PublicNpmFixture {
            name: "npm-machinepack-urls-5-0-0",
            fixture_subdir: "machinepack-urls",
            package_spec: "machinepack-urls@5.0.0",
            node_input: "machinepack-urls@5.0.0.js",
            package_input: "machinepack-urls@5.0.0.js",
        },
        PublicNpmFixture {
            name: "npm-shelljs",
            fixture_subdir: "shelljs",
            package_spec: "shelljs",
            node_input: "shelljs.js",
            package_input: "shelljs.js",
        },
        PublicNpmFixture {
            name: "npm-shelljs-0-7-6",
            fixture_subdir: "shelljs",
            package_spec: "shelljs@0.7.6",
            node_input: "shelljs@0.7.6.js",
            package_input: "shelljs@0.7.6.js",
        },
        PublicNpmFixture {
            name: "npm-shelljs-0-6-0",
            fixture_subdir: "shelljs",
            package_spec: "shelljs@0.6.0",
            node_input: "shelljs@0.6.0.js",
            package_input: "shelljs@0.6.0.js",
        },
        PublicNpmFixture {
            name: "npm-shelljs-0-1-4",
            fixture_subdir: "shelljs",
            package_spec: "shelljs@0.1.4",
            node_input: "shelljs@0.1.4.js",
            package_input: "shelljs@0.1.4.js",
        },
        PublicNpmFixture {
            name: "npm-graceful-fs",
            fixture_subdir: "graceful-fs",
            package_spec: "graceful-fs",
            node_input: "graceful-fs.js",
            package_input: "graceful-fs.js",
        },
        PublicNpmFixture {
            name: "npm-graceful-fs-3-0-8",
            fixture_subdir: "graceful-fs",
            package_spec: "graceful-fs@3.0.8",
            node_input: "graceful-fs@3.0.8.js",
            package_input: "graceful-fs@3.0.8.js",
        },
        PublicNpmFixture {
            name: "npm-buffermaker",
            fixture_subdir: "buffermaker",
            package_spec: "buffermaker",
            node_input: "buffermaker.js",
            package_input: "buffermaker.js",
        },
        PublicNpmFixture {
            name: "npm-bytes",
            fixture_subdir: "bytes",
            package_spec: "bytes",
            node_input: "bytes.js",
            package_input: "bytes.js",
        },
        PublicNpmFixture {
            name: "npm-bson-0-2-22",
            fixture_subdir: "bson",
            package_spec: "bson@0.2.22",
            node_input: "bson@0.2.22.js",
            package_input: "bson@0.2.22.js",
        },
        PublicNpmFixture {
            name: "npm-bson-0-4-0",
            fixture_subdir: "bson",
            package_spec: "bson@0.4.0",
            node_input: "bson@0.4.0.js",
            package_input: "bson@0.4.0.js",
        },
        PublicNpmFixture {
            name: "npm-compressjs",
            fixture_subdir: "compressjs",
            package_spec: "compressjs",
            node_input: "compressjs.js",
            package_input: "compressjs.js",
        },
        PublicNpmFixture {
            name: "npm-later",
            fixture_subdir: "later",
            package_spec: "later",
            node_input: "later.js",
            package_input: "later.js",
        },
        PublicNpmFixture {
            name: "npm-nconf",
            fixture_subdir: "nconf",
            package_spec: "nconf",
            node_input: "nconf.js",
            package_input: "nconf.js",
        },
        PublicNpmFixture {
            name: "npm-node-forge",
            fixture_subdir: "node-forge",
            package_spec: "node-forge",
            node_input: "node-forge.js",
            package_input: "node-forge.js",
        },
        PublicNpmFixture {
            name: "npm-node-xlsx",
            fixture_subdir: "node-xlsx",
            package_spec: "node-xlsx",
            node_input: "node-xlsx.js",
            package_input: "node-xlsx.js",
        },
        PublicNpmFixture {
            name: "npm-publicsuffixlist",
            fixture_subdir: "publicsuffixlist",
            package_spec: "publicsuffixlist",
            node_input: "publicsuffixlist.js",
            package_input: "publicsuffixlist.js",
        },
        PublicNpmFixture {
            name: "npm-oauth2orize",
            fixture_subdir: "oauth2orize",
            package_spec: "oauth2orize",
            node_input: "oauth2orize.js",
            package_input: "oauth2orize.js",
        },
        PublicNpmFixture {
            name: "npm-pgpass",
            fixture_subdir: "pgpass",
            package_spec: "pgpass",
            node_input: "pgpass.js",
            package_input: "pgpass.js",
        },
        PublicNpmFixture {
            name: "npm-pg-types",
            fixture_subdir: "pg-types",
            package_spec: "pg-types",
            node_input: "pg-types.js",
            package_input: "pg-types.js",
        },
        PublicNpmFixture {
            name: "npm-pg-types-1-0-0",
            fixture_subdir: "pg-types",
            package_spec: "pg-types@1.0.0",
            node_input: "pg-types@1.0.0.js",
            package_input: "pg-types@1.0.0.js",
        },
        PublicNpmFixture {
            name: "npm-node-zookeeper-client",
            fixture_subdir: "node-zookeeper-client",
            package_spec: "node-zookeeper-client",
            node_input: "node-zookeeper-client.js",
            package_input: "node-zookeeper-client.js",
        },
        PublicNpmFixture {
            name: "npm-npm-registry-client",
            fixture_subdir: "npm-registry-client",
            package_spec: "npm-registry-client",
            node_input: "npm-registry-client.js",
            package_input: "npm-registry-client.js",
        },
        PublicNpmFixture {
            name: "npm-pg",
            fixture_subdir: "pg",
            package_spec: "pg",
            node_input: "pg.js",
            package_input: "pg.js",
        },
        PublicNpmFixture {
            name: "npm-pg-cursor",
            fixture_subdir: "pg-cursor",
            package_spec: "pg-cursor",
            node_input: "pg-cursor.js",
            package_input: "pg-cursor.js",
        },
        PublicNpmFixture {
            name: "npm-pg-query-stream",
            fixture_subdir: "pg-query-stream",
            package_spec: "pg-query-stream",
            node_input: "pg-query-stream.js",
            package_input: "pg-query-stream.js",
        },
        PublicNpmFixture {
            name: "npm-throng",
            fixture_subdir: "throng",
            package_spec: "throng",
            node_input: "throng.js",
            package_input: "throng.js",
        },
        PublicNpmFixture {
            name: "npm-mongodb",
            fixture_subdir: "mongodb",
            package_spec: "mongodb",
            node_input: "mongodb.js",
            package_input: "mongodb.js",
        },
        PublicNpmFixture {
            name: "npm-mongodb-core",
            fixture_subdir: "mongodb-core",
            package_spec: "mongodb-core",
            node_input: "mongodb-core.js",
            package_input: "mongodb-core.js",
        },
        PublicNpmFixture {
            name: "npm-errors",
            fixture_subdir: "errors",
            package_spec: "errors",
            node_input: "errors.js",
            package_input: "errors.js",
        },
        PublicNpmFixture {
            name: "npm-geoip-lite",
            fixture_subdir: "geoip-lite",
            package_spec: "geoip-lite",
            node_input: "geoip-lite.js",
            package_input: "geoip-lite.js",
        },
        PublicNpmFixture {
            name: "npm-steam-crypto",
            fixture_subdir: "steam-crypto",
            package_spec: "steam-crypto",
            node_input: "steam-crypto.js",
            package_input: "steam-crypto.js",
        },
        PublicNpmFixture {
            name: "npm-tinify",
            fixture_subdir: "tinify",
            package_spec: "tinify",
            node_input: "tinify.js",
            package_input: "tinify.js",
        },
        PublicNpmFixture {
            name: "npm-tiny-worker",
            fixture_subdir: "tiny-worker",
            package_spec: "tiny-worker",
            node_input: "tiny-worker.js",
            package_input: "tiny-worker.js",
        },
        PublicNpmFixture {
            name: "npm-mime-types",
            fixture_subdir: "mime-types",
            package_spec: "mime-types",
            node_input: "mime-types.js",
            package_input: "mime-types.js",
        },
    ] {
        run_public_npm_fixture(&root, fixture)?;
    }
    Ok(())
}

#[test]
fn public_npm_target_node_oracle_probe_runs_when_enabled() -> Result<(), Box<dyn std::error::Error>>
{
    let Some(probe_name) = std::env::var("PKG_RUST_TARGET_ORACLE_PUBLIC_NPM")
        .ok()
        .filter(|value| !value.is_empty())
    else {
        eprintln!(
            "skipping public npm target-node oracle probe: PKG_RUST_TARGET_ORACLE_PUBLIC_NPM is not set"
        );
        return Ok(());
    };
    if !npm_fixture_installs_enabled() {
        eprintln!(
            "skipping public npm target-node oracle probe: PKG_RUST_INSTALL_NPM_FIXTURES is not enabled"
        );
        return Ok(());
    }
    let Some(cache_root) = std::env::var_os("PKG_RUST_REAL_CACHE") else {
        eprintln!("skipping public npm target-node oracle probe: PKG_RUST_REAL_CACHE is not set");
        return Ok(());
    };
    let fixture = public_npm_probe_fixture(&probe_name)
        .ok_or_else(|| format!("unknown public npm target-node oracle probe: {probe_name}"))?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-79-npm");
    let source = root.join(fixture.fixture_subdir);
    let fixture_dir = copied_fixture(&format!("{}-target-oracle-work", fixture.name), &source)?;
    install_public_npm_packages(
        &fixture_dir,
        fixture.package_spec,
        public_npm_extra_package_specs(fixture.name),
    )?;

    let expected = run_target_node_oracle(&cache_root, &fixture_dir, fixture.node_input)?;
    let output_mode = public_npm_output_mode(fixture.name);
    assert_eq!(
        public_npm_harness_stdout(&expected.stdout, output_mode),
        public_npm_success_marker(output_mode),
        "{} target-node oracle did not match the JS harness success marker: {}{}",
        fixture.name,
        String::from_utf8_lossy(&expected.stdout),
        String::from_utf8_lossy(&expected.stderr)
    );

    fs::remove_dir_all(fixture_dir)?;
    Ok(())
}

#[test]
fn native_npm_issue_fixtures_run_when_install_is_enabled() -> Result<(), Box<dyn std::error::Error>>
{
    if !native_npm_fixture_installs_enabled() {
        eprintln!("skipping native npm fixture smoke: PKG_RUST_NATIVE_NPM_FIXTURES is not enabled");
        return Ok(());
    }
    if std::env::var_os("PKG_RUST_REAL_CACHE").is_none() {
        eprintln!("skipping native npm fixture smoke: PKG_RUST_REAL_CACHE is not set");
        return Ok(());
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test");
    run_native_npm_issue_1135(&root)?;
    run_native_npm_issue_1191(&root)?;
    Ok(())
}

fn run_native_npm_issue_1135(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = root.join("test-99-#1135");
    let fixture_dir = copied_fixture("issue-1135-canvas-work", &source)?;
    install_npm_dependencies(&fixture_dir)?;

    let expected = run_node_oracle(&fixture_dir, "index.js")?;
    let Some(package_run) =
        package_and_run_real_fixture("issue-1135-canvas", &fixture_dir, "package.json")?
    else {
        fs::remove_dir_all(fixture_dir)?;
        return Ok(());
    };
    assert_eq!(package_run.run.stdout, expected.stdout);
    assert_eq!(package_run.run.stderr, expected.stderr);

    fs::remove_dir_all(fixture_dir)?;
    Ok(())
}

fn run_native_npm_issue_1191(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = root.join("test-99-#1191");
    let fixture_dir = copied_fixture("issue-1191-better-sqlite3-work", &source)?;
    install_npm_dependencies(&fixture_dir)?;

    let expected = run_node_oracle(&fixture_dir, "index.js")?;
    for (name, package_args) in [
        ("issue-1191-better-sqlite3", &[][..]),
        (
            "issue-1191-better-sqlite3-brotli",
            &["--compress", "Brotli"][..],
        ),
    ] {
        let Some(package_run) = package_and_run_real_fixture_with_options(
            name,
            &fixture_dir,
            "index.js",
            RealFixtureOptions {
                package_args,
                ..RealFixtureOptions::success()
            },
        )?
        else {
            fs::remove_dir_all(fixture_dir)?;
            return Ok(());
        };
        assert_eq!(package_run.run.stdout, expected.stdout);
        assert_eq!(package_run.run.stderr, expected.stderr);
    }

    fs::remove_dir_all(fixture_dir)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct PublicNpmFixture<'a> {
    name: &'a str,
    fixture_subdir: &'a str,
    package_spec: &'a str,
    node_input: &'a str,
    package_input: &'a str,
}

#[derive(Clone, Copy)]
enum PublicNpmOutputMode {
    ExactStdout,
    LastStdoutLine,
}

fn run_public_npm_fixture(
    root: &Path,
    fixture: PublicNpmFixture<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = root.join(fixture.fixture_subdir);
    let fixture_dir = copied_fixture(&format!("{}-work", fixture.name), &source)?;
    install_public_npm_packages(
        &fixture_dir,
        fixture.package_spec,
        public_npm_extra_package_specs(fixture.name),
    )?;

    let expected = run_node_oracle(&fixture_dir, fixture.node_input)?;
    let output_mode = public_npm_output_mode(fixture.name);
    let expected_stdout = public_npm_harness_stdout(&expected.stdout, output_mode);
    let success_marker = public_npm_success_marker(output_mode);
    assert_eq!(
        expected_stdout, success_marker,
        "{} node oracle did not match the JS harness success marker",
        fixture.name
    );

    let Some(package_run) =
        package_and_run_real_fixture(fixture.name, &fixture_dir, fixture.package_input)?
    else {
        fs::remove_dir_all(fixture_dir)?;
        return Ok(());
    };
    assert_eq!(
        public_npm_harness_stdout(&package_run.run.stdout, output_mode),
        expected_stdout
    );
    if matches!(output_mode, PublicNpmOutputMode::ExactStdout) {
        assert_eq!(
            normalize_node_warning_stderr(&package_run.run.stderr),
            normalize_node_warning_stderr(&expected.stderr)
        );
    }

    fs::remove_dir_all(fixture_dir)?;
    Ok(())
}

fn public_npm_probe_fixture(name: &str) -> Option<PublicNpmFixture<'static>> {
    match name {
        "cookie" | "npm-cookie" => Some(PublicNpmFixture {
            name: "npm-cookie",
            fixture_subdir: "cookie",
            package_spec: "cookie",
            node_input: "cookie.js",
            package_input: "cookie.js",
        }),
        "mime-types" | "npm-mime-types" => Some(PublicNpmFixture {
            name: "npm-mime-types",
            fixture_subdir: "mime-types",
            package_spec: "mime-types",
            node_input: "mime-types.js",
            package_input: "mime-types.js",
        }),
        "connect-mongodb" | "npm-connect-mongodb" => Some(PublicNpmFixture {
            name: "npm-connect-mongodb",
            fixture_subdir: "connect-mongodb",
            package_spec: "connect-mongodb",
            node_input: "connect-mongodb.js",
            package_input: "connect-mongodb.js",
        }),
        "reload" | "npm-reload" => Some(PublicNpmFixture {
            name: "npm-reload",
            fixture_subdir: "reload",
            package_spec: "reload",
            node_input: "reload.js",
            package_input: "reload.js",
        }),
        "reload@2.1.0" | "npm-reload-2-1-0" => Some(PublicNpmFixture {
            name: "npm-reload-2-1-0",
            fixture_subdir: "reload",
            package_spec: "reload@2.1.0",
            node_input: "reload@2.1.0.js",
            package_input: "reload@2.1.0.js",
        }),
        "shelljs" | "npm-shelljs" => Some(PublicNpmFixture {
            name: "npm-shelljs",
            fixture_subdir: "shelljs",
            package_spec: "shelljs",
            node_input: "shelljs.js",
            package_input: "shelljs.js",
        }),
        _ => None,
    }
}

fn public_npm_output_mode(name: &str) -> PublicNpmOutputMode {
    match name {
        "npm-browserify"
        | "npm-bson-0-2-22"
        | "npm-bson-0-4-0"
        | "npm-connect-mongodb"
        | "npm-reload"
        | "npm-reload-2-1-0" => PublicNpmOutputMode::LastStdoutLine,
        _ => PublicNpmOutputMode::ExactStdout,
    }
}

fn public_npm_extra_package_specs(name: &str) -> &'static [&'static str] {
    match name {
        "npm-express-with-jade" => &["jade"],
        "npm-pg-cursor" | "npm-pg-query-stream" => &["pg"],
        _ => &[],
    }
}

fn public_npm_success_marker(mode: PublicNpmOutputMode) -> &'static str {
    match mode {
        PublicNpmOutputMode::ExactStdout => "ok\n",
        PublicNpmOutputMode::LastStdoutLine => "ok",
    }
}

fn public_npm_harness_stdout(stdout: &[u8], mode: PublicNpmOutputMode) -> String {
    let text = String::from_utf8_lossy(stdout);
    match mode {
        PublicNpmOutputMode::ExactStdout => text.into_owned(),
        PublicNpmOutputMode::LastStdoutLine => {
            let lines = text.split('\n').collect::<Vec<_>>();
            let start = lines.len().saturating_sub(2);
            let mut last_line = lines[start..].join("\n");
            if last_line.ends_with("\r\n") {
                last_line.truncate(last_line.len() - 2);
            } else if last_line.ends_with('\n') {
                last_line.pop();
            }
            last_line
        }
    }
}

#[test]
fn node_warning_stderr_normalization_keeps_warning_content() {
    let node = b"(node:123) Warning: Accessing non-existent property 'exec' of module exports inside circular dependency\n(Use `node --trace-warnings ...` to show where the warning was created)\n";
    let packaged = b"(node:456) Warning: Accessing non-existent property 'exec' of module exports inside circular dependency\n(Use `pkg-rust-real-npm-shelljs-0-7-6-999 --trace-warnings ...` to show where the warning was created)\n";

    assert_eq!(
        normalize_node_warning_stderr(packaged),
        normalize_node_warning_stderr(node)
    );
}

#[test]
fn public_npm_last_line_mode_matches_js_harness_metadata() {
    let stdout = b"Usage: browserify [entry files] {OPTIONS}\nSpecify a parameter.\nok\n";
    assert_eq!(
        public_npm_harness_stdout(stdout, PublicNpmOutputMode::LastStdoutLine),
        "ok"
    );
}

#[test]
fn public_npm_extra_packages_match_js_harness_metadata() {
    assert_eq!(
        public_npm_extra_package_specs("npm-express-with-jade"),
        ["jade"]
    );
    assert_eq!(public_npm_extra_package_specs("npm-pg-cursor"), ["pg"]);
    assert_eq!(
        public_npm_extra_package_specs("npm-pg-query-stream"),
        ["pg"]
    );
    assert_eq!(public_npm_extra_package_specs("npm-pg"), [] as [&str; 0]);
}

fn normalize_node_warning_stderr(stderr: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stderr)
        .lines()
        .map(normalize_node_warning_line)
        .collect()
}

fn normalize_node_warning_line(line: &str) -> String {
    if let Some(rest) = line.strip_prefix("(node:") {
        if let Some(warning) = rest.split_once(") Warning: ") {
            return format!("(node:<pid>) Warning: {}", warning.1);
        }
    }

    if let Some(command) = line
        .strip_prefix("(Use `")
        .and_then(|rest| rest.split_once(" --trace-warnings ...` "))
    {
        return format!("(Use `<exec> --trace-warnings ...` {}", command.1);
    }

    line.to_owned()
}

fn install_public_npm_packages(
    fixture_dir: &Path,
    package_spec: &str,
    extra_package_specs: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("npm");
    command
        .current_dir(fixture_dir)
        .arg("install")
        .arg(package_spec);
    command.args(extra_package_specs);
    command.args(["--no-save", "--unsafe-perm", "--no-audit", "--no-fund"]);
    let install = command.output()?;

    let package_specs = std::iter::once(package_spec)
        .chain(extra_package_specs.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        install.status.success(),
        "npm install {package_specs} failed in {}: {}{}",
        fixture_dir.display(),
        String::from_utf8_lossy(&install.stdout),
        String::from_utf8_lossy(&install.stderr)
    );
    Ok(())
}

fn install_npm_dependencies(fixture_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let install = Command::new("npm")
        .current_dir(fixture_dir)
        .args(["install", "--no-audit", "--no-fund"])
        .output()?;
    assert!(
        install.status.success(),
        "npm install failed in {}: {}{}",
        fixture_dir.display(),
        String::from_utf8_lossy(&install.stdout),
        String::from_utf8_lossy(&install.stderr)
    );
    Ok(())
}

fn run_node_oracle(
    fixture_dir: &Path,
    input: &str,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let expected = Command::new("node")
        .current_dir(fixture_dir)
        .arg(input)
        .output()?;
    assert!(
        expected.status.success(),
        "node oracle failed in {}: {}{}",
        fixture_dir.display(),
        String::from_utf8_lossy(&expected.stdout),
        String::from_utf8_lossy(&expected.stderr)
    );
    Ok(expected)
}

fn run_target_node_oracle(
    cache_root: &std::ffi::OsStr,
    fixture_dir: &Path,
    input: &str,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let target_node = target_node_binary(cache_root)?;
    let input_path = fixture_dir.join(input);
    let expected = Command::new(&target_node)
        .current_dir(fixture_dir)
        .env("PKG_EXECPATH", "PKG_INVOKE_NODEJS")
        .env("PKG_RUST_ORACLE_INPUT", &input_path)
        .arg("-e")
        .arg(TARGET_NODE_ORACLE_WRAPPER)
        .output()?;
    assert!(
        expected.status.success(),
        "target-node oracle failed in {} with {}: {}{}",
        fixture_dir.display(),
        target_node.display(),
        String::from_utf8_lossy(&expected.stdout),
        String::from_utf8_lossy(&expected.stderr)
    );
    Ok(expected)
}

fn target_node_binary(cache_root: &std::ffi::OsStr) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let defaults = TargetDefaults::host("node18");
    let mut parsed = parse_targets(&real_target(), &defaults)?;
    let target = parsed
        .targets
        .pop()
        .ok_or_else(|| "real target parser returned no targets".to_owned())?;
    let cache = PkgFetchCache::new(PathBuf::from(cache_root));
    let binary = cache.binary_artifact_for(&target)?;
    let path = binary
        .path()
        .ok_or_else(|| format!("target binary for {target} did not expose a cache path"))?;
    Ok(path.to_path_buf())
}

fn run_windows_issue_1861(
    root: &Path,
    cache_root: &std::ffi::OsStr,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = root.join("test-99-#1861");
    let output = fixture_dir.join("index.exe");
    let _ignored = fs::remove_file(&output);
    package_real_fixture_to_output(&fixture_dir, "index.js", &output, cache_root)?;

    let run = Command::new(&output)
        .current_dir(&fixture_dir)
        .arg("launch")
        .output()?;
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "issue #1861 executable failed: {}{}",
        stdout,
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(stdout.contains("launch"), "missing launch output: {stdout}");
    assert!(stdout.contains("stop"), "missing stop output: {stdout}");

    fs::remove_file(output)?;
    Ok(())
}

fn run_windows_issue_1207(
    root: &Path,
    cache_root: &std::ffi::OsStr,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = root.join("test-99-#1207");
    let drive = mount_subst_drive(&fixture_dir)?;
    let drive_root = format!("{drive}\\");
    let input = format!("{drive}\\index.js");
    let output = format!("{drive}\\index.exe");
    let alternate_output = fixture_dir.join("index.exe");
    let _cleanup = SubstDriveGuard {
        drive: drive.clone(),
    };
    let _ignored = fs::remove_file(&alternate_output);

    package_real_fixture_to_output_with_cwd(
        Path::new(&drive_root),
        &input,
        Path::new(&output),
        cache_root,
    )?;

    let direct = Command::new(&output).current_dir(&drive_root).output()?;
    assert_eq!(direct.stdout, b"42\n");

    let reference = Command::new(&output)
        .current_dir(&drive_root)
        .env("DEBUG_PKG", "42")
        .output()?;
    assert!(
        String::from_utf8_lossy(&reference.stdout).ends_with("42\n"),
        "issue #1207 reference output mismatch: {}{}",
        String::from_utf8_lossy(&reference.stdout),
        String::from_utf8_lossy(&reference.stderr)
    );

    let alternate_mounted = Command::new(&alternate_output)
        .current_dir(&fixture_dir)
        .env("DEBUG_PKG", "42")
        .output()?;
    assert_eq!(alternate_mounted.stdout, reference.stdout);

    drop(_cleanup);
    let alternate_unmounted = Command::new(&alternate_output)
        .current_dir(&fixture_dir)
        .env("DEBUG_PKG", "42")
        .output()?;
    assert_eq!(alternate_unmounted.stdout, reference.stdout);

    fs::remove_file(alternate_output)?;
    Ok(())
}

fn package_real_fixture_to_output(
    fixture_dir: &Path,
    input: &str,
    output: &Path,
    cache_root: &std::ffi::OsStr,
) -> Result<(), Box<dyn std::error::Error>> {
    package_real_fixture_to_output_with_cwd(fixture_dir, input, output, cache_root)
}

fn package_real_fixture_to_output_with_cwd(
    cwd: &Path,
    input: &str,
    output: &Path,
    cache_root: &std::ffi::OsStr,
) -> Result<(), Box<dyn std::error::Error>> {
    let package_result = Command::new(env!("CARGO_BIN_EXE_pkg"))
        .current_dir(cwd)
        .env("PKG_CACHE_PATH", cache_root)
        .arg("--target")
        .arg(real_target())
        .arg("--output")
        .arg(output)
        .arg(input)
        .output()?;
    assert!(
        package_result.status.success(),
        "pkg CLI failed: {}{}",
        String::from_utf8_lossy(&package_result.stdout),
        String::from_utf8_lossy(&package_result.stderr)
    );
    Ok(())
}

#[test]
fn inspect_fixture_exits_with_node_inspect_code_when_real_cache_is_configured()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/test-50-inspect");
    let Some(_run_result) = package_and_run_real_fixture_with_options(
        "inspect",
        &fixture_dir,
        "test-x-index.js",
        RealFixtureOptions {
            run_args: &["--inspect"],
            run_env: &[("PKG_EXECPATH", "PKG_INVOKE_NODEJS")],
            run_expectation: RunExpectation::Code(9),
            ..RealFixtureOptions::success()
        },
    )?
    else {
        return Ok(());
    };
    Ok(())
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
    assert_eq!(String::from_utf8_lossy(&run_result.run.stdout), "ok\n");
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

    let stderr = String::from_utf8_lossy(&run_result.run.stderr);
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

    let stderr = String::from_utf8_lossy(&run_result.run.stderr);
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
    let stdout = String::from_utf8_lossy(&run_result.run.stdout);
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
    assert_eq!(run_result.run.stdout, expected.stdout);
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
) -> Result<Option<PackageRun>, Box<dyn std::error::Error>> {
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
) -> Result<Option<PackageRun>, Box<dyn std::error::Error>> {
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
) -> Result<Option<PackageRun>, Box<dyn std::error::Error>> {
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
) -> Result<Option<PackageRun>, Box<dyn std::error::Error>> {
    let Some(cache_root) = std::env::var_os("PKG_RUST_REAL_CACHE") else {
        eprintln!("skipping real runtime smoke: PKG_RUST_REAL_CACHE is not set");
        return Ok(None);
    };
    let target = real_target();

    let output = if options.run_from_output_dir || options.prepare_output_dir.is_some() {
        real_output_dir(name).join("test-output")
    } else {
        real_output_path(name)
    };
    let package_result = Command::new(env!("CARGO_BIN_EXE_pkg"))
        .current_dir(fixture_dir)
        .env("PKG_CACHE_PATH", cache_root)
        .envs(options.package_env.iter().copied())
        .args(options.package_args)
        .arg("--target")
        .arg(&target)
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

    if let Some(prepare_output_dir) = options.prepare_output_dir {
        let output_dir = output
            .parent()
            .ok_or_else(|| "real output path has no parent".to_owned())?;
        prepare_output_dir(fixture_dir, output_dir)?;
    }
    let run_cwd = if options.run_from_output_dir {
        output
            .parent()
            .ok_or_else(|| "real output path has no parent".to_owned())?
    } else {
        fixture_dir
    };
    let run_result = Command::new(&output)
        .current_dir(run_cwd)
        .args(options.run_args)
        .envs(options.run_env.iter().copied())
        .output()?;
    fs::remove_file(output)?;
    if options.run_from_output_dir || options.prepare_output_dir.is_some() {
        let output_dir = real_output_dir(name);
        if output_dir.is_dir() {
            fs::remove_dir_all(output_dir)?;
        }
    }
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
        RunExpectation::Code(code) => assert_eq!(
            run_result.status.code(),
            Some(code),
            "produced executable exited with unexpected status: {}{}",
            String::from_utf8_lossy(&run_result.stdout),
            String::from_utf8_lossy(&run_result.stderr)
        ),
    }
    Ok(Some(PackageRun {
        package: package_result,
        run: run_result,
    }))
}

struct PackageRun {
    package: std::process::Output,
    run: std::process::Output,
}

type PrepareOutputDir = fn(&Path, &Path) -> Result<(), Box<dyn std::error::Error>>;

struct RealFixtureOptions<'a> {
    package_args: &'a [&'a str],
    package_env: &'a [(&'a str, &'a str)],
    run_args: &'a [&'a str],
    run_env: &'a [(&'a str, &'a str)],
    run_expectation: RunExpectation,
    run_from_output_dir: bool,
    prepare_output_dir: Option<PrepareOutputDir>,
}

impl RealFixtureOptions<'_> {
    fn success() -> Self {
        Self {
            package_args: &[],
            package_env: &[],
            run_args: &[],
            run_env: &[],
            run_expectation: RunExpectation::Success,
            run_from_output_dir: false,
            prepare_output_dir: None,
        }
    }
}

enum RunExpectation {
    Success,
    Failure,
    Code(i32),
}

fn real_output_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pkg-rust-real-{name}-{}", std::process::id()))
}

fn real_output_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pkg-rust-real-{name}-{}", std::process::id()))
}

fn real_target() -> String {
    std::env::var("PKG_RUST_REAL_TARGET").unwrap_or_else(|_| {
        if cfg!(windows) {
            "node18-win-x64".to_owned()
        } else {
            DEFAULT_REAL_TARGET.to_owned()
        }
    })
}

fn mount_subst_drive(target: &Path) -> Result<String, Box<dyn std::error::Error>> {
    for drive in ["H:", "P:", "Q:", "R:"] {
        let _ignored = Command::new("subst").args([drive, "/D"]).output();
        let mount = Command::new("subst").arg(drive).arg(target).output()?;
        if mount.status.success() {
            return Ok(drive.to_owned());
        }
    }
    Err(format!("could not mount a subst drive for {}", target.display()).into())
}

struct SubstDriveGuard {
    drive: String,
}

impl Drop for SubstDriveGuard {
    fn drop(&mut self) {
        let _ignored = Command::new("subst")
            .args([self.drive.as_str(), "/D"])
            .output();
    }
}

fn copied_fixture(name: &str, source: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let target = real_output_dir(name).join("fixture");
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    copy_directory(source, &target)?;
    Ok(target)
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "node_modules"
            || name == "package-lock.json"
            || name == "output"
            || name == "dist"
            || name == "run-time"
        {
            continue;
        }
        let destination = target.join(name);
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            copy_directory(&path, &destination)?;
        } else if metadata.is_file() {
            fs::copy(&path, &destination)?;
        }
    }
    Ok(())
}

fn npm_fixture_installs_enabled() -> bool {
    std::env::var("PKG_RUST_INSTALL_NPM_FIXTURES").is_ok_and(|value| {
        let value = value.to_ascii_lowercase();
        matches!(value.as_str(), "1" | "true" | "yes")
    })
}

fn native_npm_fixture_installs_enabled() -> bool {
    std::env::var("PKG_RUST_NATIVE_NPM_FIXTURES").is_ok_and(|value| {
        let value = value.to_ascii_lowercase();
        matches!(value.as_str(), "1" | "true" | "yes")
    })
}

fn copy_plugins_d_ext(
    fixture_dir: &Path,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = output_dir.join("plugins-D-ext");
    fs::create_dir_all(&target)?;
    fs::copy(
        fixture_dir.join("plugins-D-ext/test-y-require-D.js"),
        target.join("test-y-require-D.js"),
    )?;
    Ok(())
}

fn create_output_subdir(
    _fixture_dir: &Path,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output_dir.join("output"))?;
    Ok(())
}
