## MODIFIED Requirements

### Requirement: Published package naming
The launcher SHALL publish to npm as the scoped package
`@journeykit/canon` and each prebuilt platform binary as
`@journeykit/canon-core-<platform>`; the launcher's `bin` SHALL stay
`canon` (bin name independent of package name), so
`bunx @journeykit/canon` runs the platform binary and a global install
exposes `canon`. Every scoped package SHALL publish with public access.
Every package manifest and `Cargo.toml` SHALL name
`github.com/journeyWorker/canon` as the repository.

#### Scenario: Clean-machine install runs the real Rust binary
- **WHEN** `bunx @journeykit/canon --version` runs on a clean macOS
  arm64 or Linux x64 machine with npm-registry access and no prior
  install
- **THEN** the resolver installs the matching
  `@journeykit/canon-core-<platform>` optionalDependency and the
  launcher execs its `canon` binary, printing the published version

#### Scenario: Drift guard holds names coherent
- **WHEN** the release-safety checker runs
- **THEN** it passes only while `publish.yml`'s matrix `package_name`
  values equal the platform manifests' `name` fields and the launcher's
  `optionalDependencies` keys
