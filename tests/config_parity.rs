#![allow(missing_docs)]

use std::path::Path;

use pkg_rust::PackageJson;

#[test]
fn parses_string_bin_from_package_json_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let package = PackageJson::parse(include_str!(
        "../../test/test-46-input-package-json/package.json"
    ))?;

    assert_eq!(package.name.as_deref(), Some("palookaville"));
    assert_eq!(package.package_basename().as_deref(), Some("palookaville"));
    assert_eq!(package.selected_bin().as_deref(), Some("test-x-index.js"));
    assert_eq!(
        package
            .resolve_selected_bin(Path::new(
                "../../test/test-46-input-package-json/package.json"
            ))
            .as_deref(),
        Some(Path::new(
            "../../test/test-46-input-package-json/test-x-index.js"
        ))
    );
    Ok(())
}

#[test]
fn strips_scope_for_package_basename() -> Result<(), Box<dyn std::error::Error>> {
    let package = PackageJson::parse(include_str!(
        "../../test/test-46-input-package-json-dir-scope/package.json"
    ))?;

    assert_eq!(package.name.as_deref(), Some("@org/palookaville"));
    assert_eq!(package.package_basename().as_deref(), Some("palookaville"));
    assert_eq!(package.selected_bin().as_deref(), Some("test-x-index.js"));
    Ok(())
}

#[test]
fn reads_pkg_output_path_and_targets() -> Result<(), Box<dyn std::error::Error>> {
    let output_package = PackageJson::parse(include_str!(
        "../../test/test-46-input-package-json-outputdir/package.json"
    ))?;
    assert_eq!(
        output_package.pkg.and_then(|pkg| pkg.output_path),
        Some("out".to_owned())
    );

    let target_package = PackageJson::parse(include_str!(
        "../../test/test-46-input-package-json-target/package.json"
    ))?;
    assert_eq!(
        target_package.pkg.map(|pkg| pkg.targets),
        Some(vec!["linux".to_owned(), "macos".to_owned()])
    );
    Ok(())
}

#[test]
fn object_bin_prefers_package_basename_then_first_entry() -> Result<(), Box<dyn std::error::Error>>
{
    let matching =
        PackageJson::parse(r#"{"name":"@scope/app","bin":{"other":"other.js","app":"app.js"}}"#)?;
    assert_eq!(matching.selected_bin().as_deref(), Some("app.js"));

    let fallback =
        PackageJson::parse(r#"{"name":"app","bin":{"first":"first.js","second":"second.js"}}"#)?;
    assert_eq!(fallback.selected_bin().as_deref(), Some("first.js"));
    Ok(())
}

#[test]
fn treats_non_string_main_as_absent() -> Result<(), Box<dyn std::error::Error>> {
    let package = PackageJson::parse(r#"{"name":"dunder-proto","main":false}"#)?;

    assert_eq!(package.name.as_deref(), Some("dunder-proto"));
    assert_eq!(package.main, None);
    Ok(())
}
