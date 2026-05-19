//! Parity tests for producer prelude template assembly.

use pkg_rust::prelude_template;

#[test]
fn prelude_template_matches_packer_wrapper_shape() {
    let template = prelude_template(false);

    assert!(template.starts_with(
        "return (function (REQUIRE_COMMON, VIRTUAL_FILESYSTEM, DEFAULT_ENTRYPOINT, SYMLINKS, DICT, DOCOMPRESS)"
    ));
    assert!(template.contains("const common = {};"));
    assert!(template.contains("REQUIRE_COMMON(common);"));
    assert!(template.contains("exports.STORE_BLOB = 0;"));
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

    assert!(template.contains("function installDiagnostic()"));
    assert!(template.contains("virtual file system"));
}
