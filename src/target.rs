use std::fmt;
use std::str::FromStr;

use thiserror::Error;

/// Host defaults used when a target omits node range, platform, or arch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetDefaults {
    /// Default Node.js range such as `node18`.
    pub node_range: String,
    /// Default platform.
    pub platform: Platform,
    /// Default architecture.
    pub arch: Arch,
}

impl TargetDefaults {
    /// Build defaults for the current host, using the provided Node range.
    ///
    /// # Example
    ///
    /// ```
    /// let defaults = pkg_rust::TargetDefaults::host("node18");
    /// assert_eq!(defaults.node_range, "node18");
    /// ```
    #[must_use]
    pub fn host(node_range: impl Into<String>) -> Self {
        Self {
            node_range: node_range.into(),
            platform: Platform::host(),
            arch: Arch::host(),
        }
    }
}

/// Platform part of a pkg target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    /// Alpine Linux.
    Alpine,
    /// GNU/Linux.
    Linux,
    /// Static Linux.
    LinuxStatic,
    /// Windows.
    Win,
    /// macOS.
    Macos,
    /// FreeBSD.
    Freebsd,
}

impl Platform {
    /// Return the host platform.
    #[must_use]
    pub fn host() -> Self {
        match std::env::consts::OS {
            "macos" => Self::Macos,
            "windows" => Self::Win,
            "freebsd" => Self::Freebsd,
            _ => Self::Linux,
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Alpine => "alpine",
            Self::Linux => "linux",
            Self::LinuxStatic => "linuxstatic",
            Self::Win => "win",
            Self::Macos => "macos",
            Self::Freebsd => "freebsd",
        };
        formatter.write_str(value)
    }
}

impl FromStr for Platform {
    type Err = TargetParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "alpine" => Ok(Self::Alpine),
            "linux" => Ok(Self::Linux),
            "linuxstatic" => Ok(Self::LinuxStatic),
            "win" | "win32" | "windows" => Ok(Self::Win),
            "macos" | "darwin" => Ok(Self::Macos),
            "freebsd" => Ok(Self::Freebsd),
            _ => Err(TargetParseError::UnknownToken(value.to_owned())),
        }
    }
}

/// Architecture part of a pkg target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arch {
    /// x86_64.
    X64,
    /// AArch64.
    Arm64,
    /// ARMv6.
    Armv6,
    /// ARMv7.
    Armv7,
}

impl Arch {
    /// Return the host architecture.
    #[must_use]
    pub fn host() -> Self {
        match std::env::consts::ARCH {
            "aarch64" => Self::Arm64,
            "arm" => Self::Armv7,
            _ => Self::X64,
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::X64 => "x64",
            Self::Arm64 => "arm64",
            Self::Armv6 => "armv6",
            Self::Armv7 => "armv7",
        };
        formatter.write_str(value)
    }
}

impl FromStr for Arch {
    type Err = TargetParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "x64" | "x86_64" => Ok(Self::X64),
            "arm64" | "aarch64" => Ok(Self::Arm64),
            "armv6" => Ok(Self::Armv6),
            "armv7" | "arm" => Ok(Self::Armv7),
            _ => Err(TargetParseError::UnknownToken(value.to_owned())),
        }
    }
}

/// Parsed Node/platform/arch target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeTarget {
    /// Node range, for example `node18`.
    pub node_range: String,
    /// Target platform.
    pub platform: Platform,
    /// Target architecture.
    pub arch: Arch,
}

impl fmt::Display for NodeTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}-{}-{}",
            self.node_range, self.platform, self.arch
        )
    }
}

/// Collection of parsed targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedTargets {
    /// Parsed targets in CLI order.
    pub targets: Vec<NodeTarget>,
}

/// Error returned when a target string cannot be parsed.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum TargetParseError {
    /// The target contains an unknown token.
    #[error("unknown token '{0}' in target")]
    UnknownToken(String),
}

/// Parse comma-separated pkg target items.
///
/// # Example
///
/// ```
/// let defaults = pkg_rust::TargetDefaults::host("node18");
/// let parsed = pkg_rust::parse_targets("linux,macos,win", &defaults)?;
/// assert_eq!(parsed.targets.len(), 3);
/// # Ok::<(), pkg_rust::TargetParseError>(())
/// ```
pub fn parse_targets(
    input: &str,
    defaults: &TargetDefaults,
) -> Result<ParsedTargets, TargetParseError> {
    let mut targets = Vec::new();
    for item in input.split(',').filter(|item| !item.is_empty()) {
        targets.push(parse_target_item(item, defaults)?);
    }
    Ok(ParsedTargets { targets })
}

/// Calculate output names for targets using the original `pkg` suffix rules.
///
/// # Example
///
/// ```
/// let defaults = pkg_rust::TargetDefaults::host("node18");
/// let parsed = pkg_rust::parse_targets("linux,macos,win", &defaults)?;
/// assert_eq!(
///     pkg_rust::output_names("app", &parsed.targets),
///     vec!["app-linux", "app-macos", "app-win.exe"]
/// );
/// # Ok::<(), pkg_rust::TargetParseError>(())
/// ```
#[must_use]
pub fn output_names(output: &str, targets: &[NodeTarget]) -> Vec<String> {
    let different = DifferentParts::from_targets(targets);

    targets
        .iter()
        .map(|target| {
            let mut parts = vec![output.to_owned()];
            if targets.len() > 1 && different.node_range {
                parts.push(target.node_range.clone());
            }
            if targets.len() > 1 && different.platform {
                parts.push(target.platform.to_string());
            }
            if targets.len() > 1 && different.arch {
                parts.push(target.arch.to_string());
            }

            let mut file = parts.join("-");
            if target.platform == Platform::Win && !file.ends_with(".exe") {
                file.push_str(".exe");
            }
            file
        })
        .collect()
}

fn parse_target_item(
    item: &str,
    defaults: &TargetDefaults,
) -> Result<NodeTarget, TargetParseError> {
    let mut target = NodeTarget {
        node_range: defaults.node_range.clone(),
        platform: defaults.platform,
        arch: defaults.arch,
    };

    if item != "host" {
        for token in item.split('-').filter(|token| !token.is_empty()) {
            if is_node_range(token) {
                target.node_range = token.to_owned();
            } else if let Ok(platform) = Platform::from_str(token) {
                target.platform = platform;
            } else if let Ok(arch) = Arch::from_str(token) {
                target.arch = arch;
            } else {
                return Err(TargetParseError::UnknownToken(token.to_owned()));
            }
        }
    }

    Ok(target)
}

fn is_node_range(token: &str) -> bool {
    if token == "latest" {
        return true;
    }
    token
        .strip_prefix("node")
        .is_some_and(|version| !version.is_empty() && version.chars().all(|ch| ch.is_ascii_digit()))
}

#[derive(Default)]
struct DifferentParts {
    node_range: bool,
    platform: bool,
    arch: bool,
}

impl DifferentParts {
    fn from_targets(targets: &[NodeTarget]) -> Self {
        let first = match targets.first() {
            Some(first) => first,
            None => return Self::default(),
        };

        Self {
            node_range: targets
                .iter()
                .any(|target| target.node_range != first.node_range),
            platform: targets
                .iter()
                .any(|target| target.platform != first.platform),
            arch: targets.iter().any(|target| target.arch != first.arch),
        }
    }
}
