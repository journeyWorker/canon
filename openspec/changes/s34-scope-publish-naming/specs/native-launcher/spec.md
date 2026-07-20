## MODIFIED Requirements

### Requirement: Platform binary resolution under the scoped name
The launcher SHALL resolve the platform binary from the scoped
`@journeykit/canon-core-<platform>` package in every `node_modules`
layout a package manager may use (sibling under
`node_modules/@journeykit/`, nested, hoisted), and from the monorepo
`packages/core-<platform>/bin` staging path during development,
preferring a workspace `target/**/release` build first. An unsupported
platform SHALL fail with an actionable error naming the expected
`@journeykit/canon-core-<platform>` package.

#### Scenario: Supported platform runs the packaged binary
- **WHEN** `bunx @journeykit/canon --version` runs on macOS arm64 or
  Linux x64 with `@journeykit/canon-core-<platform>` installed as a
  resolved optionalDependency
- **THEN** the launcher execs that platform's `canon` binary, passes
  stdout/stderr/exit code through unmodified

#### Scenario: Unsupported platform fails with an actionable error
- **WHEN** the launcher finds no binary for the current platform/arch
- **THEN** it prints the expected `@journeykit/canon-core-<platform>`
  package name and the build-from-source hint, exiting non-zero
