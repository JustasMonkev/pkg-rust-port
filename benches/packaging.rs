#![allow(missing_docs)]

use std::path::{Path, PathBuf};

use criterion::{Criterion, criterion_group, criterion_main};
use pkg_rust::{
    Compression, Marker, PackageJson, PackedOutput, PathStyle, PkgError, WalkerParams, pack,
    prelude_template, produce_manifest, refine_walked, render_prelude, walk,
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

/// Pack with bytecode disabled and the top-level source disclosed, so the
/// stripes are content (not blob). The producer then never spawns `node`, which
/// isolates the Rust-side assembly + compression cost.
fn packed_content_fixture(fixture_dir: &Path, entrypoint: &Path) -> Result<PackedOutput, PkgError> {
    let walked = walk(
        empty_marker()?,
        entrypoint,
        None,
        WalkerParams::new()
            .with_root(fixture_dir)
            .with_public_toplevel(true),
    )?;
    let refined = refine_walked(walked, entrypoint, PathStyle::Posix);
    pack(refined, false)
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
    let fixture_dir = PathBuf::from("test/test-50-require-resolve");
    let entrypoint = fixture_dir.join("test-x-index.js");

    criterion.bench_function("walk_refine_pack_require_resolve_fixture", |bencher| {
        bencher.iter(|| {
            let result = packed_fixture(black_box(&fixture_dir), black_box(&entrypoint));
            black_box(require(result, "failed to run walk/refine/pack benchmark"))
        });
    });

    // Fabrication-free payload assembly: bytecode disabled means content stripes
    // only, so the producer never spawns `node`. Producing a manifest from
    // bytecode stripes fails closed without an external fabricator, so these
    // content benches isolate the Rust-side assembly + compression cost across
    // every compression algorithm.
    let packed_content = require(
        packed_content_fixture(&fixture_dir, &entrypoint),
        "failed to build content-only benchmark fixture",
    );
    for (label, compression) in [
        ("none", Compression::None),
        ("gzip", Compression::Gzip),
        ("brotli", Compression::Brotli),
    ] {
        let name = format!("produce_manifest_{label}_content_only_require_resolve_fixture");
        criterion.bench_function(&name, |bencher| {
            bencher.iter_batched(
                || packed_content.clone(),
                |input| {
                    let result = produce_manifest(
                        black_box(input),
                        black_box(compression),
                        black_box(PathStyle::Posix),
                    );
                    black_box(require(
                        result,
                        "failed to run content-only manifest benchmark",
                    ))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    // Render the runtime prelude from a real manifest -- this is on the hot path
    // of every executable image write.
    let manifest = require(
        produce_manifest(packed_content.clone(), Compression::Gzip, PathStyle::Posix),
        "failed to build benchmark manifest",
    );
    let template = prelude_template(false);
    criterion.bench_function("render_prelude_require_resolve_fixture", |bencher| {
        bencher.iter(|| {
            let result = render_prelude(black_box(&template), black_box(&manifest));
            black_box(require(result, "failed to run prelude render benchmark"))
        });
    });

    criterion.bench_function("prelude_template_release", |bencher| {
        bencher.iter(|| black_box(prelude_template(black_box(false))));
    });

    criterion.bench_function("prelude_template_debug", |bencher| {
        bencher.iter(|| black_box(prelude_template(black_box(true))));
    });
}

criterion_group!(benches, packaging_benchmarks);
criterion_main!(benches);
