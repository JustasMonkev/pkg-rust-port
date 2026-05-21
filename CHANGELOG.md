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
- Added macOS Mach-O payload patching, ad-hoc signing through `codesign`/`ldid`,
  and `--no-signature` planning.
- Matched compression CLI output and invalid-algorithm wording, with gated
  runtime smoke coverage for None/GZip/Brotli payloads.
- Added gated real-runtime smoke coverage for mountpoint and mountpoint
  regression fixtures.
- Added recursive `**` package-config glob expansion and gated real-runtime
  smoke coverage for snapshot-copy and `withFileTypes` issue regressions.
- Added gated real-runtime smoke coverage for packaged `fs.stat`/`fs.lstat`
  issue regression behavior.
- Added gated real-runtime smoke coverage for worker-thread child script
  packaging through both package and file inputs.
- Added a Linux real-runtime smoke CI job using `PKG_RUST_REAL_TARGET` and a
  cached pkg-fetch binary directory.
- Added opt-in npm-install real-runtime smoke coverage for the Express/Pug
  issue `#1192` fixture across None/GZip/Brotli package modes.
- Added an opt-in real runtime smoke for the original JS API happy-path demo.
- Added typed Rust dictionary metadata for a batch of simple `pkg.scripts` and
  `pkg.assets` dictionary modules such as `body-parser`, `browserify`, `eslint`,
  `mongodb`, `svgo`, `uglify-js`, and `winston`.
- Added typed Rust dictionary patch metadata for patch-only modules such as
  `bunyan`, `cross-env`, `express-load`, `graceful-fs`, `mongodb-core`,
  `socket.io`, `v8flags`, and `xlsx`.
- Added typed Rust dictionary metadata for mixed script/asset plus patch modules
  `exceljs`, `sails`, `steam-resources`, and `umd`.
- Completed typed Rust metadata coverage for the remaining behavior-bearing
  simple dictionary modules, including `aws-sdk`, `connect`, `grpc`, `pg`,
  `socket.io-client`, `tesseract.js`, and `webdriverio`.
- Added walker-level parity coverage proving dictionary-provided script and
  asset globs affect packaged records and still obey `--no-dict`.
- Added walker-level parity coverage proving dictionary-provided patches are
  registered and applied before dependency blobs are parsed.
- Added Windows branch parity for the original common path helpers and fixed
  Windows snapshot-boundary handling for `inside_snapshot`/`strip_snapshot`.
- Added host-gated Windows real-runtime smoke hooks for issue regressions
  `#1207` and `#1861`, including `subst` drive coverage.
- Added a dedicated Windows CI smoke job that runs the Windows issue-regression
  runtime hook with a cached `node18-win-x64` pkg-fetch binary.
- Added an opt-in native npm issue smoke gate for `#1135` (`canvas`) and
  `#1191` (`better-sqlite3`) that requires native install and Node oracle
  success before packaging.
- Added opt-in public `test-79-npm` smoke coverage for dictionary-driven
  `connect`, `connect@2.3.9`, `rc`, `socket.io-client@1.7.0`, and `moment`
  fixtures, plus `any-promise`, `uglify-js`, pinned `uglify-js@2.7.5`,
  `browserify`, the `logform` formatter loader, `body-parser`, pinned
  `body-parser@1.10.2`, `express` with `jade`, and pinned `log4js@0.5.8` /
  `log4js@0.6.34` / `log4js@1.1.1`, `negotiator`, and pinned
  `negotiator@0.4.9`, plus `machinepack-urls`, pinned
  `machinepack-urls@5.0.0`, `shelljs`, pinned `shelljs@0.7.6` /
  `shelljs@0.6.0` / `shelljs@0.1.4`, `graceful-fs`, pinned
  `graceful-fs@3.0.8`, `buffermaker`, pinned `bson@0.2.22` /
  `bson@0.4.0`, `compressjs`, `errors`, `geoip-lite`, `later`, `nconf`,
  `node-forge`, `node-zookeeper-client`, `npm-registry-client`, `mongodb`,
  `mongodb-core`, `oauth2orize`, `pg`, `pg-cursor`, `pg-query-stream`,
  `pgpass`, `pg-types`, pinned `pg-types@1.0.0`, `publicsuffixlist`, and
  `steam-crypto`, plus `tinify` and `tiny-worker`.
- Matched JS walker handling for missing literal requires inside dependency
  packages: they now emit debug diagnostics instead of aborting packaging.
- Treated `*.config.json` CLI inputs as package-style configuration inputs,
  including `bin` entrypoint resolution and config-file inclusion.
- Treated empty package `main` values as absent during module resolution so
  type-only dependency packages fall through to normal `index.*` lookup.

### Behavior Changes

- The Rust walker bounds directory-link expansion to the requested package root
  to avoid host-machine-dependent records outside the package tree.
- Package-json inputs now use the package directory, rather than a `bin`
  subdirectory, as the walk root so sibling `node_modules` dependencies are
  included when `bin` points below `src/`.
- `node18-macos-arm64` is rejected unless an expected pkg-fetch hash is added;
  the embedded pkg-fetch 3.5 hash table does not currently include that artifact.
- Real runtime smoke is opt-in through `PKG_RUST_REAL_CACHE` rather than running
  in the main Rust matrix, but CI now runs a dedicated Linux smoke job.
