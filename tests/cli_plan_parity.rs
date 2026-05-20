//! Parity tests for CLI planning behavior.

use std::ffi::OsString;

use pkg_rust::{Compression, PathStyle, PkgError, Platform, plan_package};

fn output_suffixes(plan: &pkg_rust::PackagePlan) -> Vec<String> {
    plan.outputs
        .iter()
        .map(|output| output.output.to_string_lossy().into_owned())
        .collect()
}

fn assert_output_suffixes(plan: &pkg_rust::PackagePlan, suffixes: &[&str]) {
    let outputs = output_suffixes(plan);
    assert_eq!(outputs.len(), suffixes.len());
    for (output, suffix) in outputs.iter().zip(suffixes) {
        assert!(
            output.ends_with(suffix),
            "expected output {output:?} to end with {suffix:?}"
        );
    }
}

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
fn plans_default_multi_target_outputs_for_bare_input() -> Result<(), Box<dyn std::error::Error>> {
    let plan = plan_package([OsString::from("../test/test-46-input/test-x-index")])?;

    assert!(plan.entrypoint.ends_with("test-46-input/test-x-index"));
    assert_output_suffixes(
        &plan,
        &[
            "test-x-index-linux",
            "test-x-index-macos",
            "test-x-index-win.exe",
        ],
    );
    Ok(())
}

#[test]
fn plans_default_multi_target_outputs_without_js_extension()
-> Result<(), Box<dyn std::error::Error>> {
    let plan = plan_package([OsString::from("../test/test-46-input-js/test-x-index.js")])?;

    assert!(
        plan.entrypoint
            .ends_with("test-46-input-js/test-x-index.js")
    );
    assert_output_suffixes(
        &plan,
        &[
            "test-x-index-linux",
            "test-x-index-macos",
            "test-x-index-win.exe",
        ],
    );
    Ok(())
}

#[test]
fn plans_out_path_multi_target_outputs() -> Result<(), Box<dyn std::error::Error>> {
    let output_root = std::env::temp_dir().join("pkg-rust-cli-plan-out-path");
    let output_root_text = output_root
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        OsString::from("--out-path"),
        OsString::from(output_root_text),
        OsString::from("../test/test-46-outpath/test-x-index"),
    ])?;

    assert_output_suffixes(
        &plan,
        &[
            "pkg-rust-cli-plan-out-path/test-x-index-linux",
            "pkg-rust-cli-plan-out-path/test-x-index-macos",
            "pkg-rust-cli-plan-out-path/test-x-index-win.exe",
        ],
    );
    Ok(())
}

#[test]
fn plans_package_json_targets_and_output_path_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let target_plan = plan_package([OsString::from(
        "../test/test-46-input-package-json-target/package.json",
    )])?;
    assert_eq!(target_plan.outputs.len(), 2);
    assert_eq!(target_plan.outputs[0].target.platform, Platform::Linux);
    assert_eq!(target_plan.outputs[1].target.platform, Platform::Macos);
    assert_output_suffixes(&target_plan, &["palookaville-linux", "palookaville-macos"]);

    let output_path_plan = plan_package([OsString::from(
        "../test/test-46-input-package-json-outputdir",
    )])?;
    assert_output_suffixes(
        &output_path_plan,
        &[
            "out/palookaville-linux",
            "out/palookaville-macos",
            "out/palookaville-win.exe",
        ],
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

#[test]
fn file_input_inside_package_keeps_package_directory_in_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir().join("pkg-rust-cli-plan-package-file");
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        OsString::from("--target"),
        OsString::from("host"),
        OsString::from("--output"),
        OsString::from(output_text),
        OsString::from("../test/test-50-package-json-6c/beta/alpha.js"),
    ])?;

    assert!(plan.root.ends_with("test-50-package-json-6c/beta"));
    assert!(plan.snapshot_base.ends_with("test-50-package-json-6c"));
    Ok(())
}

#[test]
fn file_input_inside_node_modules_package_keeps_node_modules_in_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir().join("pkg-rust-cli-plan-node-modules-file");
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        OsString::from("--target"),
        OsString::from("host"),
        OsString::from("--output"),
        OsString::from(output_text),
        OsString::from("../test/test-50-package-json-6b/node_modules/alpha/alpha.js"),
    ])?;

    assert!(
        plan.root
            .ends_with("test-50-package-json-6b/node_modules/alpha")
    );
    assert!(plan.snapshot_base.ends_with("test-50-package-json-6b"));
    Ok(())
}

#[tokio::test]
async fn exec_treats_version_as_successful_display() -> Result<(), Box<dyn std::error::Error>> {
    pkg_rust::exec(["--version"]).await?;
    Ok(())
}
