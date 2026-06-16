# Mission: Shipping pkg-rust as an npm CLI

## Why
You want to be able to look at this repo's npm packaging and release automation and tell whether it will actually install and run for users, not merely whether the manifests look plausible.

## Success looks like
- You can trace the path from `npm install -g @jm-pkg-rust/pkg-rust` to the native `pkg` binary that runs.
- You can spot release-only failures, especially missing native binaries, wrong optional dependencies, and lost executable bits.
- You can choose the right local or CI check for a packaging concern instead of guessing from `package.json`.

## Constraints
- Keep lessons grounded in the exact files in this checkout.
- Prefer short, reviewable lessons with practical checks.
- Avoid broad npm publishing theory unless it explains a real failure mode in this repo.

## Out of scope
- Rewriting the Rust CLI internals.
- Full npm organization/account administration.
- A complete GitHub Actions course.
