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
  `validatePkgConfig`. The `sea` flag is now wired (CLI > config > default),
  resolved alongside the other build-shaping flags (see the SEA slice below).
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
  paragraph. `--sea` is now in the help (flag line, config paragraph, and
  the `pkg --sea index.js` example) with the SEA slice landed.

- [x] Walker/detector deltas (non-ESM): literal dynamic `import("x")` is
  detected as a resolvable alias (`visitorDynamicImport`); module resolution
  extensions now include `.mjs`; top-level config `ignore` glob patterns skip
  blob/content stores for matching files (`node:` builtin prefixes were
  already handled). Remaining walker deltas are the ESM-transform and
  SEA-mode paths tracked below, plus the symlink junction-point change.

- [x] Producer placeholder discovery skips apostrophe-quoted source-literal
  occurrences of the placeholder text (yao-pkg/pkg#86), preferring a later
  occurrence when one exists.

- [x] ESM support: ESM blobs (`.mjs`, or `.js` under `"type": "module"`)
  are transformed to CommonJS before detection and bytecode compilation.
  The Rust port uses SWC's `common_js` pass (the JS implementation uses
  esbuild), which natively rewrites `import.meta.url`/`.filename`/`.dirname`
  to CJS equivalents. Top-level await without exports wraps in an async IIFE
  with imports hoisted; top-level await with exports ships untransformed
  with the yao-pkg warning. Transformed `.mjs` records are marked
  `was_transformed`, relative `.mjs` require paths are rewritten to `.js`,
  and the packer renames transformed `.mjs` snapshots (including the
  entrypoint) to `.js`. Interop helper definitions (`_interop_require_default`
  and friends) are injected inline via SWC's `inline-helpers` feature so the
  CJS output is self-contained, matching esbuild's inlined helpers.
  BEHAVIOR FIX over yao-pkg: bare specifiers whose runtime `require()` would
  fail against the snapshot are rewritten to relative paths during the
  transform. Two shapes qualify: packages reachable only through the
  `import` exports condition (no `require` condition — runtime throws
  `ERR_PACKAGE_PATH_NOT_EXPORTED`), and packages whose require-reachable
  exports target is an `.mjs` file (the packer renames the transformed
  snapshot to `.js`, so runtime exports resolution points at a missing
  file). yao-pkg 6.20.0 leaves both bare and its packaged output crashes at
  startup; the Rust port loads the same file the walker packages.
  Require-resolvable specifiers keep their bare form: dual packages,
  classic `main` packages, and `.js`-under-`"type": "module"` exports
  targets (which snapshot as transformed CommonJS at their original path
  and load fine through the exports map — verified against the real
  node22 runtime).

- [x] SEA support — **simple mode + shared foundation** (`lib/sea.ts`
  `sea()` path). `--sea`/config `sea` build a Node single executable for a
  bare entry file. Ported: the deterministic core (nodejs.org os/arch
  mapping with the yao-pkg `NODE_OSES`/`NODE_ARCHS` sets and wording (Linux
  `ppc64` resolves the `ppc64le` archive token — a fix over yao-pkg, whose
  `node-<v>-linux-ppc64` URL 404s),
  archive-filename + dist/unofficial-builds URL construction, version-format
  validation, host `>= 22` assertion, single-major / min-major checks,
  up-front rejection of targets whose native injector is not implemented yet,
  and full generator selection — matching-host-target reuse, the
  version-matched host `node`, or a host-platform download pinned to the exact
  target version for cross-host builds); the I/O pipeline (download +
  `SHASUMS256.txt` checksum + zip/tar.gz extraction with `.ok` sentinels and
  the `~/.pkg-cache/sea` cache, honoring `PKG_CACHE_PATH`; blob generation via
  the host `node --experimental-sea-config`; bake = copy + inject; macOS
  ad-hoc signing reusing `mach-o.rs`); and **native** `NODE_SEA_BLOB`
  injection. DECISION: injection is native Rust rather than shelling out to
  `postject` — Node's ELF resource finder (`postject-api.h`) locates the blob
  by scanning `PT_NOTE` segments via `dl_iterate_phdr`, so injection appends
  the blob as an ELF note, maps it with a fresh `PT_LOAD`, exposes it through a
  fresh `PT_NOTE`, repoints `PT_PHDR` to the relocated header table, and flips
  the `NODE_SEA_FUSE_…:0` fuse. ELF injection is verified end to end against
  the real Node 22 runtime on Linux x64. Remaining (next slice): see item 1.

## Backlog (porting order)

1. **SEA: enhanced mode + macOS/Windows injection** (`lib/sea.ts`
   `seaEnhanced()`, `lib/sea-assets.ts`, `prelude/sea-*.js`). The simple-mode
   foundation landed (see Done); what remains:
   - **Enhanced mode**: when an input `package.json`/config is present, walk
     dependencies with `seaMode: true` (blob stores downgraded to content; no
     ESM transform; JS/ESM bodies read for shipping), build the per-file SEA
     archive + `__pkg_manifest__` (`sea-assets.ts`), and use the bundled VFS
     bootstrap (`prelude/sea-bootstrap.bundle.js`, an esbuild bundle of
     `sea-bootstrap.js` + `sea-vfs-setup.js` + `bootstrap-shared.js` — vendor
     the generated bundle or assemble it). Currently fails closed with a
     precise error.
   - **macOS (Mach-O) injection**: add the `NODE_SEA` segment + `__NODE_SEA_BLOB`
     section, then the existing Mach-O payload-patch skip + ad-hoc re-sign.
     Currently fails closed.
   - **Windows (PE) injection**: add the `NODE_SEA_BLOB` `RT_RCDATA` resource.
     Currently fails closed.
   Design notes from the 2026-06-09 study of yao-pkg 6.19.0:
   - Host requirement: Node >= 20 on the build machine; enhanced mode
     requires a single target major and Node >= 22 targets.
   - Target binaries come from official nodejs.org dist (and
     unofficial-builds for linuxstatic/armv7), checksum-verified and
     cached, NOT from pkg-fetch. Rust will need zip + tar.gz + tar.xz
     extraction (`zip`, `tar`, `flate2`, plus an xz backend).
   - Blob generation shells out to a host-compatible downloaded node:
     `node --experimental-sea-config sea-config.json`; simple mode uses
     the entry as `main`, enhanced mode uses the bundled
     `prelude/sea-bootstrap.bundle.js` (an esbuild bundle of
     `sea-bootstrap.js` + `sea-vfs-setup.js` + `bootstrap-shared.js`,
     generated at yao-pkg build time — the Rust port must either vendor
     the generated bundle or assemble it) plus walker-derived assets
     from `sea-assets.ts` (`__pkg_manifest__` + per-file assets, with
     optional per-asset compression).
   - Injection: yao-pkg uses the postject library (LIEF-based) to add
     the `NODE_SEA_BLOB` section/segment/resource with the SEA fuse
     sentinel (built by string concatenation to avoid duplicate
     sentinel occurrences). The Rust port needs a decision: implement
     native section injection for ELF/Mach-O/PE (large; no
     off-the-shelf crate covers all three) or shell out to
     `npx postject` (keeps an npm boundary). Enhanced mode forbids
     `useSnapshot` and walks with `seaMode: true` (blob stores
     downgraded to content; no ESM transform).
   - After injection: Mach-O payload patch + ad-hoc re-sign (existing
     `macho.rs` covers this), Windows signature removal is N/A (yao
     leaves PE unsigned).
2. **Misc**: prebuild-install `npm_config_<name>` env prefixing,
   `findCommonJunctionPoint` symlink change, JS `validatePkgConfig`
   unknown-key warnings/type wording, synthetic-`main` injection for
   exports-only packages, walker `unlikelyJavascript` exact-list parity
   (Rust currently retags all non-JS blobs as content, a superset), and
   any yao-pkg CHANGELOG-driven behavior fixes not covered above.
   Known pre-existing test flake: parallel lib tests that write-then-exec
   helper scripts can hit a fork/exec text-busy race; it does not
   reproduce in isolated runs.
   Known pre-existing runtime edge (found 2026-06-10, unrelated to ESM):
   packaging a file that sits one directory below the filesystem root
   (e.g. `/tmp/app.js`) produces a `Cannot find module 'tmp/app.js'`
   snapshot path at runtime; files inside any deeper project directory
   work. Needs a snapshot-base/denominator fix for near-root inputs.

## Sources

- yao-pkg/pkg @ 6.19.0 (cloned at porting time; `lib/`, `prelude/`,
  `dictionary/`, `test/`)
- This port's vercel/pkg 5.8.1 mapping in `MIGRATION.md`
