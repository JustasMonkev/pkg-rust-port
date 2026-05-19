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
