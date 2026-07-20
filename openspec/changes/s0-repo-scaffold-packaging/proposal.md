## Why

canon needs a buildable, publishable skeleton before any of its crates can carry
real behavior. Today the repo has a Bun workspace root
(`package.json`) and an `openspec/` tree but no `crates/`, no `packages/cli`,
no CI, and no shipped-skill mechanism. Every downstream spec (S1–S12) assumes
the crate layout, the `bunx canon` launcher, and the `canon skills install`
materializer already exist. S0 builds that floor.

## What Changes

- Add the Rust workspace: `crates/canon-model`, `canon-store`, `canon-ingest`,
  `canon-gate`, `canon-learn`, `canon-report`, `canon-cli` (root `Cargo.toml`
  with `[workspace]` members; each crate ships a compiling stub + one smoke
  test so `cargo test --workspace` is green from commit one).
- Add the Bun packaging workspace lifted from the vendored upstream launcher's
  pattern: `packages/canon` (thin npm alias, bin
  `canon`), `packages/cli` (`@canon/cli`, the real JS launcher — platform
  detect, binary resolution, argv/exit-code passthrough), and one
  `packages/core-<platform>` per target triple (`@canon/core-<platform>`,
  prebuilt binary only), wired as `@canon/cli`'s `optionalDependencies`.
- Add CI: a `build-native.yml`-analog matrix workflow (macOS arm64 + Linux
  x64 minimum, per this change's acceptance bar) building `canon-cli` and
  packaging each `@canon/core-<platform>`, plus a publish workflow that
  version-locks the alias, launcher, and platform packages together.
- Resolve open question §10 Q4 (npm name collision): check `canon` on the npm
  registry before publishing; record the outcome and, if taken, the fallback
  scope (`@canon-dev/*` or `fugue`) this change adopts.
- Scaffold `canon/skills/` as the single source of truth for every spec's
  companion skill (decision 9) plus the `canon skills install` materializer
  contract: content-hash + monotonic-version lock (never `generatedAt`,
  decision 11), targeting `.claude/skills/<name>/SKILL.md` and
  `.codex/skills/<name>.md` only (gemini dropped).
- Confirm the openspec scaffolding already present under `openspec/` (this
  change's own home) is complete and self-consistent for canon's own future
  specs, and track — without re-running — the vendored upstream launcher's
  vendor-audit lifecycle reaching at least
  `survey: done`, per this change's acceptance bar.

## Capabilities

### New Capabilities

- `native-launcher`: the `canon`/`@canon/cli` JS launcher resolves and execs
  the correct `@canon/core-<platform>` binary (or a local dev build), forwards
  argv/exit code, and fails with an actionable error on unsupported platforms.
- `rust-bun-workspace`: the `crates/canon-*` Cargo workspace and
  `packages/*` Bun workspace exist, build, and test green on a clean
  checkout, establishing the crate boundaries every later spec (S1–S12) fills
  in.
- `native-release-pipeline`: CI builds and publishes versioned,
  platform-matched `@canon/core-<platform>` binaries alongside the `@canon/cli`
  launcher and the `canon` alias package.
- `skill-materialization`: `canon/skills/` is the SoT for every spec's
  companion skill; `canon skills install` materializes it into consumer repos'
  `.claude/` and `.codex/` trees with a timestamp-free, content-hash +
  version lock.

### Modified Capabilities

_None — this is the first change in the repo; nothing existing to modify._

## Impact

- New Rust workspace root `Cargo.toml` + 7 crates under `crates/`.
- New Bun packages under `packages/` (`canon`, `cli`, one `core-<platform>`
  per CI target), extending the existing root `package.json` `workspaces`
  glob (`packages/*` already matches).
- New `.github/workflows/build-native.yml` + a publish workflow.
- New `canon/skills/` directory + materializer contract consumed by every
  later spec's own openspec change (each ships its skill under this SoT).
- No impact on the vendored upstream launcher's sources (read-only reference) or
  its vendor-audit tree beyond observing its phase cursor.
