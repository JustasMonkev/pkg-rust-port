use std::path::{Path, PathBuf};

use crate::config::PackageJson;
use crate::error::PkgError;

/// Resolved module path plus package metadata consumed during resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModule {
    /// Resolved JavaScript, JSON, native addon, or package file.
    pub path: PathBuf,
    /// Package metadata file whose `main` selected the resolved path.
    pub package_json: Option<PathBuf>,
}

/// Options for Node-compatible module resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveOptions {
    /// Directory containing the requiring file.
    pub basedir: PathBuf,
    /// Extensions to try after an extensionless request.
    pub extensions: Vec<String>,
}

impl ResolveOptions {
    /// Create resolver options with the `pkg` extension list.
    ///
    /// # Example
    ///
    /// ```
    /// let options = pkg_rust::ResolveOptions::new("/project/src");
    /// assert!(options.extensions.iter().any(|extension| extension == ".js"));
    /// ```
    pub fn new(basedir: impl Into<PathBuf>) -> Self {
        Self {
            basedir: basedir.into(),
            // yao-pkg MODULE_RESOLVE_EXTENSIONS: ['.js', '.json', '.node', '.mjs'].
            extensions: vec![
                ".js".to_owned(),
                ".json".to_owned(),
                ".node".to_owned(),
                ".mjs".to_owned(),
            ],
        }
    }
}

/// Resolve a module request using Node's file, directory, and `node_modules` rules.
///
/// # Example
///
/// ```
/// # use std::path::Path;
/// let options = pkg_rust::ResolveOptions::new("test/test-50-require-resolve");
/// let resolved = pkg_rust::resolve_module("./test-z-require-code-1", &options)?;
/// assert!(resolved.ends_with(Path::new("test-z-require-code-1.js")));
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn resolve_module(request: &str, options: &ResolveOptions) -> Result<PathBuf, PkgError> {
    resolve_module_with_metadata(request, options).map(|resolved| resolved.path)
}

/// Resolve a module request and report package metadata used for `main`.
pub fn resolve_module_with_metadata(
    request: &str,
    options: &ResolveOptions,
) -> Result<ResolvedModule, PkgError> {
    if is_path_request(request) {
        let candidate = if Path::new(request).is_absolute() {
            PathBuf::from(request)
        } else {
            options.basedir.join(request)
        };

        return resolve_as_path(&candidate, options).ok_or_else(|| {
            PkgError::Resolve(format!(
                "Cannot find module '{request}' from '{}'",
                options.basedir.display()
            ))
        });
    }

    resolve_node_module(request, options).ok_or_else(|| {
        PkgError::Resolve(format!(
            "Cannot find module '{request}' from '{}'",
            options.basedir.display()
        ))
    })
}

fn is_path_request(request: &str) -> bool {
    request.starts_with("./")
        || request.starts_with("../")
        || request.starts_with('/')
        || request.starts_with(".\\")
        || request.starts_with("..\\")
}

fn resolve_as_path(candidate: &Path, options: &ResolveOptions) -> Option<ResolvedModule> {
    resolve_as_file(candidate, options).or_else(|| resolve_as_directory(candidate, options))
}

fn resolve_as_file(candidate: &Path, options: &ResolveOptions) -> Option<ResolvedModule> {
    if candidate.is_file() {
        return normalize(candidate).map(|path| ResolvedModule {
            path,
            package_json: None,
        });
    }

    for extension in &options.extensions {
        let with_extension = PathBuf::from(format!("{}{}", candidate.display(), extension));
        if with_extension.is_file() {
            return normalize(&with_extension).map(|path| ResolvedModule {
                path,
                package_json: None,
            });
        }
    }

    None
}

fn resolve_as_directory(candidate: &Path, options: &ResolveOptions) -> Option<ResolvedModule> {
    if !candidate.is_dir() {
        return None;
    }

    let package_path = candidate.join("package.json");
    if package_path.is_file()
        && let Some(main) = package_main(&package_path)
        && let Some(mut resolved) = resolve_as_path(&candidate.join(main), options)
    {
        resolved.package_json = normalize(&package_path);
        return Some(resolved);
    }

    resolve_as_file(&candidate.join("index"), options)
}

fn package_main(package_path: &Path) -> Option<String> {
    let body = std::fs::read_to_string(package_path).ok()?;
    PackageJson::parse(&body)
        .ok()?
        .main
        .filter(|main| !main.is_empty())
}

fn resolve_node_module(request: &str, options: &ResolveOptions) -> Option<ResolvedModule> {
    // JS `follow`: bare specifiers try exports-field (ESM-style) resolution
    // first, but the result is only used when it lands on an actual ESM file;
    // CommonJS packages keep flowing through classic `main`/index resolution.
    if is_valid_package_name(request)
        && let Some(resolved) = resolve_with_exports(request, options)
        && is_esm_file(&resolved.path)
    {
        return Some(resolved);
    }
    for directory in ancestor_directories(&options.basedir) {
        let candidate = directory.join("node_modules").join(request);
        if let Some(resolved) = resolve_as_path(&candidate, options) {
            return Some(resolved);
        }
    }
    None
}

/// JS `follow.isValidPackageName`: lowercase npm package-name shape, with
/// scoped-package support. Generated aliases like `connectNonLiteral` fail
/// this check and skip exports resolution entirely.
fn is_valid_package_name(specifier: &str) -> bool {
    fn valid_segment(segment: &str) -> bool {
        !segment.is_empty()
            && segment
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || "_.-".contains(ch))
    }

    if let Some(scoped) = specifier.strip_prefix('@') {
        let mut parts = scoped.split('/');
        let scope = parts.next().unwrap_or_default();
        let Some(name) = parts.next() else {
            return false;
        };
        return valid_segment(scope) && valid_segment(name);
    }
    let package_name = specifier.split('/').next().unwrap_or_default();
    valid_segment(package_name)
}

/// Split a bare specifier into package name and exports subpath
/// (`"."` or `"./<rest>"`), mirroring JS `tryResolveESM`.
fn split_specifier(specifier: &str) -> Option<(String, String)> {
    if let Some(scoped) = specifier.strip_prefix('@') {
        let mut parts = scoped.splitn(3, '/');
        let scope = parts.next()?;
        let name = parts.next()?;
        let package_name = format!("@{scope}/{name}");
        let subpath = match parts.next() {
            Some(rest) => format!("./{rest}"),
            None => ".".to_owned(),
        };
        return Some((package_name, subpath));
    }
    match specifier.split_once('/') {
        Some((name, rest)) => Some((name.to_owned(), format!("./{rest}"))),
        None => Some((specifier.to_owned(), ".".to_owned())),
    }
}

/// Resolve a bare specifier through the target package's `exports` field,
/// trying the `require` condition first and falling back to `import` for
/// ESM-only packages (JS `resolveWithExports`).
fn resolve_with_exports(request: &str, options: &ResolveOptions) -> Option<ResolvedModule> {
    let (package_name, subpath) = split_specifier(request)?;
    for directory in ancestor_directories(&options.basedir) {
        let package_json = directory
            .join("node_modules")
            .join(&package_name)
            .join("package.json");
        if !package_json.is_file() {
            continue;
        }
        let body = std::fs::read_to_string(&package_json).ok()?;
        let json: serde_json::Value = serde_json::from_str(&body).ok()?;
        let exports = json.get("exports")?;
        let package_root = package_json.parent()?;
        for condition in ["require", "import"] {
            if let Some(target) = resolve_exports_subpath(exports, &subpath, condition) {
                let full = package_root.join(target.trim_start_matches("./"));
                if full.is_file() {
                    return Some(ResolvedModule {
                        path: normalize(&full)?,
                        package_json: normalize(&package_json),
                    });
                }
            }
        }
        return None;
    }
    None
}

/// Resolve one subpath through a package `exports` value for a condition set
/// of `{condition, node, default}`, the same set `resolve.exports` uses.
fn resolve_exports_subpath(
    exports: &serde_json::Value,
    subpath: &str,
    condition: &str,
) -> Option<String> {
    let subpath_map = exports
        .as_object()
        .filter(|map| !map.is_empty() && map.keys().all(|key| key.starts_with('.')));
    let Some(map) = subpath_map else {
        // Shorthand: the whole value describes the "." subpath.
        if subpath == "." {
            return resolve_exports_target(exports, condition, None);
        }
        return None;
    };

    if let Some(target) = map.get(subpath) {
        return resolve_exports_target(target, condition, None);
    }

    // Pattern subpaths: `./prefix*suffix`, longest prefix wins.
    let mut best: Option<(usize, &str, &serde_json::Value)> = None;
    for (key, value) in map {
        let Some((prefix, suffix)) = key.split_once('*') else {
            continue;
        };
        if subpath.starts_with(prefix)
            && subpath.len() >= prefix.len() + suffix.len()
            && subpath.ends_with(suffix)
            && best.is_none_or(|(len, _, _)| prefix.len() > len)
        {
            let capture = &subpath[prefix.len()..subpath.len() - suffix.len()];
            best = Some((prefix.len(), capture, value));
        }
    }
    let (_, capture, value) = best?;
    resolve_exports_target(value, condition, Some(capture))
}

fn resolve_exports_target(
    target: &serde_json::Value,
    condition: &str,
    capture: Option<&str>,
) -> Option<String> {
    match target {
        serde_json::Value::String(value) => Some(match capture {
            Some(capture) => value.replace('*', capture),
            None => value.clone(),
        }),
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| resolve_exports_target(item, condition, capture)),
        serde_json::Value::Object(map) => map.iter().find_map(|(key, value)| {
            if key == condition || key == "node" || key == "default" {
                resolve_exports_target(value, condition, capture)
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// JS `common.isESMFile`: `.mjs` is ESM, `.cjs` is CJS, `.js` follows the
/// nearest `package.json` `"type": "module"` marker.
fn is_esm_file(path: &Path) -> bool {
    let extension = path.extension().and_then(|extension| extension.to_str());
    match extension {
        Some("mjs") => return true,
        Some("cjs") => return false,
        Some("js") => {}
        _ => return false,
    }
    let mut current = path.parent();
    while let Some(directory) = current {
        let package_json = directory.join("package.json");
        if package_json.is_file() {
            return std::fs::read_to_string(&package_json)
                .ok()
                .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                .and_then(|json| {
                    json.get("type")
                        .and_then(|value| value.as_str())
                        .map(|value| value == "module")
                })
                .unwrap_or(false);
        }
        current = directory.parent();
    }
    false
}

fn ancestor_directories(start: &Path) -> Vec<PathBuf> {
    let mut current = Some(start);
    let mut directories = Vec::new();

    while let Some(directory) = current {
        directories.push(directory.to_path_buf());
        current = directory.parent();
    }

    directories
}

fn normalize(path: &Path) -> Option<PathBuf> {
    path.canonicalize()
        .ok()
        .or_else(|| Some(path.to_path_buf()))
}
