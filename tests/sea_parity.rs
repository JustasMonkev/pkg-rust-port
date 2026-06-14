//! Parity tests for the SEA (Single Executable Application) surface: the
//! deterministic nodejs.org mapping/validation helpers and CLI/config planning.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use pkg_rust::{
    Arch, PkgError, Platform, TargetDefaults, parse_targets, plan_package,
    sea_assert_host_node_version, sea_assert_single_target_major, sea_node_arch,
    sea_node_archive_filename, sea_node_dist_urls, sea_node_os,
    sea_pick_matching_host_target_index, sea_resolve_min_target_major,
    sea_validate_node_version_format,
};

fn unique_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "pkg-rust-sea-parity-{label}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn node_os_and_arch_mapping_matches_upstream() -> Result<(), PkgError> {
    assert_eq!(sea_node_os(Platform::Macos)?, "darwin");
    assert_eq!(sea_node_os(Platform::Linux)?, "linux");
    assert_eq!(sea_node_os(Platform::Win)?, "win");
    assert!(matches!(
        sea_node_os(Platform::Freebsd),
        Err(PkgError::Sea(message)) if message == "Unsupported OS: freebsd"
    ));

    assert_eq!(sea_node_arch(Arch::X64)?, "x64");
    assert_eq!(sea_node_arch(Arch::Arm64)?, "arm64");
    assert_eq!(sea_node_arch(Arch::Ppc64)?, "ppc64");
    assert_eq!(sea_node_arch(Arch::Loong64)?, "loong64");
    // yao-pkg's NODE_ARCHS uses `armv7l`/has no `x86`, so these reject.
    assert!(matches!(
        sea_node_arch(Arch::Armv7),
        Err(PkgError::Sea(message)) if message == "Unsupported architecture: armv7"
    ));
    Ok(())
}

#[test]
fn archive_filenames_and_dist_urls_match_upstream() {
    assert_eq!(
        sea_node_archive_filename("v24.15.0", "linux", "arm64"),
        "node-v24.15.0-linux-arm64.tar.gz"
    );
    assert_eq!(
        sea_node_archive_filename("v24.15.0", "win", "x64"),
        "node-v24.15.0-win-x64.zip"
    );

    let (url, sums) = sea_node_dist_urls("v24.15.0", "darwin", "arm64");
    assert_eq!(
        url,
        "https://nodejs.org/dist/v24.15.0/node-v24.15.0-darwin-arm64.tar.gz"
    );
    assert_eq!(sums, "https://nodejs.org/dist/v24.15.0/SHASUMS256.txt");

    // riscv64/loong64 route through unofficial-builds.
    let (url, sums) = sea_node_dist_urls("v22.0.0", "linux", "loong64");
    assert_eq!(
        url,
        "https://unofficial-builds.nodejs.org/download/release/v22.0.0/node-v22.0.0-linux-loong64.tar.gz"
    );
    assert_eq!(
        sums,
        "https://unofficial-builds.nodejs.org/download/release/v22.0.0/SHASUMS256.txt"
    );
}

#[test]
fn version_format_and_host_assertion() -> Result<(), PkgError> {
    assert!(sea_validate_node_version_format("22"));
    assert!(sea_validate_node_version_format("22.22"));
    assert!(sea_validate_node_version_format("22.22.2"));
    assert!(!sea_validate_node_version_format("v22"));
    assert!(!sea_validate_node_version_format("22.22.2.1"));

    assert_eq!(sea_assert_host_node_version("v22.0.0")?, 22);
    assert!(matches!(
        sea_assert_host_node_version("v18.20.0"),
        Err(PkgError::Sea(message))
            if message == "SEA support requires at least node v22.0.0, actual node version is v18.20.0"
    ));
    Ok(())
}

#[test]
fn single_major_and_generator_selection() -> Result<(), Box<dyn std::error::Error>> {
    let defaults = TargetDefaults::host("node22");
    let same = parse_targets("node22-linux,node22-win", &defaults)?.targets;
    assert!(sea_assert_single_target_major(&same, 22).is_ok());
    assert_eq!(sea_resolve_min_target_major(&same, 25), 22);

    let mixed = parse_targets("node22-linux,node24-linux", &defaults)?.targets;
    assert!(matches!(
        sea_assert_single_target_major(&mixed, 22),
        Err(PkgError::Sea(message))
            if message == "SEA mode cannot mix Node.js majors in a single run (got 22, 24). Run pkg once per Node major."
    ));

    let list = parse_targets("node22-linux-x64,node22-win-x64", &defaults)?.targets;
    assert_eq!(
        sea_pick_matching_host_target_index(Platform::Linux, Arch::X64, &list),
        Some(0)
    );
    assert_eq!(
        sea_pick_matching_host_target_index(Platform::Macos, Arch::Arm64, &list),
        None
    );
    Ok(())
}

#[test]
fn cli_sea_flag_planning_and_mode_selection() -> Result<(), Box<dyn std::error::Error>> {
    // Bare entry file, no package.json/config -> simple SEA mode.
    let dir = unique_dir("simple");
    fs::create_dir_all(&dir)?;
    let entry = dir.join("app.js");
    fs::write(&entry, "console.log('hi');")?;
    let entry_text = entry
        .to_str()
        .ok_or_else(|| PkgError::Cli("temp path must be utf-8".to_owned()))?;
    let out = dir.join("app-out");
    let out_text = out
        .to_str()
        .ok_or_else(|| PkgError::Cli("temp path must be utf-8".to_owned()))?;

    let plan = plan_package(["--sea", "-t", "node22-linux", "-o", out_text, entry_text])?;
    assert!(plan.sea, "--sea sets the sea flag");
    assert!(!plan.sea_enhanced, "bare entry uses simple SEA mode");

    let plan = plan_package(["-t", "node22-linux", "-o", out_text, entry_text])?;
    assert!(!plan.sea, "sea is off without the flag");

    let _ = fs::remove_dir_all(&dir);

    // package.json directory input -> enhanced SEA mode.
    let plan = plan_package([
        "--sea",
        "-t",
        "node22-linux",
        "test/test-46-input-package-json",
    ])?;
    assert!(plan.sea);
    assert!(
        plan.sea_enhanced,
        "package.json input uses enhanced SEA mode"
    );
    Ok(())
}

#[test]
fn config_sea_flag_resolves_with_cli_precedence() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("config");
    fs::create_dir_all(&dir)?;
    let entry = dir.join("app.js");
    fs::write(&entry, "console.log('hi');")?;
    // A bare `.pkgrc` is wrapped as `{ "pkg": { ... } }`.
    fs::write(
        dir.join(".pkgrc"),
        r#"{"sea": true, "targets": ["node22-linux"]}"#,
    )?;
    let entry_text = entry
        .to_str()
        .ok_or_else(|| PkgError::Cli("temp path must be utf-8".to_owned()))?;

    // Config enables sea.
    let plan = plan_package([entry_text])?;
    assert!(plan.sea, "config sea:true enables SEA");

    // CLI --no-sea overrides config sea:true.
    let plan = plan_package(["--no-sea", entry_text])?;
    assert!(!plan.sea, "--no-sea overrides config sea:true");

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}
