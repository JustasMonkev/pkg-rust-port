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
fn simple_script_dictionaries_carry_pkg_globs() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("blessed", json!(["lib/widgets/*.js"])),
        ("body-parser", json!(["lib/types/*.js"])),
        ("buffermaker", json!(["lib/*.js"])),
        ("coffee-script", json!(["lib/coffee-script/*.js"])),
        ("compressjs", json!(["lib/*.js"])),
        ("eslint", json!(["lib/rules/*.js", "lib/formatters/*.js"])),
        ("googleapis", json!(["apis/**/*.js"])),
        ("knex", json!(["lib/**/*.js"])),
        ("later", json!(["later.js"])),
        ("logform", json!(["*.js"])),
        ("machinepack-urls", json!(["machines/*.js"])),
        ("moment", json!(["locale/*.js"])),
        ("mongodb", json!(["lib/mongodb/**/*.js"])),
        ("negotiator", json!(["lib/*.js"])),
        ("npm", json!(["lib/*.js"])),
        ("oauth2orize", json!(["lib/**/*.js"])),
        ("pg.js", json!(["lib/**/*.js"])),
        ("pgpass", json!(["lib/helper.js"])),
        ("pm2", json!(["lib/ProcessContainerFork.js"])),
        ("reload", json!(["lib/reload-server.js"])),
        ("shelljs", json!(["src/*.js"])),
        ("usage", json!(["lib/providers/*.js"])),
        ("winston", json!(["lib/winston/transports/*.js"])),
    ];

    for (name, scripts) in cases {
        let entry = lookup_dictionary(name).ok_or("missing script dictionary")?;

        assert_eq!(entry.pkg.as_ref().map(|pkg| &pkg.scripts), Some(&scripts));
    }
    Ok(())
}

#[test]
fn simple_asset_dictionaries_carry_pkg_globs() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("browserify", json!(["bin/*.txt"])),
        (
            "data-preflight",
            json!(["src/view/**/*", "src/js/view/**/*"]),
        ),
        ("errors", json!(["lib/static/*"])),
        (
            "node-zookeeper-client",
            json!(["lib/jute/specification.json"]),
        ),
        ("tiny-worker", json!(["lib/noop.js"])),
        ("uglify-js", json!(["lib/**/*.js", "tools/*.js"])),
    ];

    for (name, assets) in cases {
        let entry = lookup_dictionary(name).ok_or("missing asset dictionary")?;

        assert_eq!(entry.pkg.as_ref().map(|pkg| &pkg.assets), Some(&assets));
    }
    Ok(())
}

#[test]
fn svgo_dictionary_carries_script_and_asset_globs() -> Result<(), Box<dyn std::error::Error>> {
    let entry = lookup_dictionary("svgo").ok_or("missing svgo dictionary")?;
    let config = entry.pkg.ok_or("missing svgo pkg config")?;

    assert_eq!(config.scripts, json!(["lib/**/*.js", "plugins/*.js"]));
    assert_eq!(config.assets, json!([".svgo.yml"]));
    Ok(())
}

#[test]
fn patch_only_dictionaries_carry_patch_operations() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "bunyan",
            "lib/bunyan.js",
            json!(["mv = require('mv' + '');", "mv = require('mv');"]),
        ),
        ("cross-env", "src/index.js", json!([{ "do": "erase" }, ""])),
        (
            "express-load",
            "lib/express-load.js",
            json!([
                "entity = path.resolve(",
                "entity = process.pkg.path.resolve("
            ]),
        ),
        (
            "graceful-fs",
            "graceful-fs.js",
            json!([
                { "do": "prepend" },
                "if ((function() {\n  var version = require('./package.json').version;\n  var major = parseInt(version.split('.')[0]);\n  if (major < 4) {\n    module.exports = require('fs');\n    return true;\n  }\n})()) return;\n"
            ]),
        ),
        (
            "j",
            "j.js",
            json!([
                "require('xl'+'sx')",
                "require('xlsx')",
                "require('xl'+'sjs')",
                "require('xlsjs')",
                "require('ha'+'rb')",
                "require('harb')"
            ]),
        ),
        (
            "liftoff",
            "index.js",
            json!([
                "resolve.sync(this.moduleName, {basedir: configBase || cwd, paths: paths})",
                "resolve.sync(this.moduleName, {basedir: configBase || require.main.filename, paths: paths})"
            ]),
        ),
        (
            "microjob",
            "dist/worker-pool.js",
            json!([
                "error.stack = message.error.stack;",
                "error.stack = message.error.stack;\nif (error.stack.indexOf(\"SyntaxError\") >= 0) {error.stack = \"Pkg: Try to specify your javascript file in 'assets' in config.\\n\" + error.stack;};"
            ]),
        ),
        (
            "rc",
            "lib/utils.js",
            json!([
                "process.cwd()",
                "require('path').dirname(require.main.filename)"
            ]),
        ),
        (
            "socket.io",
            "lib/index.js",
            json!([
                "require.resolve('socket.io-client/dist/socket.io.js.map')",
                "require.resolve('socket.io-client/dist/socket.io.js.map', 'must-exclude')"
            ]),
        ),
        (
            "v8flags",
            "index.js",
            json!([
                "execFile(process.execPath, ['--v8-options'],",
                "execFile(process.execPath, ['--v8-options'], { env: { PKG_EXECPATH: 'PKG_INVOKE_NODEJS' } },"
            ]),
        ),
        (
            "xlsx",
            "xlsx.js",
            json!([
                "require('js'+'zip')",
                "require('jszip')",
                "require('./js'+'zip')",
                "require('./jszip')",
                "require('./od' + 's')",
                "require('./ods')"
            ]),
        ),
    ];

    for (name, patch_path, operations) in cases {
        let entry = lookup_dictionary(name).ok_or("missing patch dictionary")?;
        let config = entry.pkg.ok_or("missing patch pkg config")?;

        assert_eq!(config.patches.get(patch_path), Some(&operations));
    }
    Ok(())
}

#[test]
fn mongodb_core_dictionary_carries_error_patch_operations() -> Result<(), Box<dyn std::error::Error>>
{
    let entry = lookup_dictionary("mongodb-core").ok_or("missing mongodb-core dictionary")?;
    let config = entry.pkg.ok_or("missing mongodb-core pkg config")?;
    let patch = config
        .patches
        .get("lib/error.js")
        .ok_or("missing mongodb-core patch")?;

    assert_eq!(patch.as_array().map(Vec::len), Some(4));
    assert_eq!(patch[0], "return err;");
    assert!(patch[1].as_str().is_some_and(|item| {
        item.contains("Pkg: Try to specify your javascript file in 'assets' in config.")
    }));
    Ok(())
}

#[test]
fn mixed_dictionary_modules_carry_globs_and_patches() -> Result<(), Box<dyn std::error::Error>> {
    struct Case {
        name: &'static str,
        scripts: Option<serde_json::Value>,
        assets: Option<serde_json::Value>,
        patch: (&'static str, serde_json::Value),
    }

    let cases = [
        Case {
            name: "exceljs",
            scripts: None,
            assets: Some(json!(["lib/**/*.xml"])),
            patch: (
                "lib/xlsx/xlsx.js",
                json!([
                    "require.resolve('./xml/theme1.xml')",
                    "require('path').join(__dirname, './xml/theme1.xml')"
                ]),
            ),
        },
        Case {
            name: "sails",
            scripts: Some(json!(["lib/**/*.js"])),
            assets: None,
            patch: (
                "lib/hooks/moduleloader/index.js",
                json!(["require('coffee-script/register')", ""]),
            ),
        },
        Case {
            name: "steam-resources",
            scripts: None,
            assets: Some(json!(["steam_language/**/*"])),
            patch: (
                "steam_language_parser/parser/token_analyzer.js",
                json!([
                    "text.value",
                    "require('path').join(__dirname, '../../steam_language', text.value)"
                ]),
            ),
        },
        Case {
            name: "umd",
            scripts: None,
            assets: Some(json!(["template.js"])),
            patch: (
                "index.js",
                json!([
                    "var rfile = require('rfile');",
                    "var rfile = function(f) { require('fs').readFileSync(require.resolve(f)); };"
                ]),
            ),
        },
    ];

    for case in cases {
        let entry = lookup_dictionary(case.name).ok_or("missing mixed dictionary")?;
        let config = entry.pkg.ok_or("missing mixed pkg config")?;

        if let Some(scripts) = case.scripts {
            assert_eq!(config.scripts, scripts);
        }
        if let Some(assets) = case.assets {
            assert_eq!(config.assets, assets);
        }
        assert_eq!(config.patches.get(case.patch.0), Some(&case.patch.1));
    }
    Ok(())
}

#[test]
fn mixed_dictionary_modules_carry_secondary_patches() -> Result<(), Box<dyn std::error::Error>> {
    let exceljs = lookup_dictionary("exceljs").ok_or("missing exceljs dictionary")?;
    let exceljs_config = exceljs.pkg.ok_or("missing exceljs pkg config")?;
    assert_eq!(
        exceljs_config
            .patches
            .get("lib/stream/xlsx/workbook-writer.js"),
        Some(&json!([
            "require.resolve('../../xlsx/xml/theme1.xml')",
            "require('path').join(__dirname, '../../xlsx/xml/theme1.xml')"
        ]))
    );

    let sails = lookup_dictionary("sails").ok_or("missing sails dictionary")?;
    let sails_config = sails.pkg.ok_or("missing sails pkg config")?;
    assert_eq!(
        sails_config.patches.get("lib/app/configuration/index.js"),
        Some(&json!([
            "hook = require(hookBundled);",
            "hook = require(hookBundled);require('sails-hook-sockets');"
        ]))
    );
    assert_eq!(
        sails_config
            .patches
            .get("lib/hooks/orm/backwards-compatibility/upgrade-datastore.js"),
        Some(&json!([
            "if (!fs.existsSync(modulePath)) {",
            "try { require(modulePath); } catch (e) {"
        ]))
    );
    let grunt_patch = sails_config
        .patches
        .get("lib/hooks/grunt/index.js")
        .ok_or("missing sails grunt patch")?;
    assert_eq!(grunt_patch[0], "var child = ChildProcess.fork(");
    assert!(
        grunt_patch[1]
            .as_str()
            .is_some_and(|item| item.contains("Pkg: Grunt hook is temporarily disabled"))
    );

    let steam_resources =
        lookup_dictionary("steam-resources").ok_or("missing steam-resources dictionary")?;
    let steam_config = steam_resources
        .pkg
        .ok_or("missing steam-resources pkg config")?;
    assert_eq!(
        steam_config.patches.get("steam_language_parser/index.js"),
        Some(&json!([
            "process.chdir",
            "// process.chdir",
            "'steammsg.steamd'",
            "require('path').join(__dirname, '../steam_language', 'steammsg.steamd')"
        ]))
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
