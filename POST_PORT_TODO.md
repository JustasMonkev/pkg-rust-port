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
- Made the `--build` source-build boundary first-class. `PkgFetchCache` now
  exposes the exact external `built-*` artifact requirement, force-build misses
  report that requirement, and fetch parity tests prove fetched binaries are not
  used as a fallback.
- Added a reusable one-fixture public npm promotion gate that runs the selected
  pkg-fetch target-node oracle before packaging and comparing the Rust output.
- Added a manual `Gated Runtime Validation` GitHub Actions workflow for the
  source-build boundary, npm issue fixtures, public npm smoke, target-node
  probes, one-fixture public npm promotion, and native npm smoke.

## External Preconditions

- Real Node-from-source build for `--build` remains delegated to the separate
  `pkg-fetch` source-build workflow. This crate consumes the resulting built
  cache artifact and refuses to substitute a fetched binary for a force-build
  target.
- Native addon (`prebuild-install`) and public/native npm fixture validation is
  complete as opt-in gates because it requires npm installs, network access,
  native build tooling, and a real pkg-fetch cache.
- Warm-cache release build timing needs a real fetched/built binary cache and is
  exercised through the gated real-runtime smoke rather than a micro-benchmark.

## JS oracle

The JS suite remains the oracle only for the gated network/runtime fixtures
above. Every offline-testable mapped fixture now has a Rust parity test.
