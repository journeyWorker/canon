# Surface — p-harness

**Vendor paths**: `vendors/cursor-plugins/pstack/agents/`, `vendors/cursor-plugins/pstack/automations/benny/`, `vendors/cursor-plugins/pstack/docs/guide/`, `vendors/cursor-plugins/pstack/docs/harness/`, `vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json`  
**Scale**: 15 files in the requested surface / approximately 1,300 LOC of operational documentation and agent definitions  
**Primary language(s)**: Markdown, YAML, JSON  
**Canon analog (if any)**: `canon/skills/SKILL.src.md:133-154` and `canon/skills/reference/role-orchestrator.md:31-56` — Canon already projects one workflow into several harnesses and coordinates planner, implementer, reviewer, and orchestrator sessions, but does not provide pstack's explicit Cursor-to-Claude field translation or its scheduled automation pipeline.

## 1. Overview

p-harness is the harness-facing portion of pstack: a Cursor plugin manifest, two named agent definitions, a Claude Code compatibility table, and a dormant Slack automation pack called Benny. The plugin manifest exposes `skills/` and `agents/`, while Benny is deliberately not a slash-skill surface; setup copies it into a target repository and reads its operational files directly. The pack supports two connected automations: thread-only issue triage followed by gated reproduction and optional draft-fix preparation. It is coupled to Cursor Automations, Slack actions, an issue tracker, a repository, and a user-supplied UI control adapter, rather than being a standalone execution engine.

## 2. Entry points & architecture

- **Entry file(s)**: `vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json:1-26` — declares the pstack plugin and its `skills`/`agents` roots; `vendors/cursor-plugins/pstack/automations/benny/FOR_AGENTS.md:1-73` — setup entry and user-intent contract for the two automations.
- **Core abstractions**:
  - `poteto-agent` — fresh background delegate for a single helper task; the parent owns the playbook and task list (`vendors/cursor-plugins/pstack/agents/poteto-agent.md:1-10`).
  - `comment-sicko` — report-only comment/dead-code critic with an explicit keep-list (`vendors/cursor-plugins/pstack/agents/comment-sicko.md:1-35`).
  - Benny triage automation — immutable Slack coordinates, evidence/classification, cause-aware routing, tracker dedupe, and one thread verdict (`vendors/cursor-plugins/pstack/automations/benny/skills/triage-issue-reports/SKILL.md:1-73`).
  - Benny reproduce/fix automation — trusted marker handoff, real-UI double reproduction, existing-fix verification, bounded root-cause fix, and draft PR proof (`vendors/cursor-plugins/pstack/automations/benny/skills/reproduce-and-fix-issues/SKILL.md:1-78`).
  - Claude adapter tables — maps Cursor tools, agent fields, model slugs, paths, and Cursor-only features to Claude Code equivalents or explicit skips (`vendors/cursor-plugins/pstack/docs/harness/claude-code.md:1-75`).
- **Cross-package dependencies**: pstack skills referenced by name (`how`, `why`, `tdd`, `unslop`, and principle skills); Benny's `control-adapter.md` and feature map; configured Slack and issue-tracker actions; repository history and draft pull-request action.
- **External runtime dependencies**: Cursor plugin/Automations, Slack thread and attachment actions, tracker integration, UI control adapter, repository/PR provider, and optional Cursor cloud-agent worktrees. No application framework is required by the surface itself.

## 3. Pattern catalog

### 3.1 Explicit multi-harness name and capability mapping

- **Where**: `vendors/cursor-plugins/pstack/docs/harness/claude-code.md:8-55`
- **What it does**: The adapter uses tables rather than pretending Cursor and Claude Code have the same API. It maps `Task` to `Agent`, `AskQuestion` to `AskUserQuestion`, Cursor's `generalPurpose`/`readonly`/`environment: cloud` fields to Claude's `subagent_type`/`Explore`/`isolation: worktree`, and drops unsupported fields with an explicit `Skipped in Claude Code` rule. It also maps model names, plugin paths, rule locations, and transcript locations.
- **Why it's interesting**: Harness drift is a direct risk for Canon's provider projections: a skill can appear installed while its invocation fields, role names, or isolation semantics are wrong. The table makes lossy translation visible and gives operators a stable place to add a new provider without silently passing unsupported arguments.
- **Canon applicability**: **Canon has it partially**: provider selection and projection are explicit in `canon/skills/SKILL.src.md:133-154`, and the allowed provider set/selection precedence is implemented in `crates/canon-cli/src/skills.rs:24-32`; however, Canon's projection contract does not expose a per-provider command/field mapping or an explicit unsupported-feature report comparable to the Claude table. The adapter response boundary is provider-neutral and read-only (`crates/canon-cli/src/adapter.rs:1-6`), but it validates a response after execution rather than translating a role invocation before dispatch.
- **Effort to lift**: moderate.

### 3.2 Fail-closed, immutable-coordinate automation handoff

- **Where**: `vendors/cursor-plugins/pstack/automations/benny/FOR_AGENTS.md:5-35`; `vendors/cursor-plugins/pstack/automations/benny/skills/triage-issue-reports/SKILL.md:10-31`; `vendors/cursor-plugins/pstack/automations/benny/skills/reproduce-and-fix-issues/SKILL.md:10-46`
- **What it does**: Each run freezes the source channel and root thread timestamp before delegation, requires a parent preflight before writes, and permits only one thread reply; missing, deleted, inaccessible, or uncertain parents cause no writes. The triage marker is a small handoff envelope (`[benny:bug]`, `[benny:performance]`, or `[benny:other]`, optionally with a tracker URL) accepted only from a configured identity and exact thread. Workers are read-only, receive no Slack credentials, and are explicitly forbidden from all Slack write actions.
- **Why it's interesting**: This is a concrete cross-agent boundary: immutable join keys, a closed handoff vocabulary, actor authentication, and fail-closed behavior prevent duplicate tickets, root-channel posts, and delegated side effects. It is especially useful for long-running orchestrators where the source context may change while workers run.
- **Canon applicability**: **Canon has it partially**: Canon requires distinct actor/session identities for units and reviewers (`canon/skills/reference/role-orchestrator.md:48-56`) and uses immutable join-keyed, append-only records (`canon/skills/reference/canon-plugins.md:1-20`), but it has no equivalent external-event trigger contract that freezes source coordinates and authenticates a compact handoff marker. `canon/skills/reference/canon-context.md:60-84` validates provider-neutral capabilities but explicitly does not enforce them, so the worker-side no-write rule remains a missing orchestration boundary.
- **Effort to lift**: significant.

### 3.3 Overnight loop with predicate, isolation, discard, and decision log

- **Where**: `vendors/cursor-plugins/pstack/docs/guide/07-overnight.md:1-54`
- **What it does**: The overnight contract requires a goal, a checkable finish predicate, permissions, a fresh worktree, a decision log, and an escape hatch. Each iteration checks the predicate, makes one smallest justified change, verifies the real artifact, commits on progress or discards on no progress, then appends one decision row. The morning audit stores time, phase, decision, reason, evidence pointer, and result in TSV and routes the trail to a different-model reviewer.
- **Why it's interesting**: It turns “run for hours” into a bounded state machine with observable progress and a non-progress branch. The decision log preserves why a change was kept or discarded, while worktree/cloud isolation reduces collisions; this is a useful dogfood contract for Canon's orchestrator and evidence lifecycle.
- **Canon applicability**: **Canon has it partially**: the loop and role split are present in `canon/skills/SKILL.src.md:45-57`, and orchestrator status has deterministic `next`/`waitingOn` ownership (`canon/skills/reference/role-orchestrator.md:24-29`). Canon records test evidence and reviews, but no first-class long-run iteration/decision-log record with progress/no-progress disposition, heartbeat, or pause/resume checkpoint. `canon/skills/reference/canon-gate.md:549-563` covers artifact integrity, not an overnight control loop.
- **Effort to lift**: moderate.

### 3.4 Stage-gated automation with evidence-producing stop states

- **Where**: `vendors/cursor-plugins/pstack/automations/benny/skills/reproduce-and-fix-issues/SKILL.md:48-78` and `:147-240`; `vendors/cursor-plugins/pstack/docs/guide/07-overnight.md:126-145`
- **What it does**: Benny makes triage, repro, verification, fix, and PR creation separate stages. Every stage can stop: an untrusted/missing marker, explicit human ownership, missing control-adapter capability, no confirmed double repro, a plausible existing fix, failed before/after proof, or PR failure. Successful repro requires the same exact UI symptom twice, screenshots/recording/state cross-checks, and baseline-versus-patched proof before a draft PR; no merge or deploy is allowed.
- **Why it's interesting**: The pipeline treats negative outcomes as durable operational results rather than exceptions to hide. It also distinguishes evidence sources from fix ownership and keeps a bounded fix behind independent gates, which prevents an overnight automation from converting a weak report into an unreviewed change.
- **Canon applicability**: **Canon has it partially, with a stronger final ship gate**: failure scenarios and evidence are required by the working loop (`canon/skills/SKILL.src.md:45-57`), and `verifying → shipped` is fail-closed on scenario verdicts and review (`crates/canon-cli/src/subject.rs:28-43`, `crates/canon-cli/src/subject.rs:68-87`). Missing are reusable stage-specific stop-state records and a standard “baseline/after artifact pair” contract for UI or external automation runs; provider response validation in `crates/canon-cli/src/adapter.rs:69-84` is only an envelope check.
- **Effort to lift**: significant.

### 3.5 Agent definitions that constrain delegation and preserve parent ownership

- **Where**: `vendors/cursor-plugins/pstack/agents/poteto-agent.md:1-10`; `vendors/cursor-plugins/pstack/agents/comment-sicko.md:1-35`
- **What it does**: `poteto-agent` is explicitly a delegate, not the runner: the parent owns the playbook, todo list, and every spawn, and a fresh agent is created per task. It requires reading the complete governing skill before work and forbids substituting a generic agent. `comment-sicko` is a report-only specialist with a narrowly defined remit, exact output fields, and no code edits.
- **Why it's interesting**: These definitions encode role boundaries in the agent catalog itself, not only in prose sent at dispatch time. They reduce context drift and make the worker's allowed lifecycle/output legible to a coordinator.
- **Canon applicability**: **Canon has it partially**: role references define planner/implementer/reviewer/orchestrator responsibilities and require separate reviewer sessions (`canon/skills/reference/role-orchestrator.md:31-56`), while `crates/canon-cli/src/status.rs:31-39` filters next steps by role. Canon does not currently project provider-specific agent definitions with allowed scope, required pre-read, or report-only/editing capability; its provider-neutral adapter envelope declares capabilities but does not enforce them (`canon/skills/reference/canon-context.md:71-84`).
- **Effort to lift**: moderate.

## 4. Risks & incompatibilities

- **Effect version**: not applicable; this surface is Markdown/YAML/JSON and external automation configuration, not an Effect application.
- **Framework**: not applicable; no UI framework is included. Benny depends on a target application's configured control adapter and real UI, so adapter semantics must be treated as an external prerequisite.
- **Heavy deps**: Cursor Automations, Slack actions, tracker integrations, and cloud/worktree execution are operational dependencies rather than Rust dependencies; they can be unavailable or provider-specific.
- **Coupling**: Benny only makes sense with Slack thread APIs, a tracker, a repository/PR action, and a completed feature map/control adapter. The Claude adapter is coupled to Claude Code's tool schema and filesystem layout. The agent definitions are coupled to pstack's `poteto-mode` skill and cannot be copied as generic Canon roles without their governing playbooks.
- **TOS-violation patterns**: none observed in the requested surface. It uses configured provider actions and does not impersonate model clients, forge request fingerprints, or evade rate limits.
- **License footnote**: the manifest identifies MIT licensing (`vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json:1-26`); adoption should remain a clean-room reimplementation from this audit rather than a verbatim lift.

## 5. Recommended actions

1. **INSPIRE** explicit harness mapping and unsupported-feature reporting → `canon/skills/reference/canon-plugins.md`, `canon/skills/reference/canon-context.md`, and `crates/canon-cli/src/skills.rs` — Canon already selects providers and validates provider-neutral envelopes, but a generated provider capability/mapping table would make Cursor/Claude/Codex differences and lossy dispatch visible.
2. **EXTRACT** a provider-neutral worker capability declaration and validation contract → `crates/canon-cli/src/adapter.rs` and `canon/skills/reference/canon-context.md` — reuse Canon's existing strict envelope, then add a reviewed dispatch-side declaration for read-only/worktree/secrets boundaries without making the validator pretend to sandbox providers.
3. **INSPIRE** an orchestrator long-run checkpoint/decision-log record → `canon/skills/reference/role-orchestrator.md`, `crates/canon-cli/src/status.rs`, and the Canon evidence/report surfaces — pstack's predicate, one-iteration decision, discard branch, and pause/resume trail fill a concrete gap between session evidence and multi-day coordination.
4. **INSPIRE** stage stop-state and handoff records for external automations → `canon/skills/reference/canon-context.md`, `canon/skills/reference/canon-gate.md`, and `crates/canon-cli/src/adapter.rs` — represent authenticated trigger joins, closed handoff outcomes, and baseline/after artifact relationships while keeping the existing ship guard authoritative.
5. **SKIP** verbatim Benny Slack automation setup and Cursor Automations editor workflow → `vendors/cursor-plugins/pstack/automations/benny/FOR_AGENTS.md:1-73` — it is tightly coupled to Cursor Automations, Slack actions, external credentials, and a target app's control adapter; Canon should adopt the lifecycle concepts, not this integration.
6. **INSPIRE** declarative agent-role metadata (fresh-per-task, parent-owned, report-only) → `canon/skills/reference/role-*.md` and provider projections under `canon/skills/` — Canon's four roles cover lifecycle ownership, but capability and output restrictions should be visible in generated harness instructions.

## 6. Citations

- `vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json:1-26` — plugin identity, version, license, and exposed `skills`/`agents` roots.
- `vendors/cursor-plugins/pstack/docs/harness/claude-code.md:8-55` — Cursor-to-Claude tool, subagent, model, path, and isolation mappings.
- `vendors/cursor-plugins/pstack/automations/benny/FOR_AGENTS.md:5-35` — two-automation intent, immutable thread boundary, worker restrictions, and fail-closed requirements.
- `vendors/cursor-plugins/pstack/automations/benny/skills/triage-issue-reports/SKILL.md:10-31` — triage safety rules and immutable source-coordinate lifecycle.
- `vendors/cursor-plugins/pstack/automations/benny/skills/reproduce-and-fix-issues/SKILL.md:10-46` — repro/fix safety rules, worker isolation, and double-repro requirement.
- `vendors/cursor-plugins/pstack/automations/benny/skills/reproduce-and-fix-issues/SKILL.md:48-78` — exact trusted-marker handoff and timeout/stop behavior.
- `vendors/cursor-plugins/pstack/docs/guide/07-overnight.md:1-54` — overnight contract, finish predicate, iteration loop, discard branch, and decision log.
- `vendors/cursor-plugins/pstack/agents/poteto-agent.md:1-10` — parent-owned delegate lifecycle and required governing-skill pre-read.
- `vendors/cursor-plugins/pstack/agents/comment-sicko.md:1-35` — report-only specialist contract and explicit no-edit boundary.
- `vendors/cursor-plugins/pstack/docs/guide/01-setup.md:8-34` — Cursor/Claude setup and model-role configuration differences.
