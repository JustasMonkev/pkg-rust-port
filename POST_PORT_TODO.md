# Post-Port TODO

Status of the items parked during the port.

## Done

- Bytecode fabrication now uses a host-platform fabricator binary that matches
  the output target's node range and arch (pkg's `fabricatorForTarget`), instead
  of the interim host-`node` fallback. The host-`node` path remains only as a
  seam for the deterministic in-memory test providers.
- Added a real macOS signing smoke (`signs_real_macho_with_codesign`) that
  ad-hoc signs an actual Mach-O with the installed `codesign` and verifies it.
  It is gated to macOS and skips when `codesign` is unavailable.
- Expanded the Criterion benchmarks beyond the initial scaffold: payload
  production across None/GZip/Brotli, prelude rendering from a real manifest,
  and debug/release prelude assembly.
- Added the remaining mapped JS-suite parity tests that are testable offline:
  test-77 (dictionary/fixture consistency), test-78 (version reporting), and a
  test-42 fetch-naming matrix.

## Remaining (environment-bound)

- Real Node-from-source build for `--build` lives in the separate `pkg-fetch`
  package, which is explicitly out of the pkg port's scope. The port faithfully
  passes `forceBuild` through to the binary provider and requires a built cache
  artifact for those targets; wiring an actual source build (full toolchain,
  long compile) is the only way to exercise it end to end.
- Native addon (`prebuild-install`) and public/native npm fixtures stay behind
  the opt-in `PKG_RUST_INSTALL_NPM_FIXTURES` / `PKG_RUST_NATIVE_NPM_FIXTURES`
  gates because they require npm installs, network access, and a real cache.
- Warm-cache release build timing needs a real fetched/built binary cache and is
  exercised through the gated real-runtime smoke rather than a micro-benchmark.

## JS oracle

The JS suite remains the oracle only for the gated network/runtime fixtures
above. Every offline-testable mapped fixture now has a Rust parity test.
