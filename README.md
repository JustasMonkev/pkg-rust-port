# pkg-rust

Rust port of `pkg`, the Node.js project packager. The goal is to preserve the
original `pkg` CLI shape while replacing the TypeScript implementation with
typed Rust modules and parity tests against the original JS fixtures.

This port covers the offline-testable pkg 5.8.1 behavior with Rust parity tests.
It parses targets/configs, walks and packs dependency fixtures, fetches
pkg-fetch binaries, assembles executable payloads, and runs real packaged
runtime fixtures when a cache is provided.

## Usage

Install the npm CLI wrapper:

```sh
npm install -g @jm-pkg-rust/pkg-rust
pkg --help
```

The npm package ships `bin/pkg.js`, which dispatches to a platform-specific
native package installed through optional dependencies:

- `@jm-pkg-rust/pkg-rust-darwin-arm64`
- `@jm-pkg-rust/pkg-rust-darwin-x64`
- `@jm-pkg-rust/pkg-rust-linux-arm64-gnu`
- `@jm-pkg-rust/pkg-rust-linux-arm64-musl`
- `@jm-pkg-rust/pkg-rust-linux-x64-gnu`
- `@jm-pkg-rust/pkg-rust-linux-x64-musl`
- `@jm-pkg-rust/pkg-rust-win32-x64-msvc`

If the launcher reports a missing native package, reinstall without
`--omit=optional` so npm can install the matching binary package.

```sh
cargo run -- --target node18-macos-x64 --output ./app ./index.js
```

The CLI mirrors the original command form:

```text
pkg [options] <input>
```

Common options:

```text
-t, --targets <targets>      comma-separated targets, for example node18-linux-x64
-c, --config <path>          package.json or JSON config with top-level pkg config
-o, --output <path>          output file name or template
    --out-path <path>        output directory for multiple targets
    --options <options>      comma-separated Node/V8 options to bake into output
    --no-bytecode            skip bytecode payloads when source content is available
-C, --compress <algorithm>   none, gzip, or brotli
```

Examples:

```sh
cargo run -- ./index.js
cargo run -- --target node18-linux-x64 ./index.js
cargo run -- --target node18-linux,node18-macos,node18-win ./index.js
cargo run -- --options expose-gc,max-old-space-size=1024 ./index.js
cargo run -- --compress gzip ./index.js
```

## Targets

Targets use the original `pkg` shape:

```text
node18-macos-x64
node16-linux-arm64
node18-win-x64
```

Omitted pieces are filled from the host defaults. Base binaries are read from
`$PKG_CACHE_PATH` or `~/.pkg-cache` using the pkg-fetch 3.5 cache layout. Missing
fetched binaries are downloaded from the pkg-fetch GitHub release and verified
against the embedded SHA-256 table.

## Current Limits

- Native addon handling supports cached `.node.<platform>.<nodeVersion>`
  payloads and can invoke a discoverable `prebuild-install`, but broad real npm
  native fixture coverage still depends on a configured base-binary cache.
- Mach-O patching and ad-hoc signing are wired for macOS outputs when
  `codesign` or `ldid` is available; fake-binary smoke tests use
  `--no-signature`.
- Bytecode is fabricated by a host-platform fabricator binary that matches the
  output target's node range and arch (pkg's `fabricatorForTarget`), so
  cross-platform builds do not run the output target binary; in-memory test
  providers fall back to host `node`.
- The `--build` Node-from-source path is an explicit external boundary. The CLI
  passes `forceBuild` through, `PkgFetchCache::source_build_requirement` reports
  the exact `built-*` cache artifact required, and the source build itself must
  be produced by pkg-fetch-compatible tooling.
- The JS suite remains the behavioral oracle only for the opt-in network, npm,
  and native fixtures; every offline-testable mapped fixture has a Rust parity
  test.
- The real runtime smoke is opt-in because it needs a real cached pkg-fetch
  binary and, on Apple silicon, Rosetta for the `node18-macos-x64` target.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test --doc --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features
```

Release binary check:

```sh
cargo build --release --locked
```

NPM release check:

```sh
npm test
npm_config_cache=/private/tmp/npm-cache npm pack --dry-run --json
```

The release workflow builds native packages for macOS x64/arm64, Linux
x64/arm64 with glibc and musl, and Windows x64. It publishes the native
optional-dependency packages before publishing `@jm-pkg-rust/pkg-rust`.
Publishing requires `NPM_TOKEN`; macOS release binaries also require
`APPLE_CERTIFICATE_P12`, `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_CODESIGN_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID`, and
`APPLE_APP_SPECIFIC_PASSWORD` so the workflow can codesign and notarize them.

The release profile strips symbols. On this machine, a warm-cache release
rebuild completed in `0.13s` with `target/release/pkg` already stripped.

Benchmarks:

```sh
cargo bench --bench packaging
```

The Criterion benchmark target currently tracks the `test-50-require-resolve`
walk/refine/pack pipeline and gzip producer-manifest construction.

Source-build boundary check:

```sh
cargo test --locked --test fetch_parity force_build -- --nocapture
```

That gate proves `--build` never silently falls back to fetched binaries: it only
accepts a pkg-fetch `built-*` cache artifact and reports the external source
build requirement when the artifact is absent.

To run the real runtime smoke after seeding a cache:

```sh
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
  cargo test --test runtime_smoke -- --nocapture
```

The compression smoke installs the `test/test-80-compression` fixture's pinned
npm dependencies on first run (and skips itself with a notice when npm or the
network is unavailable).

To compare selected fixtures against a real `pkg@5.8.1` oracle, seed
`PKG_CACHE_PATH` with the same pkg-fetch base binary and point
`PKG_RUST_REAL_PKG_BIN` at the oracle CLI:

```sh
mkdir -p /private/tmp/pkg-rust-real-compare/cache/v3.5
curl -L \
  https://github.com/vercel/pkg-fetch/releases/download/v3.5/node-v18.15.0-macos-x64 \
  -o /private/tmp/pkg-rust-real-compare/cache/v3.5/fetched-v18.15.0-macos-x64
chmod +x /private/tmp/pkg-rust-real-compare/cache/v3.5/fetched-v18.15.0-macos-x64
npm install pkg@5.8.1 --prefix /private/tmp/pkg-rust-real-compare/oracle --no-audit --no-fund
npm install pkg-fetch@3.5.2 \
  --prefix /private/tmp/pkg-rust-real-compare/oracle/node_modules/pkg \
  --no-audit --no-fund

PKG_RUST_REAL_PKG_COMPARE=1 \
PKG_RUST_REAL_PKG_BIN=/private/tmp/pkg-rust-real-compare/oracle/node_modules/.bin/pkg \
PKG_CACHE_PATH=/private/tmp/pkg-rust-real-compare/cache \
PKG_RUST_REAL_TARGET=node18-macos-x64 \
  cargo test --test real_pkg_compare -- --nocapture
```

The nested `pkg-fetch@3.5.2` override is required for this v3.5/Node 18.15.0
cache. A plain `npm install pkg@5.8.1` currently installs `pkg-fetch@3.4.2`,
whose Node 18 oracle asks for `v3.4/fetched-v18.5.0-*` instead.

The smoke target defaults to `node18-macos-x64` for this local Apple-silicon
workspace. Set `PKG_RUST_REAL_TARGET=node18-linux-x64` on Linux runners; the CI
workflow uses that target and caches `.pkg-rust-real-cache`.

```sh
PKG_RUST_REAL_CACHE=.pkg-rust-real-cache \
PKG_RUST_REAL_TARGET=node18-linux-x64 \
  cargo test --test runtime_smoke -- --nocapture
```

External npm fixture smoke is additionally gated because it runs `npm install`
inside copied JS fixtures:

```sh
PKG_RUST_INSTALL_NPM_FIXTURES=1 \
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
  cargo test --test runtime_smoke -- npm_issue_fixtures_run_when_install_is_enabled --nocapture
```

The same gate also covers selected public `test-79-npm` package fixtures whose
success depends on dictionary metadata, such as `connect`, `rc`, `moment`, and
`any-promise` / `hoek` / `semver` / `verror` / `uglify-js`, last-line metadata fixtures such as
`browserify`, plus pinned legacy package shapes such as `uglify-js@2.7.5` and
`body-parser@1.10.2`,
template-rendering packages such as `express` with `jade`, appender-loader
packages such as `log4js@0.5.8` /
`log4js@0.6.34` / `log4js@1.1.1`, parser modules such as `negotiator` /
`negotiator@0.4.9`, `cookie`, and `machinepack-urls` / `machinepack-urls@5.0.0`,
dictionary script packages such as `shelljs` / `shelljs@0.7.6` /
`shelljs@0.6.0` / `shelljs@0.1.4`, `buffermaker`, `bytes`, pinned `bson@0.2.22` /
`bson@0.4.0`, `compressjs`, `later`, `lodash`, `mongodb`, `mongodb-core`,
`nconf`, `node-forge`, `node-xlsx`, `npm-registry-client`, `oauth2orize`, `pg`,
`pg-cursor`, `pg-query-stream`, `pgpass`, and `pg-types` / `pg-types@1.0.0`, dictionary
asset packages such as `errors`, `geoip-lite`, `node-zookeeper-client`, and
`steam-crypto` / `throng` / `tinify` / `tiny-worker`, dictionary
lookup/utility packages such as `mime-types`, `ms`, and `underscore`, dictionary
stderr-comparing packages such as `json-stringify-safe` / `json-stringify-safe@4.0.0`, dictionary
dependency utility packages such as `debug`, `extsprintf`, and `diff`, dictionary
asset/dependency-pruning packages such as
`publicsuffixlist`, patch packages such as `graceful-fs` /
`graceful-fs@3.0.8`, and loader-heavy packages such as `logform` and
`body-parser`:

```sh
PKG_RUST_INSTALL_NPM_FIXTURES=1 \
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
  cargo test --test runtime_smoke -- public_npm_dictionary_fixtures_run_when_install_is_enabled --nocapture
```

Before promoting a modern public npm fixture, probe its plain-Node oracle with
the selected pkg-fetch target binary. This catches host-vs-target Node drift,
such as current packages that pass on the host Node but fail on the Node 18
runtime used by the packaged executable:

```sh
PKG_RUST_INSTALL_NPM_FIXTURES=1 \
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
PKG_RUST_TARGET_ORACLE_PUBLIC_NPM=cookie \
  cargo test --test runtime_smoke -- public_npm_target_node_oracle_probe_runs_when_enabled --nocapture
```

To promote one candidate through both checks, run the reusable promotion gate.
It first runs the target-node oracle, then packages the same fixture with the
Rust CLI and compares the packaged output against that oracle:

```sh
PKG_RUST_INSTALL_NPM_FIXTURES=1 \
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
PKG_RUST_PROMOTE_PUBLIC_NPM=cookie \
  cargo test --test runtime_smoke -- public_npm_fixture_promotion_workflow_runs_when_enabled --nocapture
```

Native npm issue fixtures are behind a separate gate because they run package
install scripts and must first establish a working plain Node oracle:

```sh
PKG_RUST_NATIVE_NPM_FIXTURES=1 \
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
  cargo test --test runtime_smoke -- native_npm_issue_fixtures_run_when_install_is_enabled --nocapture
```

The same gated commands are available from GitHub Actions through the manual
`Gated Runtime Validation` workflow. Select one of `source-build-boundary`,
`npm-issues`, `public-npm`, `target-oracle`, `public-npm-promote`, or
`native-npm`; for the target-oracle and promotion modes, set `fixture` to the
candidate public npm fixture name. The gate classes and promotion rules are
summarized in [`docs/gated-validation.md`](docs/gated-validation.md).
