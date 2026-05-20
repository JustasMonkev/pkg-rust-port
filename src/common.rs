/// How a discovered path is stored in the virtual filesystem payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreKind {
    /// V8 bytecode without source text.
    Blob,
    /// Raw file contents.
    Content,
    /// Directory child listing.
    Links,
    /// Serialized file metadata.
    Stat,
}

/// Path syntax to use when emulating Node path helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathStyle {
    /// POSIX path behavior.
    Posix,
    /// Windows path behavior.
    Windows,
}

impl PathStyle {
    fn separator(self) -> char {
        match self {
            Self::Posix => '/',
            Self::Windows => '\\',
        }
    }
}

/// Normalize a path string using the same broad rules as the JavaScript port.
///
/// This helper intentionally works on strings because the virtual snapshot
/// filesystem needs to preserve target-platform syntax even when tests run on a
/// different host platform.
///
/// # Example
///
/// ```
/// assert_eq!(
///     pkg_rust::normalize_path_text("/snapshot//foo//", pkg_rust::PathStyle::Posix),
///     "/snapshot/foo"
/// );
/// ```
#[must_use]
pub fn normalize_path_text(input: &str, style: PathStyle) -> String {
    let normalized = match style {
        PathStyle::Posix => normalize_posix(input),
        PathStyle::Windows => normalize_windows(input),
    };
    remove_trailing_slashes(&normalized)
}

/// Return whether a path points inside the virtual `/snapshot` root.
///
/// # Example
///
/// ```
/// assert!(pkg_rust::inside_snapshot("/snapshot/app.js", pkg_rust::PathStyle::Posix));
/// assert!(!pkg_rust::inside_snapshot("/app.js", pkg_rust::PathStyle::Posix));
/// ```
#[must_use]
pub fn inside_snapshot(input: &str, style: PathStyle) -> bool {
    match style {
        PathStyle::Posix => input.starts_with("/snapshot/") || input == "/snapshot",
        PathStyle::Windows => {
            let file = normalize_path_text(input, style);
            windows_snapshot_rest(&file).is_some()
        }
    }
}

/// Replace a snapshot-root path with the display form used by diagnostics.
///
/// # Example
///
/// ```
/// assert_eq!(
///     pkg_rust::strip_snapshot("/snapshot/app.js", pkg_rust::PathStyle::Posix),
///     "/**/app.js"
/// );
/// ```
#[must_use]
pub fn strip_snapshot(input: &str, style: PathStyle) -> String {
    let file = normalize_path_text(input, style);
    match style {
        PathStyle::Posix => {
            if file == "/snapshot" {
                "/**/".to_owned()
            } else if let Some(rest) = file.strip_prefix("/snapshot/") {
                format!("/**/{rest}")
            } else {
                input.to_owned()
            }
        }
        PathStyle::Windows => strip_windows_snapshot(input, &file),
    }
}

/// Convert an absolute host path into a virtual snapshot path.
///
/// # Example
///
/// ```
/// assert_eq!(
///     pkg_rust::snapshotify("/project/app.js", pkg_rust::PathStyle::Posix),
///     "/snapshot/project/app.js"
/// );
/// ```
#[must_use]
pub fn snapshotify(input: &str, style: PathStyle) -> String {
    match style {
        PathStyle::Posix => {
            if input == "/" {
                "/snapshot".to_owned()
            } else if input.starts_with('/') {
                format!("/snapshot{input}")
            } else {
                input.to_owned()
            }
        }
        PathStyle::Windows => snapshotify_windows(input),
    }
}

/// Remove leading `..` path segments the same way the JS common helper does.
///
/// # Example
///
/// ```
/// assert_eq!(
///     pkg_rust::remove_uplevels("../../app.js", pkg_rust::PathStyle::Posix),
///     "app.js"
/// );
/// ```
#[must_use]
pub fn remove_uplevels(input: &str, style: PathStyle) -> String {
    let prefix = match style {
        PathStyle::Posix => "../",
        PathStyle::Windows => "..\\",
    };

    let mut output = input.to_owned();
    loop {
        if let Some(rest) = output.strip_prefix(prefix) {
            output = rest.to_owned();
        } else if output == ".." {
            output = ".".to_owned();
        } else {
            break;
        }
    }
    output
}

/// Find the common path denominator index used for snapshot path shortening.
///
/// # Example
///
/// ```
/// let files = ["/long/haired/freaky/people", "/long/haired/aliens"];
/// let denominator = pkg_rust::retrieve_denominator(&files, pkg_rust::PathStyle::Posix);
/// assert_eq!(denominator, 12);
/// ```
#[must_use]
pub fn retrieve_denominator(files: &[&str], style: PathStyle) -> usize {
    if files.is_empty() {
        return 0;
    }

    let sep = style.separator();
    let mut common = format!("{}{}", without_node_modules(files[0], sep), sep);
    for file in &files[1..] {
        let next = format!("{}{}", without_node_modules(file, sep), sep);
        let len = longest_common_prefix_len(&common, &next);
        common.truncate(len);
    }

    if common.is_empty() {
        return match style {
            PathStyle::Posix => 0,
            PathStyle::Windows => 2,
        };
    }

    common.rfind(sep).unwrap_or_default()
}

/// Remove the common denominator prefix from a path.
///
/// # Example
///
/// ```
/// assert_eq!(
///     pkg_rust::substitute_denominator("/long/haired/aliens", 12, pkg_rust::PathStyle::Posix),
///     "/aliens"
/// );
/// ```
#[must_use]
pub fn substitute_denominator(input: &str, denominator: usize, style: PathStyle) -> String {
    let root_len = match style {
        PathStyle::Posix => 0,
        PathStyle::Windows => 2,
    };
    let prefix = input.get(..root_len).unwrap_or_default();
    let suffix = input.get(denominator..).unwrap_or_default();
    format!("{prefix}{suffix}")
}

fn normalize_posix(input: &str) -> String {
    if input.is_empty() {
        return ".".to_owned();
    }

    let absolute = input.starts_with('/');
    let mut parts = Vec::new();
    for part in input.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push(part);
                }
            }
            _ => parts.push(part),
        }
    }

    let mut output = String::new();
    if absolute {
        output.push('/');
    }
    output.push_str(&parts.join("/"));
    if output.is_empty() {
        ".".to_owned()
    } else {
        output
    }
}

fn normalize_windows(input: &str) -> String {
    let mut file = input.replace('/', "\\");
    if file.len() >= 3
        && file.as_bytes().get(1) == Some(&b':')
        && file.as_bytes().get(2) == Some(&b'\\')
    {
        let drive = file[..1].to_ascii_uppercase();
        file.replace_range(..1, &drive);
    }

    while file.contains("\\\\") {
        file = file.replace("\\\\", "\\");
    }

    file
}

fn remove_trailing_slashes(input: &str) -> String {
    if input == "/" {
        return input.to_owned();
    }
    if input.len() == 3 && input.as_bytes().get(1) == Some(&b':') && input.ends_with('\\') {
        return input.to_owned();
    }

    let trimmed = input.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        input.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn strip_windows_snapshot(original: &str, normalized: &str) -> String {
    let bytes = normalized.as_bytes();
    let drive = bytes.first().copied().map(char::from).unwrap_or('C');

    if let Some(rest) = windows_snapshot_rest(normalized) {
        if rest.is_empty() {
            return format!("{drive}:\\**\\");
        }
        return format!("{drive}:\\**\\{rest}");
    }
    original.to_owned()
}

fn windows_snapshot_rest(normalized: &str) -> Option<&str> {
    let tail = normalized.get(2..)?;
    if tail == "\\snapshot" {
        Some("")
    } else {
        tail.strip_prefix("\\snapshot\\")
    }
}

fn snapshotify_windows(input: &str) -> String {
    let mut file = input.replace('/', "\\");
    if file.len() >= 2 && file.as_bytes().get(1) == Some(&b':') {
        let drive = file[..1].to_ascii_uppercase();
        file.replace_range(..1, &drive);
    }

    if file.len() >= 3
        && file.as_bytes().get(1) == Some(&b':')
        && file.as_bytes().get(2) == Some(&b'\\')
    {
        if file.len() == 3 {
            file.truncate(2);
        }
        return format!("{}\\snapshot{}", &file[..2], &file[2..]);
    }

    file
}

fn longest_common_prefix_len(left: &str, right: &str) -> usize {
    let mut len = 0;
    for (left_char, right_char) in left.chars().zip(right.chars()) {
        if left_char != right_char {
            break;
        }
        len += left_char.len_utf8();
    }
    len
}

fn without_node_modules(file: &str, sep: char) -> &str {
    let needle = format!("{sep}node_modules{sep}");
    file.split_once(&needle)
        .map_or(file, |(before, _after)| before)
}

impl StoreKind {
    /// Return the original JavaScript store index for fixture parity.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(pkg_rust::StoreKind::Content.as_index(), 1);
    /// ```
    #[must_use]
    pub const fn as_index(self) -> u8 {
        match self {
            Self::Blob => 0,
            Self::Content => 1,
            Self::Links => 2,
            Self::Stat => 3,
        }
    }
}

/// How a discovered module alias should be resolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AliasKind {
    /// Resolve relative to the current source file.
    Relative,
    /// Resolve through Node package/module resolution.
    Resolvable,
}

impl AliasKind {
    /// Return the original JavaScript alias index for fixture parity.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(pkg_rust::AliasKind::Resolvable.as_index(), 1);
    /// ```
    #[must_use]
    pub const fn as_index(self) -> u8 {
        match self {
            Self::Relative => 0,
            Self::Resolvable => 1,
        }
    }
}
