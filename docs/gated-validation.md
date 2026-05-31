# Gated Validation

This repo treats the Rust port as complete for offline-testable pkg behavior.
The remaining evidence requires external state, so it is captured as explicit
validation gates instead of default CI.

## Gate Classes

| Class | Proof | Local command | Manual workflow mode |
| --- | --- | --- | --- |
| Source-build boundary | `--build` accepts only pkg-fetch `built-*` cache artifacts and reports the exact external source-build requirement when absent. | `cargo test --locked --test fetch_parity force_build -- --nocapture` | `source-build-boundary` |
| Installed npm issue fixtures | Fixtures that need `npm install` but not native install scripts still match the Node oracle after packaging. | `PKG_RUST_INSTALL_NPM_FIXTURES=1 PKG_RUST_REAL_CACHE=/path/to/cache cargo test --locked --test runtime_smoke -- npm_issue_fixtures_run_when_install_is_enabled --nocapture` | `npm-issues` |
| Public npm dictionary smoke | Promoted public `test-79-npm` packages install from npm, pass the JS harness oracle, package with Rust, and match output. | `PKG_RUST_INSTALL_NPM_FIXTURES=1 PKG_RUST_REAL_CACHE=/path/to/cache cargo test --locked --test runtime_smoke -- public_npm_dictionary_fixtures_run_when_install_is_enabled --nocapture` | `public-npm` |
| Target-node oracle probe | One candidate public package passes under the selected pkg-fetch target Node before promotion. | `PKG_RUST_INSTALL_NPM_FIXTURES=1 PKG_RUST_REAL_CACHE=/path/to/cache PKG_RUST_TARGET_ORACLE_PUBLIC_NPM=<fixture> cargo test --locked --test runtime_smoke -- public_npm_target_node_oracle_probe_runs_when_enabled --nocapture` | `target-oracle` |
| Public npm promotion | One candidate first passes the target-node oracle, then packages with Rust and matches that target oracle output. | `PKG_RUST_INSTALL_NPM_FIXTURES=1 PKG_RUST_REAL_CACHE=/path/to/cache PKG_RUST_PROMOTE_PUBLIC_NPM=<fixture> cargo test --locked --test runtime_smoke -- public_npm_fixture_promotion_workflow_runs_when_enabled --nocapture` | `public-npm-promote` |
| Native npm issue fixtures | Native install-script fixtures establish a plain Node oracle, package with Rust, and match output. | `PKG_RUST_NATIVE_NPM_FIXTURES=1 PKG_RUST_REAL_CACHE=/path/to/cache cargo test --locked --test runtime_smoke -- native_npm_issue_fixtures_run_when_install_is_enabled --nocapture` | `native-npm` |
| Real pkg comparison | Selected fixtures package with both real `pkg@5.8.1` and this port using the same cache, then compare stdout, stderr, and embedded concrete `/snapshot/...` strings. | `PKG_RUST_REAL_PKG_COMPARE=1 PKG_RUST_REAL_PKG_BIN=/path/to/node_modules/.bin/pkg PKG_CACHE_PATH=/path/to/cache PKG_RUST_REAL_TARGET=node18-macos-x64 cargo test --locked --test real_pkg_compare -- --nocapture` | n/a |

## Promotion Rule

A public npm fixture can move into the broad public npm smoke list only after
the `public-npm-promote` gate passes for the selected target. The separate
`target-oracle` gate is useful for diagnosis because it can prove registry or
target-Node drift before the Rust packager is involved.

The real pkg comparison gate is intentionally separate from normal runtime
smoke tests. The v3.5/Node 18.15.0 cache path used by this port requires
overriding `pkg@5.8.1`'s nested `pkg-fetch` dependency to `3.5.2`; a plain
`npm install pkg@5.8.1` uses `pkg-fetch@3.4.2` and looks for
`v3.4/fetched-v18.5.0-*`.

## External Source Builds

The Rust port does not run Node.js source builds. For `--build`, an external
pkg-fetch-compatible source build must create the expected `built-*` cache file.
`PkgFetchCache::source_build_requirement` is the authoritative API for the
required path.
