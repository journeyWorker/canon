# Why

S0 resolved the npm name to the scope `@canon-dev/*` (`@canon-dev/cli`
+ `@canon-dev/core-<platform>`) because the bare `canon` was taken.
Two facts have since changed that decision's basis:

1. The repo now lives at `github.com/journeyWorker/canon`, not the
   `canon-dev` org the manifests' `repository.url` still pointed at —
   the scope named an org nobody owns here (drift).
2. The operator wants the friendly one-token `bunx <name>` UX back.
   `canon` is still taken (npm v0.4.1) and so is `canonkit` (v0.2.0);
   the available single-token name chosen is **`canoncli`**.

The pipeline itself (build-native / publish / drift-guard) was already
complete and correct — it never ran only because it had no owned name,
no `NPM_TOKEN`, and no release tag. This change fixes the name; the
token + tag are operator actions.

# What Changes

- Launcher package `@canon-dev/cli` → unscoped **`canoncli`** (keeps
  `bin: { canon: … }`, so `bunx canoncli` is the ephemeral form and
  the installed binary stays `canon`).
- Platform packages `@canon-dev/core-<platform>` → unscoped
  **`canoncli-core-<platform>`**; launcher resolution updated for the
  flat (unscoped) `node_modules` layout, monorepo-dev `packages/<dir>`
  path unchanged.
- `repository.url` in all three manifests + `Cargo.toml` →
  `journeyWorker/canon`.
- `publish.yml` matrix `package_name` + step labels track the new
  names (drift-guard cross-checks them against the manifests).
- Living docs (README, getting-started EN/KR, repo-scaffold skill,
  design-doc Distribution line, presentation, site copy) →
  `bunx canoncli`. Archived openspec changes are
  left as historical record.

# Impact

- Affected specs: `native-launcher`, `native-release-pipeline`
  (naming only — resolution/pipeline shape unchanged).
- Affected code: `packages/cli` (manifest + launcher), both
  `packages/core-*` manifests, `packages/cli` build, `bun.lock`,
  `.github/workflows/publish.yml`, `Cargo.toml`, living docs.
- Not affected: Rust crate names (`canon-*`), the CLI binary name
  (`canon`), tier/gate/ingest behavior. Publishing still needs two
  operator actions: set `NPM_TOKEN`, push a `vX.Y.Z` tag.
