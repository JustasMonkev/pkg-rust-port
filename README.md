# pkg-rust

Rust port of `pkg`, the Node.js project packager. The goal is to preserve the
original `pkg` CLI shape while replacing the TypeScript implementation with
typed Rust modules and parity tests against the original JS fixtures.

This port is still in progress. It can already parse targets/configs, walk and
pack dependency fixtures, fetch pkg-fetch binaries, assemble executable payloads,
and run the small JS API happy-path demo with a real cached target binary.

## Usage

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
- Bytecode fabrication can use a real target binary when a runnable cached
  binary path is available; otherwise tests fall back to host `node`.
- The JS suite remains the behavioral oracle until every fixture has a Rust
  equivalent.
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

The release profile strips symbols. On this machine, a warm-cache release
rebuild completed in `0.13s` with `target/release/pkg` already stripped.

Benchmarks:

```sh
cargo bench --bench packaging
```

The Criterion benchmark target currently tracks the `test-50-require-resolve`
walk/refine/pack pipeline and gzip producer-manifest construction.

To run the real runtime smoke after seeding a cache:

```sh
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
  cargo test --test runtime_smoke -- --nocapture
```

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
dependency utility packages such as `debug`, dictionary
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

Native npm issue fixtures are behind a separate gate because they run package
install scripts and must first establish a working plain Node oracle:

```sh
PKG_RUST_NATIVE_NPM_FIXTURES=1 \
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
  cargo test --test runtime_smoke -- native_npm_issue_fixtures_run_when_install_is_enabled --nocapture
```
