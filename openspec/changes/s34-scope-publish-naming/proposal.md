# Why

s33 resolved the npm name to the unscoped single token **`canoncli`**
(launcher) + **`canoncli-core-<platform>`** (platform binaries) because
the bare `canon` was taken and an owned one-token name was wanted. The
operator has since secured the **`@journeykit`** npm scope and wants the
launcher published under it, freeing the name from the awkward `canoncli`
compound and grouping the platform packages under one owned namespace.

The pipeline (build-native / publish / drift-guard) is unchanged and
correct — this change only re-scopes the published names. The GitHub repo
stays `github.com/journeyWorker/canon`; `@journeykit` is the npm scope,
independent of the GitHub org.

# What Changes

- Launcher package `canoncli` → scoped **`@journeykit/canon`** (keeps
  `bin: { canon: … }`, so `bunx @journeykit/canon` is the ephemeral form
  and the installed binary stays `canon`).
- Platform packages `canoncli-core-<platform>` → scoped
  **`@journeykit/canon-core-<platform>`**; launcher resolution updated
  for the scoped `node_modules/@journeykit/` layout (sibling / nested /
  hoisted), monorepo-dev `packages/<dir>` path unchanged.
- `publish.yml` matrix `package_name` + step labels track the new scoped
  names (drift-guard cross-checks them against the manifests; already
  `--access public`).
- Living docs (README, getting-started + examples + architecture +
  index + canon concept, EN/KO, repo-scaffold skill, design-doc
  Distribution line, presentation, site copy) → `bunx @journeykit/canon`
  / `@journeykit/canon-core-*`. Archived openspec changes and vendor
  audits are left as historical record.

# Impact

- Affected specs: `native-launcher`, `native-release-pipeline`
  (naming only — resolution/pipeline shape unchanged).
- Affected code: `packages/cli` (manifest + launcher), both
  `packages/core-*` manifests, `packages/cli` build, `bun.lock`,
  `.github/workflows/publish.yml`, living docs.
- Not affected: Rust crate names (`canon-*`), the CLI binary name
  (`canon`), the GitHub repository URL (`journeyWorker/canon`),
  tier/gate/ingest behavior. Publishing still needs operator actions:
  own the `@journeykit` scope on npm with the trusted publisher /
  token configured, then push a `vX.Y.Z` tag.
