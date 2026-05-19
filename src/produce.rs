use std::collections::BTreeMap;
use std::fs;

use crate::common::{PathStyle, snapshotify};
use crate::compress::Compression;
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
    /// Total uncompressed payload size.
    pub payload_size: u64,
    /// Compression mode for payload entries.
    pub compression: Compression,
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
    compression: Compression,
    style: PathStyle,
) -> Result<ProducerManifest, PkgError> {
    if compression != Compression::None {
        return Err(PkgError::NotImplemented(
            "compressed producer payloads are not ported yet",
        ));
    }

    let mut offset = 0_u64;
    let mut vfs: BTreeMap<String, BTreeMap<u8, PayloadPointer>> = BTreeMap::new();

    for stripe in packed.stripes {
        let size = stripe_size(&stripe)?;
        let snap = snapshotify(&stripe.snap, style);
        vfs.entry(snap)
            .or_default()
            .insert(stripe.store.as_index(), PayloadPointer { offset, size });
        offset += size;
    }

    let symlinks = packed
        .symlinks
        .into_iter()
        .map(|(link, real)| (snapshotify(&link, style), snapshotify(&real, style)))
        .collect();

    Ok(ProducerManifest {
        entrypoint: snapshotify(&packed.entrypoint, style),
        symlinks,
        vfs,
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
        ("%DICT%", "{}".to_owned()),
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

fn stripe_size(stripe: &Stripe) -> Result<u64, PkgError> {
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
