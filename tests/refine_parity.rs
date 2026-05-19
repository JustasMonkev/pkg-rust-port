#![allow(missing_docs)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use pkg_rust::{
    FileRecord, Marker, PackageJson, PathStyle, PkgError, SymlinkMap, WalkOutput, WalkerParams,
    refine, walk,
};

fn empty_marker() -> Result<Marker, PkgError> {
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("test package parse failed: {error}")))?;
    Ok(Marker::new(package))
}

#[test]
fn denominates_walked_records_and_entrypoint() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");
    let output = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;
    let refined = refine(output, &entrypoint, SymlinkMap::new(), PathStyle::Posix);

    assert_eq!(refined.entrypoint, "/test-x-index.js");
    assert!(refined.records.contains_key("/test-x-index.js"));
    assert!(refined.records.contains_key("/test-z-require-code-1.js"));
    assert!(refined.records.contains_key("/test-z-require-content.css"));
    Ok(())
}

#[test]
fn denominates_symlinks_with_same_denominator_as_records() -> Result<(), PkgError> {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");
    let output = walk(
        empty_marker()?,
        &entrypoint,
        None,
        WalkerParams::new().with_root(&fixture_dir),
    )?;
    let mut symlinks = SymlinkMap::new();
    symlinks.insert(
        fixture_dir.join("test-z-require-content.css"),
        fixture_dir.join("test-z-require-code-1.js"),
    );

    let refined = refine(output, &entrypoint, symlinks, PathStyle::Posix);

    assert_eq!(
        refined
            .symlinks
            .get("/test-z-require-content.css")
            .map(String::as_str),
        Some("/test-z-require-code-1.js")
    );
    Ok(())
}

#[test]
fn purges_redundant_top_directory_chains() {
    let mut records = BTreeMap::new();
    records.insert(
        PathBuf::from("/project"),
        directory_record("/project", ["app"]),
    );
    records.insert(
        PathBuf::from("/project/app"),
        directory_record("/project/app", ["src"]),
    );
    records.insert(
        PathBuf::from("/project/app/src"),
        directory_record("/project/app/src", ["index.js"]),
    );
    records.insert(
        PathBuf::from("/project/app/src/index.js"),
        file_record("/project/app/src/index.js"),
    );

    let refined = refine(
        WalkOutput {
            records,
            task_log: Vec::new(),
        },
        "/project/app/src/index.js",
        SymlinkMap::new(),
        PathStyle::Posix,
    );

    assert!(!refined.records.contains_key("/project"));
    assert!(refined.records.contains_key("/src/index.js"));
}

fn directory_record<const N: usize>(file: &str, children: [&str; N]) -> FileRecord {
    FileRecord {
        file: PathBuf::from(file),
        blob: false,
        content: false,
        links: true,
        stat: true,
        body: None,
        children: children.into_iter().map(ToOwned::to_owned).collect(),
        metadata: None,
    }
}

fn file_record(file: &str) -> FileRecord {
    FileRecord {
        file: PathBuf::from(file),
        blob: true,
        content: false,
        links: false,
        stat: true,
        body: None,
        children: Vec::new(),
        metadata: None,
    }
}
