use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;

/// Parsed `package.json` subset needed by the port.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackageJson {
    /// Package name.
    pub name: Option<String>,
    /// Whether the package is private.
    #[serde(default)]
    pub private: bool,
    /// Common npm `license` field.
    pub license: Option<Value>,
    /// Legacy npm `licenses` field.
    pub licenses: Option<Value>,
    /// Main module entrypoint.
    pub main: Option<String>,
    /// Binary entrypoint declaration.
    pub bin: Option<BinField>,
    /// Runtime dependencies.
    #[serde(default)]
    pub dependencies: Map<String, Value>,
    /// npm files list.
    #[serde(default)]
    pub files: Vec<String>,
    /// `pkg` configuration.
    pub pkg: Option<PkgConfig>,
}

impl PackageJson {
    /// Parse a `package.json` string.
    ///
    /// # Example
    ///
    /// ```
    /// let package = pkg_rust::PackageJson::parse(r#"{"name":"app","bin":"cli.js"}"#)?;
    /// assert_eq!(package.package_basename().as_deref(), Some("app"));
    /// # Ok::<(), pkg_rust::PackageJsonError>(())
    /// ```
    pub fn parse(input: &str) -> Result<Self, PackageJsonError> {
        serde_json::from_str(input).map_err(PackageJsonError::Json)
    }

    /// Return the package name segment used for object-form `bin` lookup.
    ///
    /// # Example
    ///
    /// ```
    /// let package = pkg_rust::PackageJson::parse(r#"{"name":"@scope/app"}"#)?;
    /// assert_eq!(package.package_basename().as_deref(), Some("app"));
    /// # Ok::<(), pkg_rust::PackageJsonError>(())
    /// ```
    #[must_use]
    pub fn package_basename(&self) -> Option<String> {
        self.name
            .as_deref()
            .and_then(|name| name.rsplit('/').next())
            .map(ToOwned::to_owned)
    }

    /// Select the binary entrypoint according to the JavaScript implementation.
    ///
    /// String-form `bin` is used directly. Object-form `bin` prefers the key
    /// matching the package basename and otherwise falls back to the first key
    /// in JSON order.
    ///
    /// # Example
    ///
    /// ```
    /// let package = pkg_rust::PackageJson::parse(
    ///     r#"{"name":"@scope/app","bin":{"other":"other.js","app":"app.js"}}"#
    /// )?;
    /// assert_eq!(package.selected_bin().as_deref(), Some("app.js"));
    /// # Ok::<(), pkg_rust::PackageJsonError>(())
    /// ```
    #[must_use]
    pub fn selected_bin(&self) -> Option<String> {
        match self.bin.as_ref()? {
            BinField::String(path) => Some(path.clone()),
            BinField::Map(entries) => {
                if let Some(name) = self.package_basename()
                    && let Some(value) = entries.get(&name)
                    && let Some(path) = value.as_str()
                {
                    return Some(path.to_owned());
                }

                entries
                    .values()
                    .find_map(Value::as_str)
                    .map(ToOwned::to_owned)
            }
        }
    }

    /// Resolve the selected `bin` path relative to a package file.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::path::Path;
    /// let package = pkg_rust::PackageJson::parse(r#"{"bin":"cli.js"}"#)?;
    /// assert_eq!(
    ///     package.resolve_selected_bin(Path::new("/project/package.json")).as_deref(),
    ///     Some(Path::new("/project/cli.js"))
    /// );
    /// # Ok::<(), pkg_rust::PackageJsonError>(())
    /// ```
    #[must_use]
    pub fn resolve_selected_bin(&self, package_path: &Path) -> Option<PathBuf> {
        let bin = self.selected_bin()?;
        package_path.parent().map(|parent| parent.join(bin))
    }
}

/// npm `bin` field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum BinField {
    /// `"bin": "cli.js"`.
    String(String),
    /// `"bin": { "name": "cli.js" }`.
    Map(Map<String, Value>),
}

/// Parsed `pkg` configuration subset.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PkgConfig {
    /// Explicit script globs.
    #[serde(default)]
    pub scripts: Value,
    /// Explicit asset globs.
    #[serde(default)]
    pub assets: Value,
    /// Target list from package config.
    #[serde(default)]
    pub targets: Vec<String>,
    /// Output directory for auto-generated executable names.
    pub output_path: Option<String>,
    /// File patches.
    #[serde(default)]
    pub patches: Map<String, Value>,
    /// Extra dictionary entries.
    #[serde(default)]
    pub dictionary: Map<String, Value>,
}

/// Errors returned while parsing package metadata.
#[derive(Debug, Error)]
pub enum PackageJsonError {
    /// JSON parsing failed.
    #[error("invalid package json: {0}")]
    Json(serde_json::Error),
}
