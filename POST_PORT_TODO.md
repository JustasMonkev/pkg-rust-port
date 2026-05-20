# Post-Port TODO

Items parked until JS parity is complete.

- Replace the interim host-node bytecode fallback with target-binary-only
  fabrication once all provider paths and signing requirements are covered.
- Add portable CI coverage for the real runtime smoke by seeding a small
  verified pkg-fetch cache artifact or running a dedicated network-enabled job.
- Port Mach-O patching and signing behavior instead of relying on unsigned
  local smoke binaries.
- Expand real native npm fixture coverage now that cached prebuild selection
  and `prebuild-install` invocation are wired.
- Add criterion benchmarks for packaging throughput and warm-cache release build
  timing before optimizing.
- Retire the JS oracle only after every mapped JS fixture has a Rust parity test.
