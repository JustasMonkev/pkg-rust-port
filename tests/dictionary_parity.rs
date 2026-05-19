#![allow(missing_docs)]

use serde_json::json;

use pkg_rust::{PackageJson, active_dependencies, apply_dictionary_entry, lookup_dictionary};

#[test]
fn sequelize_dictionary_replaces_pkg_scripts() -> Result<(), Box<dyn std::error::Error>> {
    let mut package = PackageJson::parse(r#"{"name":"sequelize"}"#)?;
    let entry = lookup_dictionary("sequelize").ok_or("missing sequelize dictionary")?;

    apply_dictionary_entry(&mut package, &entry);

    assert_eq!(
        package.pkg.map(|pkg| pkg.scripts),
        Some(json!(["lib/**/*.js"]))
    );
    Ok(())
}

#[test]
fn publicsuffixlist_disables_dictionary_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let mut package = PackageJson::parse(
        r#"{
          "name": "publicsuffixlist",
          "dependencies": {
            "gulp": "*",
            "left-alone": "*"
          }
        }"#,
    )?;
    let entry =
        lookup_dictionary("publicsuffixlist").ok_or("missing publicsuffixlist dictionary")?;

    apply_dictionary_entry(&mut package, &entry);

    assert_eq!(
        package.pkg.as_ref().map(|pkg| &pkg.assets),
        Some(&json!(["effective_tld_names.dat"]))
    );
    assert_eq!(active_dependencies(&package), vec!["left-alone"]);
    assert_eq!(
        package.dependencies.get("gulp"),
        Some(&serde_json::Value::Null)
    );
    Ok(())
}

#[test]
fn express_dictionary_carries_patch_pairs() -> Result<(), Box<dyn std::error::Error>> {
    let mut package = PackageJson::parse(r#"{"name":"express"}"#)?;
    let entry = lookup_dictionary("express").ok_or("missing express dictionary")?;

    apply_dictionary_entry(&mut package, &entry);

    let patches = package
        .pkg
        .and_then(|pkg| pkg.patches.get("lib/view.js").cloned())
        .ok_or("missing express patch")?;
    assert_eq!(patches.as_array().map(Vec::len), Some(4));
    assert_eq!(patches[0], "path = join(this.root, path)");
    Ok(())
}

#[test]
fn opn_alias_uses_open_dictionary_entry() {
    assert_eq!(lookup_dictionary("opn"), lookup_dictionary("open"));
}
