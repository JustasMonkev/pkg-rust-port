use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::common::{PathStyle, StoreKind, snapshotify};
use crate::compress::Compression as PayloadCompression;
use crate::error::PkgError;
use crate::fabricate::{FabricateRequest, FabricatorPool, fabricate};
use crate::pack::{PackedOutput, Stripe};

const PAYLOAD_POSITION_PLACEHOLDER: &str = "// PAYLOAD_POSITION //";
const PAYLOAD_SIZE_PLACEHOLDER: &str = "// PAYLOAD_SIZE //";
const PRELUDE_POSITION_PLACEHOLDER: &str = "// PRELUDE_POSITION //";
const PRELUDE_SIZE_PLACEHOLDER: &str = "// PRELUDE_SIZE //";
/// Byte range for one store inside the payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadPointer {
    /// Offset from the start of the payload.
    pub offset: u64,
    /// Number of bytes stored for this entry.
    pub size: u64,
}

/// Producer-stage manifest before executable placeholder injection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerManifest {
    /// Snapshotified entrypoint path.
    pub entrypoint: String,
    /// Snapshotified symlink map.
    pub symlinks: BTreeMap<String, String>,
    /// Virtual filesystem dictionary: snapshot path -> store index -> payload pointer.
    pub vfs: BTreeMap<String, BTreeMap<u8, PayloadPointer>>,
    /// Dictionary used to compress VFS path components.
    pub path_dictionary: BTreeMap<String, String>,
    /// Total payload size after per-stripe compression.
    pub payload_size: u64,
    /// Compression mode for payload entries.
    pub compression: PayloadCompression,
}

/// Produced executable image and layout metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducedExecutable {
    /// Final executable bytes.
    pub bytes: Vec<u8>,
    /// Producer manifest used to render the prelude.
    pub manifest: ProducerManifest,
    /// Byte offset where the payload starts.
    pub payload_position: u64,
    /// Byte offset where the prelude starts.
    pub prelude_position: u64,
    /// Rendered prelude byte size.
    pub prelude_size: u64,
}

/// One placeholder discovered in a target binary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placeholder {
    /// Byte position of the placeholder.
    pub position: usize,
    /// Placeholder byte length.
    pub size: usize,
    padder: u8,
}

/// Placeholder locations required for executable metadata injection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceholderSet {
    /// Bakery argument placeholder.
    pub bakery: Option<Placeholder>,
    /// Payload position placeholder.
    pub payload_position: Option<Placeholder>,
    /// Payload size placeholder.
    pub payload_size: Option<Placeholder>,
    /// Prelude position placeholder.
    pub prelude_position: Option<Placeholder>,
    /// Prelude size placeholder.
    pub prelude_size: Option<Placeholder>,
}

/// Values written into executable placeholders after payload/prelude layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceholderValues {
    /// Encoded bakery arguments.
    pub bakery: Vec<u8>,
    /// Payload start offset in the executable.
    pub payload_position: u64,
    /// Payload byte length.
    pub payload_size: u64,
    /// Prelude start offset in the executable.
    pub prelude_position: u64,
    /// Prelude byte length.
    pub prelude_size: u64,
}

/// Build the producer manifest for uncompressed payload stripes.
///
/// # Example
///
/// ```
/// let package = pkg_rust::PackageJson::parse("{}")
///     .map_err(|error| pkg_rust::PkgError::Resolve(error.to_string()))?;
/// let marker = pkg_rust::Marker::new(package);
/// let entrypoint = "../test/test-50-require-resolve/test-z-require-content.css";
/// let walked = pkg_rust::walk(marker, entrypoint, None, pkg_rust::WalkerParams::new())?;
/// let refined = pkg_rust::refine_walked(walked, entrypoint, pkg_rust::PathStyle::Posix);
/// let packed = pkg_rust::pack(refined, true)?;
/// let manifest = pkg_rust::produce_manifest(packed, pkg_rust::Compression::None, pkg_rust::PathStyle::Posix)?;
/// assert!(manifest.entrypoint.starts_with("/snapshot/"));
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn produce_manifest(
    packed: PackedOutput,
    compression: PayloadCompression,
    style: PathStyle,
) -> Result<ProducerManifest, PkgError> {
    let native_addons = NativeAddonOptions::default();
    let (manifest, _payload) =
        build_manifest_and_payload(packed, compression, style, None, &native_addons)?;
    Ok(manifest)
}

/// Produce an executable image by appending payload and rendered prelude bytes.
///
/// This mirrors the JavaScript producer's byte layout while staying in memory:
/// binary first, payload second, prelude last, then placeholder values patched
/// back into the binary segment.
///
/// # Example
///
/// ```
/// let mut binary = Vec::from([b'\0']);
/// for _index in 0..20 {
///     binary.extend_from_slice(b"// BAKERY ");
/// }
/// binary.extend_from_slice(b"// PAYLOAD_POSITION //// PAYLOAD_SIZE //// PRELUDE_POSITION //// PRELUDE_SIZE //");
/// let package = pkg_rust::PackageJson::parse("{}")
///     .map_err(|error| pkg_rust::PkgError::Resolve(error.to_string()))?;
/// let walked = pkg_rust::walk(
///     pkg_rust::Marker::new(package),
///     "../test/test-50-require-resolve/test-z-require-content.css",
///     None,
///     pkg_rust::WalkerParams::new(),
/// )?;
/// let refined = pkg_rust::refine_walked(
///     walked,
///     "../test/test-50-require-resolve/test-z-require-content.css",
///     pkg_rust::PathStyle::Posix,
/// );
/// let packed = pkg_rust::pack(refined, true)?;
/// let produced = pkg_rust::produce_executable_image(
///     binary,
///     packed,
///     "%VIRTUAL_FILESYSTEM% %DEFAULT_ENTRYPOINT% %SYMLINKS% %DICT% %DOCOMPRESS%",
///     pkg_rust::Compression::None,
///     pkg_rust::PathStyle::Posix,
///     Vec::new(),
/// )?;
/// assert!(produced.bytes.len() > produced.payload_position as usize);
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn produce_executable_image(
    binary: Vec<u8>,
    packed: PackedOutput,
    prelude_template: &str,
    compression: PayloadCompression,
    style: PathStyle,
    bakery: Vec<u8>,
) -> Result<ProducedExecutable, PkgError> {
    let native_addons = NativeAddonOptions::default();
    let (manifest, payload) =
        build_manifest_and_payload(packed, compression, style, None, &native_addons)?;
    let prelude = prelude_buffer_from_prelude(&render_prelude(prelude_template, &manifest)?);
    let payload_position = binary.len() as u64;
    let payload_size = payload.len() as u64;
    let prelude_position = payload_position + payload_size;
    let prelude_size = prelude.len() as u64;

    let mut bytes = binary;
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&prelude);

    let placeholders = discover_placeholders(&bytes);
    let values = PlaceholderValues {
        bakery,
        payload_position,
        payload_size,
        prelude_position,
        prelude_size,
    };
    inject_placeholders(
        &mut bytes,
        &placeholders,
        &values,
        &[
            PlaceholderKind::Bakery,
            PlaceholderKind::PayloadPosition,
            PlaceholderKind::PayloadSize,
            PlaceholderKind::PreludePosition,
            PlaceholderKind::PreludeSize,
        ],
    )?;

    Ok(ProducedExecutable {
        bytes,
        manifest,
        payload_position,
        prelude_position,
        prelude_size,
    })
}

/// Produce an executable image and write it to disk.
///
/// The returned value contains the same bytes written to `output`, which keeps
/// tests and later CLI orchestration able to inspect the computed layout.
///
/// # Example
///
/// ```
/// let mut binary = Vec::from([b'\0']);
/// for _index in 0..20 {
///     binary.extend_from_slice(b"// BAKERY ");
/// }
/// binary.extend_from_slice(b"// PAYLOAD_POSITION //// PAYLOAD_SIZE //// PRELUDE_POSITION //// PRELUDE_SIZE //");
/// let packed = pkg_rust::PackedOutput {
///     entrypoint: "/project/app.js".to_owned(),
///     symlinks: std::collections::BTreeMap::new(),
///     stripes: vec![pkg_rust::Stripe {
///         snap: "/project/app.js".to_owned(),
///         store: pkg_rust::StoreKind::Content,
///         file: None,
///         buffer: Some(b"console.log('hi');".to_vec()),
///     }],
/// };
/// let output = std::env::temp_dir().join(format!("pkg-rust-output-{}", std::process::id()));
/// let produced = pkg_rust::write_executable_image(
///     &output,
///     binary,
///     packed,
///     "%VIRTUAL_FILESYSTEM% %DEFAULT_ENTRYPOINT% %SYMLINKS% %DICT% %DOCOMPRESS%",
///     pkg_rust::Compression::None,
///     pkg_rust::PathStyle::Posix,
///     Vec::new(),
/// )?;
/// assert_eq!(std::fs::read(&output).map_err(|source| pkg_rust::PkgError::Io {
///     path: output.display().to_string(),
///     source,
/// })?, produced.bytes);
/// let _ = std::fs::remove_file(output);
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn write_executable_image(
    output: impl AsRef<Path>,
    binary: Vec<u8>,
    packed: PackedOutput,
    prelude_template: &str,
    compression: PayloadCompression,
    style: PathStyle,
    bakery: Vec<u8>,
) -> Result<ProducedExecutable, PkgError> {
    write_executable_image_with_fabricator(
        output,
        binary,
        packed,
        prelude_template,
        ProducerBuildOptions {
            compression,
            style,
            bakery,
            fabricator_path: None,
            native_addons: NativeAddonOptions::default(),
        },
    )
}

pub(crate) struct ProducerBuildOptions<'a> {
    pub(crate) compression: PayloadCompression,
    pub(crate) style: PathStyle,
    pub(crate) bakery: Vec<u8>,
    pub(crate) fabricator_path: Option<&'a Path>,
    pub(crate) native_addons: NativeAddonOptions,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeAddonOptions {
    pub(crate) platform: Option<String>,
    pub(crate) install_platform: Option<String>,
    pub(crate) arch: Option<String>,
    pub(crate) node_version: Option<String>,
    pub(crate) prebuild_install: Option<PathBuf>,
}

pub(crate) fn write_executable_image_with_fabricator(
    output: impl AsRef<Path>,
    binary: Vec<u8>,
    packed: PackedOutput,
    prelude_template: &str,
    options: ProducerBuildOptions<'_>,
) -> Result<ProducedExecutable, PkgError> {
    let output = output.as_ref();
    let (manifest, payload) = build_manifest_and_payload(
        packed,
        options.compression,
        options.style,
        options.fabricator_path,
        &options.native_addons,
    )?;
    let prelude = prelude_buffer_from_prelude(&render_prelude(prelude_template, &manifest)?);
    let payload_position = binary.len() as u64;
    let payload_size = payload.len() as u64;
    let prelude_position = payload_position + payload_size;
    let prelude_size = prelude.len() as u64;

    let mut bytes = binary;
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&prelude);

    let placeholders = discover_placeholders(&bytes);
    let values = PlaceholderValues {
        bakery: options.bakery,
        payload_position,
        payload_size,
        prelude_position,
        prelude_size,
    };
    inject_placeholders(
        &mut bytes,
        &placeholders,
        &values,
        &[
            PlaceholderKind::Bakery,
            PlaceholderKind::PayloadPosition,
            PlaceholderKind::PayloadSize,
            PlaceholderKind::PreludePosition,
            PlaceholderKind::PreludeSize,
        ],
    )?;

    let produced = ProducedExecutable {
        bytes,
        manifest,
        payload_position,
        prelude_position,
        prelude_size,
    };
    fs::write(output, &produced.bytes).map_err(|source| PkgError::Io {
        path: output.display().to_string(),
        source,
    })?;
    Ok(produced)
}

fn build_manifest_and_payload(
    packed: PackedOutput,
    compression: PayloadCompression,
    style: PathStyle,
    fabricator_path: Option<&Path>,
    native_addons: &NativeAddonOptions,
) -> Result<(ProducerManifest, Vec<u8>), PkgError> {
    let mut offset = 0_u64;
    let mut payload = Vec::new();
    let mut vfs: BTreeMap<String, BTreeMap<u8, PayloadPointer>> = BTreeMap::new();
    let mut path_dictionary = PathDictionary::default();

    for stripe in packed.stripes {
        let snap = snapshotify(&stripe.snap, style);
        let stripe_bytes = stripe_bytes(&stripe, native_addons)?;
        let payload_bytes = if stripe.store == StoreKind::Blob {
            // DECISION: prefer target-specific bytecode when the provider
            // exposes a runnable target binary path; fall back to host `node`
            // for deterministic in-memory test providers.
            let mut pool = FabricatorPool::new();
            let request = match fabricator_path {
                Some(path) => FabricateRequest::new(&snap, &stripe_bytes).with_executable(path),
                None => FabricateRequest::new(&snap, &stripe_bytes),
            };
            fabricate(&mut pool, request)?
        } else {
            stripe_bytes
        };
        let payload_bytes = compress_payload(&payload_bytes, compression)?;
        let size = payload_bytes.len() as u64;
        let vfs_key = path_dictionary.make_key(compression, &snap);
        vfs.entry(vfs_key)
            .or_default()
            .insert(stripe.store.as_index(), PayloadPointer { offset, size });
        offset += size;
        payload.extend_from_slice(&payload_bytes);
    }

    let symlinks = packed
        .symlinks
        .into_iter()
        .map(|(link, real)| {
            let link = snapshotify(&link, style);
            let real = snapshotify(&real, style);
            (
                path_dictionary.make_key(compression, &link),
                path_dictionary.make_key(compression, &real),
            )
        })
        .collect();

    Ok((
        ProducerManifest {
            entrypoint: snapshotify(&packed.entrypoint, style),
            symlinks,
            vfs,
            path_dictionary: path_dictionary.entries,
            payload_size: offset,
            compression,
        },
        payload,
    ))
}

fn prelude_buffer_from_prelude(prelude: &str) -> Vec<u8> {
    format!(
        "(function(process, require, console, EXECPATH_FD, PAYLOAD_POSITION, PAYLOAD_SIZE) {{ {prelude}\n}})"
    )
    .into_bytes()
}

/// Render a prelude template by replacing the JavaScript producer placeholders.
///
/// # Example
///
/// ```
/// let manifest = pkg_rust::ProducerManifest {
///     entrypoint: "/snapshot/app.js".to_owned(),
///     symlinks: std::collections::BTreeMap::new(),
///     vfs: std::collections::BTreeMap::new(),
///     path_dictionary: std::collections::BTreeMap::new(),
///     payload_size: 0,
///     compression: pkg_rust::Compression::None,
/// };
/// let rendered = pkg_rust::render_prelude(
///     "%VIRTUAL_FILESYSTEM% %DEFAULT_ENTRYPOINT% %SYMLINKS% %DICT% %DOCOMPRESS%",
///     &manifest,
/// )?;
/// assert!(rendered.contains("\"/snapshot/app.js\""));
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn render_prelude(template: &str, manifest: &ProducerManifest) -> Result<String, PkgError> {
    let vfs = manifest_vfs_json(manifest);
    let replacements = [
        (
            "%VIRTUAL_FILESYSTEM%",
            serde_json::to_string(&vfs)
                .map_err(|error| PkgError::Pack(format!("vfs json failed: {error}")))?,
        ),
        (
            "%DEFAULT_ENTRYPOINT%",
            serde_json::to_string(&manifest.entrypoint)
                .map_err(|error| PkgError::Pack(format!("entrypoint json failed: {error}")))?,
        ),
        (
            "%SYMLINKS%",
            serde_json::to_string(&manifest.symlinks)
                .map_err(|error| PkgError::Pack(format!("symlink json failed: {error}")))?,
        ),
        (
            "%DICT%",
            serde_json::to_string(&manifest.path_dictionary)
                .map_err(|error| PkgError::Pack(format!("dictionary json failed: {error}")))?,
        ),
        ("%DOCOMPRESS%", manifest.compression.as_index().to_string()),
    ];

    let mut rendered = template.to_owned();
    for (placeholder, value) in replacements {
        rendered = rendered.replace(placeholder, &value);
    }
    Ok(rendered)
}

/// Discover producer placeholders in a binary buffer.
///
/// # Example
///
/// ```
/// let mut binary = Vec::new();
/// binary.extend_from_slice(b"prefix");
/// binary.extend_from_slice(b"// PAYLOAD_SIZE //");
/// let placeholders = pkg_rust::discover_placeholders(&binary);
/// assert!(placeholders.payload_size.is_some());
/// ```
#[must_use]
pub fn discover_placeholders(binary: &[u8]) -> PlaceholderSet {
    let bakery = bakery_placeholder();
    PlaceholderSet {
        bakery: discover_placeholder(binary, &bakery, b'\0'),
        payload_position: discover_placeholder(
            binary,
            PAYLOAD_POSITION_PLACEHOLDER.as_bytes(),
            b' ',
        ),
        payload_size: discover_placeholder(binary, PAYLOAD_SIZE_PLACEHOLDER.as_bytes(), b' '),
        prelude_position: discover_placeholder(
            binary,
            PRELUDE_POSITION_PLACEHOLDER.as_bytes(),
            b' ',
        ),
        prelude_size: discover_placeholder(binary, PRELUDE_SIZE_PLACEHOLDER.as_bytes(), b' '),
    }
}

fn bakery_placeholder() -> Vec<u8> {
    let mut value = Vec::from([b'\0']);
    for _index in 0..20 {
        value.extend_from_slice(b"// BAKERY ");
    }
    value
}

/// Inject producer placeholder values into a mutable binary buffer.
///
/// # Example
///
/// ```
/// let mut binary = b"// PAYLOAD_SIZE //".to_vec();
/// let placeholders = pkg_rust::discover_placeholders(&binary);
/// let values = pkg_rust::PlaceholderValues {
///     bakery: Vec::new(),
///     payload_position: 0,
///     payload_size: 42,
///     prelude_position: 0,
///     prelude_size: 0,
/// };
/// pkg_rust::inject_placeholders(&mut binary, &placeholders, &values, &[pkg_rust::PlaceholderKind::PayloadSize])?;
/// assert!(String::from_utf8_lossy(&binary).starts_with("42"));
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn inject_placeholders(
    binary: &mut [u8],
    placeholders: &PlaceholderSet,
    values: &PlaceholderValues,
    kinds: &[PlaceholderKind],
) -> Result<(), PkgError> {
    for kind in kinds {
        let (placeholder, value) = match kind {
            PlaceholderKind::Bakery => (&placeholders.bakery, values.bakery.clone()),
            PlaceholderKind::PayloadPosition => (
                &placeholders.payload_position,
                values.payload_position.to_string().into_bytes(),
            ),
            PlaceholderKind::PayloadSize => (
                &placeholders.payload_size,
                values.payload_size.to_string().into_bytes(),
            ),
            PlaceholderKind::PreludePosition => (
                &placeholders.prelude_position,
                values.prelude_position.to_string().into_bytes(),
            ),
            PlaceholderKind::PreludeSize => (
                &placeholders.prelude_size,
                values.prelude_size.to_string().into_bytes(),
            ),
        };
        inject_placeholder(binary, *kind, placeholder, &value)?;
    }
    Ok(())
}

/// Placeholder field to inject.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaceholderKind {
    /// Bakery argument placeholder.
    Bakery,
    /// Payload position placeholder.
    PayloadPosition,
    /// Payload size placeholder.
    PayloadSize,
    /// Prelude position placeholder.
    PreludePosition,
    /// Prelude size placeholder.
    PreludeSize,
}

fn discover_placeholder(binary: &[u8], needle: &[u8], padder: u8) -> Option<Placeholder> {
    find_subslice(binary, needle).map(|position| Placeholder {
        position,
        size: needle.len(),
        padder,
    })
}

fn inject_placeholder(
    binary: &mut [u8],
    kind: PlaceholderKind,
    placeholder: &Option<Placeholder>,
    value: &[u8],
) -> Result<(), PkgError> {
    let Some(placeholder) = placeholder else {
        return Err(PkgError::Pack(format!(
            "placeholder {kind:?} was not found"
        )));
    };
    if value.len() > placeholder.size {
        return Err(PkgError::Pack(format!(
            "placeholder {kind:?} value is too large"
        )));
    }

    let Some(target) =
        binary.get_mut(placeholder.position..placeholder.position + placeholder.size)
    else {
        return Err(PkgError::Pack(format!(
            "placeholder {kind:?} range is outside binary"
        )));
    };
    let (value_target, padding_target) = target.split_at_mut(value.len());
    value_target.copy_from_slice(value);
    padding_target.fill(placeholder.padder);
    Ok(())
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn manifest_vfs_json(manifest: &ProducerManifest) -> BTreeMap<String, BTreeMap<String, [u64; 2]>> {
    manifest
        .vfs
        .iter()
        .map(|(path, stores)| {
            let stores = stores
                .iter()
                .map(|(store, pointer)| (store.to_string(), [pointer.offset, pointer.size]))
                .collect();
            (path.clone(), stores)
        })
        .collect()
}

#[derive(Default)]
struct PathDictionary {
    entries: BTreeMap<String, String>,
    counter: usize,
}

impl PathDictionary {
    fn make_key(&mut self, compression: PayloadCompression, full_path: &str) -> String {
        if compression == PayloadCompression::None {
            return full_path.to_owned();
        }

        full_path
            .split('/')
            .map(|part| self.get_or_create_hash(part))
            .collect::<Vec<_>>()
            .join("/")
    }

    fn get_or_create_hash(&mut self, value: &str) -> String {
        if let Some(existing) = self.entries.get(value) {
            return existing.clone();
        }

        let next = base36(self.counter);
        self.counter += 1;
        self.entries.insert(value.to_owned(), next.clone());
        next
    }
}

fn base36(mut value: usize) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_owned();
    }

    let mut output = Vec::new();
    while value > 0 {
        let digit = value % 36;
        if let Some(byte) = DIGITS.get(digit) {
            output.push(char::from(*byte));
        }
        value /= 36;
    }
    output.iter().rev().collect()
}

fn stripe_bytes(stripe: &Stripe, native_addons: &NativeAddonOptions) -> Result<Vec<u8>, PkgError> {
    if let Some(buffer) = stripe.buffer.as_ref() {
        return Ok(buffer.clone());
    }

    let Some(file) = stripe.file.as_ref() else {
        return Err(PkgError::Pack(format!(
            "stripe '{}' has neither buffer nor file",
            stripe.snap
        )));
    };
    let selected_file =
        native_addon_file(file, native_addons).unwrap_or_else(|| PathBuf::from(file));
    fs::read(&selected_file).map_err(|source| PkgError::Io {
        path: selected_file.display().to_string(),
        source,
    })
}

fn native_addon_file(file: &str, native_addons: &NativeAddonOptions) -> Option<PathBuf> {
    if Path::new(file)
        .extension()
        .is_none_or(|extension| extension != "node")
    {
        return None;
    }

    let platform = native_addons.platform.as_deref()?;
    let node_version = native_addons.node_version.as_deref()?;
    let candidate = PathBuf::from(format!("{file}.{platform}.{node_version}"));
    if candidate.is_file() {
        return Some(candidate);
    }

    try_native_prebuild_install(Path::new(file), &candidate, native_addons)
        .ok()
        .and_then(|installed| installed.then_some(candidate))
        .filter(|installed| installed.is_file())
}

fn try_native_prebuild_install(
    node_file: &Path,
    native_file: &Path,
    native_addons: &NativeAddonOptions,
) -> Result<bool, PkgError> {
    let Some(prebuild_install) = native_addons.prebuild_install.as_ref() else {
        return Ok(false);
    };
    let Some(install_platform) = native_addons.install_platform.as_deref() else {
        return Ok(false);
    };
    let Some(arch) = native_addons.arch.as_deref() else {
        return Ok(false);
    };
    let Some(node_version) = native_addons.node_version.as_deref() else {
        return Ok(false);
    };
    let Some(package_dir) = find_package_dir(node_file) else {
        return Ok(false);
    };

    let backup = PathBuf::from(format!("{}.bak", node_file.display()));
    if !backup.is_file() {
        fs::copy(node_file, &backup).map_err(|source| PkgError::Io {
            path: backup.display().to_string(),
            source,
        })?;
    }

    let result = run_prebuild_install(
        prebuild_install,
        &package_dir,
        install_platform,
        arch,
        node_version,
        package_uses_napi_versions(&package_dir)?,
    )
    .and_then(|()| {
        fs::copy(node_file, native_file).map_err(|source| PkgError::Io {
            path: native_file.display().to_string(),
            source,
        })?;
        Ok(())
    });

    let restore_result = restore_native_backup(&backup, node_file);
    result.and(restore_result)?;
    Ok(true)
}

fn find_package_dir(node_file: &Path) -> Option<PathBuf> {
    let mut dir = node_file.parent()?;
    loop {
        let package_json = dir.join("package.json");
        if package_json.is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

fn package_uses_napi_versions(package_dir: &Path) -> Result<bool, PkgError> {
    let package_json = package_dir.join("package.json");
    let body = fs::read_to_string(&package_json).map_err(|source| PkgError::Io {
        path: package_json.display().to_string(),
        source,
    })?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| PkgError::Pack(format!("package.json parse failed: {error}")))?;
    Ok(value
        .get("binary")
        .and_then(|binary| binary.get("napi_versions"))
        .is_some_and(|versions| !versions.is_null()))
}

fn run_prebuild_install(
    prebuild_install: &Path,
    package_dir: &Path,
    install_platform: &str,
    arch: &str,
    node_version: &str,
    uses_napi_versions: bool,
) -> Result<(), PkgError> {
    let mut command = Command::new(prebuild_install);
    command
        .arg("--platform")
        .arg(install_platform)
        .arg("--arch")
        .arg(arch)
        .current_dir(package_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if !uses_napi_versions {
        command.arg("--target").arg(node_version);
    }

    let output = command.output().map_err(|source| PkgError::Io {
        path: prebuild_install.display().to_string(),
        source,
    })?;
    if output.status.success() {
        return Ok(());
    }

    Err(PkgError::Pack(format!(
        "prebuild-install failed for {}: {}",
        package_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn restore_native_backup(backup: &Path, node_file: &Path) -> Result<(), PkgError> {
    if !backup.is_file() {
        return Ok(());
    }
    match fs::remove_file(node_file) {
        Ok(()) => {}
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(PkgError::Io {
                path: node_file.display().to_string(),
                source,
            });
        }
    }
    fs::rename(backup, node_file).map_err(|source| PkgError::Io {
        path: node_file.display().to_string(),
        source,
    })
}

fn compress_payload(payload: &[u8], compression: PayloadCompression) -> Result<Vec<u8>, PkgError> {
    match compression {
        PayloadCompression::None => Ok(payload.to_vec()),
        PayloadCompression::Gzip => gzip_payload(payload),
        PayloadCompression::Brotli => brotli_payload(payload),
    }
}

fn gzip_payload(payload: &[u8]) -> Result<Vec<u8>, PkgError> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(payload)
        .map_err(|error| PkgError::Pack(format!("gzip write failed: {error}")))?;
    encoder
        .finish()
        .map_err(|error| PkgError::Pack(format!("gzip finish failed: {error}")))
}

fn brotli_payload(payload: &[u8]) -> Result<Vec<u8>, PkgError> {
    // DECISION: Node's `createBrotliCompress()` uses zlib's default Brotli
    // parameters. The Rust port uses the standard max-quality/window defaults
    // exposed by the `brotli` crate until fixture parity requires tuning.
    let mut reader = brotli::CompressorReader::new(payload, 4096, 11, 22);
    let mut output = Vec::new();
    reader
        .read_to_end(&mut output)
        .map_err(|error| PkgError::Pack(format!("brotli compression failed: {error}")))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::common::StoreKind;

    #[cfg(unix)]
    #[test]
    fn explicit_fabricator_path_is_used_for_blob_payload() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir =
            std::env::temp_dir().join(format!("pkg-rust-fabricator-path-{}", std::process::id()));
        let _ignored = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir)?;
        let fabricator = temp_dir.join("fake-node");
        fs::write(
            &fabricator,
            "#!/bin/sh\ncat >/dev/null\nprintf TARGET_BYTECODE\n",
        )?;
        let mut permissions = fs::metadata(&fabricator)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fabricator, permissions)?;

        let produced = write_executable_image_with_fabricator(
            temp_dir.join("out"),
            binary_with_placeholders(),
            PackedOutput {
                entrypoint: "/app.js".to_owned(),
                symlinks: BTreeMap::new(),
                stripes: vec![Stripe {
                    snap: "/app.js".to_owned(),
                    store: StoreKind::Blob,
                    file: None,
                    buffer: Some(b"module.exports = 42;".to_vec()),
                }],
            },
            "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
            ProducerBuildOptions {
                compression: PayloadCompression::None,
                style: PathStyle::Posix,
                bakery: Vec::new(),
                fabricator_path: Some(&fabricator),
                native_addons: NativeAddonOptions::default(),
            },
        )?;
        let pointer = produced
            .manifest
            .vfs
            .get("/snapshot/app.js")
            .and_then(|stores| stores.get(&StoreKind::Blob.as_index()))
            .ok_or_else(|| PkgError::Pack("blob payload pointer was missing".to_owned()))?;
        let start = produced.payload_position as usize + pointer.offset as usize;
        let end = start + pointer.size as usize;

        assert_eq!(
            produced
                .bytes
                .get(start..end)
                .ok_or_else(|| PkgError::Pack("payload range was outside image".to_owned()))?,
            b"TARGET_BYTECODE"
        );

        fs::remove_dir_all(temp_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn prebuild_install_caches_native_addon_and_restores_original()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "pkg-rust-prebuild-install-cache-{}",
            std::process::id()
        ));
        let _ignored = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir)?;
        let addon = temp_dir.join("addon.node");
        fs::write(&addon, b"ORIGINAL_NATIVE")?;
        fs::write(
            temp_dir.join("package.json"),
            r#"{"name":"demo","binary":{}}"#,
        )?;
        let prebuild_install = fake_prebuild_install(&temp_dir)?;

        let produced = produce_with_native_addon(
            &temp_dir,
            &addon,
            NativeAddonOptions {
                platform: Some("linux".to_owned()),
                install_platform: Some("linux".to_owned()),
                arch: Some("x64".to_owned()),
                node_version: Some("v18.5.0".to_owned()),
                prebuild_install: Some(prebuild_install),
            },
        )?;

        let native_file = PathBuf::from(format!("{}.linux.v18.5.0", addon.display()));
        assert!(contains_bytes(&produced.bytes, b"GENERATED_NATIVE"));
        assert!(!contains_bytes(&produced.bytes, b"ORIGINAL_NATIVE"));
        assert_eq!(fs::read(&native_file)?, b"GENERATED_NATIVE");
        assert_eq!(fs::read(&addon)?, b"ORIGINAL_NATIVE");
        let args = fs::read_to_string(temp_dir.join("args.txt"))?;
        assert!(args.contains("--platform\nlinux"));
        assert!(args.contains("--arch\nx64"));
        assert!(args.contains("--target\nv18.5.0"));

        fs::remove_dir_all(temp_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn prebuild_install_skips_target_when_package_uses_napi_versions()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "pkg-rust-prebuild-install-napi-{}",
            std::process::id()
        ));
        let _ignored = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir)?;
        let addon = temp_dir.join("addon.node");
        fs::write(&addon, b"ORIGINAL_NATIVE")?;
        fs::write(
            temp_dir.join("package.json"),
            r#"{"name":"demo","binary":{"napi_versions":[3]}}"#,
        )?;
        let prebuild_install = fake_prebuild_install(&temp_dir)?;

        let produced = produce_with_native_addon(
            &temp_dir,
            &addon,
            NativeAddonOptions {
                platform: Some("linux".to_owned()),
                install_platform: Some("linux".to_owned()),
                arch: Some("x64".to_owned()),
                node_version: Some("v18.5.0".to_owned()),
                prebuild_install: Some(prebuild_install),
            },
        )?;

        assert!(contains_bytes(&produced.bytes, b"GENERATED_NATIVE"));
        let args = fs::read_to_string(temp_dir.join("args.txt"))?;
        assert!(!args.contains("--target"));
        assert_eq!(fs::read(&addon)?, b"ORIGINAL_NATIVE");

        fs::remove_dir_all(temp_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn failed_prebuild_install_falls_back_to_original_addon()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "pkg-rust-prebuild-install-fallback-{}",
            std::process::id()
        ));
        let _ignored = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir)?;
        let addon = temp_dir.join("addon.node");
        fs::write(&addon, b"ORIGINAL_NATIVE")?;
        fs::write(
            temp_dir.join("package.json"),
            r#"{"name":"demo","binary":{}}"#,
        )?;
        let prebuild_install = temp_dir.join("prebuild-install-fail");
        fs::write(
            &prebuild_install,
            "#!/bin/sh\nprintf BROKEN_NATIVE > addon.node\nprintf failed >&2\nexit 9\n",
        )?;
        let mut permissions = fs::metadata(&prebuild_install)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&prebuild_install, permissions)?;

        let produced = produce_with_native_addon(
            &temp_dir,
            &addon,
            NativeAddonOptions {
                platform: Some("linux".to_owned()),
                install_platform: Some("linux".to_owned()),
                arch: Some("x64".to_owned()),
                node_version: Some("v18.5.0".to_owned()),
                prebuild_install: Some(prebuild_install),
            },
        )?;

        let native_file = PathBuf::from(format!("{}.linux.v18.5.0", addon.display()));
        assert!(contains_bytes(&produced.bytes, b"ORIGINAL_NATIVE"));
        assert!(!contains_bytes(&produced.bytes, b"BROKEN_NATIVE"));
        assert!(!native_file.exists());
        assert_eq!(fs::read(&addon)?, b"ORIGINAL_NATIVE");

        fs::remove_dir_all(temp_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    fn fake_prebuild_install(temp_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let prebuild_install = temp_dir.join("prebuild-install");
        fs::write(
            &prebuild_install,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\nprintf GENERATED_NATIVE > addon.node\n",
        )?;
        let mut permissions = fs::metadata(&prebuild_install)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&prebuild_install, permissions)?;
        Ok(prebuild_install)
    }

    #[cfg(unix)]
    fn produce_with_native_addon(
        temp_dir: &Path,
        addon: &Path,
        native_addons: NativeAddonOptions,
    ) -> Result<ProducedExecutable, PkgError> {
        let file = addon
            .to_str()
            .ok_or_else(|| PkgError::Pack("addon path must be utf-8".to_owned()))?
            .to_owned();
        write_executable_image_with_fabricator(
            temp_dir.join("out"),
            binary_with_placeholders(),
            PackedOutput {
                entrypoint: "/app.js".to_owned(),
                symlinks: BTreeMap::new(),
                stripes: vec![Stripe {
                    snap: "/addon.node".to_owned(),
                    store: StoreKind::Content,
                    file: Some(file),
                    buffer: None,
                }],
            },
            "%VIRTUAL_FILESYSTEM%\n%DEFAULT_ENTRYPOINT%\n%SYMLINKS%\n%DICT%\n%DOCOMPRESS%",
            ProducerBuildOptions {
                compression: PayloadCompression::None,
                style: PathStyle::Posix,
                bakery: Vec::new(),
                fabricator_path: None,
                native_addons,
            },
        )
    }

    fn contains_bytes(bytes: &[u8], needle: &[u8]) -> bool {
        bytes.windows(needle.len()).any(|window| window == needle)
    }

    fn binary_with_placeholders() -> Vec<u8> {
        let mut binary = Vec::from([b'\0']);
        for _index in 0..20 {
            binary.extend_from_slice(b"// BAKERY ");
        }
        binary.extend_from_slice(b"// PAYLOAD_POSITION //");
        binary.extend_from_slice(b"// PAYLOAD_SIZE //");
        binary.extend_from_slice(b"// PRELUDE_POSITION //");
        binary.extend_from_slice(b"// PRELUDE_SIZE //");
        binary
    }
}
