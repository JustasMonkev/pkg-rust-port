#![allow(missing_docs)]

use pkg_rust::{Arch, Platform, TargetDefaults, TargetParseError, output_names, parse_targets};

fn defaults() -> TargetDefaults {
    TargetDefaults {
        node_range: "node18".to_owned(),
        platform: Platform::Linux,
        arch: Arch::X64,
    }
}

#[test]
fn parses_platform_only_targets_with_defaults() -> Result<(), TargetParseError> {
    let parsed = parse_targets("linux,macos,win", &defaults())?;

    assert_eq!(parsed.targets[0].to_string(), "node18-linux-x64");
    assert_eq!(parsed.targets[1].to_string(), "node18-macos-x64");
    assert_eq!(parsed.targets[2].to_string(), "node18-win-x64");
    Ok(())
}

#[test]
fn parses_full_targets_in_any_token_order() -> Result<(), TargetParseError> {
    let parsed = parse_targets("node16-win-arm64,linux-node18-x64", &defaults())?;

    assert_eq!(parsed.targets[0].to_string(), "node16-win-arm64");
    assert_eq!(parsed.targets[1].to_string(), "node18-linux-x64");
    Ok(())
}

#[test]
fn output_names_match_multi_target_suffix_rules() -> Result<(), TargetParseError> {
    let parsed = parse_targets("linux,macos,win", &defaults())?;

    assert_eq!(
        output_names("test-output", &parsed.targets),
        vec![
            "test-output-linux",
            "test-output-macos",
            "test-output-win.exe"
        ]
    );

    assert_eq!(
        output_names("test-output.exe", &parsed.targets),
        vec![
            "test-output.exe-linux",
            "test-output.exe-macos",
            "test-output.exe-win.exe"
        ]
    );
    Ok(())
}

#[test]
fn single_windows_target_gets_exe_extension() -> Result<(), TargetParseError> {
    let parsed = parse_targets("win", &defaults())?;

    assert_eq!(output_names("app", &parsed.targets), vec!["app.exe"]);
    assert_eq!(output_names("app.exe", &parsed.targets), vec!["app.exe"]);
    Ok(())
}
