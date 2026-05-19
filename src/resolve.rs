use std::path::{Path, PathBuf};

use crate::config::PackageJson;
use crate::error::PkgError;

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
            extensions: vec![".js".to_owned(), ".json".to_owned(), ".node".to_owned()],
        }
    }
}

/// Resolve a module request using Node's file, directory, and `node_modules` rules.
///
/// # Example
///
/// ```
/// # use std::path::Path;
/// let options = pkg_rust::ResolveOptions::new("../test/test-50-require-resolve");
/// let resolved = pkg_rust::resolve_module("./test-z-require-code-1", &options)?;
/// assert!(resolved.ends_with(Path::new("test-z-require-code-1.js")));
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn resolve_module(request: &str, options: &ResolveOptions) -> Result<PathBuf, PkgError> {
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

fn resolve_as_path(candidate: &Path, options: &ResolveOptions) -> Option<PathBuf> {
    resolve_as_file(candidate, options).or_else(|| resolve_as_directory(candidate, options))
}

fn resolve_as_file(candidate: &Path, options: &ResolveOptions) -> Option<PathBuf> {
    if candidate.is_file() {
        return normalize(candidate);
    }

    for extension in &options.extensions {
        let with_extension = PathBuf::from(format!("{}{}", candidate.display(), extension));
        if with_extension.is_file() {
            return normalize(&with_extension);
        }
    }

    None
}

fn resolve_as_directory(candidate: &Path, options: &ResolveOptions) -> Option<PathBuf> {
    if !candidate.is_dir() {
        return None;
    }

    let package_path = candidate.join("package.json");
    if package_path.is_file()
        && let Some(main) = package_main(&package_path)
        && let Some(resolved) = resolve_as_path(&candidate.join(main), options)
    {
        return Some(resolved);
    }

    resolve_as_file(&candidate.join("index"), options)
}

fn package_main(package_path: &Path) -> Option<String> {
    let body = std::fs::read_to_string(package_path).ok()?;
    PackageJson::parse(&body).ok()?.main
}

fn resolve_node_module(request: &str, options: &ResolveOptions) -> Option<PathBuf> {
    for directory in ancestor_directories(&options.basedir) {
        let candidate = directory.join("node_modules").join(request);
        if let Some(resolved) = resolve_as_path(&candidate, options) {
            return Some(resolved);
        }
    }
    None
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
