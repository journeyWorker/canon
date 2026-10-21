# Surface — m-meta

**Vendor paths**: `vendors/mattpocock-skills/docs/`, `vendors/mattpocock-skills/GLOSSARY.md`, `vendors/mattpocock-skills/SCOPE.md`, `vendors/mattpocock-skills/.out-of-scope/`, `vendors/mattpocock-skills/.changeset/`, `vendors/mattpocock-skills/scripts/`, `vendors/mattpocock-skills/_vendor_agents/`, `vendors/mattpocock-skills/_vendor_claude-plugin/`, `vendors/mattpocock-skills/_vendor_CLAUDE.md`, `vendors/mattpocock-skills/package.json`, `vendors/mattpocock-skills/CHANGELOG.md`

**Scale**: ~60 files in the audited surface, including 9 out-of-scope records, 16 changeset entries, distribution metadata, scripts, documentation, and release history.

**Primary language(s)**: Markdown, Bash, JSON, JavaScript.

**Canon analog (if any)**: `crates/canon-cli/src/skills.rs` — projects the canonical skill to selected harnesses, records an install lock, and checks drift; `crates/canon-cli/src/deprecation.rs` — maintains a release-bounded deprecation table.

## 1. Overview

This surface defines the repository’s maintenance policy rather than an individual agent skill. `SCOPE.md` establishes an observed-failure and philosophy bar for issues, while `.out-of-scope/` stores durable decisions for rejected concepts. `GLOSSARY.md` provides canonical domain vocabulary, and `docs/agents/triage-labels.md` maps those roles to tracker labels. Distribution is split between managed plugin marketplaces and editable `skills.sh` installs, with `_vendor_agents/install-block.md` serving as the canonical installation contract. Release metadata is maintained through Changesets, `package.json`, plugin manifests, a version-sync script, and the generated `CHANGELOG.md`; retired skills are removed rather than preserved as a compatibility bucket.

## 2. Entry points & architecture

- **Entry file(s)**:
  - `vendors/mattpocock-skills/SCOPE.md:1-35` — issue admission bar, bucket policy, prior rejection lookup, and unresolved-issue timeout.
  - `vendors/mattpocock-skills/GLOSSARY.md:1-28` — canonical terms, relationships, and forbidden synonyms.
  - `vendors/mattpocock-skills/_vendor_agents/install-block.md:1-91` — one installation/update contract propagated to README, Changesets, and docs.
  - `vendors/mattpocock-skills/package.json:1-24` — package version and Changesets release scripts.
  - `vendors/mattpocock-skills/scripts/sync-plugin-version.mjs:1-44` — synchronizes package and plugin versions or checks them read-only.
  - `vendors/mattpocock-skills/scripts/link-skills.sh:1-66` — maintainer-only symlink installation for Claude and Agent Skills-compatible harnesses.
  - `vendors/mattpocock-skills/_vendor_claude-plugin/plugin.json:1-48` — explicit promoted-skill manifest and plugin version.
- **Core abstractions**:
  - **Scope bar** — two-part predicate requiring both a real observed failure and philosophical fit before an issue remains open (`SCOPE.md:7-17`).
  - **Out-of-scope record** — one rejected concept per Markdown file, with rationale and prior requests (`.out-of-scope/new-skills.md:1-24`).
  - **Canonical vocabulary** — terms such as Issue tracker, Issue, Decision ticket, and Triage role, with relationships and “avoid” synonyms (`GLOSSARY.md:3-28`).
  - **Canonical install block** — one prose-and-command source for managed and manual installation routes (`_vendor_agents/install-block.md:1-91`).
  - **Promotion manifest** — explicit `skills` array defining which skills ship in the plugin (`_vendor_claude-plugin/plugin.json:17-46`).
  - **Changeset** — package-scoped SemVer intent plus release-note prose (`.changeset/implement-ticket-reference.md:1-7`).
  - **Deprecated bucket policy** — retired skills are deleted, with the removal Changeset naming the replacement (`skills/deprecated/README.md:1-4`).
- **Cross-package dependencies**:
  - The surface governs the repository’s `skills/` buckets and each skill’s `agents/openai.yaml`.
  - `docs/agents/triage-labels.md` is consumed by the `triage`, `to-spec`, `to-tickets`, and `wayfinder` skills.
  - `_vendor_agents/install-block.md` is propagated into `README.md`, `.changeset/*`, and `docs/`.
  - `_vendor_claude-plugin/plugin.json` is synchronized from `package.json`.
- **External runtime dependencies**:
  - `@changesets/cli` and `@changesets/changelog-github` (`package.json:11-16`).
  - npm 10.9.4 as the declared package manager (`package.json:17`).
  - Bash, `find`, symlinks, and the user’s home-directory harness locations for the maintainer-only linker (`scripts/link-skills.sh:1-17`).
  - Claude Code, Codex, Copilot, VS Code, Gemini CLI, and `skills.sh` installation ecosystems (`_vendor_agents/install-block.md:7-63`).

## 3. Pattern catalog

### 3.1. Two-part scope gate with durable rejection records

- **Where**: `vendors/mattpocock-skills/SCOPE.md:7-35`; `vendors/mattpocock-skills/.out-of-scope/new-skills.md:1-24`; `vendors/mattpocock-skills/.out-of-scope/harness-name-collisions.md:1-27`
- **What it does**: An issue remains open only when it reports an observed failure from a real session and fits the repository’s philosophy. The second predicate explicitly excludes hypothetical improvements, harness-specific branches, personal workflow tweaks, and concepts already rejected. Rejections are not only tracker states: each `.out-of-scope/*.md` file records the concept, rationale, and prior requests so later triage can detect and quote the earlier decision.
- **Why it's interesting**: Canon has a rigorous brief and ship gate, but it does not have an equivalent durable policy layer for deciding whether proposed scope should enter the system. This pattern separates “not yet specified” from “intentionally not accepted” and makes repeated proposals cheap to reject consistently.
- **Canon applicability**: **Partial.** Canon has scoped briefs and an explicit human approval boundary in `canon/skills/SKILL.src.md:16-31`, and it records blocker/should-fix/note findings in `crates/canon-cli/src/finding.rs:329-332`. It lacks a first-class repository-local out-of-scope decision record and a pre-intake policy check for proposals. A possible target is `canon/skills/reference/canon-subject.md` plus a future `.canon/policy/` or `.canon/decisions/` record type; existing `canon finding add` is not a substitute because findings describe defects in an adopted change.
- **Effort to lift**: moderate.

### 3.2. Canonical glossary with relationship constraints and forbidden synonyms

- **Where**: `vendors/mattpocock-skills/GLOSSARY.md:3-28`; `vendors/mattpocock-skills/docs/agents/triage-labels.md:3-15`; `vendors/mattpocock-skills/skills/engineering/setup-matt-pocock-skills/SKILL.md:22-36`
- **What it does**: The glossary defines terms as domain objects, gives explicit relationships, and states which words must not be used. “Issue tracker” is the host system, “Issue” is the tracked unit, “Decision ticket” is a qualified subtype, and “Triage role” is a state-machine label. Tracker-specific labels are kept in a separate mapping document, while skills consume the canonical role names. Setup writes repo-local mappings without editing the shared skills.
- **Why it's interesting**: Canon’s command and record vocabulary spans briefs, changes, subjects, scenarios, evidence, reviews, findings, and divergences. A small canonical vocabulary with synonym prohibitions could reduce agent confusion across CLI help, skill text, schemas, and release notes, while keeping tracker/provider-specific names in a mapping layer.
- **Canon applicability**: **Partial.** Canon already has a typed vocabulary and schema guidance in `canon/skills/reference/canon-vocab.md` and `canon/skills/reference/canon-context.md:141-142`, and its 0.24 release standardized command names (`README.md:109-141`). However, it does not appear to maintain one concise human-facing glossary with explicit “avoid” terms and relationship rules. The concrete target is `canon/skills/reference/canon-vocab.md`, with generated or checked alignment against `crates/canon-cli/src/schema.rs` and `crates/canon-cli/src/deprecation.rs`.
- **Effort to lift**: trivial to moderate.

### 3.3. One install contract across managed and editable distribution routes

- **Where**: `vendors/mattpocock-skills/_vendor_agents/install-block.md:1-29`; `vendors/mattpocock-skills/_vendor_agents/install-block.md:31-91`; `vendors/mattpocock-skills/README.md:22-75`; `vendors/mattpocock-skills/scripts/link-skills.sh:1-18`
- **What it does**: The repository keeps one canonical installation narrative and requires README, Changesets, and docs to repeat it. It distinguishes managed, self-updating plugin routes from editable manual installs, states update semantics per harness, and explicitly tells users to choose one route to avoid duplicate installations. The maintainer-only linker intentionally excludes `deprecated/` and `misc/`, includes `in-progress/`, and symlinks the selected skill directories into Claude and Agent Skills-compatible locations.
- **Why it's interesting**: Distribution behavior is part of the product contract for agent skills. The explicit promoted set, route-specific update behavior, and separation between read-only managed bundles and editable copies prevent users from mistaking a stale projection for the source.
- **Canon applicability**: **Has it, with a narrower scope.** Canon already materializes its bundle to Claude, Codex, OMP, and Pi, selects providers from explicit flags/config/lock/detection, and records the source/version in `.canon/skills/.install-lock.json` (`crates/canon-cli/src/skills.rs:1-32`, `crates/canon-cli/src/skills.rs:67-101`). `canon skills install`, `check`, and `doctor` are exposed in `crates/canon-cli/src/main.rs:1637-1680`. The missing opportunity is mainly the vendor’s single-source installation prose and explicit managed-vs-editable route policy, not the projection mechanism itself.
- **Effort to lift**: trivial.

### 3.4. Versioned promotion manifest with synchronized package/plugin versions

- **Where**: `vendors/mattpocock-skills/package.json:1-24`; `vendors/mattpocock-skills/_vendor_claude-plugin/plugin.json:1-48`; `vendors/mattpocock-skills/scripts/sync-plugin-version.mjs:1-44`; `vendors/mattpocock-skills/.changeset/config.json:1-14`
- **What it does**: `package.json` is the version source, while `plugin.json` carries the same version and an explicit list of promoted skills. The release script runs Changesets and then synchronizes the plugin manifest. The sync script has a read-only `--check` mode that exits nonzero when versions diverge, and its write path changes only the version field. Changesets are package-scoped and feed GitHub-linked changelog entries.
- **Why it's interesting**: Canon already has a source bundle, provider projections, and install-lock versioning, but the lock describes the installed canon bundle rather than a separately versioned promoted subset. A manifest-plus-check pattern would make the exact shipped skill surface and release compatibility independently auditable.
- **Canon applicability**: **Partial.** Canon’s install lock records the canon version and source checkout `HEAD`, and `skills check` reports locks from another canon version (`crates/canon-cli/src/skills.rs:16-20`, `crates/canon-cli/src/skills.rs:260-281`). It does not have a comparable explicit promoted-bundle manifest or a README-facing release check that verifies all distribution metadata agree. Concrete targets are `crates/canon-cli/src/skills.rs`, the generated provider projections, and the release/conformance process documented in `README.md:85-141`.
- **Effort to lift**: moderate.

### 3.5. Changesets as release-intent records, with changelog materialization

- **Where**: `vendors/mattpocock-skills/.changeset/implement-ticket-reference.md:1-7`; `vendors/mattpocock-skills/.changeset/wizard-template-fixes.md:1-7`; `vendors/mattpocock-skills/.changeset/config.json:1-14`; `vendors/mattpocock-skills/CHANGELOG.md:1-27`
- **What it does**: Each Changeset records the affected package, SemVer class, and release-note text alongside the code change. The generated changelog groups entries by version and change class and preserves pull request, commit, and contributor references. The release command versions the package, updates the changelog, and synchronizes plugin metadata.
- **Why it's interesting**: Canon’s README release notes are detailed and useful, but release intent is currently represented in plans and prose rather than as a small per-change artifact that can be checked before release. A Changeset-like record could separate “this change affects release semantics” from the implementation plan and make release-note completeness machine-checkable.
- **Canon applicability**: **Partial.** Canon has a strong release-note practice and conformance/release packet expectations, including naming changed contracts in `README.md:85-141` and release acceptance criteria in `docs/superpowers/plans/2026-10-09-canon-0.12.0-conformance.md:91-120`. It does not have a dedicated release-intent record or automated aggregation of per-change release notes. A concrete target would be `docs/superpowers/plans/` plus a new repository-local `.canon/changesets/` convention, not the scenario/evidence ledger.
- **Effort to lift**: moderate.

### 3.6. Explicit retirement policy: delete deprecated skills and name the replacement

- **Where**: `vendors/mattpocock-skills/skills/deprecated/README.md:1-4`; `vendors/mattpocock-skills/CHANGELOG.md:78-112`; `vendors/mattpocock-skills/.out-of-scope/frozen-misc-skills.md:1-24`
- **What it does**: The deprecated bucket is intentionally empty: a retired skill is deleted, and the Changeset that removes it must identify the replacement. The changelog records removals, promotions, renames, and breaking changes as release entries rather than leaving an indefinitely supported alias. Separately, `misc/` is frozen and unmaintained, with issues closed and users directed to editable copies.
- **Why it's interesting**: This gives deprecation a lifecycle endpoint instead of treating it as permanent compatibility debt. It also distinguishes “retired and removed” from “present but frozen,” which is useful when agents need to know whether a path is authoritative, supported, or merely preserved for historical/reference reasons.
- **Canon applicability**: **Has a command-level analog; missing a bundle/documentation-level policy.** Canon has a centralized, release-bounded deprecation table: old spellings run for one release, emit warnings and JSON metadata, are listed by schema, and become usage errors when the removal version arrives (`crates/canon-cli/src/deprecation.rs:1-16`, `crates/canon-cli/src/deprecation.rs:38-54`, `crates/canon-cli/src/deprecation.rs:207-214`). It lacks an equally explicit policy for retiring skill files, references, provider projections, or docs. A target is `canon/skills/SKILL.src.md` plus `crates/canon-cli/src/skills.rs` and release notes, with every removed projection and replacement named.
- **Effort to lift**: moderate.

### 3.7. Triage labels as a mapped state vocabulary, not a hard-coded tracker implementation

- **Where**: `vendors/mattpocock-skills/docs/agents/triage-labels.md:3-15`; `vendors/mattpocock-skills/SCOPE.md:31-35`; `vendors/mattpocock-skills/skills/engineering/triage/SKILL.md:21-37`
- **What it does**: Skills use five canonical roles while a repo-local mapping translates those roles to actual tracker label strings. The mapping is committed as configuration and does not create labels or mutate the tracker. Triage checks scope before assigning a state; incomplete issues receive `needs-info`, and rejected issues close as “not planned.” The repository’s documented label set is `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`.
- **Why it's interesting**: This is a clean separation between semantic state and provider-specific representation. It also provides a useful comparison for the requested “watch/next” analog: the vendor uses explicit tracker states, while canon uses `status` and `meta.next` guidance rather than issue labels.
- **Canon applicability**: **Partial, with a mismatch in lifecycle semantics.** Canon has role-aware `status`/`meta.next`, where every step names an actor and role and waiting work is counted (`crates/canon-cli/src/status.rs:24-35`), and findings have `blocker`, `should-fix`, and `note` severities (`crates/canon-cli/src/finding.rs:329-332`). It does not have mapped issue labels, nor a “watch/next” label pair. The concrete target is `canon/skills/reference/canon-subject.md` and `canon/skills/reference/canon-review.md`: preserve `meta.next` as actionable guidance, but consider a documented mapping layer only for external tracker integrations.
- **Effort to lift**: moderate.

## 4. Risks & incompatibilities

- **Effect version**: Not applicable. This surface is Markdown, Bash, JSON, and Node.js rather than an Effect application.
- **Framework**: Not applicable. No UI framework is involved.
- **Heavy deps**: `@changesets/cli` and `@changesets/changelog-github` are release-time development dependencies (`package.json:11-16`), not runtime skill dependencies. The linker depends on shell utilities and writes to user-level harness directories, so it should not be copied into canon as a default installer model.
- **Coupling**:
  - The installation block is coupled to the repository’s supported harness marketplace behavior and must be updated as those ecosystems change.
  - `plugin.json`, `package.json`, Changesets, and `CHANGELOG.md` form one release pipeline; changing one without the others creates distribution drift.
  - `triage-labels.md` is coupled to the tracker configuration generated by `setup-matt-pocock-skills`, but intentionally keeps shared skills tracker-neutral.
  - `.out-of-scope/` is coupled to the triage process: its value depends on agents checking it before recommending a disposition.
- **TOS-violation patterns**: None observed in the audited surface. It contains no provider impersonation, forged request fingerprinting, or rate-limit evasion.
- **License footnote**: The audited package metadata declares MIT (`package.json:9`); canon adoption should remain a clean-room reimplementation from the mechanisms above rather than a verbatim lift.

## 5. Recommended actions

1. **EXTRACT** the two-part scope gate and one-file rejection records → canon `canon/skills/reference/canon-subject.md` and a new repository-local scope-decision record convention — canon needs a durable distinction between “not yet approved,” “out of scope,” and “rejected after review,” rather than representing all non-work as absent.
2. **EXTRACT** the canonical glossary’s relationship and “avoid” vocabulary → `canon/skills/reference/canon-vocab.md` — consolidate CLI/skill terminology and check it against `crates/canon-cli/src/schema.rs`.
3. **INSPIRE** the mapped triage-label layer for external tracker adapters → canon integration documentation around `canon subject`, `canon finding`, and `canon status` — keep semantic canon states stable while allowing tracker-specific labels to be configured separately.
4. **SKIP** the maintainer-only `scripts/link-skills.sh` symlink installer → no canon target — canon already has a safer provider-aware projection, lock, drift check, and symlink protections in `crates/canon-cli/src/skills.rs`.
5. **EXTRACT** the explicit promoted-skill manifest and synchronized package/plugin version check → `crates/canon-cli/src/skills.rs` and the release/conformance workflow — make the exact shipped bundle and its version independently checkable, rather than relying only on projected-file hashes.
6. **INSPIRE** Changesets as small release-intent records → `docs/superpowers/plans/` or a future `.canon/changesets/` convention — allow release automation to aggregate SemVer intent and changelog prose while leaving scenario evidence as the behavioral proof.
7. **EXTRACT** the retirement lifecycle “remove the skill and name the replacement” → `canon/skills/SKILL.src.md`, `crates/canon-cli/src/skills.rs`, and README release notes — extend canon’s existing command deprecation model to skill/reference/provider-surface retirement.
8. **SKIP** permanent compatibility aliases or an indefinitely populated `deprecated/` skill bucket → no canon target — canon’s existing release-bounded deprecation table already provides a cleaner one-release migration path (`crates/canon-cli/src/deprecation.rs:1-16`).
9. **INSPIRE** the distinction between supported, frozen, and retired surfaces → canon skills documentation and `canon skills doctor` diagnostics — report stale, legacy, frozen, or unsupported projections distinctly instead of treating every non-current path as the same drift state.
10. **SKIP** the vendor’s route-specific managed-update promises as canon behavior → canon core — marketplace update cadence and auto-update guarantees belong to each harness/provider, while canon should continue to report deterministic local projection and lock state.

## 6. Citations

- `vendors/mattpocock-skills/SCOPE.md:7-17` — observed-failure plus philosophy admission bar.
- `vendors/mattpocock-skills/SCOPE.md:19-35` — bucket policy, out-of-scope index, unresolved issue timeout, and rejection disposition.
- `vendors/mattpocock-skills/GLOSSARY.md:3-28` — canonical terms, relationships, and forbidden synonyms.
- `vendors/mattpocock-skills/.out-of-scope/new-skills.md:1-24` — durable rejection record for new-skill proposals.
- `vendors/mattpocock-skills/.out-of-scope/harness-name-collisions.md:1-27` — durable rejection record for harness-driven renames and aliases.
- `vendors/mattpocock-skills/.out-of-scope/frozen-misc-skills.md:1-24` — frozen-surface lifecycle and user-owned fork policy.
- `vendors/mattpocock-skills/docs/agents/triage-labels.md:3-15` — canonical triage roles mapped to tracker labels.
- `vendors/mattpocock-skills/_vendor_agents/install-block.md:1-29` — single-source installation policy and managed/manual distinction.
- `vendors/mattpocock-skills/_vendor_agents/install-block.md:31-91` — provider-specific install commands, update behavior, and exclusive-route rule.
- `vendors/mattpocock-skills/scripts/link-skills.sh:9-18` — maintainer-only symlink distribution and destination roots.
- `vendors/mattpocock-skills/scripts/link-skills.sh:20-45` — deprecated/misc exclusion and in-progress inclusion policy.
- `vendors/mattpocock-skills/package.json:1-24` — package version, Changesets scripts, and release dependencies.
- `vendors/mattpocock-skills/_vendor_claude-plugin/plugin.json:1-48` — plugin metadata, version, and explicit promoted-skill list.
- `vendors/mattpocock-skills/scripts/sync-plugin-version.mjs:1-44` — package-to-plugin version synchronization and read-only check.
- `vendors/mattpocock-skills/.changeset/implement-ticket-reference.md:1-7` — per-change package/SemVer/release-note record.
- `vendors/mattpocock-skills/.changeset/config.json:1-14` — Changesets configuration and changelog adapter.
- `vendors/mattpocock-skills/CHANGELOG.md:1-27` — generated versioned release notes with PR, commit, and contributor references.
- `vendors/mattpocock-skills/skills/deprecated/README.md:1-4` — deletion-based retirement policy.
- `crates/canon-cli/src/skills.rs:1-32` — canon provider projection, install lock, drift, and legacy-remnant model.
- `crates/canon-cli/src/skills.rs:67-101` — lock and provider selection data shapes.
- `crates/canon-cli/src/skills.rs:260-281` — installed-by version metadata and outdated-lock diagnostics.
- `crates/canon-cli/src/main.rs:1637-1680` — `skills install`, `skills check`, and `skills doctor` command surfaces.
- `crates/canon-cli/src/deprecation.rs:1-16` — release-bounded deprecation lifecycle.
- `crates/canon-cli/src/deprecation.rs:38-54` — centralized deprecated flag/value table.
- `crates/canon-cli/src/deprecation.rs:207-214` — warning and JSON metadata behavior.
- `crates/canon-cli/src/status.rs:24-35` — role-aware status and `meta.next` contract.
- `crates/canon-cli/src/finding.rs:329-332` — canon’s `blocker`, `should-fix`, and `note` finding severities.
- `canon/skills/SKILL.src.md:16-31` — canon’s human-approved scope boundary.
- `canon/skills/SKILL.src.md:135-154` — canon’s provider installation and projection contract.
- `README.md:85-141` — canon’s release-note and deprecation documentation practice.
