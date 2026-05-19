use serde_json::{Map, Value};

use crate::config::{PackageJson, PkgConfig};

/// A dependency directive from a dictionary entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DictionaryDependency {
    name: String,
    version: Option<String>,
}

impl DictionaryDependency {
    /// Include or override a dependency with a version/range string.
    ///
    /// # Example
    ///
    /// ```
    /// let dependency = pkg_rust::DictionaryDependency::enabled("left-pad", "*");
    /// assert_eq!(dependency.name(), "left-pad");
    /// ```
    #[must_use]
    pub fn enabled(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: Some(version.into()),
        }
    }

    /// Disable a dependency the same way dictionary `undefined` values do in JS.
    ///
    /// # Example
    ///
    /// ```
    /// let dependency = pkg_rust::DictionaryDependency::disabled("gulp");
    /// assert_eq!(dependency.version(), None);
    /// ```
    #[must_use]
    pub fn disabled(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
        }
    }

    /// Dependency package name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Dependency version/range, or `None` when disabled.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

/// Typed representation of one `dictionary/*.js` module.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DictionaryEntry {
    /// Dependency directives to merge into package dependencies.
    pub dependencies: Vec<DictionaryDependency>,
    /// Replacement `pkg` config from the dictionary.
    pub pkg: Option<PkgConfig>,
    /// Warning/log directives emitted when the dictionary activates.
    pub logs: Vec<DictionaryLog>,
}

impl DictionaryEntry {
    /// Build an empty dictionary entry.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(pkg_rust::DictionaryEntry::empty().dependencies.len(), 0);
    /// ```
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a dictionary entry with a replacement `pkg` config.
    ///
    /// # Example
    ///
    /// ```
    /// let entry = pkg_rust::DictionaryEntry::with_pkg(pkg_rust::PkgConfig::with_scripts(["lib/**/*.js"]));
    /// assert!(entry.pkg.is_some());
    /// ```
    #[must_use]
    pub fn with_pkg(pkg: PkgConfig) -> Self {
        Self {
            dependencies: Vec::new(),
            pkg: Some(pkg),
            logs: Vec::new(),
        }
    }

    /// Add a dependency directive.
    ///
    /// # Example
    ///
    /// ```
    /// let entry = pkg_rust::DictionaryEntry::empty()
    ///     .with_dependency(pkg_rust::DictionaryDependency::enabled("debug", "*"));
    /// assert_eq!(entry.dependencies.len(), 1);
    /// ```
    #[must_use]
    pub fn with_dependency(mut self, dependency: DictionaryDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// Add a warning/log directive.
    ///
    /// # Example
    ///
    /// ```
    /// let entry = pkg_rust::DictionaryEntry::empty()
    ///     .with_log(pkg_rust::DictionaryLog::StylusResolveImports);
    /// assert_eq!(entry.logs.len(), 1);
    /// ```
    #[must_use]
    pub fn with_log(mut self, log: DictionaryLog) -> Self {
        self.logs.push(log);
        self
    }
}

/// Data-only representation of dictionary log callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictionaryLog {
    /// Stylus import resolution warning from `dictionary/stylus.js`.
    StylusResolveImports,
}

/// Look up a typed dictionary entry by package name.
///
/// This is intentionally data-only; Rust runtime behavior must not execute the
/// JavaScript dictionary modules.
///
/// # Example
///
/// ```
/// let entry = pkg_rust::lookup_dictionary("sequelize");
/// assert!(entry.and_then(|entry| entry.pkg).is_some());
/// ```
#[must_use]
pub fn lookup_dictionary(package_name: &str) -> Option<DictionaryEntry> {
    match package_name {
        "busboy" => Some(busboy()),
        "express" => Some(express()),
        "log4js" => Some(log4js()),
        "open" | "opn" => Some(open()),
        "publicsuffixlist" => Some(publicsuffixlist()),
        "sequelize" => Some(sequelize()),
        "stylus" => Some(stylus()),
        _ => None,
    }
}

/// Apply a dictionary entry to package metadata using JS `stepActivate` merge semantics.
///
/// # Example
///
/// ```
/// let mut package = pkg_rust::PackageJson::parse(r#"{"name":"sequelize"}"#)?;
/// let entry = pkg_rust::lookup_dictionary("sequelize").expect("static entry exists");
/// pkg_rust::apply_dictionary_entry(&mut package, &entry);
/// assert_eq!(package.pkg.unwrap().scripts, serde_json::json!(["lib/**/*.js"]));
/// # Ok::<(), pkg_rust::PackageJsonError>(())
/// ```
pub fn apply_dictionary_entry(package: &mut PackageJson, entry: &DictionaryEntry) {
    for dependency in &entry.dependencies {
        if let Some(version) = dependency.version() {
            package.dependencies.insert(
                dependency.name().to_owned(),
                Value::String(version.to_owned()),
            );
        } else {
            // DECISION: JavaScript dictionaries use `undefined` to override a dependency so
            // `if (dependencies[name])` skips it later. JSON has no undefined value, so the
            // typed Rust representation stores `null` as the explicit disabled marker.
            package
                .dependencies
                .insert(dependency.name().to_owned(), Value::Null);
        }
    }

    if let Some(pkg) = entry.pkg.clone() {
        package.pkg = Some(pkg);
    }
}

/// Return dependency names that would be traversed after dictionary activation.
///
/// # Example
///
/// ```
/// let mut package = pkg_rust::PackageJson::parse(r#"{"dependencies":{"debug":"*"}}"#)?;
/// pkg_rust::apply_dictionary_entry(
///     &mut package,
///     &pkg_rust::DictionaryEntry::empty()
///         .with_dependency(pkg_rust::DictionaryDependency::disabled("debug")),
/// );
/// assert!(pkg_rust::active_dependencies(&package).is_empty());
/// # Ok::<(), pkg_rust::PackageJsonError>(())
/// ```
#[must_use]
pub fn active_dependencies(package: &PackageJson) -> Vec<String> {
    package
        .dependencies
        .iter()
        .filter(|(_name, value)| dependency_value_is_active(value))
        .map(|(name, _value)| name.clone())
        .collect()
}

fn dependency_value_is_active(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => false,
        Value::Number(number) => number.as_i64() != Some(0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) | Value::Bool(true) => true,
    }
}

fn busboy() -> DictionaryEntry {
    DictionaryEntry::with_pkg(PkgConfig::with_scripts(["lib/types/*.js"]))
}

fn express() -> DictionaryEntry {
    let mut patches = Map::new();
    patches.insert(
        "lib/view.js".to_owned(),
        serde_json::json!([
            "path = join(this.root, path)",
            "path = process.pkg.path.resolve(this.root, path)",
            "loc = resolve(root, name)",
            "loc = process.pkg.path.resolve(root, name)"
        ]),
    );

    DictionaryEntry::with_pkg(PkgConfig {
        patches,
        ..PkgConfig::default()
    })
}

fn log4js() -> DictionaryEntry {
    DictionaryEntry::with_pkg(PkgConfig::with_scripts(["lib/appenders/*.js"]))
}

fn open() -> DictionaryEntry {
    DictionaryEntry::empty()
}

fn publicsuffixlist() -> DictionaryEntry {
    DictionaryEntry::with_pkg(PkgConfig::with_assets(["effective_tld_names.dat"]))
        .with_dependency(DictionaryDependency::disabled("gulp"))
        .with_dependency(DictionaryDependency::disabled("gulp-di"))
        .with_dependency(DictionaryDependency::disabled("gulp-istanbul"))
        .with_dependency(DictionaryDependency::disabled("gulp-jshint"))
        .with_dependency(DictionaryDependency::disabled("gulp-mocha"))
        .with_dependency(DictionaryDependency::disabled("mocha"))
}

fn sequelize() -> DictionaryEntry {
    DictionaryEntry::with_pkg(PkgConfig::with_scripts(["lib/**/*.js"]))
}

fn stylus() -> DictionaryEntry {
    DictionaryEntry::with_pkg(PkgConfig::with_assets(["lib/**/*.styl"]))
        .with_log(DictionaryLog::StylusResolveImports)
}
