# Test fixture provenance

The fixtures under `test/` are JavaScript test programs used as **inputs** to
the Rust parity tests (the packager bundles and runs them). They are not Rust
source and are reviewed as vendored data, not handwritten code.

## Source

- Upstream repository: <https://github.com/vercel/pkg>
- Tag: `5.8.1`
- Release tarball SHA-256:
  `de1771e0c773ee5159e8c9ef95122bf17b0c02497bf20cc2fb3b23a682e9279c`
- Original location in the tarball: `pkg-5.8.1/test/<same-subdirectory>`

Every `test/test-*` directory is a verbatim copy of the identically named
directory in that tarball. To re-verify, extract the tarball and compare the
referenced directories.

## `test/dictionary-modules.txt`

The canonical pkg dictionary module names, generated from the upstream
`dictionary/*.js` filenames (`.js` stripped) at tag `5.8.1`. Used by the
test-77 parity test to check dictionary/fixture consistency.

## Exceptions (not from the 5.8.1 tree)

- `test/test-99-#1861/` — recreated locally. This regression fixture is
  referenced by a Windows-only, real-cache-gated smoke test but does not exist
  in the 5.8.1 tag. It is a minimal self-relaunch program written for this port,
  not an upstream copy.
