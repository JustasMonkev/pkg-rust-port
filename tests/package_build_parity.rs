//! Parity tests for package build orchestration.

use pkg_rust::{
    NodeTarget, PkgError, TargetBinaryProvider, build_package_with_provider, plan_package,
};

struct StubBinary;

impl TargetBinaryProvider for StubBinary {
    fn binary_for(&self, _target: &NodeTarget) -> Result<Vec<u8>, PkgError> {
        Ok(binary_with_placeholders())
    }
}

#[test]
fn builds_outputs_from_plan_with_stub_target_binary() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        std::env::temp_dir().join(format!("pkg-rust-package-build-{}", std::process::id()));
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        "--target",
        "linux",
        "--output",
        output_text,
        "--options",
        "trace-warnings",
        "../test/test-50-require-resolve/test-x-index.js",
    ])?;

    let build = build_package_with_provider(
        &plan,
        &StubBinary,
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
    )?;

    assert_eq!(build.outputs.len(), 1);
    assert_eq!(std::fs::read(&output)?, build.outputs[0].image.bytes);
    let binary_prefix = String::from_utf8_lossy(&build.outputs[0].image.bytes[..200]);
    assert!(binary_prefix.contains("--trace-warnings"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(&output)?.permissions().mode() & 0o111;
        assert_eq!(mode, 0o111);
    }

    std::fs::remove_file(output)?;
    Ok(())
}

#[test]
fn creates_missing_output_parent_directories() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir()
        .join(format!("pkg-rust-package-parent-{}", std::process::id()))
        .join("nested")
        .join("demo");
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        "--target",
        "linux",
        "--output",
        output_text,
        "../test/test-50-api/test-x-index.js",
    ])?;

    build_package_with_provider(
        &plan,
        &StubBinary,
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
    )?;

    assert!(output.is_file());
    let root = output
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or_else(|| PkgError::Cli("temporary output root is missing".to_owned()))?;
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn node_modules_file_input_synthesizes_intermediate_snapshot_directories()
-> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir().join(format!(
        "pkg-rust-package-node-modules-{}",
        std::process::id()
    ));
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        "--target",
        "linux",
        "--output",
        output_text,
        "../test/test-50-package-json-6b/node_modules/alpha/alpha.js",
    ])?;

    let build = build_package_with_provider(
        &plan,
        &StubBinary,
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
    )?;

    let image = String::from_utf8_lossy(&build.outputs[0].image.bytes);
    assert!(image.contains("\"/snapshot/node_modules\""));
    assert!(image.contains("\"/snapshot/node_modules/alpha/beta.js\""));

    std::fs::remove_file(output)?;
    Ok(())
}

#[test]
fn refuses_to_overwrite_non_file_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir().join(format!(
        "pkg-rust-package-output-dir-{}",
        std::process::id()
    ));
    let _ignored = std::fs::remove_dir_all(&output);
    std::fs::create_dir_all(&output)?;
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let plan = plan_package([
        "--target",
        "linux",
        "--output",
        output_text,
        "../test/test-50-api/test-x-index.js",
    ])?;

    let error = build_package_with_provider(
        &plan,
        &StubBinary,
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
    )
    .err();

    assert!(
        matches!(error, Some(PkgError::Cli(message)) if message.contains("Refusing to overwrite non-file output"))
    );
    std::fs::remove_dir_all(output)?;
    Ok(())
}

fn binary_with_placeholders() -> Vec<u8> {
    let mut binary = Vec::from([b'\0']);
    for _index in 0..20 {
        binary.extend_from_slice(b"// BAKERY ");
    }
    binary.extend_from_slice(b"// PAYLOAD_POSITION //");
    binary.extend_from_slice(b"// PAYLOAD_SIZE //");
    binary.extend_from_slice(b"// PRELUDE_POSITION //");
    binary.extend_from_slice(b"// PRELUDE_SIZE //");
    binary
}
