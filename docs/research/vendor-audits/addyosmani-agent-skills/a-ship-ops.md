# Surface — a-ship-ops

**Vendor paths**: `vendors/addyosmani-agent-skills/skills/ci-cd-and-automation/SKILL.md`, `vendors/addyosmani-agent-skills/skills/git-workflow-and-versioning/SKILL.md`, `vendors/addyosmani-agent-skills/skills/shipping-and-launch/SKILL.md`, `vendors/addyosmani-agent-skills/skills/deprecation-and-migration/SKILL.md`, `vendors/addyosmani-agent-skills/skills/documentation-and-adrs/SKILL.md`, `vendors/addyosmani-agent-skills/skills/observability-and-instrumentation/SKILL.md`
**Scale**: 6 files / approximately 2,000 lines of Markdown
**Primary language(s)**: Markdown, with YAML, TypeScript, and shell examples
**Canon analog (if any)**: `canon/skills/SKILL.src.md`, `crates/canon-cli/src/subject.rs`, `crates/canon-cli/src/deprecation.rs`, and `crates/canon-cli/src/divergence.rs` — Canon governs brief approval, evidence, review, divergence, and the `verifying → shipped` transition, but does not provide a general deployment-operations surface.

## 1. Overview

This surface is a standalone set of operational guidance for taking software from a reviewed change to a production launch and then maintaining or removing it. It covers CI quality gates, trunk-based Git workflows, release/versioning discipline, staged rollouts, rollback, feature-flag cleanup, deprecation and database migration, ADRs, changelogs, structured telemetry, and alert runbooks. The six skills are coupled conceptually: CI produces verification inputs, shipping consumes them, observability provides post-launch signals, and deprecation/documentation preserve the lifecycle context. The content is prescriptive guidance rather than an executable package, so it has no internal runtime or package dependencies.

## 2. Entry points & architecture

- **Entry file(s)**:
  - `vendors/addyosmani-agent-skills/skills/ci-cd-and-automation/SKILL.md:1-45` — introduces CI as an enforcement mechanism and defines a quality-gate pipeline.
  - `vendors/addyosmani-agent-skills/skills/shipping-and-launch/SKILL.md:1-45` — introduces the production launch checklist and its operational goals.
  - `vendors/addyosmani-agent-skills/skills/deprecation-and-migration/SKILL.md:1-45` — introduces deprecation as an explicit lifecycle and migration discipline.
  - `vendors/addyosmani-agent-skills/skills/documentation-and-adrs/SKILL.md:1-75` — introduces ADRs as records of context, alternatives, and consequences.
  - `vendors/addyosmani-agent-skills/skills/observability-and-instrumentation/SKILL.md:1-55` — introduces observability as a prerequisite for diagnosing production behavior.
- **Core abstractions**:
  - **Quality-gate pipeline** — ordered lint, type-check, test, build, integration, E2E, security, and bundle checks.
  - **Feature-flag lifecycle** — deploy disabled, enable for internal users, canary, gradual rollout, full rollout, then remove the flag and dead code.
  - **Expand/contract migration** — additive schema change, dual-write/backfill, read cutover, then a later destructive cleanup.
  - **ADR lifecycle** — `PROPOSED → ACCEPTED → SUPERSEDED or DEPRECATED`, with old decisions retained.
  - **Signal selection model** — structured logs answer “what happened,” metrics answer aggregate rate/latency, and traces answer where time went.
  - **Runbook-backed alert** — every alert has an actionable symptom, threshold/duration, and a first diagnostic path.
- **Cross-package dependencies**: None. These are independent skill documents, although `shipping-and-launch` explicitly points to the sibling observability skill and the vendor’s other checklist/reference skills.
- **External runtime dependencies**: None required by the surface. Examples mention GitHub Actions, npm, TypeScript, Playwright, Vercel, Prometheus/OpenTelemetry, React, PostgreSQL, and feature-flag services, but these are illustrative integrations rather than dependencies of the skill pack.

## 3. Pattern catalog

### 3.1. Ordered quality gates with failure feedback

- **Where**: `vendors/addyosmani-agent-skills/skills/ci-cd-and-automation/SKILL.md:1-45`, `vendors/addyosmani-agent-skills/skills/ci-cd-and-automation/SKILL.md:90-170`, `vendors/addyosmani-agent-skills/skills/ci-cd-and-automation/SKILL.md:230-290`
- **What it does**: The skill defines a fixed pipeline from pull-request or push trigger through lint, type checking, unit tests, build, integration tests, optional E2E, security audit, and bundle-size validation. It also defines a failure loop in which the CI output is returned to the agent, the agent fixes the concrete failure, and the pipeline runs again. Deployment is separately staged through staging, production, monitoring, and rollback.
- **Why it's interesting**: Canon already models evidence and independent review, but it does not prescribe how external CI results become a repeatable, scenario-bound agent feedback loop. The vendor pattern gives agents an operational ordering and makes “quality gate passed” more concrete than an undifferentiated test command.
- **Canon applicability**: **Partial.** Canon has the evidence/review/ship loop in `canon/skills/SKILL.src.md:1-8` and the evidence gate in `canon/skills/reference/canon-gate.md:1-33`; `canon evidence add` can bind a JUnit or Cucumber report, but there is no dedicated CI pipeline record or built-in mapping from a failed CI job to a subsequent agent repair cycle. The existing evidence model is the appropriate target rather than a second gate mechanism.
- **Effort to lift**: moderate.

### 3.2. Staged rollout with explicit thresholds and rollback

- **Where**: `vendors/addyosmani-agent-skills/skills/shipping-and-launch/SKILL.md:90-190`, `vendors/addyosmani-agent-skills/skills/shipping-and-launch/SKILL.md:200-285`
- **What it does**: A release moves through staging, production with the feature disabled, internal enablement, a 5% canary, 25/50/100% rollout, and a post-rollout observation period. Each stage has a monitoring window and decision thresholds for error rate, P95 latency, client errors, and business metrics. Rollback triggers include doubled error rate, severe latency regression, user-report spikes, integrity problems, and security vulnerabilities; the rollback plan records trigger conditions, steps, database considerations, and expected recovery time.
- **Why it's interesting**: Canon’s `verifying → shipped` guard proves that a subject’s scenarios have evidence and review, but “shipped” does not currently describe progressive exposure, operational hold points, or post-deploy rollback readiness. This is a concrete lifecycle gap for changes that are behaviorally correct yet operationally risky.
- **Canon applicability**: **Partial.** Canon has the ship transition and fail-closed guards in `crates/canon-cli/src/subject.rs:1-30` and `crates/canon-cli/src/subject.rs:650-760`, and it supports reopen with a reason after shipping. It lacks a release-stage record, rollout percentage, observation window, threshold table, rollback reference, or post-launch verification state.
- **Effort to lift**: significant.

### 3.3. Feature-flag lifecycle with owner, expiry, and cleanup

- **Where**: `vendors/addyosmani-agent-skills/skills/shipping-and-launch/SKILL.md:90-130`
- **What it does**: The feature-flag mechanism separates deployment from release: code may be deployed while disabled, enabled for a team or beta cohort, gradually rolled out, monitored, and finally cleaned up. Each flag has an owner and expiration date; flags should not be nested, both states should be tested in CI, and the flag plus dead code should be removed after full rollout.
- **Why it's interesting**: This gives a durable lifecycle to a common operational escape hatch. Without ownership and expiry, flags become permanent branches that complicate testing and make the actual shipped behavior ambiguous.
- **Canon applicability**: **Missing.** Canon’s `change new --kind chore` supports non-behavioral CI/build/infra work through `canon/skills/reference/role-planner.md:38-47`, and `canon subject status` records the product lifecycle, but there is no feature-flag entity, owner/expiry field, rollout state, or cleanup obligation. A future canon target could be a release/rollout manifest linked to a subject or change, not a new scenario type.
- **Effort to lift**: significant.

### 3.4. Expand/contract migrations with reversible phases

- **Where**: `vendors/addyosmani-agent-skills/skills/deprecation-and-migration/SKILL.md:166-255`
- **What it does**: Database changes are divided into independently deployable phases: add the new nullable shape, dual-write, backfill existing data in batches, switch reads, stop writing the old shape, and drop the old column in a later deploy. The pattern explicitly requires tested down paths, throttled backfills, non-blocking index construction, and feature-flagged cutovers where risk is high.
- **Why it's interesting**: Canon records evidence and divergence for implementation behavior, but it does not currently express migration phase ordering or destructive-step isolation. This pattern is concrete enough to become a chore/change checklist or typed operational artifact.
- **Canon applicability**: **Partial.** Canon’s chore mechanism explicitly supports dependency, build, CI, and infrastructure changes, with invariants and checks in `canon/skills/reference/role-planner.md:38-47` and `canon/skills/reference/canon-vocab.md:1-40`. A migration can therefore be documented as a chore today, but Canon has no migration phase schema, down-path field, backfill status, or contract gate.
- **Effort to lift**: moderate.

### 3.5. Single-table deprecation warnings with a removal boundary

- **Where**: `vendors/addyosmani-agent-skills/skills/deprecation-and-migration/SKILL.md:70-165`
- **What it does**: The vendor distinguishes advisory and compulsory deprecation, requires a replacement before deprecation, documents status/replacement/removal date/reason, migrates consumers incrementally, verifies zero active usage, and only then removes old code and notices. The lifecycle treats migration documentation and tooling as part of the deprecation rather than optional follow-up work.
- **Why it's interesting**: Canon already has a deprecation subsystem, but it is specifically about CLI spellings and values. The vendor pattern suggests a broader, domain-neutral deprecation record with replacement, policy (advisory or compulsory), migration evidence, usage verification, and removal release.
- **Canon applicability**: **Partial.** `crates/canon-cli/src/deprecation.rs:1-16` defines one mechanism and a removal release, while `crates/canon-cli/src/deprecation.rs:38-64` stores deprecated flag/value mappings. Canon has warnings, hidden aliases, JSON metadata, and a planned removal version, but no general API/feature deprecation lifecycle or migration proof. This should extend or inspire the existing mechanism rather than replace it.
- **Effort to lift**: moderate.

### 3.6. ADRs and changelog entries as durable change context

- **Where**: `vendors/addyosmani-agent-skills/skills/documentation-and-adrs/SKILL.md:1-75`, `vendors/addyosmani-agent-skills/skills/documentation-and-adrs/SKILL.md:76-145`
- **What it does**: The skill requires an ADR for decisions that are expensive to reverse, with status, date, context, decision, alternatives, and consequences. It retains superseded ADRs instead of deleting them and separately describes human-readable changelog entries grouped by user impact (`Added`, `Changed`, `Fixed`, `Deprecated`, `Removed`, `Security`).
- **Why it's interesting**: Canon’s brief captures intent, desired outcome, numbered flow, acceptance, assumptions, and scope, but it is primarily a pre-build approval record. It does not provide a durable alternatives/consequences decision record or a release-facing changelog model.
- **Canon applicability**: **Partial.** The brief template and change lifecycle are defined in `canon/skills/reference/role-planner.md:15-35` and `crates/canon-cli/src/brief_template.rs:75-117`; the brief can preserve rationale and acceptance. Canon has no ADR command, ADR status lifecycle, or changelog record. An ADR should remain a complementary documentation artifact linked from a change, not be forced into Gherkin scenarios.
- **Effort to lift**: moderate.

### 3.7. Question-driven structured observability with bounded metrics

- **Where**: `vendors/addyosmani-agent-skills/skills/observability-and-instrumentation/SKILL.md:1-55`, `vendors/addyosmani-agent-skills/skills/observability-and-instrumentation/SKILL.md:56-145`, `vendors/addyosmani-agent-skills/skills/observability-and-instrumentation/SKILL.md:146-235`
- **What it does**: Instrumentation begins with 2–4 explicit on-call questions, then selects structured logs, metrics, or traces for each question. Logs use stable event names and correlation IDs, entry points are propagated explicitly, metrics use bounded labels and histograms with p95/p99, traces propagate across async boundaries, and alerts are symptom-based with thresholds, duration, and runbooks.
- **Why it's interesting**: Canon is strong at proving pre-ship behavior but does not require evidence that a production feature is diagnosable after deployment. This pattern supplies a concrete operational acceptance surface without treating raw logs or dashboards as generic “proof.”
- **Canon applicability**: **Missing, with a partial evidence bridge.** Canon’s evidence records can bind reports and artifacts, and `canon-gate` re-checks bound bytes as described in `canon/skills/reference/canon-gate.md:1-33`; however, there is no telemetry-question field, structured-log contract, metric cardinality check, alert/runbook relation, or post-launch instrumentation evidence type. A future target could be an optional operational checklist or typed evidence kind linked to a shipped subject.
- **Effort to lift**: significant.

## 4. Risks & incompatibilities

- **Toolchain mismatch**: The vendor examples are primarily JavaScript/TypeScript-oriented and use GitHub Actions, npm, React, Prometheus, and OpenTelemetry. Canon is a Rust CLI with a file/ledger evidence model; the examples should be clean-room concepts, not copied implementation.
- **Framework and platform coupling**: GitHub Actions, Vercel, Playwright, React error boundaries, and PostgreSQL commands are deployment-specific. Any canon feature should model provider-neutral lifecycle data and allow repository-specific commands or runbook references.
- **Operational-scope expansion**: Progressive rollout, feature flags, SLO/error budgets, and telemetry introduce production control-plane concepts beyond the current subject/evidence/review loop. They should be optional and linked to a subject/change, not required for every local CLI change.
- **Potential contradiction with Canon’s evidence semantics**: The vendor describes monitoring and manual verification as launch steps, while Canon requires evidence to be bound to concrete scenarios and independent review. Any adoption must preserve Canon’s distinction between “a check exists,” “the verdict passed,” and “a reviewer independently signed.”
- **Deprecation overlap**: Canon already has a deliberately narrow, centralized deprecation table. A second ad hoc warning mechanism would conflict with the one-table design documented in `crates/canon-cli/src/deprecation.rs:1-16`.
- **Divergence handling**: Rollout thresholds and production telemetry may expose a known external gap. Canon should record that through its existing divergence lifecycle rather than allowing operational exceptions to bypass the ship guard. `crates/canon-cli/src/divergence.rs:1-24` makes divergence a first-class staged/promoted record, and `crates/canon-cli/src/divergence.rs:49-64` requires explicit status and expiry data for deferred cases.
- **License footnote**: No license issue was identified from these Markdown skill files. Any future verbatim-lift PR should independently verify repository licensing; the recommended actions below assume clean-room reimplementation from the cited behavior.

## 5. Recommended actions

1. **EXTRACT** the vendor’s ordered quality-gate and CI-failure feedback checklist → Canon’s evidence/review documentation and a future CI adapter around `canon evidence add` — Canon already binds reports, so the useful extraction is the explicit ordering, failure handoff, and “no skipped gate” policy rather than a second evidence ledger.
2. **INSPIRE** staged rollout, monitoring windows, threshold bands, and rollback plans for an optional release/operations record linked to `subject status --to shipped` → `crates/canon-cli/src/subject.rs:650-760` — the current ship guard proves readiness but has no post-deploy exposure or rollback state.
3. **INSPIRE** feature-flag owner, expiry, rollout percentage, and cleanup fields → a future subject-linked rollout manifest — `change new --kind chore` is appropriate for flag cleanup work, but it is not itself a feature-flag lifecycle.
4. **EXTRACT** expand/contract phase rules and tested down-path requirements → chore brief guidance and a typed migration checklist under `canon/skills/reference/role-planner.md` / `canon/skills/reference/canon-vocab.md` — these phases are concrete invariants that can be reviewed and evidenced without adding behavior scenarios.
5. **EXTRACT** the single deprecation lifecycle and removal boundary → extend the existing centralized mechanism in `crates/canon-cli/src/deprecation.rs` — Canon already has canonical replacement names, warning metadata, and `removedIn`; the vendor adds replacement readiness, consumer migration, usage verification, and final cleanup.
6. **INSPIRE** ADR status, alternatives, consequences, and retained supersession history → a documentation convention linked from a Canon change or brief — Canon’s PRD brief captures approved intent, but an ADR would preserve expensive-to-reverse architectural reasoning after the change ships.
7. **INSPIRE** question-driven telemetry, bounded metric labels, correlation IDs, alert thresholds, and runbook links → an optional operational acceptance/evidence surface for production-bound subjects — this fills the gap between Canon’s pre-ship evidence and post-ship diagnosability without making telemetry mandatory for every chore or local CLI change.
8. **SKIP** vendor-specific GitHub Actions, Vercel rollback commands, React error-boundary code, npm audit commands, and provider-specific deployment syntax → no direct Canon target — these are examples for application teams, not portable canon records.
9. **SKIP** replacing Canon’s divergence or deprecation mechanisms with freeform launch checklists → `crates/canon-cli/src/divergence.rs` and `crates/canon-cli/src/deprecation.rs` — Canon already has authoritative lifecycle machinery, and parallel conventions would create conflicting sources of truth.

## 6. Citations

- `vendors/addyosmani-agent-skills/skills/ci-cd-and-automation/SKILL.md:1-45` — CI as an enforcement mechanism and ordered quality-gate pipeline.
- `vendors/addyosmani-agent-skills/skills/ci-cd-and-automation/SKILL.md:90-170` — concrete CI jobs, integration tests, secrets handling, and E2E artifact upload.
- `vendors/addyosmani-agent-skills/skills/ci-cd-and-automation/SKILL.md:230-290` — staged deployment, monitoring window, rollback workflow, and environment separation.
- `vendors/addyosmani-agent-skills/skills/shipping-and-launch/SKILL.md:90-190` — feature-flag lifecycle, staged rollout sequence, and rollout percentages.
- `vendors/addyosmani-agent-skills/skills/shipping-and-launch/SKILL.md:200-285` — threshold table, rollback triggers, post-launch checks, and error-budget gate.
- `vendors/addyosmani-agent-skills/skills/deprecation-and-migration/SKILL.md:70-165` — replacement-first deprecation process and incremental consumer migration.
- `vendors/addyosmani-agent-skills/skills/deprecation-and-migration/SKILL.md:166-255` — strangler/adapter patterns and expand/contract database migration.
- `vendors/addyosmani-agent-skills/skills/documentation-and-adrs/SKILL.md:1-75` — ADR purpose, decision criteria, convention matching, and template.
- `vendors/addyosmani-agent-skills/skills/documentation-and-adrs/SKILL.md:76-145` — ADR lifecycle, retained supersession history, and changelog structure.
- `vendors/addyosmani-agent-skills/skills/observability-and-instrumentation/SKILL.md:1-55` — on-call questions and signal-selection model.
- `vendors/addyosmani-agent-skills/skills/observability-and-instrumentation/SKILL.md:56-145` — structured events, correlation IDs, entry-point propagation, and bounded metric labels.
- `vendors/addyosmani-agent-skills/skills/observability-and-instrumentation/SKILL.md:146-235` — tracing, symptom-based alerts, runbooks, and telemetry verification.
- `canon/skills/SKILL.src.md:1-8` — Canon’s approved brief, scenario, evidence, independent review, and ship-transition loop.
- `canon/skills/reference/role-planner.md:38-47` — chore changes and their evidence/review requirements.
- `canon/skills/reference/canon-gate.md:1-33` — Canon’s evidence/review gate and failure classes.
- `crates/canon-cli/src/subject.rs:1-30` — subject lifecycle and fail-closed `verifying → shipped` semantics.
- `crates/canon-cli/src/subject.rs:650-760` — implementation of shipping/reopen guards and policy-gated transitions.
- `crates/canon-cli/src/deprecation.rs:1-16` — centralized deprecation table and removal-release mechanism.
- `crates/canon-cli/src/deprecation.rs:38-64` — deprecated flag/value data shapes and canonical replacements.
- `crates/canon-cli/src/divergence.rs:1-24` — staged/promoted divergence lifecycle.
- `crates/canon-cli/src/divergence.rs:49-64` — explicit divergence statuses and deferred reason/expiry requirements.
