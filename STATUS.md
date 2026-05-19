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
