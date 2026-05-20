# Status

## 2026-05-19 - Map started

Shipped: created the Rust port workspace directory as documentation-only scaffolding and began mapping the JS codebase into Rust modules.

Next: finish `MIGRATION.md`, commit the map, then wire the Cargo skeleton and CI before implementation code.

Decisions made: keep the first commit free of Rust logic to satisfy the required "map before implementation" workflow.

Blockers worked around: `./rust-port` did not exist, so it was created as the target repo directory.

## 2026-05-19 - Map shipped

Shipped: wrote `MIGRATION.md` with the JS module map, dependency-to-crate map, public export signature map, test-suite parity map, and initial implementation slices.

Next: commit this documentation-only map, then create the Cargo crate and Rust CI skeleton.

Decisions made: start as a single crate, convert dictionary JS modules into typed static data, keep Node bytecode fabrication process-based, and implement Node resolution in Rust instead of depending on JS at runtime.

Blockers worked around: none.

## 2026-05-19 - Skeleton shipped

Shipped: created the Cargo binary/library package, pinned MSRV to Rust 1.85.0, added typed public skeleton seams, compileable doc examples, parity seed tests, and Rust CI for check/clippy/fmt/test/doctest/doc.

Next: port the first leaf module slice: `common`, `compress`, and target parsing with fixture-backed parity tests.

Decisions made: keep the package as one crate named `pkg-rust` with a `pkg` binary, deny `unwrap`/`expect` through clippy lints, forbid unsafe code, and use `missing_docs` as a hard lint so public API docs stay mandatory.

Blockers worked around: the sandbox could not resolve `index.crates.io`, so Cargo dependency resolution was rerun with approved network access. `missing_docs` applies to integration tests and binary crates under `--all-targets`, so the test crate now explicitly allows that lint and `src/main.rs` has crate-level docs.

## 2026-05-19 - First leaf slice shipped

Shipped: ported the first pure helpers: compression aliases, store/alias indices, POSIX path normalization/snapshot helpers from `test-48-common`, and target parsing/output suffix rules from the `test-46-*target*` fixtures.

Next: add configuration/package-json parsing and begin dictionary conversion as typed data.

Decisions made: path helpers operate on explicit `PathStyle` strings rather than host `PathBuf` so Rust tests can model target-platform paths independently of the machine running the test.

Blockers worked around: doctest examples initially used an off-by-one denominator literal; parity tests showed the JS-equivalent denominator is `12`, and the docs were corrected.

## 2026-05-19 - Config slice shipped

Shipped: added typed `package.json` parsing for `name`, scoped package basename handling, string/object `bin`, `pkg.outputPath`, and `pkg.targets`, with parity tests using the JS package-json fixtures.

Next: start dictionary conversion and the dependency/config activation behavior that depends on it.

Decisions made: enabled `serde_json`'s `preserve_order` feature because JS object-form `bin` fallback uses the first key in JSON order.

Blockers worked around: enabling order preservation added new registry packages that the sandbox could not unpack into `~/.cargo`, so Cargo tests were rerun with approved access.

## 2026-05-19 - Dictionary activation slice shipped

Shipped: added typed dictionary entries, lookup, package activation merge semantics, active dependency filtering, and parity tests for `sequelize`, `publicsuffixlist`, `express`, and the `opn` to `open` alias.

Next: port JavaScript source detection (`detector.ts`) against `test-50-ast-parsing*` and require/import fixtures.

Decisions made: dictionary entries are Rust data and do not execute JS. A disabled dictionary dependency is represented as JSON `null` because JavaScript used `undefined` and the downstream traversal only checks truthiness.

Blockers worked around: none.

## 2026-05-19 - Detector slice shipped

Shipped: added SWC-based JavaScript parsing, typed source detections for static `require`, `require.resolve`, `import`, `path.join(__dirname, ...)`, non-literal requires, malformed requires, and ambiguous `path.resolve(...)`, with parity tests for `test-50-ast-parsing` and `test-50-ast-parsing-2`.

Next: port Node-compatible module resolution and begin the deterministic walker queue that consumes detector derivatives.

Decisions made: use SWC crates pinned in `Cargo.lock` per the migration map. Keep debug-line helpers because the JS test suite directly checks `visitorSuccessful(node, true)` and dynamic-require diagnostic reconstruction.

Blockers worked around: the sandbox could not resolve `index.crates.io` for the new SWC crates, so Cargo check was rerun with approved network access. Detector traversal initially missed object-literal function expressions; fixture parity exposed that and traversal now descends into object values and methods.

## 2026-05-19 - Resolver slice shipped

Shipped: added initial sync Node-style module resolution for relative/absolute path requests, exact file hits, `.js`/`.json`/`.node` extension fallback, directory `package.json` main resolution, directory index fallback, and ancestor `node_modules` lookup. Added parity tests for `test-50-require-resolve` and `test-50-package-json-6c`.

Next: build the deterministic walker queue that combines config activation, detector derivatives, and resolver results into file records.

Decisions made: keep the first resolver API synchronous because library APIs default sync unless the workload is I/O-bound across network/process boundaries. The walker can call this directly and async orchestration can wrap it later if needed.

Blockers worked around: none.

## 2026-05-19 - Walker queue slice shipped

Shipped: added the initial deterministic Rust walker with FIFO task processing, typed marker/output/record/stat APIs, content/blob/stat/link stores, detector derivative traversal, Node built-in skipping, resolver-backed dependency expansion, and fixture parity tests for `test-50-require-resolve`.

Next: expand walker activation for package config scripts/assets/files, dictionary package markers, patches, symlink tracking, and broader package-json/module-parent parity fixtures.

Decisions made: kept `walk` synchronous for the library API and bounded directory-link expansion to the entry tree by default so records do not depend on machine-local parent directories outside the package under test.

Blockers worked around: doctest examples initially used `PackageJson::parse("{}")?` in functions returning `PkgError`; examples now map parse errors explicitly so `cargo test --doc` compiles.

## 2026-05-19 - Walker config activation slice shipped

Shipped: activated package config `pkg.scripts` as blob entries and `pkg.assets` as content entries, with deterministic expansion for the current JS config fixture. Added parity coverage for `test-50-require-with-config`.

Next: add package `files` semantics, dictionary-provided config markers, patches, symlink tracking, and full glob parity for nested/negated patterns.

Decisions made: implemented a small deterministic `*` matcher instead of adding a crate in this slice because the covered JS fixture only needs basename globs; broader `globby` parity remains a separate walker/config slice.

Blockers worked around: none.

## 2026-05-19 - Package files activation slice shipped

Shipped: added package `files` activation for explicit files, directories, and slash-prefixed entries, with top-level JavaScript files stored as blobs and non-JavaScript files stored as content. Added parity coverage for `test-50-package-json-8` and `test-50-package-json-8b`.

Next: create package markers for resolved dependency `package.json` files so dependency-local config and dictionary activation can be tested, then add patch handling.

Decisions made: slash-prefixed package `files` entries are treated like Node `path.join(base, entry)` rather than host absolute paths, matching the JS fixture behavior.

Blockers worked around: none.

## 2026-05-19 - Dependency marker slice shipped

Shipped: added dependency package marker creation for resolved `node_modules` packages, package-local activation de-duplication, and parity coverage for dependency `files` and `pkg.scripts`/`pkg.assets` fixtures in `test-50-package-json-9` and `test-50-package-json-9p`.

Next: add patch registration/application and symlink tracking before moving toward record refinement.

Decisions made: new package markers are only created for package.json files under `node_modules`; otherwise local fixture files could incorrectly activate this repo root's `package.json` instead of behaving like plain relative project files.

Blockers worked around: focused walker parity exposed the repo-root package leakage, and the marker boundary now follows the Node dependency package path.

## 2026-05-19 - Patch application slice shipped

Shipped: registered `pkg.patches` during marker activation and applied string/object patch operations before blob detection or content storage. Added parity coverage for `test-50-package-json-3`.

Next: add symlink tracking and start a typed refiner/VFS output layer over walker records.

Decisions made: patch operations are represented as a private enum instead of raw JSON arrays so the walker applies explicit replace/erase/prepend/append behavior without stringly typed control flow at use sites.

Blockers worked around: none.

## 2026-05-19 - Refiner slice shipped

Shipped: added a typed refiner that purges redundant top directory chains, computes the common path denominator, rewrites records and entrypoint to snapshot-relative path strings, and carries symlink maps through the same transform. Added parity tests for walked records, symlink denomination, and top-directory purging.

Next: add walker symlink discovery/tracking and then begin a typed packer stripe layer over refined records.

Decisions made: `refine` takes an explicit `PathStyle` so denominator/substitution behavior stays target-platform aware instead of depending on the host OS.

Blockers worked around: the first tests passed relative paths into `refine`, while the JS refiner receives normalized absolute paths from the walker. The Rust API now canonicalizes entrypoint and symlink paths when possible, and tests use existing fixture files for symlink denomination.

## 2026-05-19 - Packer stripe slice shipped

Shipped: added a typed packer pass that converts refined records into ordered blob/content/link/stat stripes, serializes links and stat metadata, preserves file-vs-buffer payload shape, and enforces the JS `--no-bytecode` failure boundary when a blob has no source content. Added parity tests over refined `test-50-require-resolve` records.

Next: add walker symlink discovery/tracking and then wire prelude/producer scaffolding around the stripe output.

Decisions made: this slice stops at stripe generation and does not interpolate the JavaScript prelude yet, keeping executable production separate from record packing semantics.

Blockers worked around: none.

## 2026-05-19 - Walker symlink tracking slice shipped

Shipped: added symlink discovery to walker enqueueing for blob/content/link tasks, exposed discovered symlinks on `WalkOutput`, and added a Unix parity test that walks a symlinked entrypoint and records the real file as the blob target.

Next: wire the walker symlink map directly into the refiner/packer pipeline and begin prelude/producer scaffolding.

Decisions made: `walk` now preserves the raw entrypoint path until append-time so symlinked entrypoints are observable; normal non-symlink paths are still canonicalized when queued.

Blockers worked around: the first symlink test showed entrypoint canonicalization erased the link before traversal, so canonicalization was moved to the append path where symlink metadata is still available.

## 2026-05-19 - Symlink pipeline slice shipped

Shipped: added `refine_walked` so the walker-collected symlink map flows directly into refinement, retained refined symlinks in `PackedOutput`, and added a Unix packer parity test proving a symlinked entrypoint survives through walk, refine, and pack.

Next: start producer/prelude scaffolding that consumes packed stripes, entrypoint, and symlinks.

Decisions made: symlink link paths now canonicalize only their parent directory during refinement; canonicalizing the full link path follows the symlink and collapses it into the real target.

Blockers worked around: the first pipeline test exposed that full-path canonicalization erased `/link.js` from the refined symlink map. The refiner now preserves the link basename while still normalizing the containing directory.

## 2026-05-19 - Producer manifest slice shipped

Shipped: added an uncompressed producer manifest stage that consumes packed stripes, computes VFS payload pointers, snapshotifies entrypoint and symlinks, and reports total payload size. Added parity tests for VFS manifest shape and symlink snapshotification.

Next: implement compressed payload accounting/key dictionary behavior, then wire manifest data into prelude rendering and executable placeholder injection.

Decisions made: compressed producer payloads return an explicit `NotImplemented` error for now because this slice computes real byte offsets and sizes only for uncompressed stripes; guessing compressed lengths would create false producer parity.

Blockers worked around: none.

## 2026-05-19 - Prelude rendering slice shipped

Shipped: added prelude placeholder rendering for `%VIRTUAL_FILESYSTEM%`, `%DEFAULT_ENTRYPOINT%`, `%SYMLINKS%`, `%DICT%`, and `%DOCOMPRESS%` from the producer manifest. Added a producer parity test that verifies VFS pointer arrays, entrypoint JSON, dictionary placeholder, and compression enum replacement.

Next: implement compressed manifest key-dictionary behavior and then executable binary streaming/placeholder injection.

Decisions made: VFS pointers render as JavaScript-compatible `[offset, size]` arrays even though the Rust manifest keeps typed `PayloadPointer` structs internally.

Blockers worked around: none.

## 2026-05-19 - Compressed producer manifest slice shipped

Shipped: added real gzip and Brotli payload byte accounting, compressed VFS path-key dictionary generation, compressed symlink key/value mapping, and `%DICT%` rendering from the producer manifest. Added producer parity tests for gzip, Brotli, and compressed prelude dictionary output.

Next: implement executable binary streaming and placeholder injection around the manifest/prelude data.

Decisions made: use `flate2` for gzip and `brotli` for Brotli so compression is native Rust and the producer can compute actual compressed payload offsets instead of guessing.

Blockers worked around: the sandbox could not unpack the new Cargo dependencies into `~/.cargo`, so the focused producer test was rerun with approved Cargo registry access.

## 2026-05-19 - Producer placeholder slice shipped

Shipped: added binary placeholder discovery and in-buffer injection for bakery, payload position/size, and prelude position/size placeholders. Added producer parity tests for discovery, injection, padding, and missing-placeholder errors.

Next: connect packed payload/prelude bytes to an executable writer that streams the binary, payload, and rendered prelude, then injects these placeholder values.

Decisions made: placeholder injection works on a mutable byte buffer first; file-backed writing can reuse the same validation and byte replacement logic when the producer starts writing target binaries.

Blockers worked around: none.

## 2026-05-19 - Executable image slice shipped

Shipped: added an in-memory producer image writer that appends payload and rendered prelude bytes to a binary buffer, computes payload/prelude positions and sizes, and injects bakery/payload/prelude placeholders. Added producer parity tests for successful image production and missing-placeholder failure.

Next: turn the in-memory image into file-backed executable output, then integrate target binary selection/fetch and final CLI flow.

Decisions made: keep this slice buffer-based so placeholder and layout semantics are verified before adding file I/O and platform-specific executable handling.

Blockers worked around: the first public doc example omitted the bakery placeholder even though the producer requires it; the example now includes the full binary placeholder contract and passes doctests.

## 2026-05-19 - Filesystem executable-bit helper shipped

Shipped: ported the JS `chmod.plusx` leaf as `plus_x`, preserving existing Unix mode bits while OR-ing owner/group/other executable bits. Added parity tests for mode preservation and missing-file errors, and public docs with a compiling example.

Next: use `plus_x` from file-backed producer output so non-Windows artifacts get executable permissions after payload/prelude writing.

Decisions made: keep the non-Unix implementation as metadata validation plus no-op because POSIX executable bits are not available through `std::fs`, and the JS CLI only invokes this path for non-Windows targets.

Blockers worked around: the new integration test initially failed under crate-wide `missing_docs`; adding a crate-level test doc fixed the compile gate without relaxing lints.

## 2026-05-19 - File-backed producer output shipped

Shipped: added `write_executable_image`, which reuses the in-memory producer image builder and writes the resulting bytes to an output path. Added parity coverage that verifies the output file exactly matches the produced executable image.

Next: integrate target/platform decisions so the CLI can select a binary, write each requested output, and apply executable permissions for non-Windows targets.

Decisions made: keep permission changes outside `write_executable_image` because the JS producer only writes bytes; the CLI orchestration applies signing and `plusx` after production based on target platform.

Blockers worked around: none.

## 2026-05-19 - CLI planning slice shipped

Shipped: added `plan_package`, `PackagePlan`, and `PlannedOutput` so CLI arguments now resolve input/package-json entrypoints, output names, targets, compression, bytecode mode, bake options, and path style before fetch/production. `exec --version` and help/version display paths now exit successfully instead of returning the skeleton not-implemented error.

Next: connect the plan to target binary acquisition and the walk/refine/pack/write pipeline for a minimal host-target packaging flow.

Decisions made: host target planning asks `node --version` for the default Node range and falls back to `node18` when Node is unavailable; this preserves JS behavior where possible without making planning fail on machines that can still parse explicit targets.

Blockers worked around: none.

## 2026-05-19 - Provider-backed package build shipped

Shipped: added `TargetBinaryProvider` plus `build_package_with_provider`, which consumes a `PackagePlan`, walks/refines/packs the entrypoint, writes each planned executable image, injects bakery options, and applies executable bits for non-Windows targets. Added parity coverage using a deterministic stub target binary.

Next: implement the real target binary provider backed by pkg-fetch-compatible cache/download behavior, then wire `exec` through the provider-backed build path.

Decisions made: introduce a provider trait before network fetch so package orchestration can be parity-tested without depending on remote binary availability.

Blockers worked around: none.

## 2026-05-19 - pkg-fetch cache provider shipped

Shipped: added `PkgFetchCache`, `BinaryKind`, and a cache-backed `TargetBinaryProvider` implementation using pkg-fetch 3.5 cache naming (`v3.5/fetched-v18.15.0-platform-arch` and `built-v...`). Added parity tests for cache paths, fetched-before-built precedence, and missing-cache errors.

Next: add network download plus expected-hash verification, then use `PkgFetchCache::default_cache` from `exec`.

Decisions made: make the first real provider cache-only so path compatibility and local reuse are tested before layering in remote GitHub release downloads.

Blockers worked around: npm tarball inspection initially failed under sandbox DNS; reran `npm pack pkg-fetch@3.5.2` with approved network access to inspect the exact local/remote naming contract. The first doctest also failed because `TargetParseError` did not convert into `PkgError`; the example now maps that error explicitly.

## 2026-05-19 - pkg-fetch hash verification shipped

Shipped: embedded pkg-fetch 3.5 expected SHA-256 data, added streaming SHA-256 verification for fetched cache binaries, and matched JS fallback behavior by deleting a mismatched fetched binary before trying the built cache entry. Added parity tests for mismatch removal with and without a built fallback.

Next: add GitHub release download into the fetched cache path, verify the downloaded hash, then wire `exec` through `PkgFetchCache::default_cache`.

Decisions made: use RustCrypto `sha2` 0.11.0 for SHA-256 because it is pure Rust, maintained under the RustCrypto hashes repository, MIT/Apache licensed, and supports this crate's MSRV.

Blockers worked around: sandbox DNS blocked the first dependency resolution; reran the focused fetch test with approved Cargo registry access to lock and download `sha2`.

## 2026-05-19 - pkg-fetch download primitive shipped

Shipped: added `PkgFetchCache::download_fetched`, which downloads a fetched target binary from the pkg-fetch GitHub release path, writes it through a `.downloading` temp file, applies executable bits, verifies SHA-256, and renames it into the fetched cache location. Added unit coverage for successful verified storage and hash-mismatch cleanup.

Next: wire `exec` to use `PkgFetchCache::default_cache` and `build_package_with_provider`, then exercise a cached/download-backed host packaging smoke path.

Decisions made: keep the reusable byte-storage verifier separate from the HTTP request so cache write, permission, hash, and cleanup semantics remain testable without external network access.

Blockers worked around: local `TcpListener` test setup failed in the sandbox with `Operation not permitted`, so the tests were moved down to the storage/verifier layer while leaving the production HTTP download path intact.

## 2026-05-19 - CLI provider integration shipped

Shipped: `exec` now plans arguments, creates `PkgFetchCache::default_cache`, and calls `build_package_with_provider` with the default producer prelude template. `PkgFetchCache::new` remains offline/cache-only for deterministic tests, while `default_cache` enables GitHub release downloads on cache misses.

Next: replace the temporary producer prelude template with the real runtime bootstrap data and add a cached/download-backed CLI smoke test.

Decisions made: split cache construction into offline `new` and download-enabled `with_downloads`/`default_cache` so tests do not accidentally perform network I/O, but the real CLI can fetch when the cache is empty.

Blockers worked around: none.

## 2026-05-19 - Runtime prelude template slice shipped

Shipped: added `prelude_template`, which assembles the producer prelude wrapper with the original bootstrap runtime, optional diagnostic runtime, a generated common-helper body, and the producer placeholders. `exec` now uses this real prelude template instead of the temporary placeholder-only string. Added parity tests for wrapper shape and diagnostic inclusion.

Next: validate the generated common-helper behavior against runtime expectations, then add a cached/download-backed CLI smoke test using a real placeholder-bearing target binary.

Decisions made: during migration, reference the original bootstrap and diagnostic files from the parent JS repo at compile time instead of copying them into `rust-port`; this preserves runtime parity while avoiding vendoring those JS files into the Rust target tree. The common helper body is generated in Rust because the source `lib/common.ts` is TypeScript, while the runtime prelude needs plain JavaScript.

Blockers worked around: none.

## 2026-05-19 - Runtime common helper parity shipped

Shipped: corrected the generated runtime common helper used by `prelude_template` so `insideSnapshot`, `stripSnapshot`, and `removeUplevels` match the JavaScript `lib/common.ts` behavior instead of the temporary simplified versions. Added a unit test that executes the generated helper with `node` and checks snapshot display paths plus leading-uplevel removal.

Next: add a cached/download-backed CLI smoke path using a placeholder-bearing target binary, then continue closing runtime parity gaps around bytecode/fabrication and platform signing.

Decisions made: keep the generated helper as plain JavaScript inside `src/prelude.rs`, but test it by executing the generated source rather than only checking template text; this catches runtime drift while still avoiding vendored TypeScript source under `rust-port`.

Blockers worked around: none.

## 2026-05-19 - Cached CLI smoke path shipped

Shipped: added an integration smoke test that spawns the compiled `pkg` binary, points `PKG_CACHE_PATH` at a temp pkg-fetch-compatible cache containing a built placeholder target, and packages the JS require-resolve fixture through the real `exec` path. The test verifies bake-option injection, manifest rendering, placeholder replacement, and executable permissions.

Next: port bytecode/fabricator behavior or explicitly route `--no-bytecode`-style fallbacks where JS behavior requires bytecode generation; then cover the next high-value JS fixture end to end.

Decisions made: use a `built-v18.15.0-macos-arm64` placeholder cache entry instead of a fetched entry so the smoke test avoids network downloads and expected-SHA validation while still exercising the real `PkgFetchCache::default_cache` path through `PKG_CACHE_PATH`.

Blockers worked around: none.

## 2026-05-19 - Host bytecode fabrication slice shipped

Shipped: producer blob stripes now run through a Node `vm.Script(... produceCachedData: true)` fabricator before payload insertion, so `STORE_BLOB` entries contain V8 cached data instead of source bytes. Added payload-level coverage for fabricated blob data. While validating this, fixed a walker parity bug where retagged non-JS blob tasks were still marked as blob stores; JSON and other non-JS files now remain content-only like the JS walker.

Next: replace the host-`node` interim fabricator with target-binary fabrication once the provider layer carries binary paths, then cover real executable runtime smoke with an actual cached pkg-fetch binary.

Decisions made: use host `node` as the interim bytecode fabricator and leave a `// DECISION:` comment in `src/produce.rs`; this is closer to JS behavior than mislabeled source blobs, but target-specific bytecode generation still remains to be ported.

Blockers worked around: the first full test run exposed JSON and CSS files being compiled as JavaScript bytecode. Root cause was in the walker, not the fabricator: Rust retagged them as content but still marked the original blob task complete. Fixed the store-completion path and added fixture assertions so this does not regress.

## 2026-05-19 - Target-aware binary artifact slice shipped

Shipped: added `TargetBinary` so providers can return target binary bytes plus the cache path they came from. `PkgFetchCache` now preserves fetched/built paths, and package builds pass runnable target binary paths into the producer bytecode fabricator. Added coverage that proves an explicit fabricator path is used for blob payloads and that cache artifacts expose their built path.

Next: seed or download a real pkg-fetch binary and add a runtime smoke test that executes the produced package, then continue replacing host-only bytecode fallbacks and platform signing gaps.

Decisions made: keep byte-only provider implementations source-compatible through a default `binary_artifact_for` method. When a cached file is not recognizably executable, the producer falls back to host `node`; this keeps placeholder-binary tests deterministic while real ELF/Mach-O/PE/shebang target binaries use target-specific fabrication.

Blockers worked around: no `~/.pkg-cache` exists on this machine, so real-binary runtime smoke remains blocked until a binary is downloaded or seeded. The cached CLI smoke uses a placeholder file and therefore intentionally exercises the non-runnable fallback path.

## 2026-05-19 - Real API runtime smoke shipped

Shipped: fixed the async CLI boundary by moving synchronous packaging into `tokio::task::spawn_blocking`, fixed producer prelude serialization by wrapping the rendered prelude the way JS `makePreludeBufferFromPrelude` does, and added a gated real runtime smoke test for the JS API happy-path demo. With `PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache`, the Rust CLI packaged `test/test-50-api/test-x-index.js` with `node18-macos-x64`, executed the produced binary, and got `42\n`.

Next: make the real runtime smoke portable in CI by either seeding a cache artifact or adding an explicit network-enabled job, then expand runtime smoke coverage beyond the trivial API fixture.

Decisions made: keep the real runtime smoke opt-in through `PKG_RUST_REAL_CACHE` so normal CI does not download a large pkg-fetch binary or require Rosetta. The test still lives in the suite and runs the complete package-and-execute path when the cache is provided.

Blockers worked around: `node18-macos-arm64` is absent from the embedded pkg-fetch expected-hash table, so the real smoke used the supported `node18-macos-x64` binary. The first x64 attempt also exposed a Tokio/`reqwest::blocking` panic, now fixed by running the blocking package build off the async runtime.

## 2026-05-19 - Rust port docs artifacts shipped

Shipped: added `README.md`, `CHANGELOG.md`, and `POST_PORT_TODO.md` under `rust-port`. The README mirrors the original CLI shape while clearly stating current Rust-port limits and validation commands. The changelog records the rewrite and behavior changes found so far. Post-port ideas and non-parity improvements are parked outside the implementation path.

Next: keep expanding Rust parity tests against the remaining JS fixtures, and make the real runtime smoke portable enough for CI.

Decisions made: document current behavior conservatively instead of presenting the Rust port as a completed replacement. This keeps the docs useful during migration without hiding incomplete native-addon, Mach-O, and JS-oracle retirement work.

Blockers worked around: none.

## 2026-05-19 - Release profile verification shipped

Shipped: configured the Cargo release profile to strip symbols and documented release build verification in the Rust README. A cold `cargo build --release --locked` populated the release cache, then `/usr/bin/time -p cargo build --release --locked` completed in `0.13s` on a warm cache. `nm target/release/pkg` showed only undefined external symbols plus the Mach-O header, consistent with stripped Rust symbols.

Next: continue closing parity gaps in native addon handling, Mach-O signing, and broader runtime fixture coverage.

Decisions made: use Cargo's built-in `strip = "symbols"` release profile setting instead of a post-build shell step, so local builds and CI release artifacts use the same stripping behavior.

Blockers worked around: none.

## 2026-05-19 - Output path preparation parity shipped

Shipped: matched the JS output-preparation behavior before package fabrication. Rust builds now remove an existing file output, refuse to overwrite a non-file output, and create missing parent directories. Added parity tests for nested output directory creation and non-file output refusal.

Next: broaden runtime fixture parity beyond the API happy path, with require/resolve and asset fixtures as the next leaf candidates.

Decisions made: keep output preparation inside `build_package_with_provider` so all callers, including tests with custom binary providers, pass through the same preflight checks before the producer writes the executable image.

Blockers worked around: none.

## 2026-05-19 - Require.resolve runtime smoke shipped

Shipped: added a gated real-runtime smoke test for `test/test-50-require-resolve`. The test compares the packaged executable output with the Node oracle output when `PKG_RUST_REAL_CACHE` points at a seeded pkg-fetch cache, and otherwise skips cleanly in normal CI.

Next: use the same gated runtime-smoke pattern for filesystem asset fixtures, then continue into harder gaps like native addons and platform patching.

Decisions made: compute the oracle output by running `node test-x-index.js` in the fixture directory instead of hardcoding the long expected text. That keeps the Rust test pinned to the JS suite as the source of truth while still exercising the packaged binary path when a real cache is present.

Blockers worked around: the real cache is machine-local and intentionally not required by default CI, so the new test remains opt-in through `PKG_RUST_REAL_CACHE`.

## 2026-05-19 - Filesystem asset runtime parity shipped

Shipped: added a gated real-runtime smoke test for `test/test-50-fs-runtime-layer` and made it pass against the Node oracle. Fixed three parity gaps exposed by that fixture: CLI packaging now runs on a dedicated larger-stack build thread, stat payload JSON now uses the field names expected by the JS prelude, and package-directory snapshot refinement now preserves the package directory name while synthesizing the bounded `/snapshot` root record.

Next: continue runtime fixture expansion into package-json, spawn, and native-addon edges; keep the real-cache tests opt-in until CI has a seeded target binary strategy.

Decisions made: keep the walker bounded for deterministic records and handle package-directory snapshot shape in refinement instead of walking every sibling directory under the host `test/` tree. The stat struct remains idiomatic Rust and uses serde renames at the prelude boundary.

Blockers worked around: the first filesystem fixture attempt stack-overflowed in Tokio's default blocking worker; the larger-stack build thread avoids requiring `RUST_MIN_STACK` from users or CI.

## 2026-05-19 - Package.json files runtime parity shipped

Shipped: added gated real-runtime smoke coverage for `test-50-package-json-7`, `8`, `8b`, `9`, and `9p`. Fixed dependency package `files` semantics by tracking top-level vs dependency markers: top-level JavaScript `files` remain blobs, while dependency JavaScript `files` are stored as content, matching the JS suite's function source visibility expectations.

Next: continue into package-json edge fixtures that are not yet runtime-smoked, then spawn and native-addon paths.

Decisions made: represent marker role as an explicit typed boolean on `Marker` rather than inferring from path shape or `node_modules` repeatedly. The dependency marker constructor stays private so public callers keep creating top-level markers through `new`, `with_package_path`, or `from_package_path`.

Blockers worked around: `test-50-package-json-9` initially failed at runtime with `AssertionError: no "test" in main`; the failing path proved dependency `files` JavaScript was bytecode/blob instead of source/content.

## 2026-05-19 - Package main runtime parity shipped

Shipped: added gated real-runtime smoke coverage for `test-50-package-json-6c`, `7p`, and `8p`. File inputs inside an immediate package now keep that package directory under `/snapshot`, and local package directories now include their `package.json` so runtime `require('../package-dir')` can resolve `main`.

Next: continue package-json runtime coverage for the remaining edge fixtures, then move into spawn and native-addon paths.

Decisions made: only the immediate parent `package.json` influences file-input snapshot base selection, and local package marker discovery is bounded to the active walker root. That covers local package-main resolution without accidentally treating repository ancestor packages as fixture package metadata.

Blockers worked around: `test-50-package-json-6c` first packaged as `/snapshot/alpha.js`, then as `/snapshot/beta/alpha.js` without package metadata; both failed `require('../beta')`. The final shape preserves `/snapshot/beta/alpha.js` and includes `/snapshot/beta/package.json`.

## 2026-05-19 - Remaining package-json runtime parity shipped

Shipped: expanded gated real-runtime package-json coverage to `test-50-package-json`, `2`, `3`, `4`, `5`, `6`, `6b`, `6d`, and `A`. Added typed `busboy` and `log4js` dictionary entries, non-fatal dependency-derived resolution for metadata-only packages, and node_modules-aware snapshot-base refinement with synthesized intermediate directory records.

Next: move into spawn and native-addon runtime paths, then broader invalid/config/error fixtures.

Decisions made: dictionary additions remain inert Rust data instead of executing JS dictionary modules. Dependency aliases from `package.json` remain warning-equivalent when their runtime entrypoint is missing, matching JS behavior for `@types/*`. Direct file inputs under `node_modules` keep `node_modules` in `/snapshot` so bare self-subpath requires resolve through the prelude.

Blockers worked around: `test-50-package-json-4` first missed dictionary script globs, `test-50-package-json-5` failed on metadata-only `@types/omega`, and `test-50-package-json-6b` included `beta.js` but lacked the `/snapshot/node_modules` directory link needed for runtime module resolution.

## 2026-05-19 - Spawn runtime parity shipped

Shipped: added gated real-runtime smoke coverage for the full `test-50-spawn` non-child fixture matrix: cluster, child_process fork, exec, execFile, execSync, spawn, spawnSync, and direct node execution cases. The detector now accepts CommonJS top-level `return` and traverses assignment right-hand sides so child process `require.resolve(...)` targets are bundled.

Next: move into native-addon runtime fixtures and invalid/error-path fixtures.

Decisions made: keep spawn parity in the runtime smoke suite because the behavior lives mostly in the JS prelude and must be verified by executing the produced binary. Treat top-level `return` as valid detector input because Node wraps CommonJS modules before execution.

Blockers worked around: `test-cluster.js` initially failed SWC parsing with `ReturnNotAllowed`, and `test-cpfork-a-1.js` initially omitted `test-cpfork-a-child.js` because assignment RHS expressions were not traversed.

## 2026-05-19 - Native-addon runtime parity shipped

Shipped: added gated real-runtime smoke coverage for `test-50-native-addon`, `2`, `3`, and `4`. Fixed escaped dependency snapshot refinement so an entrypoint under `lib/` can still bundle and resolve sibling `node_modules` `.node` files.

Next: move into invalid package/config/error-path fixtures, then broader runtime fixtures not yet covered by the Rust smoke suite.

Decisions made: keep `.node` files stored as content when they are encountered as blob tasks, matching JS walker behavior. When records escape a forced snapshot base, fall back to common-denominator snapshotting and synthesize POSIX parent directory records so the runtime prelude can traverse generated paths.

Blockers worked around: `test-50-native-addon-3` initially generated a broken `e_modules/dependency/time-d.node` snapshot key by slicing a sibling `node_modules` path against the `lib/` base. After the denominator fallback, the file existed but module resolution still needed a synthetic `/snapshot/node_modules` directory link.

## 2026-05-19 - Hard invalid CLI parity shipped

Shipped: added CLI smoke parity coverage for the hard invalid fixtures: missing input, missing package.json for a directory input, missing package `bin`, missing package `bin` target file, and unknown target token. The Rust CLI now exits with code 2, writes the JS-style `> Error!` line to stdout, leaves stderr empty, and avoids ANSI escapes for those paths.

Next: cover the warning-only invalid package-json fixture where dependency package metadata has no `main`, then continue through config/error-path fixtures.

Decisions made: keep fatal CLI formatting at the binary boundary while preserving typed `PkgError` values for library callers. CLI input and package-bin metadata misses use JS-style "does not exist" wording because those messages are part of the behavioral oracle.

Blockers worked around: none.

## 2026-05-19 - Missing dependency main warning parity shipped

Shipped: added typed walker warnings and CLI stdout rendering for the warning-only `test-50-invalid-package-json-2` fixture. A direct `require` of a dependency package that has package metadata but no resolvable `main` now succeeds with a JS-style warning instead of failing the build.

Next: continue through the remaining invalid/config/error-path fixtures, then broaden runtime smoke coverage for fixtures still outside the Rust suite.

Decisions made: warnings are collected as `PackageWarning` values in the library and rendered by the CLI, keeping stdout formatting out of lower-level modules. The missing-main path queues the dependency `package.json` as content so package metadata remains visible to later walker activation.

Blockers worked around: the first implementation only treated dependency-list aliases as warning-capable; the fixture proved a direct `require('crusader')` needs the same specific non-fatal handling when the package metadata exists but lacks `main`.

## 2026-05-19 - Config log warning parity shipped

Shipped: added the `stylus` dictionary entry as typed data, including its asset glob and dictionary log callback. The walker now turns that dictionary log into a `PackageWarning`, and the CLI smoke suite covers the `test-50-config-log` stdout warning contract.

Next: continue through remaining config/error fixtures outside the current Rust parity suite, then resume broader runtime fixture coverage.

Decisions made: dictionary callbacks remain inert Rust data through `DictionaryLog` rather than executable JS. The CLI still owns stdout rendering; lower-level modules only return typed warning values.

Blockers worked around: none.

## 2026-05-20 - CLI output matrix parity locked

Shipped: expanded `cli_plan_parity` coverage for the `test-46` output-name matrix. Rust now has fixture-backed tests for default bare-file multi-target outputs, `.js` stem stripping, `--out-path` multi-target outputs, package `pkg.targets`, and package `pkg.outputPath`.

Next: continue closing remaining `test-46` planning edges, then return to unresolved config/error and runtime smoke fixtures.

Decisions made: keep these filename contracts at the planning layer because the JS fixtures assert output filenames after packaging, while the Rust planner is the authoritative source for basename, suffix, extension, and output directory decisions.

Blockers worked around: none.

## 2026-05-20 - CLI single-output planning parity locked

Shipped: added planner parity coverage for explicit `--output` host-target fallback, single-target `--out-path` output without platform suffix, scoped package directory basename normalization, and refusal to overwrite the input path.

Next: finish any remaining `test-46` planning-only edges, then continue through unresolved config/error and runtime smoke fixtures.

Decisions made: keep overwrite refusal in the planner because it is a pre-production CLI contract in the JS implementation and does not depend on target binary fetching or payload writing.

Blockers worked around: none.

## 2026-05-20 - May-exclude diagnostic parity shipped

Shipped: matched the `test-50-may-exclude-must-exclude` diagnostic contract. The detector now traverses source nodes in JS-style breadth-first order, dynamic second-argument requires fall through to malformed diagnostics, the walker records debug-vs-warning diagnostics as typed values, and the CLI renders `> Warning` / `> [debug]` lines only when appropriate.

Next: add real-runtime parity for `test-50-may-exclude`, then continue through remaining not-found/error wording and runtime smoke fixtures.

Decisions made: keep diagnostic collection in the walker and rendering in the CLI. The lower layers preserve typed diagnostics, while `--debug` controls which messages become user-visible.

Blockers worked around: the first Rust detector walked function and `try` bodies depth-first, which reordered diagnostics relative to JS. The JS detector uses a queue traversal, so Rust now mirrors that traversal model instead of sorting messages after the fact.

## 2026-05-20 - May-exclude runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-may-exclude`. When `PKG_RUST_REAL_CACHE` is configured, the test packages the fixture and compares the produced executable output with the Node oracle output.

Next: continue through remaining not-found/error wording fixtures and broader runtime smoke coverage.

Decisions made: keep this as an opt-in real-cache smoke test, matching the rest of the runtime suite, because normal CI still should not require a seeded pkg-fetch binary.

Blockers worked around: none.

## 2026-05-20 - Not-found wording runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-not-found-wording` and `test-50-not-found-wording-2`. The first fixture asserts pkg-specific filesystem and require error wording, and the second compares dynamic require/resolve failures against the Node oracle.

Next: continue expanding runtime smoke coverage across remaining `test-50` fixtures, prioritizing small error/runtime slices that exercise existing ported components.

Decisions made: keep not-found wording checks in the runtime suite because the behavior lives in the generated executable prelude rather than in CLI planning or static walking.

Blockers worked around: none.

## 2026-05-20 - Filesystem write-guard runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-fs-runtime-layer-3`, which asserts the packaged filesystem rejects writes to bundled files with the JS prelude's `Cannot write to packaged file` wording.

Next: cover the more version-sensitive `test-50-fs-runtime-layer-2` fixture with the same line-normalization the JS oracle uses, then continue broader runtime fixture expansion.

Decisions made: split layer 3 from layer 2 because layer 3 has a fixed stdout contract, while layer 2 needs Node-version-specific normalization around out-of-range errors.

Blockers worked around: none.

## 2026-05-20 - Filesystem async runtime layer smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-fs-runtime-layer-2`. The Rust smoke test compares packaged executable output with the Node oracle and applies the same out-of-range error wording normalization as the JS fixture.

Next: continue broader runtime fixture expansion across small `test-50` slices, then revisit larger compression and npm fixture groups.

Decisions made: keep the normalization local to the runtime smoke suite because it is only an oracle comparison detail; the executable output itself remains unmodified.

Blockers worked around: none.

## 2026-05-20 - Argument forwarding runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-arguments`. The runtime suite now packages the fixture and executes the produced binary with positional, short-flag, and long-flag arguments to verify `process.argv` forwarding.

Next: continue adding small real-runtime smoke fixtures that exercise already-ported prelude behavior, then return to larger groups such as compression and npm package fixtures.

Decisions made: package-and-run smoke helpers now accept runtime arguments while keeping the existing no-argument helper as the default wrapper.

Blockers worked around: none.

## 2026-05-20 - Modern JS runtime smoke fixtures locked

Shipped: added gated real-runtime smoke coverage for `test-50-class-to-string`, `test-50-object-spread`, `test-50-for-await-of`, and `test-50-non-ascii`. Each fixture now packages through the Rust CLI and compares executable stdout against the Node oracle.

Next: continue filling small `test-50` runtime smoke gaps, then move back to broader feature groups such as compression, mountpoints, and npm package fixtures.

Decisions made: group syntax/runtime-only fixtures together because they all exercise the same already-ported packaging path and do not need fixture-specific assertions.

Blockers worked around: none.

## 2026-05-20 - Path and resolution runtime smoke fixtures locked

Shipped: added gated real-runtime smoke coverage for `test-50-path-as-buffer`, `test-50-path-separators`, `test-50-module-parent`, and `test-50-resolve-and-nearby`. These fixtures now compare produced executable stdout against the Node oracle for path object handling and resolution edge cases.

Next: continue runtime-smoke expansion for remaining small fixtures, with special handling where fixtures need environment variables, stderr comparisons, or intentional failure assertions.

Decisions made: keep this slice on stdout-compatible fixtures and leave `test-50-chdir-env-var` for a helper that can set packaging environment variables.

Blockers worked around: none.

## 2026-05-20 - Deploy-file warnings shipped

Shipped: parsed `pkg.deployFiles` and emitted typed walker warnings for external files that cannot be embedded in the executable, including custom deploy target paths and file kinds.

Next: continue npm fixture parity around dictionary deploy-file metadata and eventual deploy-file copying in higher-level fixture harnesses.

Decisions made: keep deploy files as warnings at the walker boundary, matching upstream's behavior that these files must be distributed beside the executable rather than bundled into the VFS.

Blockers worked around: this slice does not copy deploy files; npm fixture harness parity remains separate from package production.

## 2026-05-20 - Force-build target planning shipped

Shipped: carried CLI `--build` into each planned target and taught the pkg-fetch cache provider to require a built cache artifact for force-build targets instead of using fetched/downloaded binaries.

Next: complete real source-build orchestration for missing built artifacts, then continue deploy-file and npm fixture parity.

Decisions made: this slice does not fake pkg-fetch source builds. Until that workflow is ported, force-build targets fail with a clear built-binary cache error when no built artifact is present.

Blockers worked around: external Node source builds remain out of scope for this increment; cache policy and planning are now parity-tested.

## 2026-05-20 - Dictionary selection parity shipped

Shipped: wired CLI `--no-dict` into package planning and walker execution, including JS-style wildcard collapse and built-in dictionary filename matching. The walker now also honors root `pkg.dictionary` overrides as typed dictionary entries.

Next: continue expanding npm/public package fixture parity, especially cases that depend on deploy files or a configured real base-binary cache.

Decisions made: preserve the JS distinction between built-in dictionary filenames such as `busboy.js` and package names such as `busboy`; custom `pkg.dictionary` entries remain root package configuration and are still loaded unless dictionaries are globally disabled with `*`.

Blockers worked around: no real executable cache is configured in this shell, so this slice is locked with CLI-plan and walker parity tests.

## 2026-05-20 - Dictionary disclosure walker slice shipped

Shipped: ported the JS walker's `hasDictionary` disclosure rule. Packages that activate a typed dictionary entry now store JavaScript blobs as content too, matching upstream's `marker.public || marker.hasDictionary` behavior.

Next: continue through remaining runtime fixture groups, especially public package and npm package fixtures that need a configured real base-binary cache for executable-level proof.

Decisions made: keep dictionary disclosure as marker state instead of treating typed dictionary entries as package licenses or CLI public flags; this preserves the original walker's distinct source-disclosure reasons.

Blockers worked around: real-runtime proof remains gated on `PKG_RUST_REAL_CACHE`, so this slice is locked with walker parity over dictionary fixture packages.

## 2026-05-20 - Public disclosure walker slice shipped

Shipped: wired CLI `--public` and `--public-packages` through package planning into the dependency walker, and taught the walker to disclose JavaScript source for public top-level packages, selected public dependency packages, wildcard public packages, and permissively licensed packages.

Next: extend real-runtime smoke coverage for `test-50-public-packages` once the local base-binary cache is configured, then continue the remaining public/dictionary disclosure edge cases.

Decisions made: keep disclosure as walker state and marker metadata, matching the JS walker boundary, while leaving dictionary `hasDictionary` source disclosure as a separate parity slice.

Blockers worked around: no real cache is configured in this shell, so this slice uses non-gated walker and CLI parity tests rather than packaging the fixture executable.

## 2026-05-20 - CHDIR entrypoint parity shipped

Shipped: preserved the JS CLI entrypoint's `CHDIR` environment override before argument execution, and added gated real-runtime smoke coverage for `test-50-chdir-env-var`.

Next: continue runtime-smoke expansion for fixtures needing stderr comparisons or intentional failure assertions, then return to broader compression and npm package fixture groups.

Decisions made: keep `CHDIR` handling in `src/main.rs`, matching the JS `lib/bin.ts` boundary, instead of moving it into the public `exec` API.

Blockers worked around: normal Rust gates still skip real executable packaging unless `PKG_RUST_REAL_CACHE` is configured, so this fixture is locked as opt-in real-runtime coverage like the rest of `runtime_smoke`.

## 2026-05-20 - Console trace runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-console-trace`. The smoke test now validates stderr stack path rewriting by checking the top traced file and the prelude frame path, matching the JS fixture's core assertion.

Next: continue fixtures that need non-stdout assertions, especially intentional runtime failures such as source-position errors.

Decisions made: keep a tiny stack-frame parser inside the runtime smoke suite because it is fixture assertion logic, not product behavior.

Blockers worked around: none.

## 2026-05-20 - Source-position failure smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-error-source-position`. The runtime suite can now package with extra CLI args, run produced executables that are expected to fail, and assert the original source error message and caret pointer in stderr.

Next: continue intentional-failure and stderr-focused runtime fixtures, then move back to broader groups such as compression, mountpoints, and public package behavior.

Decisions made: model runtime-smoke execution options with a small test-only options struct instead of adding more positional helper arguments.

Blockers worked around: none.

## 2026-05-20 - Require edge-case runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-require-edge-cases`, comparing produced executable stdout against the Node oracle for dynamic path require, false exports, and empty-file module behavior.

Next: continue expanding require/runtime fixtures, especially cases that need path normalization across different run directories.

Decisions made: keep this fixture in the stdout compare helper because its JS oracle only asserts process output equality.

Blockers worked around: no real cache is configured in this shell, so the test compiles and skips real packaging until `PKG_RUST_REAL_CACHE` is supplied.

## 2026-05-20 - Require-with-config runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-require-with-config`, packaging the fixture as a package directory so `package.json` `pkg.scripts` and `pkg.assets` participate in the runtime smoke path.

Next: cover the original JS fixture's alternate run-directory normalization for require/config cases, then continue larger package and compression groups.

Decisions made: this smoke test uses the standard Node stdout oracle for the compile-time fixture directory; the more complex run-time/run-time-3 host overlay behavior remains a separate helper slice.

Blockers worked around: no real cache is configured in this shell, so the test compiles and skips real packaging until `PKG_RUST_REAL_CACHE` is supplied.

## 2026-05-20 - Global object runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-global-object`, packaging the fixture as a directory so `package.json` `pkg.scripts` includes dynamically required files at runtime.

Next: continue package-directory runtime fixtures and then return to environment-specific cases such as inspect/signal behavior.

Decisions made: keep this as a package-directory smoke because the fixture's runtime behavior depends on package config script activation rather than direct static detection alone.

Blockers worked around: no real cache is configured in this shell, so the test compiles and skips real packaging until `PKG_RUST_REAL_CACHE` is supplied.

## 2026-05-20 - Promisify runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-promisify`, comparing produced executable stdout against the Node oracle for promisified `child_process.exec`, `fs.exists`, and `fs.read` behavior.

Next: continue package/runtime fixtures with environment-specific behavior, then return to compression and larger public package groups.

Decisions made: keep the oracle comparison at stdout level because the fixture already normalizes behavior through printed JSON and boolean results.

Blockers worked around: no real cache is configured in this shell, so the test compiles and skips real packaging until `PKG_RUST_REAL_CACHE` is supplied.

## 2026-05-20 - Inspect runtime smoke locked

Shipped: added gated real-runtime smoke coverage for `test-50-inspect`. The runtime smoke harness now supports run-time environment variables and exact exit-code assertions, and the inspect fixture checks the `--inspect`/`PKG_EXECPATH=PKG_INVOKE_NODEJS` exit-code contract.

Next: continue environment-specific runtime fixtures, including signal and debug flows, then return to compression and larger package groups.

Decisions made: represent exact process status as `RunExpectation::Code` instead of folding it into generic failure so fixtures can lock the JS suite's expected exit status.

Blockers worked around: no real cache is configured in this shell, so the test compiles and skips real packaging until `PKG_RUST_REAL_CACHE` is supplied.

## 2026-05-20 - Open dictionary deploy metadata shipped

Shipped: ported the `open`/`opn` dictionary entry from an empty alias into typed Rust data carrying the JS `xdg-open` patch and `deployFiles` metadata. Added dictionary parity coverage and walker coverage proving dictionary-sourced deploy files emit the same external-distribution warning path as package config deploy files.

Next: continue npm fixture parity for the remaining deploy-file dictionaries, especially directory-style entries such as `leveldown`, `puppeteer`, and `zeromq`, then wire any needed fixture-harness copying behavior.

Decisions made: keep dictionary deploy metadata inside `PkgConfig` so the existing activation path registers patches and warnings uniformly for package config, built-in dictionaries, and root `pkg.dictionary` overrides.

Blockers worked around: this slice still does not copy deploy files beside produced executables; it locks the metadata and warning boundary first.

## 2026-05-20 - Directory deploy dictionaries shipped

Shipped: ported the `leveldown`, `puppeteer`, and `zeromq` dictionaries as typed Rust entries carrying their JS path patches and directory-style `deployFiles` metadata. Added dictionary parity coverage for all three entries and walker coverage proving the deploy warning preserves the `directory` file kind.

Next: continue deploy-file parity for the remaining npm fixture dictionaries with executable/file payloads, then add the higher-level copy behavior that places deploy files beside produced executables.

Decisions made: keep the third `deployFiles` tuple item as the warning file kind rather than normalizing directory entries into generic files, matching the JS fixture harness and upstream warning text.

Blockers worked around: real npm fixture execution remains gated on a configured base-binary cache; this slice proves the metadata and warning path without running the npm fixtures end to end.

## 2026-05-20 - File deploy dictionaries shipped

Shipped: ported `exiftool.exe`, `exiftool.pl`, `google-closure-compiler`, and `google-closure-compiler-java` into typed dictionary data with their JS path patches and file deploy tuples. Added dictionary parity coverage that asserts each package activates the expected patch target and external file mapping.

Next: continue the remaining deploy-file dictionaries with larger platform-specific lists, especially `electron`, `phantom`, `phantomjs-prebuilt`, `node-notifier`, `sharp`, `drivelist`, and `nightmare`, then add producer or fixture-harness deploy-file copying.

Decisions made: keep these file mappings in `PkgConfig.deploy_files` instead of special-casing archive or binary-like names; the walker already treats absent third tuple items as `file`, matching the JS dictionary shape.

Blockers worked around: this slice does not prove executable-level npm fixture behavior because deploy-file copying beside produced outputs remains a later increment.

## 2026-05-20 - Remaining deploy dictionaries shipped

Shipped: ported the remaining deploy-file dictionaries as typed data: `drivelist`, `electron`, `nightmare`, `node-notifier`, `phantom`, `phantomjs-prebuilt`, and `sharp`. The parity test now asserts each entry's deploy-file list plus representative patch/script metadata, so every JS dictionary containing `deployFiles` has a Rust `lookup_dictionary` entry.

Next: add the higher-level package output behavior that copies deploy files beside produced executables for npm fixture parity, then revisit real npm fixture smoke coverage once a base-binary cache is configured.

Decisions made: `sharp` keeps its dictionary `scripts` glob alongside deploy metadata; platform packages with nested node_modules deploy targets remain plain deploy tuples instead of becoming package dependency aliases.

Blockers worked around: the Rust port still emits deploy-file warnings but does not copy external deploy files to output directories, so executable-level npm deploy fixtures remain incomplete.

## 2026-05-20 - Deploy-file output copying shipped

Shipped: package production now copies deploy files discovered during walking beside each produced executable. It skips missing deploy sources, copies individual files to their configured target path, recursively expands directory deploy entries, creates target parent directories, and preserves source file permissions. Added package-build parity coverage for file deploy entries, directory deploy entries, missing sources, and Unix mode preservation.

Next: add npm fixture smoke coverage for deploy-file packages once a configured base-binary cache is available, and decide whether automatic native addon copying without explicit deploy metadata belongs in Rust package production or remains npm harness behavior.

Decisions made: use deploy warnings as the package-production copy plan so package config and dictionary metadata share one path without a second deploy-file parser.

Blockers worked around: this is still covered with stub target binaries rather than real npm executable fixtures because `PKG_RUST_REAL_CACHE` is not configured in this shell.

## 2026-05-20 - Native addon cached prebuild selection shipped

Shipped: wired CLI `--no-native-build` into `PackagePlan` and package production. Producer payload reads now prefer an existing `.node.<platform>.<nodeVersion>` sibling when native build is enabled and the target binary cache path exposes a Node version, matching the JS producer's cached-prebuild branch before it invokes `prebuild-install`. Added package-build coverage proving native build uses the platform payload and `--no-native-build` keeps the original addon payload.

Next: port the missing `prebuild-install` invocation path for native addons that do not already have a platform-specific prebuild file, then continue toward real native npm fixture coverage with a configured cache.

Decisions made: this slice only selects already-existing platform prebuilds. Running `prebuild-install`, backing up/restoring the original `.node`, and parsing package `binary.napi_versions` remain separate because they require an external tool boundary and more failure-mode tests.

## 2026-05-20 - Native addon prebuild-install invocation shipped

Shipped: producer native-addon selection now tries a discoverable `prebuild-install` when the cached `.node.<platform>.<nodeVersion>` sibling is missing. The Rust path finds the owning package, backs up the original `.node`, passes the JS platform mapping plus target arch, adds `--target <nodeVersion>` only when `binary.napi_versions` is absent/null, caches the generated native payload under the platform/version suffix, and restores the original `.node`. Installer errors are swallowed at the native-addon selection boundary so packaging falls back to the original payload, matching the JS producer's catch-and-fallback behavior. Added fake-installer coverage for cache creation, N-API target omission, and failure fallback.

Next: add real native npm fixture smoke once a configured base-binary cache and installer environment are available, then continue platform patching/signing gaps.

Decisions made: support `PKG_PREBUILD_INSTALL`, source-tree/local `node_modules/.bin/prebuild-install`, and `PATH` discovery instead of hard-coding only the JS `__dirname` path. Rust restores the backed-up `.node` even on installer failure before falling back, which avoids leaving fixture/package directories mutated after failed attempts.

Blockers worked around: tests use a stub target binary path named like the pkg-fetch cache artifact to prove Node-version parsing without requiring a real downloaded base binary.

## 2026-05-20 - Mach-O patching and signature flag shipped

Shipped: added `src/macho.rs` with the JS `patchMachOExecutable` behavior for little-endian 64-bit Mach-O load commands and the ad-hoc signing wrapper that tries `codesign -f --sign -` before `ldid -Cadhoc -S`. Package production now carries CLI signature intent, honors `--no-signature`, patches/signs macOS outputs when signatures are enabled, and emits a non-fatal arm64 warning if signing tools fail. Added unit coverage for `__LINKEDIT`/symtab patching, truncated Mach-O rejection, and `codesign` to `ldid` fallback; CLI smoke tests that use fake macOS cache binaries now pass `--no-signature`.

Next: add real macOS signing smoke coverage when a real cached Mach-O base binary and signing tools are available, then continue broader fixture coverage and JS oracle retirement.

Decisions made: keep fake-binary smoke tests unsigned because placeholder test binaries are not valid Mach-O files. The production path still patches before signing for real macOS outputs, preserving the JS behavior boundary.

## 2026-05-20 - Compression CLI parity shipped

Shipped: matched the JS compression CLI surface by rendering invalid algorithms as `Invalid compression algorithm <value> ( should be None, Brotli or Gzip)` and printing `compression:  GZip` / `compression:  Brotli` for compressed builds. Added CLI smoke coverage for the invalid-compression fixture path, preserved the original `GZip` enum label through `Compression::cli_label`, and added gated real-runtime smoke coverage for `test-80-compression` across None, GZip, and Brotli package modes.

Next: continue runtime fixture expansion into mountpoints and issue-regression groups, then make the real-cache smoke portable enough for CI.

Decisions made: return both packaging output and executable output from the real-runtime harness so tests can assert CLI diagnostics without weakening the existing Node-oracle stdout comparisons.

## 2026-05-20 - Mountpoint runtime coverage shipped

Shipped: added gated real-runtime smoke coverage for `test-50-mountpoints`, `test-99-#1120-mkdir-mountpoints`, and `test-99-#1121-regexp-mountpoints`. The runtime harness can now run produced executables from their output directory and prepare external files beside the executable before launch, matching the JS fixture shape for string and RegExp mountpoints.

Next: continue issue-regression runtime coverage, especially snapshot file-copy and withFileTypes fixtures, then make the real-cache smoke portable enough for CI.

Decisions made: keep this as a harness/parity-test slice because the embedded JS prelude already owns mountpoint behavior; no Rust runtime logic needed to change before executable-level proof.

## 2026-05-20 - Issue-regression runtime coverage shipped

Shipped: added gated real-runtime smoke coverage for `test-99-#420-copy-from-snapshot`, `test-99-#938-withfiletypes`, and `test-99-#1130`. Fixed package-config glob expansion so recursive `**` asset/script patterns such as `input/**/*` include nested files in the VFS, and added walker parity coverage for the copied snapshot asset.

Next: continue issue-regression runtime coverage across the remaining `test-99-*` fixtures, then make the real-cache smoke portable enough for CI.

Decisions made: implement recursive glob matching in the Rust walker instead of special-casing the runtime smoke, because the failing executable path proved `pkg.assets` data was absent from the packed snapshot.

## 2026-05-20 - Stat issue-regression runtime coverage shipped

Shipped: added gated real-runtime smoke coverage for `test-99-#1505`, comparing Node-oracle and packaged executable output for callback and promise `fs.stat`/`fs.lstat` metadata on packaged files.

Next: continue remaining `test-99-*` issue fixtures, separating portable runtime coverage from native-install and Windows-only cases.

Decisions made: keep `#1505` inside the existing issue-regression smoke group because it exercises already-ported snapshot filesystem metadata behavior and does not require a separate production change.

## 2026-05-20 - Worker-thread issue-regression runtime coverage shipped

Shipped: added gated real-runtime smoke coverage for `test-99-#775`, including both package-directory input and direct `a.js` input. The smoke compares Node-oracle output with the packaged executable to prove worker-thread child script bundling still works in both packaging modes.

Next: continue remaining `test-99-*` fixtures that need external installs (`#1135`, `#1191`, `#1192`) or platform-specific handling (`#1207`, `#1861`).

Decisions made: keep this as runtime coverage because the existing detector/config paths already include the worker child script; the new executable proof guards the issue regression without broadening production logic.

## 2026-05-20 - Portable real-runtime CI smoke shipped

Shipped: added `PKG_RUST_REAL_TARGET` support to the runtime smoke harness while preserving the local default `node18-macos-x64`. The Rust CI workflow now has a dedicated Ubuntu real-runtime smoke job that uses `node18-linux-x64`, caches `.pkg-rust-real-cache`, and runs `cargo test --test runtime_smoke -- --nocapture`. Documentation now shows both the local cache command and the Linux target override, and the local cache directory is ignored.

Next: continue the remaining external-install and Windows-specific issue fixtures, then decide how much native package setup belongs in CI versus local gated smoke.

Decisions made: keep the main Rust matrix free of large binary downloads, and put executable-level proof in a separate job with an explicit cache and target. This preserves fast unit/parity gates while making the real runtime smoke portable beyond this machine.

## 2026-05-20 - Npm issue-regression runtime coverage shipped

Shipped: added opt-in npm-install real-runtime smoke coverage for `test-99-#1192`, the Express/Pug issue fixture. The smoke copies the fixture, installs dependencies with `npm install --ignore-scripts --no-audit --no-fund`, compares the Node oracle against packaged executables, and runs the package in None, GZip, and Brotli modes. Fixed package-json input planning so a package whose `bin` points below `src/` still walks from the package directory and includes sibling `node_modules`. Also made package metadata parsing tolerant of `"main": false`, which appears in real npm dependency package markers.

Next: continue the remaining external-install/native fixtures (`#1135`, `#1191`) and Windows-specific fixtures (`#1207`, `#1861`), keeping them behind explicit gates until the CI/runtime setup is sized for them.

Decisions made: keep npm-install fixture coverage behind `PKG_RUST_INSTALL_NPM_FIXTURES` because it reaches the npm registry and the Brotli packaging path is materially slower over installed dependencies.

## 2026-05-20 - Native npm fixture probe logged

Shipped: probed the remaining `test-99-#1191` better-sqlite3 fixture in a copied `/private/tmp` workspace before adding any committed smoke coverage. `npm install --ignore-scripts --no-audit --no-fund` installs dependencies quickly, but the Node oracle fails because no native binding is present. `npm rebuild better-sqlite3` then fails under the current host Node 25 toolchain, so this fixture is not yet a reliable local smoke candidate.

Next: defer `#1191` runtime proof until the harness can install or run native dependencies against a compatible Node target, or until CI provides a known-good native prebuild/build environment. Continue with fixture-level planning/parity that does not depend on host-native addon compilation.

Decisions made: do not add a gated `#1191` smoke until the native dependency setup itself is reproducible; a test that cannot establish the plain Node oracle would not be useful Rust-port evidence.

## 2026-05-20 - Simple dictionary metadata batch shipped

Shipped: ported a batch of behavior-bearing JS dictionary modules that only carry `pkg.scripts` and/or `pkg.assets` metadata. The Rust dictionary now exposes typed entries for `blessed`, `body-parser`, `browserify`, `buffermaker`, `coffee-script`, `compressjs`, `data-preflight`, `errors`, `eslint`, `googleapis`, `knex`, `later`, `logform`, `machinepack-urls`, `moment`, `mongodb`, `negotiator`, `node-zookeeper-client`, `npm`, `oauth2orize`, `pg.js`, `pgpass`, `pm2`, `reload`, `shelljs`, `svgo`, `tiny-worker`, `uglify-js`, `usage`, and `winston`, with focused parity tests against the JS dictionary file contents.

Next: continue dictionary parity for modules with patches and dependency mutations, then decide whether empty dictionary modules should become explicit no-op Rust entries or remain absent until a full dictionary coverage audit.

Decisions made: use small private helpers for simple script/asset dictionary entries rather than adding one function per package. This keeps the data inert and typed while avoiding a large amount of repetitive boilerplate.
