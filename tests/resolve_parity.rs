#![allow(missing_docs)]

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use pkg_rust::{PkgError, ResolveOptions, resolve_module};

#[test]
fn resolves_exact_unknown_extension_file() -> Result<(), PkgError> {
    let options = ResolveOptions::new("../test/test-50-require-resolve");
    let resolved = resolve_module("./test-y-resolve.any", &options)?;

    assert!(resolved.ends_with(Path::new("test-y-resolve.any")));
    Ok(())
}

#[test]
fn resolves_js_and_json_extension_fallbacks() -> Result<(), PkgError> {
    let options = ResolveOptions::new("../test/test-50-require-resolve");

    let js = resolve_module("./test-z-require-code-1", &options)?;
    let json = resolve_module("./test-z-require-json-1", &options)?;

    assert!(js.ends_with(Path::new("test-z-require-code-1.js")));
    assert!(json.ends_with(Path::new("test-z-require-json-1.json")));
    Ok(())
}

#[test]
fn resolves_directory_package_json_main() -> Result<(), PkgError> {
    let options = ResolveOptions::new("../test/test-50-package-json-6c/beta");

    let resolved = resolve_module("../beta", &options)?;

    assert!(resolved.ends_with(Path::new("test-50-package-json-6c/beta/beta.js")));
    Ok(())
}

#[test]
fn empty_package_main_falls_through_to_index_resolution() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture_dir = temp_root("empty-package-main")?;
    let package_dir = fixture_dir.join("node_modules/@types/triple-beam");
    std::fs::create_dir_all(&package_dir)?;
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"name":"@types/triple-beam","main":""}"#,
    )?;

    let options = ResolveOptions::new(&fixture_dir);
    let result = resolve_module("@types/triple-beam", &options);

    assert!(
        matches!(result, Err(PkgError::Resolve(message)) if message.contains("Cannot find module '@types/triple-beam'"))
    );

    std::fs::remove_dir_all(&fixture_dir)?;
    Ok(())
}

fn temp_root(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "pkg-rust-resolve-parity-{name}-{}-{nanos}",
        std::process::id()
    )))
}
