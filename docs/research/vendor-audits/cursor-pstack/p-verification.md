# Surface — p-verification

**Vendor paths**: `vendors/cursor-plugins/pstack/skills/create-verification-skill`, `vendors/cursor-plugins/pstack/skills/maintain-verification-skill`, `vendors/cursor-plugins/pstack/skills/create-formal-verification-skill`, `vendors/cursor-plugins/pstack/skills/maintain-formal-verification-skill`, `vendors/cursor-plugins/pstack/skills/benchmark-checklist`, `vendors/cursor-plugins/pstack/skills/blast-radius`, `vendors/cursor-plugins/pstack/skills/correct`, `vendors/cursor-plugins/pstack/skills/show-me-your-work`, `vendors/cursor-plugins/pstack/skills/interrogate`, `vendors/cursor-plugins/pstack/skills/arena`, `vendors/cursor-plugins/pstack/skills/swarm`, `vendors/cursor-plugins/pstack/skills/tdd`

**Scale**: 12 skill files / approximately 500 lines of workflow guidance

**Primary language(s)**: Markdown skill instructions with shell/test/tool examples

**Canon analog (if any)**: `canon/skills/SKILL.src.md`, `canon/skills/reference/canon-gate.md`, and `canon/skills/reference/canon-review.md` — canon already has scenario-bound evidence, pinned independent reviews, findings, divergence, and a ship guard, but not a generated per-repository verification harness or a generalized proof-quality taxonomy.

## 1. Overview

p-verification is a set of agent workflows for turning “the change works” into executable, reviewable proof. Its strongest seam is the project-local verification skill generator: it interviews the repository, writes Launch/Doctor/Drive/Evidence/Cleanup instructions and a feature map, then runs one real feature before handing the skill over. Maintenance treats the feature map as a living verification index and separates documentation drift, harness gaps, and product regressions. Formal verification adds bounded/model-based checks with explicit properties, assumptions, sync commits, broken-variant sensitivity tests, and a drift-versus-bug lifecycle. The companion skills add adversarial multi-model review, blast-radius proof, decision trails, and stricter TDD/b…

## 2. Entry points & architecture

- **Entry file(s)**:
  - `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:7-44` — interviews the repo, generates a project-local skill and feature map, and proves one generated journey.
  - `vendors/cursor-plugins/pstack/skills/create-formal-verification-skill/SKILL.md:7-41` — selects a high-risk/low-test-reach property, chooses a verifier, records scope and sync metadata, and validates failure sensitivity.
  - `vendors/cursor-plugins/pstack/skills/interrogate/SKILL.md:7-66` — fans a diff to configured model-family reviewers and synthesizes classified findings without auto-applying them.
  - `vendors/cursor-plugins/pstack/skills/blast-radius/SKILL.md:7-58` — requires reducing a broad risk question to one safety fact and proving it with the real code.
- **Core abstractions**:
  - **Project-local verification skill** — executable agent instructions with Launch, Doctor, Drive, Evidence, Cleanup, Helpers, and feature-map paths (`create-verification-skill/SKILL.md:23-36`).
  - **Feature map** — README index plus one file per user-facing feature; each file records user entry, harness recipe, observable end state, and gotchas (`create-verification-skill/SKILL.md:34-36`).
  - **Formal verification record** — property, covered code, bounds/assumptions, tool command/version, and last sync commit (`create-formal-verification-skill/SKILL.md:22-33`).
  - **Verification maintenance outcome** — `in sync`, `updated`, `bug`, or `blocked`, with a required drift/bug decision (`maintain-formal-verification-skill/SKILL.md:11-20`).
  - **Interrogation synthesis** — reviewers per configured model, then `Act on` / `Consider` / `Noted` / `Dismissed` buckets and an agreement map (`interrogate/SKILL.md:7-10`, `interrogate/SKILL.md:39-66`).
  - **Decision trail row** — append-only TSV row `(ts, phase, decision, why, evidence, result)` (`show-me-your-work/SKILL.md:7-20`, `show-me-your-work/SKILL.md:31-45`).
- **Cross-package dependencies**: pstack model routing (`~/.cursor/rules/pstack-models.mdc`), the Task/subagent mechanism, repository-native launch/test harnesses, and the companion `how`, `why`, `unslop`, `prove-it-works`, and `poteto-mode` workflows. The generated project skill depends on the target repository rather than a fixed framework.
- **External runtime dependencies**: optional browser/CDP, PTY/tmux, HTTP, model-checker/prover/SMT tooling, benchmark profilers, and configured LLM model families. No single application framework is required.

## 3. Pattern catalog

### 3.1 Generated, executable verification contract plus feature map

- **Where**: `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:7-36`; `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:38-44`
- **What it does**: The generator derives the surface, launch command, interaction mechanism, observable evidence, and instance-isolation rules from repository source instead of asking the operator to invent them. It emits a project-local `verify-<app>` skill with exact Launch/Doctor/Drive/Evidence/Cleanup/Helpers sections and a feature-map README plus one file per user-facing feature. Before handoff it runs one mapped feature end to end, checks that proof artifacts survive cleanup, and points subsequent agents at the maintenance loop.
- **Why it's interesting**: Canon has a durable scenario/evidence ledger, but no command that turns a repository's actual user surface into a runnable verification harness or indexes the full user feature surface. This pattern makes “evidence exists” operationally reproducible rather than merely a manually supplied report/ref string, and it gives later agents a maintained map against which missing coverage can be found.
- **Canon applicability**: **Missing/partial.** `canon evidence add` binds reports and artifacts and the gate rechecks their bytes (`crates/canon-cli/src/evidence_attach.rs:65-69`, `crates/canon-cli/src/evidence.rs:641-655`), while `canon inventory sync` maintains scenario digests (`crates/canon-cli/src/inventory.rs:11-16`, `crates/canon-cli/src/inventory.rs:912-930`). However, canon's skill installer only projects the canonical canon skill to selected providers (`canon/skills/SKILL.src.md:137-153`; `crates/canon-cli/src/main.rs:70-73`) and does not generate a repo-specific verifier, launch doctor, or feature map. A concrete target is a new opt-in `canon verify init`/`canon verify maintain` surface that writes a project-local verifier and links feature-map e…
- **Effort to lift**: significant.

### 3.2 Formal proof has explicit scope, falsifiability, and drift lifecycle

- **Where**: `vendors/cursor-plugins/pstack/skills/create-formal-verification-skill/SKILL.md:11-29`; `vendors/cursor-plugins/pstack/skills/create-formal-verification-skill/SKILL.md:31-41`; `vendors/cursor-plugins/pstack/skills/maintain-formal-verification-skill/SKILL.md:11-20`; `vendors/cursor-plugins/pstack/skills/maintain-formal-verification-skill/SKILL.md:30-58`
- **What it does**: The workflow chooses a property where harm is high and ordinary tests have poor reach, then records the exact code covered, bounds and assumptions, tool choice, rerun command, and sync commit. A proof is not accepted merely because it passes: the agent breaks a real variant and requires the checker to catch it. On later code changes, the maintenance pass distinguishes unchanged verification (`in sync`), updated model/harness (`updated`), a current-code counterexample (`bug`), and inability to run (`blocked`), and refuses to weaken a property to obtain green.
- **Why it's interesting**: Canon's evidence records can carry verdicts and reports, but they do not require a proof's semantic scope, assumptions, known failing mutation, or a machine-readable “this verifier describes commit X” marker. Without those fields, a green report can be a stale model or a checker that never demonstrated sensitivity.
- **Canon applicability**: **Partial.** Canon already pins scenario meaning and the latest evidence check identity in reviews (`crates/canon-gate/src/freshness.rs:82-130`) and treats stale evidence as a gate class (`canon/skills/reference/canon-gate.md:31-39`). It does not model formal-check scope, bounds, mutation/falsifiability, or drift outcomes. Target `canon evidence add --kind formal-check` (or a typed context extension) with `covered_refs`, `property`, `bounds`, `assumptions`, `tool_version`, `sync_sha`, `mutation_ref`, and `result: pass|counterexample|blocked`; make `canon gate check` flag a pass whose `sync_sha` is behind covered code, while preserving the existing report/artifact binding.
- **Effort to lift**: significant.

### 3.3 Adversarial review uses model diversity and a non-automatic synthesis

- **Where**: `vendors/cursor-plugins/pstack/skills/interrogate/SKILL.md:7-10`; `vendors/cursor-plugins/pstack/skills/interrogate/SKILL.md:17-38`; `vendors/cursor-plugins/pstack/skills/interrogate/SKILL.md:39-66`; `vendors/cursor-plugins/pstack/skills/arena/SKILL.md:13-24`, `vendors/cursor-plugins/pstack/skills/arena/SKILL.md:46-48`
- **What it does**: `interrogate` sends the same intent, diff, context, rubric, and code-quality lens to one reviewer per configured model family, then merges findings and explicitly tracks agreement/disagreement. The lead classifies each finding into `Act on`, `Consider`, `Noted`, or `Dismissed`, with a reason, and never auto-applies a change. `arena` generalizes the same rigor to competing implementations: it declares a rubric, isolates candidate outputs, cross-judges them, picks a base, grafts selected improvements, and verifies the synthesis.
- **Why it's interesting**: Independent model families can expose different blind spots, while explicit dismissal reasons prevent a review feed from becoming an unfiltered TODO list. The candidate/rubric/cross-judge lifecycle is also useful for high-risk canon policy or gate changes where one design attempt can lock in a bad shape.
- **Canon applicability**: **Partial.** Canon requires a different reviewer session and role, pins the scenario digest and evidence checks, and records provider/model/harness metadata (`canon/skills/reference/canon-review.md:91-126`, `crates/canon-model/src/records.rs:838-881`). Findings have explicit severity/disposition and scenario joins (`canon/skills/reference/canon-review.md:194-246`; `crates/canon-cli/src/finding.rs:513-530`). Canon therefore has the independent-review and issue-lifecycle spine, but one `review add` is not a multi-model review run and has no agreement map or dismissal rationale. Target `canon review campaign` (or an evidence/report schema) that records reviewer seats, model family, shared review input pin, per-finding source seats, me…
- **Effort to lift**: moderate.

### 3.4 Blast-radius analysis reduces risk to one executable safety fact

- **Where**: `vendors/cursor-plugins/pstack/skills/blast-radius/SKILL.md:13-18`; `vendors/cursor-plugins/pstack/skills/blast-radius/SKILL.md:19-37`; `vendors/cursor-plugins/pstack/skills/blast-radius/SKILL.md:39-58`
- **What it does**: Instead of producing a plausible list of callers, the workflow follows hidden boundaries (library source, wire formats, database fields, flags, async teardown) and identifies the one fact on which safety depends. It grades confidence from assertion, to source citation, to failure-path reasoning, to a real script/test, to a running-app reproduction; the handoff must state risks, cleared risks, and the cheapest pre-merge repro, with the safety fact marked proven or unproven.
- **Why it's interesting**: This gives a concrete meaning to “reviewed the blast radius”: a reviewer must name a falsifiable safety predicate and attach executable proof, not only summarize changed files. It complements scenario evidence by covering non-local consumers and compatibility surfaces that a feature scenario may not exercise.
- **Canon applicability**: **Missing/partial.** Canon's reviewer instructions already require running the checks and checking that each test actually proves the Then steps (`canon/skills/reference/role-reviewer.md:31-43`), and review artifacts are digest-bound (`canon/skills/reference/canon-review.md:272-300`). But there is no first-class blast-radius claim, cleared-risk list, confidence level, or proof command/result attached to a finding or review. Target `canon review add --blast-radius-report` or a typed review attachment with `safety_facts[] {claim, risk_refs, proof_command, result, artifact}` and `cleared[]`; gate only the existence/currentness of the record, not a prose judgment about whether the fact is true.
- **Effort to lift**: moderate.

### 3.5 Proof records distinguish real behavior, mutation sensitivity, and measurement validity

- **Where**: `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:27-32`; `vendors/cursor-plugins/pstack/skills/create-formal-verification-skill/SKILL.md:22-29`; `vendors/cursor-plugins/pstack/skills/tdd/SKILL.md:7-35`; `vendors/cursor-plugins/pstack/skills/benchmark-checklist/SKILL.md:7-34`; `vendors/cursor-plugins/pstack/skills/benchmark-checklist/SKILL.md:36-48`
- **What it does**: The skills define proof as a real user path plus action and resulting state, side-effect checks, and preserved artifacts—not a final screen or internal setter. TDD requires a focused check that fails before the fix for the intended reason and passes after it, while allowing a cheaper executable repro when a test harness is impractical. Benchmark claims must account for errors, tuning, physical limits, repetition, end-to-end relevance, and whether the timed work actually happened; unproven/inconclusive claims remain inconclusive.
- **Why it's interesting**: These are reusable proof-quality rules across UI, CLI, regression, and performance evidence. They prevent a green command, a present-but-wrong artifact, a benchmark timeout, or a test that only mirrors implementation details from being treated as a successful proof.
- **Canon applicability**: **Partial.** Canon has typed verdicts, report formats, attached artifacts, fabrication-marker refusals, contradiction checks, and the always-on `false-success` audit class (`canon/skills/reference/canon-gate.md:31-39`, `canon/skills/reference/canon-gate.md:218-222`; `crates/canon-cli/src/evidence.rs:305-310`; `crates/canon-cli/src/evidence_attach.rs:91-100`). It does not require a before/after failure demonstration for ordinary test-run evidence, nor does it have typed benchmark validity or proof-quality fields. Target `canon evidence add` optional typed metadata for `regression` (`failed_before`, `passed_after`, `failure_reason`) and `benchmark` (`runs`, `range`, `errors`, `tuning`, `limiter`, `work_observed`, `verdict`), with `in…
- **Effort to lift**: moderate.

## 4. Risks & incompatibilities

- **Effect version**: none. The surface is Markdown and delegates to repository-native tools; any formal checker, browser, PTY, profiler, or test runner remains an external per-repository choice.
- **Framework**: framework-neutral in the generator, but generated Drive/Evidence instructions are necessarily app-specific. Canon should store stable references and digests rather than assume browser screenshots or a particular UI framework.
- **Heavy deps**: multi-model interrogation, browser/CDP, model checkers, profilers, and arena/swarm workers can be expensive. They should be opt-in and report their runtime/model/tool metadata rather than becoming a mandatory gate for every change.
- **Coupling**: `interrogate`, `arena`, and `swarm` depend on pstack's Task tool and model-routing rule; `create-verification-skill` depends on the target repo's launch surface; formal verification depends on a suitable verifier. The portable unit is the record shape and lifecycle, not the orchestration implementation.
- **TOS-violation patterns**: none observed in this surface. It routes to configured reviewers and repository-native tools; it does not impersonate provider clients, forge request fingerprints, or evade rate limits.
- **License footnote**: clean-room reimplementation from this audit is the default. No license routing issue is identified for these documentation-only paths.

## 5. Recommended actions

1. **EXTRACT** the generated verifier contract and feature-map lifecycle → canon `canon verify init|maintain` plus a project-local verifier directory linked to scenario IDs. The repo-specific Launch/Doctor/Drive/Evidence/Cleanup shape is concrete and fills the gap between a scenario and runnable proof.
2. **INSPIRE** formal-check metadata and drift outcomes → canon `evidence add --kind formal-check` and the gate's stale-evidence checks. Preserve explicit bounds, assumptions, covered code, sync commit, mutation sensitivity, and `in sync|updated|bug|blocked` without pretending a bounded proof covers the whole product.
3. **INSPIRE** multi-model interrogation and arena synthesis → canon `review campaign` / `review add` provenance. Record shared input pins, reviewer model families, agreement, dissent, and dismissal rationale, while retaining a human/role-qualified final review and findings as the gate authority.
4. **EXTRACT** the blast-radius safety-fact contract → canon review attachments or a typed review report with proof command, result, risk references, and cleared risks. This makes non-local risk review executable and auditable without asking the gate to judge prose.
5. **INSPIRE** TDD and benchmark validity fields → canon evidence schemas for regression and performance claims. Require the evidence to say what failed before, what passed after, whether work/errors were observed, and when a result is inconclusive; keep the fields optional by evidence kind so expensive paths are not forced into fake tests.
6. **INSPIRE** the append-only decision trail → canon's evidence/review provenance or a `canon run log` artifact. Use one-row decision records with evidence pointers and explicit `INCONCLUSIVE`/`open` results for long-running orchestrations; do not make a TSV the source of gate truth when typed ledger records already exist.
7. **SKIP** copying pstack's raw model-routing, temporary worktree, and Task-tool mechanics into canon. They are harness-specific orchestration details; canon should record provider/model/session provenance and verified outputs, not require one vendor's agent runtime.
8. **SKIP** making every proof require a formal checker or a deliberately broken product variant. The vendor distinguishes user-path evidence, cheap regression checks, formal bounds, and benchmarks; a universal obligation would be expensive, brittle, and weaker than typed per-kind requirements.

## 6. Citations

- `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:7-21` — repository interview, real surface discovery, and instance isolation.
- `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:23-36` — generated skill sections and feature-map data shape.
- `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:38-44` — mandatory live proof and artifact-preserving cleanup.
- `vendors/cursor-plugins/pstack/skills/maintain-verification-skill/SKILL.md:23-36` — index hygiene, source wave, reconciliation, and live maintenance pass.
- `vendors/cursor-plugins/pstack/skills/create-formal-verification-skill/SKILL.md:22-37` — proof properties, bounds, assumptions, sync commit, and broken-variant test.
- `vendors/cursor-plugins/pstack/skills/maintain-formal-verification-skill/SKILL.md:11-20` — explicit verification outcomes.
- `vendors/cursor-plugins/pstack/skills/maintain-formal-verification-skill/SKILL.md:30-58` — drift-versus-bug analysis and report requirements.
- `vendors/cursor-plugins/pstack/skills/interrogate/SKILL.md:7-10` — model-diverse adversarial review and no auto-apply rule.
- `vendors/cursor-plugins/pstack/skills/interrogate/SKILL.md:39-66` — synthesis buckets, disagreement handling, and output contract.
- `vendors/cursor-plugins/pstack/skills/blast-radius/SKILL.md:13-37` — one safety fact, confidence ladder, and executable proof.

[Some lines truncated to 768 chars. Use agent://AuditPVerification:raw:13-13 to read line 13 whole; cut lines run to line 71, each read the same way]
