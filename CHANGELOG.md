# Changelog

## Unreleased

- Started the Rust rewrite of `pkg` under `rust-port`.
- Added typed public APIs for compression, stores, aliases, targets, package
  config, walking, packing, producing executable images, and cache-backed target
  binary fetching.
- Added pkg-fetch 3.5 cache/download support with SHA-256 verification for
  fetched binaries.
- Added producer prelude assembly using the original runtime bootstrap and a
  generated plain-JS common helper.
- Added bytecode payload fabrication via Node cached data, with target-binary
  path support when a runnable cached binary is available.
- Added native addon prebuild selection and `prebuild-install` invocation for
  missing platform/version `.node` payloads.
- Added an opt-in real runtime smoke for the original JS API happy-path demo.

### Behavior Changes

- The Rust walker bounds directory-link expansion to the requested package root
  to avoid host-machine-dependent records outside the package tree.
- `node18-macos-arm64` is rejected unless an expected pkg-fetch hash is added;
  the embedded pkg-fetch 3.5 hash table does not currently include that artifact.
- Real runtime smoke is opt-in through `PKG_RUST_REAL_CACHE` rather than running
  unconditionally in normal CI.
