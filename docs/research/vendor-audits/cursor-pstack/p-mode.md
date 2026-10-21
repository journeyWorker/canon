# Surface — p-mode

**Vendor paths**: `vendors/cursor-plugins/pstack/skills/poteto-mode/`, `vendors/cursor-plugins/pstack/skills/poteto-help/`, `vendors/cursor-plugins/pstack/skills/setup-pstack/`  
**Scale**: 46 files, approximately 2,500 lines across one main mode, 23 playbooks, setup/help skills, scripts, and orchestration support.  
**Primary language(s)**: Markdown, TypeScript, shell, JSON.  
**Canon analog (if any)**: `canon/skills/reference/role-orchestrator.md`, `canon status`, `meta.next`, and role selection through `--as` or `CANON_ROLE`.

## 1. Overview

p-mode is a front door for rigorous agent work. The user states a goal and a completion condition, and `poteto-mode` selects one of 23 task playbooks, copies its steps into the active todo list, and invokes supporting skills as each step requires. The surface also configures model families and reasoning budgets by task type, including separate models for implementation, judgment, exploration, panels, and verification. `poteto-help` is a non-executing router for users who need to choose a skill or phrase a prompt, while `setup-pstack` persists model and budget choices for later sessions.

The surface is coupled to Cursor's task and custom-mode mechanisms. Its routing and lifecycle rules are mostly declarative Markdown, but its orchestration playbook also depends on Cursor cloud agents, worktrees, `/loop`, a durable orchestration store, and helper scripts. Canon already has a deterministic state machine, role-specific command views, `canon status`, and write-returned `meta.next`, but it does not currently provide an equivalent task-type playbook registry or model-routing policy.

## 2. Entry points & architecture

- **Entry file(s)**:
  - `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:1-35` defines the mode's front door, trigger rules, non-negotiables, and principles index.
  - `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:90-125` defines subagent model selection, fresh-agent policy, and response obligations.
  - `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:145-190` defines the playbook catalog and task-to-playbook matching.
  - `vendors/cursor-plugins/pstack/skills/setup-pstack/SKILL.md:1-31` defines persistent model and budget configuration.
  - `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:1-45` defines the non-executing help router.
- **Core abstractions**:
  - **Poteto mode**. A dispatcher that matches user intent to a named playbook and carries the selected steps across turns.
  - **Playbook**. A lifecycle-specific step list such as Investigation, Feature, Autonomous run, Pause safely, or Orchestrate.
  - **Task-role model mapping**. A persisted table mapping task classes and panel roles to model slugs or parent-model aliases.
  - **Subagent policy**. Rules for model effort, speed, fresh versus resumed agents, worktree isolation, and verifier independence.
  - **Help map**. A read-only lookup from user intent to skills, playbooks, guide pages, and example prompts.
  - **Orchestration store**. Durable files such as `preferences.md`, `units.tsv`, `frontier.json`, `ledger.tsv`, `inbox/`, `gates.md`, and `decisions.tsv` for long-lived programs.
- **Cross-package dependencies**:
  - Cursor built-in `/loop`, Custom Modes, AskQuestion, Task subagents, and cloud-agent execution.
  - Cursor's built-in `/create-skill` and related skill infrastructure.
  - The companion `cursor-team-kit` plugin for `deslop`, `control-cli`, and `control-ui`.
  - The pstack skills referenced by the mode, including `how`, `why`, `architect`, `arena`, `swarm`, `interrogate`, `show-me-your-work`, and verification skills.
- **External runtime dependencies**:
  - Cursor's task model slugs and effort levels.
  - Git branches and worktrees.
  - GitHub CLI or Origin CLI for PR and stack operations.
  - Bun and TypeScript for the orchestration helper scripts.
  - Cursor cloud agents for isolated workers and verifiers.

## 3. Pattern catalog

### 3.1 Intent-driven playbook registry

- **Where**: `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:145-190`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/investigation.md:1-15`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/feature.md:1-28`
- **What it does**: The mode maintains a registry of task classes with explicit lifecycle semantics. Investigation means read-only cited analysis, Bug fix means reproduce, root-cause, fix, and verify, Feature means design, delegate, verify, and prepare a PR, and Autonomous run means drive a declared predicate until it is met. The selected playbook's steps are copied into the todo list, and skipped steps remain visible with a `skip: <reason>` entry rather than disappearing.
- **Why it's interesting**: Canon's current workflow is state- and role-driven, but the next command is primarily derived from ledger state, policy, brief coverage, evidence, and reviews. It does not expose a first-class registry for task intent such as investigation, bug fix, migration, prototype, or long-running execution. A task-class layer could improve how agents select the correct canon role reference and command sequence without replacing canon's existing gates.
- **Canon applicability**: **Partial.** Canon already exposes role-aware next steps through `canon status`, and its role references define phase-specific command sets. `canon/skills/reference/role-orchestrator.md:23-29` says `canon status` reports each next step with its owning role, while `crates/canon-cli/src/status.rs:31-40` defines role-filtered steps and `waitingOn`. The missing piece is a task-intent-to-playbook registry that selects a workflow before or alongside subject state. This should complement, not override, the deterministic status engine.
- **Effort to lift**: moderate.

### 3.2 Persistent model routing and reasoning budgets

- **Where**: `vendors/cursor-plugins/pstack/skills/setup-pstack/SKILL.md:15-31`; `vendors/cursor-plugins/pstack/skills/setup-pstack/SKILL.md:43-85`; `vendors/cursor-plugins/pstack/README.md:35-41`
- **What it does**: `setup-pstack` writes a persistent rule containing a budget label and one model mapping per task type or panel. The four budgets map to effort levels, from `small` and `medium` through `large` and `unlimited`. Panel values are lists, and one subagent runs per list entry, so the list itself controls fan-out. `auto` and `inherit-parent` omit the child model so the child inherits the parent chat's model.
- **Why it's interesting**: Canon records actor IDs, session IDs, roles, evidence, reviews, and run provenance, but it does not currently express a policy for matching task complexity to model family, reasoning budget, or verifier diversity. A typed routing record could make expensive or judgment-heavy work auditable and could prevent a reviewer from silently running with the same capability profile as the implementer.
- **Canon applicability**: **Missing, with related provenance already present.** Canon's role system identifies planner, implementer, reviewer, and orchestrator, but not model or effort. `crates/canon-cli/src/role.rs:1-13` describes role declaration as harness guidance, and `crates/canon-cli/src/role.rs:74-85` validates only the four role names. `crates/canon-cli/src/dispatch.rs:699-702` records injected guidance in a run, but no model-routing or reasoning-budget field is surfaced in the cited role or dispatch contract. A future context-pack or dispatch manifest could carry a normalized routing policy.
- **Effort to lift**: significant.

### 3.3 Fresh subagent invocation with explicit role and verifier separation

- **Where**: `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:90-125`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/feature.md:12-28`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/orchestrate.md:1-40`
- **What it does**: The mode separates the parent runner from its delegates. Code work uses `poteto-agent`, routed workflow skills choose their own diverse subagent types, and the parent owns synthesis. New work normally receives a fresh agent with a consolidated brief. Long-running orchestration uses one writer per worktree or branch and places verifiers on a different model family from workers.
- **Why it's interesting**: This gives concrete operational meaning to “independent review” and “different session.” It also prevents context loss from interrupt-chained resumes and prevents sibling workers from serializing through shared files. The distinction between a worker's self-report and a verifier's independent verdict is especially useful for canon's evidence and review records.
- **Canon applicability**: **Partial.** Canon already requires a separate reviewer session and records actor/session identity. `canon/skills/reference/role-orchestrator.md:31-48` requires one agent per role and tells the orchestrator to declare the role through `CANON_ROLE` or `--as`. The same file at lines 48-55 requires the reviewer to be a separate process with its own session ID. What is missing is a standard spawn brief shape, fresh-agent default, model-family separation, and explicit worktree ownership policy attached to a canon dispatch.
- **Effort to lift**: moderate.

### 3.4 State-driven continuation through `meta.next` and visible waiting owners

- **Where**: `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:145-190`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/feature.md:1-28`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/orchestrate.md:75-125`
- **What it does**: p-mode keeps the selected playbook's steps in a visible todo list and uses the current task context to decide whether to continue, switch with “new task,” or stop at a lifecycle boundary. Canon's analogous mechanism is stronger at the command level. Every write computes the next step from the same status engine and returns it through `meta.next`, filtered to the touched scope and declared role. The vendor pattern is therefore useful mainly as a user-facing playbook layer above canon's existing state engine.
- **Why it's interesting**: Agents often re-run a broad status command after every write, lose the intended playbook, or treat a follow-up question as the next implementation step. A visible playbook identity plus a scoped continuation token can preserve intent while still allowing canon's gate-derived next command to remain authoritative.
- **Canon applicability**: **Already solved at the command-continuation level, partial at the task-intent level.** `crates/canon-cli/src/next.rs:1-16` states that writes and `canon status` share one deterministic next-step engine. `crates/canon-cli/src/next.rs:89-139` scopes returned steps to the touched subject and declared role. `crates/canon-cli/src/status.rs:392-420` filters owned steps and counts other owners. Canon does not currently persist a selected playbook or distinguish “new task” from continuation, so only the intent layer remains a possible adoption target.
- **Effort to lift**: trivial for a non-authoritative display field, moderate if persisted and used for routing.

### 3.5 Explicit stop points, resumability, and durable orchestration state

- **Where**: `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/autonomous-run.md:1-18`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/pause-safely.md:1-16`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/session-pickup.md:1-16`; `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/orchestrate.md:75-125`
- **What it does**: Autonomous runs begin with a checkable exit predicate and continue until that predicate is met or a genuine dead end is documented. Pause safely stops at an atomic boundary, writes a checkpoint, and records a cold-start resume note. Session pickup treats the prior trail as authoritative input, reconstructs branch and work state, and verifies inherited claims against the original goal. Orchestrate persists standing orders, units, frontier, ledger, inbox, gates, decisions, and derived status, with one writer per durable file.
- **Why it's interesting**: Canon already has explicit human approval stops, role ownership, state transitions, evidence gates, reviews, and reopen behavior. The vendor adds a useful operational layer for stop conditions that are not gate failures, including cancellation, compaction, agent death, timeouts, queue draining, and resumption after a harness restart.
- **Canon applicability**: **Partial.** Canon has explicit human stops and state transitions. `canon/skills/reference/role-orchestrator.md:57-65` requires the orchestrator to stop at a human approval step and not start implementation before approval. `canon/skills/reference/role-orchestrator.md:69-80` defines the subject transition path and reopen behavior. Canon also has dispatch run lifecycle records, including a running manifest in `crates/canon-cli/src/dispatch.rs:699-756`. It lacks a standard pause/resume note, durable child-unit queue, heartbeat or stuck-agent protocol, and explicit predicate-based autonomous-run record.
- **Effort to lift**: significant.

### 3.6 Non-executing help and prompt shaping

- **Where**: `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:1-20`; `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:21-45`; `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:70-115`
- **What it does**: `poteto-help` distinguishes a question about how to work from a request to perform work. It reads the relevant skill before recommending it, checks setup state when that changes the answer, returns a prompt the user can send, and does not start a costly run. It also gives close-call distinctions such as `/how` versus `/why`, `/arena` versus `/swarm`, and `/interrogate` versus `/blast-radius`.
- **Why it's interesting**: Canon's `canon status` and role references are operationally precise, but an agent entering a canon repository still needs a low-cost way to choose the correct role, command family, or reference document before making a write. A non-executing guidance surface could reduce accidental role mixing and explain when a human gate is expected without changing repository state.
- **Canon applicability**: **Partial.** Canon's generated onboarding block already points agents to `canon status`, the skill, declared roles, and the eight-step loop. `crates/canon-cli/src/init.rs:208-235` defines that block and states that agents should load only their role reference. However, there is no dedicated help command or task-to-reference map that answers “which canon role or playbook applies?” before execution. This could be added as documentation or a read-only `canon help workflow` surface.
- **Effort to lift**: moderate.

## 4. Risks & incompatibilities

- **Effect version**: Not applicable. The surface is Markdown, TypeScript, shell, and Cursor skill metadata. It does not use Effect.
- **Framework**: Not applicable. There is no UI framework dependency. Cursor Custom Modes and cloud agents are host-platform features rather than application frameworks.
- **Heavy deps**: The orchestration helper depends on Bun, TypeScript, Git, and forge CLIs. The main mode itself is lightweight, but the Orchestrate playbook introduces a substantial durable-store and queue protocol.
- **Coupling**: Strong coupling to Cursor's Task schema, model slugs, effort tokens, Custom Modes, `/loop`, cloud worktrees, and companion `cursor-team-kit` skills. The model-routing tables cannot be lifted verbatim into canon without a provider-neutral model capability schema.
- **TOS-violation patterns**: None observed in this surface. The reviewed material does not impersonate provider clients, forge request fingerprints, or evade provider rate limits.
- **License footnote**: The audit recommends clean-room reimplementation from these findings. No verbatim lift is required.

## 5. Recommended actions

1. **INSPIRE** the task-intent registry in `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:145-190` for `canon/skills/reference/` and a future read-only workflow-help surface. Canon should map intents such as investigation, feature, bug fix, migration, review, and long-running execution to the relevant role reference and first command, while keeping `canon status` authoritative for actual repository state.
2. **EXTRACT** the explicit four-level reasoning-budget concept from `vendors/cursor-plugins/pstack/skills/setup-pstack/SKILL.md:15-31` into a provider-neutral dispatch or context-pack policy. The concrete canon target is `crates/canon-cli/src/dispatch.rs` plus a typed context or policy schema that records requested capability tier, model family, and whether a child inherits the parent model.
3. **INSPIRE** the per-task and per-panel model mapping from `vendors/cursor-plugins/pstack/skills/setup-pstack/SKILL.md:43-85` for canon's dispatch provenance. Do not copy Cursor model slugs. Record normalized routing intent such as `implementation`, `judgment`, `exploration`, `review`, and `verification`, with an optional provider-resolved model identity.
4. **EXTRACT** the fresh-worker and independent-verifier contract from `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:90-125` and `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/orchestrate.md:1-40` into `canon/skills/reference/role-orchestrator.md` and dispatch manifests. The target should require exclusive worktree or branch ownership, fresh-session defaults, and a verifier that is independent by session and preferably by model family.
5. **EXTRACT** the durable stop and resume record shape from `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/pause-safely.md:1-16` and `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/session-pickup.md:1-16` into a future `canon dispatch pause|resume` or session-handoff record. The record should bind intent, current state, verified claims, next command, key files, and known gotchas without weakening existing gate checks.
6. **INSPIRE** the predicate-first autonomous-run rule from `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/autonomous-run.md:1-18` for a future canon run record. A run should declare a checkable completion predicate and record each iteration's evidence, rather than treating a green build or agent self-report as completion.
7. **INSPIRE** the non-executing help router from `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:1-20` for a read-only `canon help workflow` command or expanded generated skill documentation. It should explain the applicable role, first command, expected human stop, and whether the request is read-only or write-bearing.
8. **SKIP** direct persistence of Cursor-specific Custom Mode state, `/loop` syntax, and Cursor model slugs. These are host-specific and conflict with canon's harness-neutral role declaration through `CANON_ROLE` or `--as`, documented at `crates/canon-cli/src/role.rs:1-13` and `crates/canon-cli/src/role.rs:74-85`.
9. **SKIP** replacing `meta.next` or `canon status` with a vendor-style todo engine. Canon already derives both from one deterministic status engine, as documented at `crates/canon-cli/src/next.rs:1-16` and `crates/canon-cli/src/next.rs:89-139`. A playbook label may be additive, but it must not become a second source of truth.
10. **SKIP** treating a model choice as equivalent to a role. Canon roles identify ownership and permission guidance, while model routing identifies execution capability. Keep them as separate typed fields and preserve session, actor, role, and evidence provenance.

## 6. Citations

- `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:1-35` defines the front-door mode, trigger rules, and non-negotiables.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:90-125` defines model selection, reasoning effort, fresh subagents, and verifier expectations.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:145-190` catalogs the task playbooks and their matching semantics.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/investigation.md:1-15` defines read-only investigation as a separate lifecycle with cited output and no PR.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/feature.md:1-28` defines blocking gates, workstream decomposition, delegation, verification, and ordered delivery.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/autonomous-run.md:1-18` defines predicate-first execution, per-iteration verification, and stopping conditions.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/pause-safely.md:1-16` defines safe-boundary stopping and cold-start resume notes.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/session-pickup.md:1-16` defines authoritative trail reconstruction and inherited-claim verification.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/orchestrate.md:1-40` defines coordinator, worker, verifier, fresh-agent, and exclusive-worktree roles.
- `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/orchestrate.md:75-125` defines durable orchestration files, queue draining, verification ledgers, liveness, retry, and escalation.
- `vendors/cursor-plugins/pstack/skills/setup-pstack/SKILL.md:15-31` defines budget levels, effort mapping, model inheritance aliases, and panel fan-out.
- `vendors/cursor-plugins/pstack/skills/setup-pstack/SKILL.md:43-85` defines model validation, persisted role mappings, and the generated rule shape.
- `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:1-20` defines non-executing help behavior and source-linked routing.
- `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:21-45` defines setup-state checks and cost-aware guidance.
- `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:70-115` defines the skill-selection map and close-call distinctions.
- `vendors/cursor-plugins/pstack/README.md:35-41` describes setup, default model split, and the front-door workflow.
- `vendors/cursor-plugins/pstack/README.md:43-76` describes `/poteto-mode`, playbook selection, and the task-class catalog.
- `canon/skills/reference/role-orchestrator.md:23-29` defines status ownership, `waitingOn`, and write-returned continuation.
- `canon/skills/reference/role-orchestrator.md:31-55` defines role spawning and separate reviewer sessions.
- `canon/skills/reference/role-orchestrator.md:57-65` defines the human approval stop.
- `canon/skills/reference/role-orchestrator.md:69-80` defines subject transitions and gate-driven movement.
- `crates/canon-cli/src/next.rs:1-16` defines deterministic `meta.next`, role filtering, and the shared status engine.
- `crates/canon-cli/src/next.rs:89-139` defines scoped next steps and `waitingOn` metadata.
- `crates/canon-cli/src/status.rs:31-40` defines role-aware status output.
- `crates/canon-cli/src/status.rs:392-420` defines owned-step filtering and waiting-owner counts.
- `crates/canon-cli/src/role.rs:1-13` defines role declaration as harness guidance.
- `crates/canon-cli/src/role.rs:74-85` defines `--as` precedence and `CANON_ROLE` validation.
- `crates/canon-cli/src/dispatch.rs:699-756` defines dispatch run creation, injected guidance, and persisted run manifests.
- `crates/canon-cli/src/init.rs:208-235` defines canon's generated agent instructions, role loading, human stop, review, and transition loop.
