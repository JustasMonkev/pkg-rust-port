#![allow(missing_docs)]

use std::path::{Path, PathBuf};

use criterion::{Criterion, criterion_group, criterion_main};
use pkg_rust::{
    Compression, Marker, PackageJson, PackedOutput, PathStyle, PkgError, WalkerParams, pack,
    produce_manifest, refine_walked, walk,
};
use std::hint::black_box;

fn empty_marker() -> Result<Marker, PkgError> {
    let package = PackageJson::parse("{}")
        .map_err(|error| PkgError::Resolve(format!("benchmark package parse failed: {error}")))?;
    Ok(Marker::new(package))
}

fn packed_fixture(fixture_dir: &Path, entrypoint: &Path) -> Result<PackedOutput, PkgError> {
    let walked = walk(
        empty_marker()?,
        entrypoint,
        None,
        WalkerParams::new().with_root(fixture_dir),
    )?;
    let refined = refine_walked(walked, entrypoint, PathStyle::Posix);
    pack(refined, true)
}

fn require<T>(result: Result<T, PkgError>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{context}: {error}");
            std::process::exit(1);
        }
    }
}

fn packaging_benchmarks(criterion: &mut Criterion) {
    let fixture_dir = PathBuf::from("../test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");
    let packed = require(
        packed_fixture(&fixture_dir, &entrypoint),
        "failed to build benchmark fixture",
    );

    criterion.bench_function("walk_refine_pack_require_resolve_fixture", |bencher| {
        bencher.iter(|| {
            let result = packed_fixture(black_box(&fixture_dir), black_box(&entrypoint));
            black_box(require(result, "failed to run walk/refine/pack benchmark"))
        });
    });

    criterion.bench_function("produce_manifest_gzip_require_resolve_fixture", |bencher| {
        bencher.iter(|| {
            let result = produce_manifest(
                black_box(packed.clone()),
                black_box(Compression::Gzip),
                black_box(PathStyle::Posix),
            );
            black_box(require(result, "failed to run producer manifest benchmark"))
        });
    });
}

criterion_group!(benches, packaging_benchmarks);
criterion_main!(benches);
