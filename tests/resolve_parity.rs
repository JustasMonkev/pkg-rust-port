#![allow(missing_docs)]

use std::path::Path;

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
