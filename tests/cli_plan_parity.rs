//! Parity tests for CLI planning behavior.

use std::ffi::OsString;

use pkg_rust::{Compression, PathStyle, PkgError, Platform, plan_package};

#[test]
fn plans_package_json_input_outputs_and_targets() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir().join("pkg-rust-cli-plan");
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        OsString::from("--targets"),
        OsString::from("linux,win"),
        OsString::from("--output"),
        OsString::from(output_text),
        OsString::from("../test/test-46-input-package-json"),
    ])?;

    assert!(plan.entrypoint.ends_with("test-x-index.js"));
    assert!(plan.snapshot_base.ends_with("test"));
    assert_eq!(plan.compression, Compression::None);
    assert!(plan.bytecode);
    assert_eq!(plan.outputs.len(), 2);
    assert_eq!(plan.outputs[0].target.platform, Platform::Linux);
    assert_eq!(plan.outputs[0].path_style, PathStyle::Posix);
    assert!(plan.outputs[0].output.ends_with("pkg-rust-cli-plan-linux"));
    assert_eq!(plan.outputs[1].target.platform, Platform::Win);
    assert_eq!(plan.outputs[1].path_style, PathStyle::Windows);
    assert!(
        plan.outputs[1]
            .output
            .ends_with("pkg-rust-cli-plan-win.exe")
    );
    Ok(())
}

#[test]
fn plans_options_and_compression() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir().join("pkg-rust-cli-plan-options");
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        OsString::from("--target"),
        OsString::from("host"),
        OsString::from("--output"),
        OsString::from(output_text),
        OsString::from("--options"),
        OsString::from("trace-warnings,max-old-space-size=64"),
        OsString::from("--compress"),
        OsString::from("br"),
        OsString::from("--no-bytecode"),
        OsString::from("../test/test-50-require-resolve/test-x-index.js"),
    ])?;

    assert_eq!(plan.compression, Compression::Brotli);
    assert!(plan.snapshot_base.ends_with("test-50-require-resolve"));
    assert!(!plan.bytecode);
    assert_eq!(
        plan.bakes,
        vec!["--trace-warnings", "--max-old-space-size=64"]
    );
    assert_eq!(plan.outputs.len(), 1);
    Ok(())
}

#[tokio::test]
async fn exec_treats_version_as_successful_display() -> Result<(), Box<dyn std::error::Error>> {
    pkg_rust::exec(["--version"]).await?;
    Ok(())
}
