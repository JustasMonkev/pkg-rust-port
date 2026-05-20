//! Parity tests for pkg-fetch cache naming.

use std::fs;

use pkg_rust::{
    BinaryKind, PkgError, PkgFetchCache, TargetBinaryProvider, TargetDefaults, parse_targets,
};

#[test]
fn cache_path_matches_pkg_fetch_local_place() -> Result<(), Box<dyn std::error::Error>> {
    let cache = PkgFetchCache::new("/tmp/pkg-cache");
    let defaults = TargetDefaults::host("node18");
    let target = parse_targets("node18-macos-arm64", &defaults)?
        .targets
        .remove(0);

    assert_eq!(
        cache.binary_path(&target, BinaryKind::Fetched)?,
        std::path::PathBuf::from("/tmp/pkg-cache/v3.5/fetched-v18.15.0-macos-arm64")
    );
    assert_eq!(
        cache.binary_path(&target, BinaryKind::Built)?,
        std::path::PathBuf::from("/tmp/pkg-cache/v3.5/built-v18.15.0-macos-arm64")
    );
    Ok(())
}

#[test]
fn cache_provider_removes_bad_fetched_and_reads_built() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("pkg-rust-fetch-cache-{}", std::process::id()));
    let cache = PkgFetchCache::new(&root);
    let defaults = TargetDefaults::host("node18");
    let target = parse_targets("linux-x64", &defaults)?.targets.remove(0);
    let fetched = cache.binary_path(&target, BinaryKind::Fetched)?;
    let built = cache.binary_path(&target, BinaryKind::Built)?;
    fs::create_dir_all(
        fetched
            .parent()
            .ok_or_else(|| PkgError::Fetch("cache path has no parent".to_owned()))?,
    )?;
    fs::write(&built, b"built")?;
    fs::write(&fetched, b"fetched")?;

    let artifact = cache.binary_artifact_for(&target)?;
    assert_eq!(artifact.bytes(), b"built");
    assert_eq!(artifact.path(), Some(built.as_path()));
    assert!(!fetched.exists());
    fs::remove_file(built)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn cache_provider_removes_bad_fetched_when_built_is_absent()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "pkg-rust-fetch-cache-bad-fetched-{}",
        std::process::id()
    ));
    let cache = PkgFetchCache::new(&root);
    let defaults = TargetDefaults::host("node18");
    let target = parse_targets("linux-x64", &defaults)?.targets.remove(0);
    let fetched = cache.binary_path(&target, BinaryKind::Fetched)?;
    fs::create_dir_all(
        fetched
            .parent()
            .ok_or_else(|| PkgError::Fetch("cache path has no parent".to_owned()))?,
    )?;
    fs::write(&fetched, b"not the expected binary")?;

    let error = cache.binary_for(&target).err();

    assert!(
        matches!(error, Some(PkgError::Fetch(message)) if message.contains("no cached binary"))
    );
    assert!(!fetched.exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn cache_provider_errors_when_binary_is_absent() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "pkg-rust-fetch-cache-missing-{}",
        std::process::id()
    ));
    let cache = PkgFetchCache::new(root);
    let defaults = TargetDefaults::host("node18");
    let target = parse_targets("linux-x64", &defaults)?.targets.remove(0);

    let error = cache.binary_for(&target).err();

    assert!(
        matches!(error, Some(PkgError::Fetch(message)) if message.contains("no cached binary"))
    );
    Ok(())
}

#[test]
fn force_build_reads_only_built_cache_artifacts() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "pkg-rust-fetch-cache-force-build-{}",
        std::process::id()
    ));
    let cache = PkgFetchCache::new(&root);
    let defaults = TargetDefaults::host("node18");
    let mut target = parse_targets("linux-x64", &defaults)?.targets.remove(0);
    target.force_build = true;
    let fetched = cache.binary_path(&target, BinaryKind::Fetched)?;
    let built = cache.binary_path(&target, BinaryKind::Built)?;
    fs::create_dir_all(
        built
            .parent()
            .ok_or_else(|| PkgError::Fetch("cache path has no parent".to_owned()))?,
    )?;
    fs::write(&fetched, b"fetched")?;
    fs::write(&built, b"built")?;

    let artifact = cache.binary_artifact_for(&target)?;

    assert_eq!(artifact.bytes(), b"built");
    assert_eq!(artifact.path(), Some(built.as_path()));
    assert!(fetched.exists());
    fs::remove_file(fetched)?;
    fs::remove_file(built)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn force_build_errors_when_built_cache_artifact_is_absent() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::env::temp_dir().join(format!(
        "pkg-rust-fetch-cache-force-build-missing-{}",
        std::process::id()
    ));
    let cache = PkgFetchCache::new(&root);
    let defaults = TargetDefaults::host("node18");
    let mut target = parse_targets("linux-x64", &defaults)?.targets.remove(0);
    target.force_build = true;
    let fetched = cache.binary_path(&target, BinaryKind::Fetched)?;
    fs::create_dir_all(
        fetched
            .parent()
            .ok_or_else(|| PkgError::Fetch("cache path has no parent".to_owned()))?,
    )?;
    fs::write(&fetched, b"fetched")?;

    let error = cache.binary_for(&target).err();

    assert!(matches!(error, Some(PkgError::Fetch(message)) if message.contains("no built binary")));
    assert!(fetched.exists());
    fs::remove_file(fetched)?;
    fs::remove_dir_all(root)?;
    Ok(())
}
