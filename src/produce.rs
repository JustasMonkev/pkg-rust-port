use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};

use crate::common::{PathStyle, snapshotify};
use crate::compress::Compression as PayloadCompression;
use crate::error::PkgError;
use crate::pack::{PackedOutput, Stripe};

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
    let mut offset = 0_u64;
    let mut vfs: BTreeMap<String, BTreeMap<u8, PayloadPointer>> = BTreeMap::new();
    let mut path_dictionary = PathDictionary::default();

    for stripe in packed.stripes {
        let size = stripe_payload_size(&stripe, compression)?;
        let snap = snapshotify(&stripe.snap, style);
        let vfs_key = path_dictionary.make_key(compression, &snap);
        vfs.entry(vfs_key)
            .or_default()
            .insert(stripe.store.as_index(), PayloadPointer { offset, size });
        offset += size;
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

    Ok(ProducerManifest {
        entrypoint: snapshotify(&packed.entrypoint, style),
        symlinks,
        vfs,
        path_dictionary: path_dictionary.entries,
        payload_size: offset,
        compression,
    })
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

fn stripe_payload_size(stripe: &Stripe, compression: PayloadCompression) -> Result<u64, PkgError> {
    if compression == PayloadCompression::None {
        return uncompressed_stripe_size(stripe);
    }

    Ok(compress_payload(&stripe_bytes(stripe)?, compression)?.len() as u64)
}

fn uncompressed_stripe_size(stripe: &Stripe) -> Result<u64, PkgError> {
    if let Some(buffer) = stripe.buffer.as_ref() {
        return Ok(buffer.len() as u64);
    }

    let Some(file) = stripe.file.as_ref() else {
        return Err(PkgError::Pack(format!(
            "stripe '{}' has neither buffer nor file",
            stripe.snap
        )));
    };
    fs::metadata(file)
        .map(|metadata| metadata.len())
        .map_err(|source| PkgError::Io {
            path: file.clone(),
            source,
        })
}

fn stripe_bytes(stripe: &Stripe) -> Result<Vec<u8>, PkgError> {
    if let Some(buffer) = stripe.buffer.as_ref() {
        return Ok(buffer.clone());
    }

    let Some(file) = stripe.file.as_ref() else {
        return Err(PkgError::Pack(format!(
            "stripe '{}' has neither buffer nor file",
            stripe.snap
        )));
    };
    fs::read(file).map_err(|source| PkgError::Io {
        path: file.clone(),
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
