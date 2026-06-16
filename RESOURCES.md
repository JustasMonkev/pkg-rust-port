# pkg-rust npm release resources

## Knowledge

- [Local: root npm manifest](package.json)
  Defines the public wrapper package, `bin/pkg.js`, root package allowlist, Node engine, and native `optionalDependencies`.
- [Local: launcher](bin/pkg.js)
  Maps `process.platform`, `process.arch`, and Linux libc to a native optional package, then executes its binary with `spawnSync`.
- [Local: release workflow](.github/workflows/release.yml)
  Builds the native matrix, signs/notarizes macOS binaries, stages artifacts, verifies native packages, and publishes to npm.
- [Local: package checks](scripts/check-npm-package.js)
  Encodes the current packaging invariants: required metadata, exact native package set, and launcher argument forwarding.
- [npm Docs: package.json](https://docs.npmjs.com/cli/v10/configuring-npm/package-json/)
  Primary source for `name`, `version`, `files`, `bin`, `optionalDependencies`, `os`, `cpu`, `engines`, and `publishConfig`.
- [GitHub actions/upload-artifact: permission loss](https://github.com/actions/upload-artifact#permission-loss)
  Primary source for the release bug: uploaded artifacts do not preserve Unix file permissions.
- [Node.js Docs: child_process.spawnSync](https://nodejs.org/api/child_process.html#child_processspawnsynccommand-args-options)
  Primary source for how the launcher executes the native binary and why direct executability matters.

## Wisdom (Communities)

- [GitHub Actions community discussions](https://github.com/orgs/community/discussions/categories/actions)
  Use for workflow edge cases that only show up in hosted runners or artifact behavior.
- [npm feedback repository](https://github.com/npm/feedback)
  Use for npm registry/package behavior questions that are unclear from CLI docs.

## Gaps

- The release workflow has not yet been run on GitHub with real signing and npm secrets.
- The lesson does not cover npm provenance or trusted publishing.
