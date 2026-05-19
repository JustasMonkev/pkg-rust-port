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
