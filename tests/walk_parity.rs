#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};

use pkg_rust::{Marker, PackageJson, PkgError, StoreKind, WalkerParams, walk};

fn empty_marker() -> Result<Marker, PkgError> {
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    Ok(Marker::new(package))
}

#[test]
fn walks_require_resolve_fixture_dependencies_in_fifo_order() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");
    let output = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;

    assert!(output.contains_store(&entrypoint, StoreKind::Blob));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-code-1.js"),
        StoreKind::Blob
    ));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-code-2.js"),
        StoreKind::Blob
    ));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-code-3.js"),
        StoreKind::Blob
    ));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-code-4.js"),
        StoreKind::Blob
    ));
    assert!(output.contains_store(fixture_dir.join("test-y-resolve.any"), StoreKind::Content));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-content.css"),
        StoreKind::Content
    ));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-json-1.json"),
        StoreKind::Content
    ));
    assert!(!output.contains_store(
        fixture_dir.join("test-z-require-json-1.json"),
        StoreKind::Blob
    ));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-json-5.json"),
        StoreKind::Content
    ));
    assert!(!output.contains_store(
        fixture_dir.join("test-z-require-json-5.json"),
        StoreKind::Blob
    ));
    assert!(
        output
            .task_log
            .first()
            .is_some_and(|task| task.file.ends_with(Path::new("test-x-index.js"))
                && task.store == StoreKind::Blob)
    );
    assert!(
        output
            .task_log
            .get(1)
            .is_some_and(|task| task.file.ends_with(Path::new("test-x-index.js"))
                && task.store == StoreKind::Stat)
    );

    Ok(())
}

#[test]
fn explicit_addition_is_stored_as_content() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let addition = fixture_dir.join("test-z-require-content.css");

    let output = walk(
        empty_marker()?,
        fixture_dir.join("test-z-require-code-1.js"),
        Some(addition.clone()),
        WalkerParams::new().with_root(&fixture_dir),
    )?;

    assert!(output.contains_store(addition, StoreKind::Content));
    Ok(())
}

#[test]
fn activates_package_config_scripts_and_assets() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-with-config");
    let marker = Marker::from_package_path(fixture_dir.join("package.json"))?;
    let output = walk(
        marker,
        fixture_dir.join("test-x-index.js"),
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;

    assert!(output.contains_store(
        fixture_dir.join("test-z-require-code-I.js"),
        StoreKind::Blob
    ));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-code-J.js"),
        StoreKind::Blob
    ));
    assert!(output.contains_store(fixture_dir.join("test-y-resolve-A.txt"), StoreKind::Content));
    assert!(output.contains_store(fixture_dir.join("test-y-resolve-H.txt"), StoreKind::Content));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-content-K.txt"),
        StoreKind::Content
    ));
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-content-N.txt"),
        StoreKind::Content
    ));
    Ok(())
}

#[test]
fn dictionary_log_records_config_warning() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-config-log");
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    let output = walk(
        Marker::new(package),
        fixture_dir.join("test-x-index.js"),
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;

    assert!(output.warnings.iter().any(|warning| {
        let message = warning.to_cli_message();
        message.contains("stylus options to resolve imports")
            && message.contains("stylus/package.json")
    }));
    Ok(())
}

#[test]
fn activates_package_files_directories_and_absolute_style_entries() -> Result<(), PkgError> {
    for fixture in [
        "../test/test-50-package-json-8",
        "../test/test-50-package-json-8b",
    ] {
        let fixture_dir = PathBuf::from(fixture);
        let marker = Marker::from_package_path(fixture_dir.join("package.json"))?;
        let output = walk(
            marker,
            fixture_dir.join("sub/test-x-index.js"),
            None,
            WalkerParams::new().with_root(&fixture_dir),
        )?;

        assert!(output.contains_store(
            fixture_dir.join("sub/sub/test-y-require.js"),
            StoreKind::Blob
        ));
        assert!(output.contains_store(fixture_dir.join("sub/test-z-require.js"), StoreKind::Blob));
        assert!(output.contains_store(fixture_dir.join("test-z-data.css"), StoreKind::Content));
    }

    Ok(())
}

#[test]
fn dependency_package_markers_activate_dependency_files_and_pkg_config() -> Result<(), PkgError> {
    let cases = [
        ("../test/test-50-package-json-9", StoreKind::Content),
        ("../test/test-50-package-json-9p", StoreKind::Blob),
    ];

    for (fixture, dependency_main_store) in cases {
        let fixture_dir = PathBuf::from(fixture);
        let package = PackageJson::parse("{}")
            .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
        let output = walk(
            Marker::new(package),
            fixture_dir.join("test-x-index.js"),
            None,
            WalkerParams::new().with_root(&fixture_dir),
        )?;

        assert!(output.contains_store(
            fixture_dir.join("node_modules/test-y-require/package.json"),
            StoreKind::Content
        ));
        assert!(output.contains_store(
            fixture_dir.join("node_modules/test-y-require/sub/sub/test-y-require.js"),
            dependency_main_store
        ));
        assert!(output.contains_store(
            fixture_dir.join("node_modules/test-y-require/sub/test-z-require.js"),
            StoreKind::Blob
        ));
        assert!(output.contains_store(
            fixture_dir.join("node_modules/test-y-require/test-z-data.css"),
            StoreKind::Content
        ));
    }

    Ok(())
}

#[test]
fn local_package_directory_requires_include_package_json_for_runtime_resolution()
-> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-package-json-6c");
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    let output = walk(
        Marker::new(package),
        fixture_dir.join("beta/alpha.js"),
        None,
        WalkerParams::new().with_root(fixture_dir.join("beta")),
    )?;

    assert!(output.contains_store(fixture_dir.join("beta/package.json"), StoreKind::Content));
    assert!(output.contains_store(fixture_dir.join("beta/beta.js"), StoreKind::Blob));
    Ok(())
}

#[test]
fn dependency_without_main_still_activates_package_json_dependencies() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-package-json-5");
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    let output = walk(
        Marker::new(package),
        fixture_dir.join("node_modules/input/test-x-index.js"),
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;

    assert!(output.contains_store(
        fixture_dir.join("node_modules/input/node_modules/@types/omega/package.json"),
        StoreKind::Content
    ));
    assert!(output.contains_store(
        fixture_dir.join("node_modules/input/node_modules/@types/delta/index.js"),
        StoreKind::Blob
    ));
    Ok(())
}

#[test]
fn dependency_without_main_records_js_warning() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-invalid-package-json-2");
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    let output = walk(
        Marker::new(package),
        fixture_dir.join("test-x-index.js"),
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;

    assert_eq!(output.warnings.len(), 1);
    let message = output.warnings[0].to_cli_message();
    assert!(message.contains("Entry 'main' not found"));
    assert!(message.contains("crusader/package.json"));
    Ok(())
}

#[test]
fn dependency_package_self_subpath_require_includes_target() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-package-json-6b");
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    let output = walk(
        Marker::new(package),
        fixture_dir.join("node_modules/alpha/alpha.js"),
        None,
        WalkerParams::new().with_root(fixture_dir.join("node_modules/alpha")),
    )?;

    assert!(output.contains_store(
        fixture_dir.join("node_modules/alpha/beta.js"),
        StoreKind::Blob
    ));
    Ok(())
}

#[test]
fn applies_package_config_patches_before_blob_detection() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-package-json-3");
    let marker = Marker::from_package_path(fixture_dir.join("package.json"))?;
    let entrypoint = fixture_dir.join("test-x-index.js");
    let output = walk(
        marker,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;

    assert!(output.record(&entrypoint).is_some_and(|record| {
        record.body.as_ref().is_some_and(|body| {
            let body = String::from_utf8_lossy(body);
            body.contains("process.cwd() + '/' + dataPath")
                && !body.contains("__dirname + '/' + dataPath")
        })
    }));

    Ok(())
}

#[cfg(unix)]
#[test]
fn tracks_blob_symlinks_to_real_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir =
        PathBuf::from("/private/tmp").join(format!("pkg-rust-symlink-{}", std::process::id()));
    let real_file = fixture_dir.join("real.js");
    let link_file = fixture_dir.join("link.js");
    let _ignored = fs::remove_dir_all(&fixture_dir);
    fs::create_dir_all(&fixture_dir)?;
    fs::write(&real_file, "'use strict';\nmodule.exports = 1;\n")?;
    std::os::unix::fs::symlink(&real_file, &link_file)?;

    let output = walk(
        empty_marker()?,
        &link_file,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;

    assert_eq!(output.symlinks.get(&link_file), Some(&real_file));
    assert!(output.contains_store(&real_file, StoreKind::Blob));

    fs::remove_dir_all(&fixture_dir)?;
    Ok(())
}
