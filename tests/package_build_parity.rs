//! Parity tests for package build orchestration.

use pkg_rust::{
    NodeTarget, PkgError, TargetBinary, TargetBinaryProvider, build_package_with_provider,
    plan_package,
};

struct StubBinary;

impl TargetBinaryProvider for StubBinary {
    fn binary_for(&self, _target: &NodeTarget) -> Result<Vec<u8>, PkgError> {
        Ok(binary_with_placeholders())
    }
}

struct StubBinaryWithPath {
    path: std::path::PathBuf,
}

impl TargetBinaryProvider for StubBinaryWithPath {
    fn binary_for(&self, _target: &NodeTarget) -> Result<Vec<u8>, PkgError> {
        Ok(binary_with_placeholders())
    }

    fn binary_artifact_for(&self, target: &NodeTarget) -> Result<TargetBinary, PkgError> {
        self.binary_for(target)
            .map(TargetBinary::from_bytes)
            .map(|binary| binary.with_path(self.path.clone()))
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
fn copies_deploy_files_next_to_output_executable() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "pkg-rust-package-deploy-copy-{}",
        std::process::id()
    ));
    let _ignored = std::fs::remove_dir_all(&root);
    let package_dir = root.join("package");
    let output = root.join("dist").join("demo");
    std::fs::create_dir_all(package_dir.join("assets/nested"))?;
    std::fs::write(
        package_dir.join("app.js"),
        "'use strict';\nconsole.log('ok');\n",
    )?;
    std::fs::write(package_dir.join("tool.sh"), "#!/bin/sh\n")?;
    std::fs::write(package_dir.join("assets/nested/data.txt"), "payload\n")?;
    std::fs::write(
        package_dir.join("package.json"),
        r#"{
          "name": "demo",
          "bin": "app.js",
          "pkg": {
            "deployFiles": [
              ["tool.sh", "tools/tool.sh"],
              ["assets", "assets-copy", "directory"],
              ["missing.txt", "missing.txt"]
            ]
          }
        }"#,
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(
            package_dir.join("tool.sh"),
            PermissionsExt::from_mode(0o744),
        )?;
    }

    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let package_text = package_dir
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary package path must be utf-8".to_owned()))?;
    let plan = plan_package(["--target", "linux", "--output", output_text, package_text])?;

    let build = build_package_with_provider(
        &plan,
        &StubBinary,
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
    )?;

    assert_eq!(build.outputs.len(), 1);
    assert!(output.is_file());
    assert_eq!(
        std::fs::read_to_string(root.join("dist/tools/tool.sh"))?,
        "#!/bin/sh\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("dist/assets-copy/nested/data.txt"))?,
        "payload\n"
    );
    assert!(!root.join("dist/missing.txt").exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(root.join("dist/tools/tool.sh"))?
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o744);
    }

    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn native_build_uses_cached_platform_addon_when_available() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::env::temp_dir().join(format!(
        "pkg-rust-package-native-cache-{}",
        std::process::id()
    ));
    let _ignored = std::fs::remove_dir_all(&root);
    let package_dir = root.join("package");
    let output = root.join("dist").join("demo");
    std::fs::create_dir_all(&package_dir)?;
    std::fs::write(package_dir.join("app.js"), "require('./addon.node');\n")?;
    let addon = package_dir.join("addon.node");
    std::fs::write(&addon, b"ORIGINAL_NATIVE")?;
    let canonical_addon = addon.canonicalize()?;
    std::fs::write(
        format!("{}.linux.v18.5.0", canonical_addon.display()),
        b"PLATFORM_NATIVE",
    )?;
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"name":"demo","bin":"app.js"}"#,
    )?;
    let binary_path = root.join("cache").join("fetched-v18.5.0-linux-x64");
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let package_text = package_dir
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary package path must be utf-8".to_owned()))?;
    let plan = plan_package(["--target", "linux", "--output", output_text, package_text])?;

    let build = build_package_with_provider(
        &plan,
        &StubBinaryWithPath { path: binary_path },
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
    )?;

    let image = &build.outputs[0].image.bytes;
    assert!(contains_bytes(image, b"PLATFORM_NATIVE"));
    assert!(!contains_bytes(image, b"ORIGINAL_NATIVE"));

    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn no_native_build_keeps_original_addon_payload() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "pkg-rust-package-no-native-cache-{}",
        std::process::id()
    ));
    let _ignored = std::fs::remove_dir_all(&root);
    let package_dir = root.join("package");
    let output = root.join("dist").join("demo");
    std::fs::create_dir_all(&package_dir)?;
    std::fs::write(package_dir.join("app.js"), "require('./addon.node');\n")?;
    let addon = package_dir.join("addon.node");
    std::fs::write(&addon, b"ORIGINAL_NATIVE")?;
    let canonical_addon = addon.canonicalize()?;
    std::fs::write(
        format!("{}.linux.v18.5.0", canonical_addon.display()),
        b"PLATFORM_NATIVE",
    )?;
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"name":"demo","bin":"app.js"}"#,
    )?;
    let binary_path = root.join("cache").join("fetched-v18.5.0-linux-x64");
    let output_text = output
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary output path must be utf-8".to_owned()))?;
    let package_text = package_dir
        .to_str()
        .ok_or_else(|| PkgError::Cli("temporary package path must be utf-8".to_owned()))?;
    let plan = plan_package([
        "--target",
        "linux",
        "--output",
        output_text,
        "--no-native-build",
        package_text,
    ])?;

    let build = build_package_with_provider(
        &plan,
        &StubBinaryWithPath { path: binary_path },
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
    )?;

    let image = &build.outputs[0].image.bytes;
    assert!(contains_bytes(image, b"ORIGINAL_NATIVE"));
    assert!(!contains_bytes(image, b"PLATFORM_NATIVE"));

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
fn escaped_dependency_falls_back_to_common_snapshot_denominator()
-> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir().join(format!(
        "pkg-rust-package-native-escape-{}",
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
        "../test/test-50-native-addon-3/lib/test-x-index.js",
    ])?;

    let build = build_package_with_provider(
        &plan,
        &StubBinary,
        "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
    )?;

    let image = String::from_utf8_lossy(&build.outputs[0].image.bytes);
    assert!(image.contains("\"/snapshot/lib/test-x-index.js\""));
    assert!(image.contains("\"/snapshot/node_modules\""));
    assert!(image.contains("\"/snapshot/node_modules/dependency/time-d.node\""));
    assert!(!image.contains("\"e_modules/dependency/time-d.node\""));

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

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
