# Post-Port TODO

Items parked until JS parity is complete.

- Replace the interim host-node bytecode fallback with target-binary-only
  fabrication once all provider paths and signing requirements are covered.
- Add real macOS signing smoke coverage with an actual cached Mach-O base
  binary and available signing tools.
- Expand remaining real native npm fixture coverage now that cached prebuild
  selection and `prebuild-install` invocation are wired.
- Expand Criterion benchmarks beyond the initial packaging pipeline scaffold,
  including warm-cache release build timing, before optimizing.
- Retire the JS oracle only after every mapped JS fixture has a Rust parity test.
