## Context

canon is a new standalone repo (design decision 1)
distributed as a Rust core + Bun launcher, `bunx canon` from any consumer repo
(the donor monorepo, the donor parity harness, the donor vocabulary project, …),
following the vendored upstream launcher's packaging pattern exactly
(design decision 2). The vendored upstream launcher was vendored via
`git subtree --squash` at repo creation (decision 3) — this change consumes the
packaging pattern as reference, not by importing from the vendored sources
(clean-room posture).

Every later spec change (S1–S12, design §5/§6) assumes: (a) a crate exists to
put its code in, (b) `bunx canon` already resolves to a real binary, (c)
`canon/skills/` is the place a spec's companion skill lands (design decision
9 — "a spec's openspec change is not done until its skill ships").

## Goals / Non-Goals

**Goals:**
- A `cargo test --workspace` — green, empty-but-structured — Rust workspace
  with the seven crate boundaries the architecture diagram (design §4)
  names: `canon-model`, `canon-store`, `canon-ingest`, `canon-gate`,
  `canon-learn`, `canon-report`, `canon-cli`.
- A working `bunx canon --version` on a clean macOS arm64 or Linux x64
  machine, resolving through the alias → launcher → platform-binary chain.
- CI that builds and publishes that chain with matched versions.
- A skill-materialization contract (`canon/skills/` SoT → `canon skills
  install` → `.claude/` + `.codex/`) that every later spec's change can rely
  on without redefining it.
- The npm `canon` name-collision question (§10 Q4) resolved, not deferred.

**Non-Goals:**
- Any real behavior inside `canon-model`/`canon-store`/`canon-ingest`/
  `canon-gate`/`canon-learn`/`canon-report` — those crates get their
  first real types/logic in S1 (canon-model) and S2 (canon-store) onward.
  S0 ships them as compiling stubs (one public marker function + one test)
  so the workspace boundary and CI wiring exist before behavior does.
- Windows / musl targets. The vendored upstream launcher ships 8 platform packages; S0's
  acceptance bar is macOS arm64 + Linux x64 only. The packaging shape
  (`@canon/core-<platform>` naming, optionalDependencies) generalizes to more
  targets later without a structural change — adding a target is a CI matrix
  row + a new `packages/core-<platform>/`, not a redesign.
- Running the vendored upstream launcher's vendor-audit survey/audit/synthesize phases. A
  parallel research workflow owns that audit;
  this change only requires that cursor to have reached `survey: done` by
  the time S0 is applied (its own acceptance bar), and must not re-do that
  agent's work.
- Building `canon skills install`'s domain-specific rendering (per-domain
  handoff templates, S1; typed vocabulary, S10). S0 ships the install/
  materialize mechanics (locking, file placement) generically; content comes
  from each spec's own skill.

## Decisions

**D1 — Three-level npm chain (`canon` → `@canon/cli` → `@canon/core-<platform>`),
not the vendored upstream launcher's two-level chain (scoped `@<launcher>/cli` →
`@<launcher>/cli-<platform>`).** The vendored upstream launcher ties its friendly
bin name to a scoped package (its `@<launcher>/cli`'s `bin` entry) — there is no
unscoped alias to lose if the scoped name collides. canon's decision 2 explicitly chains
through an unscoped alias so a `canon` name collision (§10 Q4) only costs the
friendly `bunx canon` entrypoint, never the scoped implementation consumers
can still reach via `bunx @canon/cli`. `packages/canon/package.json`: `name:
"canon"`, `bin: { canon: "./bin.js" }`, `dependencies: { "@canon/cli": <ver>
}`, `bin.js` re-execs `require.resolve("@canon/cli/bin.js")`.
Alternative considered: skip the alias, publish `@canon/cli` only and
document `bunx @canon/cli`. Rejected — `bunx canon` is the whole point of
decision 2's "distributed like the vendored upstream launcher" framing and every design-doc example
invocation (`canon fmt`, `canon gate task`, …) assumes the bare name.

**D2 — `packages/cli` (`@canon/cli`) is a line-for-line adaptation of
the vendored upstream launcher's platform-detection +
binary-resolution logic**, not a fresh implementation: `resolveTargetPackageName`
(platform/arch → `@canon/core-<platform>` name), libc-kind detection on Linux
(gnu/musl loader probing) generalizes even though S0 only ships gnu-glibc
Linux, self-reference guarding (realpath comparison against `process.argv[1]`
to prevent the wrapper from re-invoking itself — the fork-bomb guard), and
the search-path order (workspace `target/<triple>/release/`, workspace
`target/release/`, package `bin/`, then the resolved `optionalDependency`).
Rationale: that logic is dense, adversarially-tested (it ships with its own
package-launcher test script); re-deriving it risks reintroducing bugs the
vendored upstream launcher already fixed. Binary name: `canon` (`canon.exe`
on win32, unreachable until a Windows target ships).

**D3 — CI matrix mirrors `build-native.yml`'s shape (target-triple matrix →
`cross`/native `cargo build --release` → package `@canon/core-<platform>` →
upload artifact) scoped to two rows**: `aarch64-apple-darwin` (macOS arm64)
and `x86_64-unknown-linux-gnu` (Linux x64) — exactly S0's acceptance bar.
Publish workflow version-locks all three package kinds (`canon`, `@canon/cli`,
each `@canon/core-<platform>`) to one `workspace.package.version` read from
the root `Cargo.toml`, so `bunx canon@X` always pulls binaries built at Rust
version X (the vendored upstream launcher keeps its four version numbers — root
Cargo.toml, cli package.json, cli-<platform> package.jsons — in sync the same way; canon
generalizes it to a single source read at publish time rather than four
independently bumped files).

**D4 — Skill materialization borrows the donor monorepo's agent-manifest
content-hash-lock shape (
`$generatedBy`/`$kind`/`$manifestVersion` fields, no `generatedAt`), not its
per-agent-kind manifest schema.** canon's unit is a *skill*, not an agent
kind: `canon/skills/<name>/SKILL.md` is the SoT (mirrors `.claude/skills/
<name>/SKILL.md`'s existing shape so `canon skills install` output is
byte-for-byte a valid Claude Code skill). `canon skills install` writes:
(a) `.claude/skills/<name>/SKILL.md` — copied verbatim; (b)
`.codex/skills/<name>.md` — a canon-defined flattening (Codex has no native
skill-directory concept today, unlike its `.codex/agents/<kind>.toml`
convention; canon establishes `.codex/skills/<name>.md` as its own
convention and is the format authority for it per decision 4); (c) a lock
file `canon/skills/.install-lock.json` keyed by `{name, contentHash,
version}` per skill — content hash changes bump `version`; identical content
reruns are a no-op diff. Gemini is dropped per decision 11 (Claude Code +
Codex only).

**D5 — Crate boundary is fixed now even though only `canon-cli` compiles to
anything runnable in S0.** Each of the six library crates
(`canon-model`/`store`/`ingest`/`gate`/`learn`/`report`) exports one public
marker item (e.g. `pub const CRATE: &str = "canon-model";`) and one
`#[test]` asserting it, so `cargo test --workspace` is meaningfully green
(catches workspace-wiring breakage) without inventing behavior S1+ owns.
`canon-cli` is the one crate that actually runs: a `clap`-based binary
whose only command in S0 is `canon --version` (prints the workspace
version), the literal acceptance-criterion surface.

**D6 — npm name-collision check happens before any package.json is
written**, not after. §10 Q4 is a go/no-go for every subsequent package name
in this change and every later spec's `@canon/*` package. Outcome recorded
in this change's tasks.md (task 1.1) and, if `canon` is unavailable, the
fallback scope propagates to every `packages/*/package.json` `name` field
authored in this change — there is no "publish under `canon`, rename later"
path, since renaming an already-referenced npm scope after S1+ specs start
depending on `@canon/cli` imports is exactly the churn decision 2's alias
layer (D1) exists to avoid.

## Risks / Trade-offs

- [Risk] Two-row CI matrix (macOS arm64 + Linux x64 only) under-covers
  contributors on Linux musl or Windows. → Mitigation: D3's packaging shape
  (`@canon/core-<platform>` naming + optionalDependencies) adds a target by
  adding a matrix row and a `packages/core-<platform>/`, not a redesign —
  explicitly scoped as a non-goal, not a permanent limitation.
- [Risk] `.codex/skills/<name>.md` is a canon-invented convention with no
  existing Codex-side consumer to validate against (unlike `.claude/skills/`
  which Claude Code already reads). → Mitigation: canon is the declared
  format authority (decision 4); the companion-skill task for every spec
  (including this one, task group 4) exercises the materializer against a
  real skill file, and `canon selftest` (S5/§8) will fixture-check both
  output shapes going forward.
- [Risk] Stub crates with marker-only tests give false confidence that
  `cargo test --workspace` "passing" means something once S1 lands real
  types. → Mitigation: none needed structurally — S1's design explicitly
  replaces `canon-model`'s stub with real fixture round-trip tests (this
  change's design is explicit that D5's marker tests are wiring checks, not
  behavior checks).
- [Risk] A version-based lock (D4) is only actually timestamp-free if the
  artifact it hashes never embeds a wall-clock field itself — the donor
  monorepo's own agent-manifest package demonstrates the failure mode directly
  and is the root cause design decision 11's "timestamp-free" requirement is
  written against: its registry builder embeds
  `generatedAt: new Date().toISOString()` INSIDE the `Registry` object
  serialized to `_registry.json`, and its materializer computes its
  own change-detection `registrySnapshotSha` by hashing those raw file
  bytes (not a canonicalized semantic subset) — so every `materialize` run
  produces a different hash purely from the registry's own re-stamped
  timestamp, even when zero skills/MCP servers changed, poisoning every
  agent kind's lock every run (empirically confirmed live: the committed
  `.agent-lock.json`'s `registrySnapshotSha` is already stale against the
  current `_registry.json`). → Mitigation: `canon skills install`'s D4
  lock computation MUST hash a canonicalized projection of only semantic
  fields (sorted skill/MCP lists) — never the raw bytes of an artifact
  that embeds its own generation time — before computing `contentHash`;
  generalizes to any canon hash computed over a generated artifact for
  idempotence/change-detection (S2 digest-based aging, S11 `canon fmt`/
  `migrate` byte-stability).
- [Trade-off] Version-locking all npm packages to `Cargo.toml`'s single
  version (D3) means a JS-only launcher fix still bumps the whole chain's
  version, even with no Rust change. Accepted: matches the vendored upstream launcher's existing
  practice and keeps `bunx canon@X` unambiguous about which binary it runs.

## Migration Plan

N/A — first change in a new repo; nothing to migrate from. Rollback is
`git revert` of this change's commit(s); no external consumers exist yet
(no repo depends on `bunx canon` before this change ships).

## Open Questions

- §10 Q4 (npm name collision) is resolved BY this change (task 1.1), not
  left open past it — recorded here as a design input, not a deferred
  question, because every other decision in this document (D1, D6) is
  conditioned on its answer.
