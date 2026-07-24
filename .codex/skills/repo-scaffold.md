# repo-scaffold

> How to extend canon's Rust workspace and Bun packaging scaffold — adding a crate to the Cargo workspace, adding a CI matrix target / platform package, and running the skill materializer. Use when adding a new crates/canon-* member, a new packages/core-<platform> platform package, a new CI matrix row, or running `canon skills install`.

# repo-scaffold

canon's Rust core lives at `crates/canon-*`, its Bun launcher at
`packages/*`. This skill covers the three ways that scaffold grows.

## Add a new crate to the workspace

The root `Cargo.toml`'s `[workspace] members` list is the single audit
point for which crates exist — there is no glob membership
(`rust-bun-workspace` spec: "A new crate cannot silently join without a
name match").

1. Create `crates/canon-<name>/Cargo.toml` and `crates/canon-<name>/src/`.
2. Add `"crates/canon-<name>"` to the root `Cargo.toml`'s `[workspace]
   members` array, in the same change that adds the crate directory.
3. Use `version.workspace = true`, `edition.workspace = true`,
   `license.workspace = true` — never a crate-local version/edition/license
   (design D3's single-version-source discipline extends to every crate).
4. Every library crate ships at minimum one public item and one `#[test]`
   exercising it — `cargo test --workspace` must stay meaningfully green,
   never a crate that compiles but asserts nothing (design D5).
5. Run `cargo test --workspace` and confirm the new crate's test appears in
   the output before committing.

## Add a new CI matrix target / platform package

canon's platform packages follow `@journeykit/canon-core-<platform>` naming
(design D1/D3). S0 ships two: `core-darwin-arm64`, `core-linux-x64`. Adding
a target (e.g. `core-linux-arm64`, `core-darwin-x64`) is a CI matrix row
plus a new package directory — never a redesign of the launcher (design
non-goals).

1. Create `packages/core-<platform>/package.json`: copy an existing
   platform package's shape, set `"os"`/`"cpu"` to the new target's values,
   rename `description`/`directory`.
2. Add a `bin/.gitkeep` (the actual binary is staged by CI at build/publish
   time — never committed; see the root `.gitignore`'s `packages/core-*/
   bin/*` rule).
3. Add the new target as a matrix row in both `.github/workflows/
   build-native.yml` and `.github/workflows/publish.yml`'s `build` job
   (`host`, `target`, `package_dir` — `package_name` too, in `publish.yml`).
4. Add the new platform package to `packages/cli/package.json`'s
   `optionalDependencies` as `"@journeykit/canon-core-<platform>": "workspace:*"`.
5. If the target needs a new libc/arch branch, extend
   `packages/cli/src/index.ts`'s `resolveTargetPackageName` /
   `resolveRustTargetTriple` — both already generalize past S0's two rows
   (design D2), so this is usually a one-line addition, not new logic.
6. Add the platform package to `publish.yml`'s binary-download and
   `npm publish` steps.

## Run `canon skills install` locally

```bash
cargo build --release -p canon-cli
./target/release/canon skills install --source canon/skills --target .
./target/release/canon skills install --source canon/skills-dev --target .
```

`--source` defaults to `canon/skills` (the user-facing companion-skill
tree, materialized into consumer repos) and `--target` defaults to the
current directory. canon's own repo also carries the developer-skill
tree `canon/skills-dev/`, materialized by the second invocation; each
source keeps its own `.install-lock.json`. Run either install twice in
a row with no source change and diff its lock — it must be
byte-identical (skill-materialization spec).
