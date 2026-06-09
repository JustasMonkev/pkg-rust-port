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

## Backlog (porting order)

1. **External config file support** (`lib/config.ts`, ~1040 lines)
   - `-c/--config` accepting `package.json`, `*.json`, `.js`, `.cjs`, `.mjs`
   - Auto-discovery of `.pkgrc`, `.pkgrc.json`, `pkg.config.js`,
     `pkg.config.cjs`, `pkg.config.mjs` (first match wins)
   - All build-shaping flags settable in config (camelCase keys); CLI
     overrides config (`resolveFlags`, precedence CLI > config > default)
   - Note: `.js`/`.cjs`/`.mjs` config files execute JS; the Rust port will
     need a decision (Node subprocess evaluation vs JSON-only support).
2. **`--fallback-to-source`**: when bytecode fabrication fails for a file,
   ship plain source instead of skipping it (fabricator/producer flag).
3. **`--signature`** positive flag (override `signature: false` from config).
4. **Exports-field-aware resolver** (`lib/resolver.ts` + `resolve.exports`):
   package.json `exports` resolution with `require` condition first, then
   `import` fallback for ESM-only packages.
5. **ESM support** (`lib/esm-transformer.ts`, ~430 lines): transform/bundle
   ESM entrypoints and `.mjs` files to CJS via esbuild; `wasTransformed`
   record flag; packer renames transformed `.mjs` → `.js` in the snapshot.
   Requires a bundler decision for Rust (SWC bundling vs esbuild subprocess).
6. **Walker/detector/refiner deltas vs 5.8.1** (`lib/walker.ts` is now ~1320
   lines): diff and port behavior changes, including `wasTransformed`
   propagation and new dictionary handling.
7. **pkg-fetch 3.6.x targets** (`@yao-pkg/pkg-fetch` 3.6.3): node18/20/22/24
   binaries, updated expected-hash table, updated cache version naming, new
   default node range. The embedded `fetch_expected_shas.json` is still the
   vercel pkg-fetch 3.5 table.
8. **SEA support** (`--sea`, `lib/sea.ts` ~930 lines, `lib/sea-assets.ts`,
   `prelude/sea-*.js`): Node single-executable-application pipeline via
   postject; simple mode (plain .js, no package.json) and enhanced mode
   (walker-backed VFS assets, compression support).
9. **Dictionary deltas**: diff `dictionary/*.js` against the Rust typed data.
10. **Help text / CLI surface**: update to the yao-pkg help output (new
    flags, examples, config-file mention) and picocolors-equivalent styling.
11. **Misc**: `compression:` info line for Zstd targets gating, yao-pkg
    CHANGELOG-driven behavior fixes not covered above.

## Sources

- yao-pkg/pkg @ 6.19.0 (cloned at porting time; `lib/`, `prelude/`,
  `dictionary/`, `test/`)
- This port's vercel/pkg 5.8.1 mapping in `MIGRATION.md`
