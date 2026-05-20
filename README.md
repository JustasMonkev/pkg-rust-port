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
success depends on dictionary metadata, such as `connect`, `rc`, and
`moment`, plus pinned legacy package shapes such as `uglify-js@2.7.5`:

```sh
PKG_RUST_INSTALL_NPM_FIXTURES=1 \
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
  cargo test --test runtime_smoke -- public_npm_dictionary_fixtures_run_when_install_is_enabled --nocapture
```

Native npm issue fixtures are behind a separate gate because they run package
install scripts and must first establish a working plain Node oracle:

```sh
PKG_RUST_NATIVE_NPM_FIXTURES=1 \
PKG_RUST_REAL_CACHE=/private/tmp/pkg-rust-real-cache \
  cargo test --test runtime_smoke -- native_npm_issue_fixtures_run_when_install_is_enabled --nocapture
```
