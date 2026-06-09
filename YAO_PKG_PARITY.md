# yao-pkg/pkg Parity Backlog

This Rust port was originally mapped from `vercel/pkg` 5.8.1. The new parity
target is [`yao-pkg/pkg`](https://github.com/yao-pkg/pkg) 6.19.0, the actively
maintained fork. This file tracks the feature gap between the two and the
porting order. Items move to "Done" as they land with parity tests.

## Done

- [x] Runtime prelude updated to the yao-pkg 6.19.0 split prelude:
  `bootstrap.js` + `bootstrap-shared.js`, the `REQUIRE_SHARED` wrapper
  parameter, and the inline debug diagnostic that calls
  `REQUIRE_SHARED.installDiagnostic`. Version reporting is now `6.19.0`.
- [x] Zstd compression (`--compress Zstd|zs|zstd`): `CompressType.Zstd = 3`,
  native Rust zstd payload encoding, `%DOCOMPRESS%` = 3, and the yao-pkg
  invalid-algorithm error wording
  (`Invalid compression algorithm "x" (accepted: None/none, Brotli/br, GZip/gz/gzip, or Zstd/zs/zstd)`).
  Note: the JS implementation compresses through Node's zlib zstd bindings and
  therefore requires a Node >= 22.15 build host; the Rust port encodes
  natively, so the build-host constraint does not apply. The produced binary
  still requires a target Node >= 22.15 to decompress (enforced at runtime by
  `bootstrap-shared.js`).

- [x] `--fallback-to-source`: failed bytecode fabrication ships the file as
  plain source (`STORE_CONTENT`) with the yao-pkg warning wording instead of
  skipping it. Without the flag, skipped-blob warnings keep this port's
  fail-closed behavior and now use the yao-pkg wording with the
  `--fallback-to-source` hint.
- [x] `--signature` positive flag (overrides `--no-signature` and config
  `signature: false`).
- [x] External config file support: `-c/--config` accepting `package.json`,
  `*.json`, `.js`, `.cjs`, `.mjs`; auto-discovery of `.pkgrc`, `.pkgrc.json`,
  `pkg.config.js`, `pkg.config.cjs`, `pkg.config.mjs` (first match wins, with
  the `Using config` info line and the "takes precedence" warning when
  `package.json` also has a `pkg` field); bare configs wrapped as
  `{ "pkg": ... }`; build-shaping flags resolvable from config with
  CLI > config > default precedence (`debug`, `compress`, `bytecode`,
  `nativeBuild`, `signature`, `fallbackToSource`, `public`, `publicPackages`,
  `noDictionary`, `options`), including the hidden positive/negative CLI flag
  pairs (`--bytecode`, `--native-build`, `--no-debug`, `--no-public`,
  `--no-fallback-to-source`). JS config modules are evaluated through the
  host `node` (same external boundary as bytecode fabrication). Not yet
  ported: unknown-key warnings and per-key type-error wording from
  `validatePkgConfig`, and the `sea` flag (blocked on the SEA slice).
- [x] pkg-fetch retargeted to `@yao-pkg/pkg-fetch` 3.6.3: cache tag `v3.6`,
  release downloads from `yao-pkg/pkg-fetch`, the 3.6.3 patched node version
  set (8/10/12/14/16.20.2/18.20.8/20.20.2/22.22.3/24.15.0/26.2.0), the 3.6.3
  expected-SHA table, and the yao-pkg known-arch set (adds `x86`, `ppc64`,
  `s390x`, `riscv64`, `loong64`; drops `armv6`).

- [x] Exports-field-aware resolver: bare specifiers resolve through the
  target package's `exports` field (`require` condition first, `import`
  fallback; `{condition, node, default}` set; exact, shorthand, and `*`
  pattern subpaths), gated like JS `follow.ts` so only actual ESM files
  (`.mjs`, or `.js` under `"type": "module"`) use the exports result while
  CJS packages keep classic `main` resolution. Not yet ported: the
  synthetic-`main` injection for exports-only packages at walk time.

- [x] Dictionary deltas vs yao-pkg 6.19.0: added `sqlite3`
  (`pkg.assets: build/Release/*.node`) and `thread-stream`
  (`pkg.scripts: lib/worker.js`) plus the `thread-stream` test-79-npm
  fixture; `tesseract.js` already carried the yao value.
- [x] Help text / CLI surface updated to the yao-pkg help: config-file
  discovery wording, signature flag wording, Brotli/GZip/Zstd compression
  line, node22/node24 examples, Zstd example, and the config-file
  paragraph. `--sea` stays out of the help until the SEA slice lands.

## Backlog (porting order)

1. **ESM support** (`lib/esm-transformer.ts`, ~430 lines): transform/bundle
   ESM entrypoints and `.mjs` files to CJS via esbuild; `wasTransformed`
   record flag; packer renames transformed `.mjs` → `.js` in the snapshot.
   Requires a bundler decision for Rust (SWC bundling vs esbuild subprocess).
2. **Walker/detector/refiner deltas vs 5.8.1** (`lib/walker.ts` is now ~1320
   lines): diff and port behavior changes, including `wasTransformed`
   propagation and new dictionary handling.
3. **SEA support** (`--sea`, `lib/sea.ts` ~930 lines, `lib/sea-assets.ts`,
   `prelude/sea-*.js`): Node single-executable-application pipeline via
   postject; simple mode (plain .js, no package.json) and enhanced mode
   (walker-backed VFS assets, compression support).
4. **Misc**: `compression:` info line for Zstd targets gating, yao-pkg
    CHANGELOG-driven behavior fixes not covered above.

## Sources

- yao-pkg/pkg @ 6.19.0 (cloned at porting time; `lib/`, `prelude/`,
  `dictionary/`, `test/`)
- This port's vercel/pkg 5.8.1 mapping in `MIGRATION.md`
