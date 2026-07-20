## ADDED Requirements

### Requirement: CI builds and publishes version-matched packages
CI SHALL build `canon-cli` for every matrix target (macOS arm64, Linux x64
— D3), package each into its `@canon/core-<platform>` npm package, and
publish `canon`, `@canon/cli`, and every `@canon/core-<platform>` at one
shared version read from the workspace `Cargo.toml`.

#### Scenario: Tagged release produces matched-version packages
- **WHEN** a release is tagged at workspace version `X.Y.Z`
- **THEN** the publish workflow publishes `canon@X.Y.Z`, `@canon/cli@X.Y.Z`,
  and `@canon/core-<platform>@X.Y.Z` for every CI matrix target in the same
  run, so no consumer can install a launcher and a binary at mismatched
  versions.

#### Scenario: Clean-machine install runs the real Rust binary
- **WHEN** `bunx canon --version` runs on a clean macOS arm64 or Linux x64
  machine with network access to the npm registry and no prior canon
  install
- **THEN** npm/bun resolves the matching `@canon/core-<platform>`
  optionalDependency, the launcher execs the Rust binary, and the printed
  version matches the published package version (this is S0's top-line
  acceptance criterion).

#### Scenario: Missing platform binary fails the workflow, not the consumer
- **WHEN** a CI matrix row fails to build its target binary
- **THEN** the publish workflow SHALL fail closed (no partial publish of the
  JS launcher without its matching binaries) rather than ship a
  `@canon/cli` version with a missing `@canon/core-<platform>` counterpart.
