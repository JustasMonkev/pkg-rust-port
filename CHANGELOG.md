# Changelog

## Unreleased

- Added SEA (Single Executable Application) support — simple mode plus the
  shared foundation. `--sea` (and the `sea` config key) build a Node single
  executable by downloading an official `nodejs.org/dist` binary
  (checksum-verified, extracted, and cached under `~/.pkg-cache/sea`),
  generating a SEA prep blob via the host `node --experimental-sea-config`,
  and natively injecting the `NODE_SEA_BLOB` resource + flipping the SEA fuse.
  Cross-host builds download a host-platform Node pinned to the exact target
  version to generate the blob (avoiding SEA header version skew). Native ELF
  injection is implemented and verified end to end against the real Node 22
  runtime; macOS (Mach-O) and Windows (PE) injection, and enhanced SEA mode
  (package.json projects: walker + per-file archive + VFS bootstrap), are not
  implemented yet — unsupported targets are rejected up front (before any
  download) with a precise error. `--sea` is now documented in the help output.
- Fixed two ESM transform bugs: interop helpers (`_interop_require_default`
  and friends) are now injected inline into transformed CommonJS output
  (previously any ESM default/namespace import crashed packaged binaries
  with `ReferenceError: _interop_require_default is not defined`), and bare
  imports that runtime `require()` cannot load — ESM-only packages without
  a `require` exports condition, and packages whose require-reachable
  exports target is an `.mjs` file that the packer renames — are rewritten
  to relative paths pointing at the packaged file. Both rewrites are
  behavior fixes over yao-pkg, which crashes the same way as of 6.20.0.
- Retargeted the port from `vercel/pkg` 5.8.1 to the maintained fork
  `yao-pkg/pkg` 6.19.0 (see `YAO_PKG_PARITY.md` for the gap backlog).
- Replaced the embedded runtime prelude with the yao-pkg 6.19.0 split prelude
  (`bootstrap.js` + `bootstrap-shared.js`), including the `REQUIRE_SHARED`
  wrapper parameter and the inline debug diagnostic that calls
  `REQUIRE_SHARED.installDiagnostic`. Version reporting and
  `process.versions.pkg` are now `6.19.0`.
- Added ESM support: ESM modules transform to CommonJS through SWC before
  bytecode compilation, with async-IIFE wrapping for top-level await,
  `import.meta` rewriting, `.mjs` require-path rewriting, and packer
  renaming of transformed `.mjs` snapshots to `.js`.
- Added exports-field-aware module resolution for ESM packages, literal
  dynamic `import()` detection, `.mjs` resolve extensions, and top-level
  config `ignore` patterns.
- Added external config file support: `-c/--config` with JSON and JS config
  modules, `.pkgrc`/`pkg.config.*` auto-discovery, bare-config wrapping, and
  CLI > config > default resolution for build-shaping flags.
- Added `--fallback-to-source` and the `--signature` positive flag, plus the
  hidden positive/negative pairs for config-overridable boolean flags.
- Retargeted binary fetching to `@yao-pkg/pkg-fetch` 3.6.3 (cache tag `v3.6`,
  node 16.20.2/18.20.8/20.20.2/22.22.3/24.15.0/26.2.0, new expected-SHA
  table, new arch tokens).
- Added Zstd payload compression (`--compress Zstd|zs|zstd`, enum index 3)
  with native Rust encoding at libzstd default level. Produced binaries
  require target Node >= 22.15 to decompress (enforced by the runtime
  prelude). Updated the invalid-algorithm error to the yao-pkg wording.
- Started the Rust rewrite of `pkg` under `rust-port`.
- Added typed public APIs for compression, stores, aliases, targets, package
  config, walking, packing, producing executable images, and cache-backed target
  binary fetching.
- Added pkg-fetch 3.5 cache/download support with SHA-256 verification for
  fetched binaries.
- Added producer prelude assembly using the original runtime bootstrap and a
  generated plain-JS common helper.
- Added bytecode payload fabrication via Node cached data. Bytecode is produced
  by a host-platform fabricator binary that matches the output target's node
  range and architecture (pkg's `fabricatorForTarget`), so cross-platform
  builds no longer depend on running the output target binary or host `node`.
- Embedded the pkg 5.8.1 runtime prelude as Rust string constants
  (`src/prelude_assets.rs`) and vendored the referenced `test/` fixtures so the
  crate is fully self-contained, has no `.js` source files, and builds without
  the original JS repository alongside it.
- Reported the mirrored pkg version (5.8.1) for `-v`/`--version` and the
  `pkg@5.8.1` startup banner, and injected it as `process.versions.pkg` in the
  runtime prelude, matching the JS package.
- Ported test-77 dictionary/fixture consistency, test-78 version reporting, and
  a test-42 fetch-naming matrix as offline parity tests.
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
  fixtures, plus `any-promise`, `cookie`, `hoek`, `semver`, `verror`,
  `uglify-js`, pinned `uglify-js@2.7.5`,
  `browserify`, the `logform` formatter loader, `body-parser`, pinned
  `body-parser@1.10.2`, `express` with `jade`, and pinned `log4js@0.5.8` /
  `log4js@0.6.34` / `log4js@1.1.1`, `negotiator`, and pinned
  `negotiator@0.4.9`, plus `machinepack-urls`, pinned
  `machinepack-urls@5.0.0`, `shelljs`, pinned `shelljs@0.7.6` /
  `shelljs@0.6.0` / `shelljs@0.1.4`, `graceful-fs`, pinned
  `graceful-fs@3.0.8`, `buffermaker`, `bytes`, pinned `bson@0.2.22` /
  `bson@0.4.0`, `compressjs`, `errors`, `geoip-lite`, `later`, `lodash`,
  `nconf`, `node-forge`, `node-xlsx`, `node-zookeeper-client`,
  `npm-registry-client`, `mongodb`, `mongodb-core`, `oauth2orize`, `pg`, `pg-cursor`,
  `pg-query-stream`, `pgpass`, `pg-types`, pinned `pg-types@1.0.0`,
  `publicsuffixlist`, and `steam-crypto`, plus `throng`, `tinify`, and
  `tiny-worker`.
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
