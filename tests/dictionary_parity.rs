#![allow(missing_docs)]

use serde_json::json;

use pkg_rust::{
    DictionaryLog, PackageJson, active_dependencies, apply_dictionary_entry, lookup_dictionary,
};

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
fn dynamic_require_dictionaries_carry_script_globs() -> Result<(), Box<dyn std::error::Error>> {
    let busboy = lookup_dictionary("busboy").ok_or("missing busboy dictionary")?;
    let log4js = lookup_dictionary("log4js").ok_or("missing log4js dictionary")?;

    assert_eq!(
        busboy.pkg.as_ref().map(|pkg| &pkg.scripts),
        Some(&json!(["lib/types/*.js"]))
    );
    assert_eq!(
        log4js.pkg.as_ref().map(|pkg| &pkg.scripts),
        Some(&json!(["lib/appenders/*.js"]))
    );
    Ok(())
}

#[test]
fn stylus_dictionary_carries_asset_glob_and_log() -> Result<(), Box<dyn std::error::Error>> {
    let stylus = lookup_dictionary("stylus").ok_or("missing stylus dictionary")?;

    assert_eq!(
        stylus.pkg.as_ref().map(|pkg| &pkg.assets),
        Some(&json!(["lib/**/*.styl"]))
    );
    assert_eq!(stylus.logs, vec![DictionaryLog::StylusResolveImports]);
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

#[test]
fn open_dictionary_carries_xdg_open_patch_and_deploy_file() -> Result<(), Box<dyn std::error::Error>>
{
    let mut package = PackageJson::parse(r#"{"name":"open"}"#)?;
    let entry = lookup_dictionary("open").ok_or("missing open dictionary")?;

    apply_dictionary_entry(&mut package, &entry);

    let config = package.pkg.ok_or("missing open pkg config")?;
    assert_eq!(config.deploy_files, json!([["xdg-open", "xdg-open"]]));
    assert_eq!(
        config.patches.get("index.js"),
        Some(&json!([
            "path.join(__dirname, 'xdg-open')",
            "path.join(path.dirname(process.execPath), 'xdg-open')"
        ]))
    );
    Ok(())
}

#[test]
fn native_dictionary_entries_carry_directory_deploy_files() -> Result<(), Box<dyn std::error::Error>>
{
    let cases = [
        (
            "leveldown",
            json!([["prebuilds", "prebuilds", "directory"]]),
            "binding.js",
            json!(["__dirname", "require('path').dirname(process.execPath)"]),
        ),
        (
            "puppeteer",
            json!([[".local-chromium", "puppeteer", "directory"]]),
            "utils/ChromiumDownloader.js",
            json!([
                "path.join(__dirname, '..', '.local-chromium')",
                "path.join(path.dirname(process.execPath), 'puppeteer')"
            ]),
        ),
        (
            "zeromq",
            json!([["prebuilds", "prebuilds", "directory"]]),
            "lib/native.js",
            json!([
                "path.join(__dirname, \"..\")",
                "path.dirname(process.execPath)"
            ]),
        ),
    ];

    for (name, deploy_files, patch_path, patch_ops) in cases {
        let mut package = PackageJson::parse(&format!(r#"{{"name":"{name}"}}"#))?;
        let entry = lookup_dictionary(name).ok_or("missing native dictionary")?;

        apply_dictionary_entry(&mut package, &entry);

        let config = package.pkg.ok_or("missing native pkg config")?;
        assert_eq!(config.deploy_files, deploy_files);
        assert_eq!(config.patches.get(patch_path), Some(&patch_ops));
    }

    Ok(())
}

#[test]
fn file_deploy_dictionaries_carry_patch_and_deploy_files() -> Result<(), Box<dyn std::error::Error>>
{
    let cases = [
        (
            "exiftool.exe",
            json!([["vendor/exiftool.exe", "exiftool.exe"]]),
            "index.js",
            json!([
                "path.join(__dirname, 'vendor', 'exiftool.exe')",
                "path.join(path.dirname(process.execPath), 'exiftool.exe')"
            ]),
        ),
        (
            "exiftool.pl",
            json!([["vendor/exiftool", "exiftool"]]),
            "index.js",
            json!([
                "path.join(__dirname, 'vendor', 'exiftool')",
                "path.join(path.dirname(process.execPath), 'exiftool')"
            ]),
        ),
        (
            "google-closure-compiler",
            json!([["compiler.jar", "compiler/compiler.jar"]]),
            "lib/node/closure-compiler.js",
            json!([
                "require.resolve('../../compiler.jar')",
                "require('path').join(require('path').dirname(process.execPath), 'compiler/compiler.jar')"
            ]),
        ),
        (
            "google-closure-compiler-java",
            json!([["compiler.jar", "compiler/compiler.jar"]]),
            "index.js",
            json!([
                "require.resolve('./compiler.jar')",
                "require('path').join(require('path').dirname(process.execPath), 'compiler/compiler.jar')"
            ]),
        ),
    ];

    for (name, deploy_files, patch_path, patch_ops) in cases {
        let mut package = PackageJson::parse(&format!(r#"{{"name":"{name}"}}"#))?;
        let entry = lookup_dictionary(name).ok_or("missing file deploy dictionary")?;

        apply_dictionary_entry(&mut package, &entry);

        let config = package.pkg.ok_or("missing file deploy pkg config")?;
        assert_eq!(config.deploy_files, deploy_files);
        assert_eq!(config.patches.get(patch_path), Some(&patch_ops));
    }

    Ok(())
}

#[test]
fn remaining_deploy_dictionaries_carry_expected_pkg_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    struct Case {
        name: &'static str,
        scripts: Option<serde_json::Value>,
        deploy_files: serde_json::Value,
        patch: Option<(&'static str, serde_json::Value)>,
    }

    let cases = [
        Case {
            name: "drivelist",
            scripts: None,
            deploy_files: json!([
                ["build/Release/drivelist.node", "drivelist.node"],
                ["scripts/darwin.sh", "drivelist/darwin.sh"],
                ["scripts/linux.sh", "drivelist/linux.sh"],
                ["scripts/win32.bat", "drivelist/win32.bat"]
            ]),
            patch: Some((
                "build/scripts.js",
                json!([
                    "path.join(__dirname, '..', 'scripts')",
                    "path.join(path.dirname(process.execPath), 'drivelist')"
                ]),
            )),
        },
        Case {
            name: "electron",
            scripts: None,
            deploy_files: json!([
                ["dist", "electron/dist", "directory"],
                ["../sliced/index.js", "node_modules/sliced/index.js"],
                [
                    "../deep-defaults/lib/index.js",
                    "node_modules/deep-defaults/index.js"
                ]
            ]),
            patch: Some((
                "index.js",
                json!([
                    "path.join(__dirname, fs",
                    "path.join(path.dirname(process.execPath), 'electron', fs"
                ]),
            )),
        },
        Case {
            name: "nightmare",
            scripts: None,
            deploy_files: json!([
                ["lib/runner.js", "nightmare/runner.js"],
                ["lib/frame-manager.js", "nightmare/frame-manager.js"],
                ["lib/ipc.js", "nightmare/ipc.js"],
                ["lib/preload.js", "nightmare/preload.js"]
            ]),
            patch: Some((
                "lib/nightmare.js",
                json!([
                    "path.join(__dirname, 'runner.js')",
                    "path.join(path.dirname(process.execPath), 'nightmare/runner.js')"
                ]),
            )),
        },
        Case {
            name: "node-notifier",
            scripts: None,
            deploy_files: json!([
                ["vendor/notifu/notifu.exe", "notifier/notifu.exe"],
                ["vendor/notifu/notifu64.exe", "notifier/notifu64.exe"],
                [
                    "vendor/terminal-notifier.app/Contents/MacOS/terminal-notifier",
                    "notifier/terminal-notifier"
                ],
                [
                    "vendor/snoreToast/snoretoast-x64.exe",
                    "notifier/snoretoast-x64.exe"
                ],
                [
                    "vendor/snoreToast/snoretoast-x86.exe",
                    "notifier/snoretoast-x86.exe"
                ]
            ]),
            patch: Some((
                "notifiers/notificationcenter.js",
                json!([
                    "__dirname,\n  '../vendor/terminal-notifier.app/Contents/MacOS/terminal-notifier'",
                    "path.dirname(process.execPath), 'notifier/terminal-notifier'"
                ]),
            )),
        },
        Case {
            name: "phantom",
            scripts: None,
            deploy_files: json!([
                ["lib/shim/index.js", "phantom/index.js"],
                [
                    "lib/shim/function_bind_polyfill.js",
                    "phantom/function_bind_polyfill.js"
                ]
            ]),
            patch: Some((
                "lib/phantom.js",
                json!([
                    "__dirname + '/shim/index.js'",
                    "_path2.default.join(_path2.default.dirname(process.execPath), 'phantom/index.js')"
                ]),
            )),
        },
        Case {
            name: "phantomjs-prebuilt",
            scripts: None,
            deploy_files: json!([
                ["lib/phantom/bin/phantomjs", "phantom/phantomjs"],
                ["lib/phantom/bin/phantomjs.exe", "phantom/phantomjs.exe"]
            ]),
            patch: Some((
                "lib/phantomjs.js",
                json!([
                    "__dirname, location.location",
                    "path.dirname(process.execPath), 'phantom', path.basename(location.location)"
                ]),
            )),
        },
        Case {
            name: "sharp",
            scripts: Some(json!(["lib/*.js"])),
            deploy_files: json!([
                ["build/Release", "sharp/build/Release", "directory"],
                ["vendor/lib", "sharp/vendor/lib", "directory"]
            ]),
            patch: None,
        },
    ];

    for case in cases {
        let mut package = PackageJson::parse(&format!(r#"{{"name":"{}"}}"#, case.name))?;
        let entry = lookup_dictionary(case.name).ok_or("missing remaining deploy dictionary")?;

        apply_dictionary_entry(&mut package, &entry);

        let config = package.pkg.ok_or("missing remaining deploy pkg config")?;
        assert_eq!(config.deploy_files, case.deploy_files);
        if let Some(scripts) = case.scripts {
            assert_eq!(config.scripts, scripts);
        }
        if let Some((path, patch_ops)) = case.patch {
            assert_eq!(config.patches.get(path), Some(&patch_ops));
        }
    }

    Ok(())
}
