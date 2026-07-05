//! Parity tests for producer prelude template assembly.

use pkg_rust::{PKG_VERSION, prelude_template};

/// test-78-verify-pkg-version: the packaged app prints `process.versions.pkg`,
/// which the JS bootstrap sets to `'%VERSION%'`. The rendered prelude must
/// substitute the original pkg version so the produced executable reports it.
#[test]
fn prelude_injects_pkg_version_for_process_versions() {
    let template = prelude_template(false);

    assert_eq!(PKG_VERSION, "6.19.0");
    assert!(template.contains(&format!("process.versions.pkg = '{PKG_VERSION}';")));
    assert!(!template.contains("process.versions.pkg = '%VERSION%';"));
}

#[test]
fn prelude_template_matches_packer_wrapper_shape() {
    let template = prelude_template(false);

    assert!(template.starts_with(
        "return (function (REQUIRE_COMMON, REQUIRE_SHARED, VIRTUAL_FILESYSTEM, DEFAULT_ENTRYPOINT, SYMLINKS, DICT, DOCOMPRESS)"
    ));
    assert!(template.contains("const common = {};"));
    assert!(template.contains("REQUIRE_COMMON(common);"));
    assert!(template.contains("exports.STORE_BLOB = 0;"));
    assert!(template.contains("(function () { var module = { exports: {} };"));
    assert!(template.contains("return module.exports; })(),"));
    assert!(template.contains("pickDecompressorSync"));
    assert!(template.contains("%VIRTUAL_FILESYSTEM%"));
    assert!(template.contains("%DEFAULT_ENTRYPOINT%"));
    assert!(template.contains("%SYMLINKS%"));
    assert!(template.contains("%DICT%"));
    assert!(template.contains("%DOCOMPRESS%"));
    assert!(!template.contains("%VERSION%"));
}

#[test]
fn debug_prelude_includes_diagnostic_runtime() {
    let template = prelude_template(true);

    assert!(template.contains("REQUIRE_SHARED.installDiagnostic(snapshotPrefix);"));
    assert!(template.contains("------------------------------- path dictionary"));
}

#[test]
fn release_prelude_omits_diagnostic_runtime() {
    let template = prelude_template(false);

    assert!(!template.contains("REQUIRE_SHARED.installDiagnostic(snapshotPrefix);"));
}
