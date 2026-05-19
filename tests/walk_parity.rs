#![allow(missing_docs)]

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
    assert!(output.contains_store(
        fixture_dir.join("test-z-require-json-5.json"),
        StoreKind::Content
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
