## ADDED Requirements

### Requirement: Canonical crate boundaries exist and build clean
The repo SHALL provide a Cargo workspace at `crates/` with exactly the seven
member crates named in the architecture (design §4): `canon-model`,
`canon-store`, `canon-ingest`, `canon-gate`, `canon-learn`, `canon-report`,
`canon-cli`, and a Bun workspace at `packages/` matching the root
`package.json`'s `workspaces: ["packages/*"]` glob.

#### Scenario: Fresh checkout builds and tests green
- **WHEN** `cargo test --workspace` runs on a clean clone with no prior
  `target/` directory
- **THEN** every one of the seven crates compiles and its smoke test passes,
  with zero crate implementing S1+ business logic (each of the six library
  crates exposes only a marker constant + one assertion test; `canon-cli`
  is the sole crate with a real, runnable `--version` command).

#### Scenario: Bun workspace resolves internal packages
- **WHEN** `bun install` runs at the repo root
- **THEN** `packages/canon`, `packages/cli`, and every `packages/core-<platform>`
  under the CI matrix (D3) are discovered as workspace members and any
  intra-workspace dependency (`@canon/cli` depending on `@canon/core-<platform>`
  as an optionalDependency) resolves without a registry round-trip for
  local development.

#### Scenario: A new crate cannot silently join without a name match
- **WHEN** a future change adds a crate to `crates/` that is not one of the
  seven named above
- **THEN** the root `Cargo.toml` workspace member list must be updated in
  the same change (no implicit glob membership), keeping the crate roster
  auditable from one file.
