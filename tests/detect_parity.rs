#![allow(missing_docs)]

use pkg_rust::{
    AliasKind, DetectionKind, PkgError, detect, non_literal_and_cwd_debug_lines,
    successful_debug_lines,
};

#[test]
fn successful_debug_lines_match_ast_parsing_fixture() -> Result<(), PkgError> {
    let source = include_str!("../../test/test-50-ast-parsing/test-y-data.txt");
    let expected = source
        .lines()
        .filter_map(|line| line.split_once("/***/ ").map(|(_prefix, suffix)| suffix))
        .map(|line| line.replace(['\'', '`'], "\""))
        .collect::<Vec<_>>();

    assert_eq!(successful_debug_lines(source)?, expected);
    Ok(())
}

#[test]
fn non_literal_and_cwd_lines_match_ast_parsing_2_fixture() -> Result<(), PkgError> {
    let source = include_str!("../../test/test-50-ast-parsing-2/test-x-index.js");
    let expected = source
        .lines()
        .filter_map(|line| line.split("/**/").nth(1))
        .map(str::trim)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    assert_eq!(non_literal_and_cwd_debug_lines(source)?, expected);
    Ok(())
}

#[test]
fn detect_returns_typed_static_derivatives() -> Result<(), PkgError> {
    let uses = detect(
        r#"require("pkg"); require.resolve("other", "may-exclude"); path.join(__dirname, "asset.css");"#,
    )?;

    let derivatives = uses
        .into_iter()
        .filter_map(|detected| match detected.kind {
            DetectionKind::Successful(derivative) => Some(derivative),
            DetectionKind::NonLiteral { .. }
            | DetectionKind::Malformed { .. }
            | DetectionKind::AmbiguousCwd { .. } => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(derivatives.len(), 3);
    assert_eq!(derivatives[0].alias, "pkg");
    assert_eq!(derivatives[0].alias_kind, AliasKind::Resolvable);
    assert_eq!(derivatives[1].alias, "other");
    assert!(derivatives[1].may_exclude);
    assert_eq!(derivatives[2].alias, "asset.css");
    assert_eq!(derivatives[2].alias_kind, AliasKind::Relative);
    Ok(())
}

#[test]
fn detect_accepts_commonjs_top_level_return() -> Result<(), PkgError> {
    let source = include_str!("../../test/test-50-spawn/test-cluster.js");
    let uses = detect(source)?;

    assert!(uses.iter().any(|detected| matches!(
        &detected.kind,
        DetectionKind::Successful(derivative) if derivative.alias == "./test-cluster-child.js"
    )));
    Ok(())
}

#[test]
fn detect_finds_spawn_child_require_resolve() -> Result<(), PkgError> {
    let source = include_str!("../../test/test-50-spawn/test-cpfork-a-1.js");
    let uses = detect(source)?;

    assert!(uses.iter().any(|detected| matches!(
        &detected.kind,
        DetectionKind::Successful(derivative) if derivative.alias == "./test-cpfork-a-child.js"
    )));
    Ok(())
}
