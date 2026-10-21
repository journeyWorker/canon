# Surface — a-define-plan

**Vendor paths**: `vendors/addyosmani-agent-skills/skills/spec-driven-development/SKILL.md`, `vendors/addyosmani-agent-skills/skills/idea-refine/SKILL.md`, `vendors/addyosmani-agent-skills/skills/interview-me/SKILL.md`, `vendors/addyosmani-agent-skills/skills/planning-and-task-breakdown/SKILL.md`, `vendors/addyosmani-agent-skills/skills/constraint-driven-development/SKILL.md`, `vendors/addyosmani-agent-skills/skills/doubt-driven-development/SKILL.md`, `vendors/addyosmani-agent-skills/skills/source-driven-development/SKILL.md`, `vendors/addyosmani-agent-skills/skills/context-engineering/SKILL.md`, `vendors/addyosmani-agent-skills/skills/using-agent-skills/SKILL.md`, `vendors/addyosmani-agent-skills/commands/`, `vendors/addyosmani-agent-skills/_vendor_claude/c…
**Scale**: 18 primary skill/command entry files, approximately 2.5k LOC, plus supporting references, scripts, hooks, fixtures, and evals.
**Primary language(s)**: Markdown and TOML; shell scripts and JavaScript support the pack.
**Canon analog (if any)**: `canon/skills/reference/role-planner.md` and `crates/canon-cli/src/brief_template.rs` — Canon already gates a PRD brief and human approval, but does not provide the vendor's distinct pre-brief interview, capability-map, explicit task-plan, or quality-bar artifacts as first-class define-phase surfaces.

## 1. Overview

This surface is a define/plan workflow for coding agents. It routes an incoming request through intent extraction, idea refinement, specification, quality constraints, task decomposition, context selection, source verification, and adversarial doubt before implementation. The `using-agent-skills` meta-skill supplies a phase-to-skill routing table, while `commands/` and `_vendor_claude/commands/` expose entry points such as `/spec`, `/planning`, `/constraints`, `/build`, `/review`, and `/ship`. The skills are coupled by prose and expected files (`SPEC.md`, `tasks/plan.md`, `tasks/todo.md`, `CONSTRAINTS.md`) rather than by a shared typed data model; the human is repeatedly asked to approve intermediate artifacts.

## 2. Entry points & architecture

- **Entry file(s)**: `vendors/addyosmani-agent-skills/skills/using-agent-skills/SKILL.md:1-38` — phase routing and core operating behaviors; `vendors/addyosmani-agent-skills/commands/spec.toml:1-16` and `vendors/addyosmani-agent-skills/commands/planning.toml:1-13` — provider-neutral command prompts; `vendors/addyosmani-agent-skills/_vendor_claude/commands/spec.md:1-16` and `vendors/addyosmani-agent-skills/_vendor_claude/commands/plan.md:1-16` — Claude command projections.
- **Core abstractions**:
  - `interview-me` hypothesis/guess loop — one question at a time until an explicit statement-of-intent confirmation (`vendors/addyosmani-agent-skills/skills/interview-me/SKILL.md:29-85`).
  - `idea-refine` three-phase divergent/convergent/sharpen workflow — produces a one-page concept with assumptions and a Not Doing list (`vendors/addyosmani-agent-skills/skills/idea-refine/SKILL.md:35-74`).
  - `spec-driven-development` capability map and four gated phases — maps multi-capability work, then Specify → Plan → Tasks → Implement (`vendors/addyosmani-agent-skills/skills/spec-driven-development/SKILL.md:17-43`).
  - `planning-and-task-breakdown` dependency graph and vertical slice — task records carry acceptance, verification, dependencies, files, and scope (`vendors/addyosmani-agent-skills/skills/planning-and-task-breakdown/SKILL.md:25-90`).
  - `constraint-driven-development` `CONSTRAINTS.md` contract — floor, numeric rules, measured ratchets, exceptions, and lifecycle placement (`vendors/addyosmani-agent-skills/skills/constraint-driven-development/SKILL.md:18-28`, `:65-105`).
  - `doubt-driven-development` extracted ARTIFACT + CONTRACT reviewed by a fresh adversarial context, then reconciled (`vendors/addyosmani-agent-skills/skills/doubt-driven-development/SKILL.md:44-104`).
- **Cross-package dependencies**: The selected skills refer to neighboring vendor skills, especially `incremental-implementation`, `test-driven-development`, `code-review-and-quality`, `debugging-and-error-recovery`, `security-and-hardening`, `performance-optimization`, `api-and-interface-design`, and `shipping-and-launch`; the command prompts invoke those skills rather than importing code (`vendors/addyosmani-agent-skills/skills/spec-driven-development/SKILL.md:205-258`, `vendors/addyosmani-agent-skills/commands/build.toml:1-79`).
- **External runtime dependencies**: No runtime library is required by the selected Markdown/TOML entry points. The workflow assumes a host agent CLI with interactive questioning and, for `/ship` and doubt, isolated subagents; optional quality tools include `tsc`, linters, gitleaks, Semgrep, osv-scanner, Lighthouse, axe, dependency-cruiser, and Stryker (`vendors/addyosmani-agent-skills/skills/constraint-driven-development/SKILL.md:106-165`).

## 3. Pattern catalog

### 3.1. Explicit intent interview before defining scope

- **Where**: `vendors/addyosmani-agent-skills/skills/interview-me/SKILL.md:29-85`
- **What it does**: The agent writes a one-sentence hypothesis with a confidence percentage, asks exactly one focused question with a guessed answer, then iterates toward a structured restatement containing outcome, user, why now, success, constraint, and out of scope. It requires explicit confirmation and has a terminal stop rule: after confirmation, offer downstream paths but do not start the spec or implementation in the same turn.
- **Why it's interesting**: It prevents a conventional label such as “dashboard” or “build X” from silently becoming requirements. It also makes uncertainty inspectable before a PRD exists, which is useful where Canon's brief currently records questions and assumptions but does not prescribe how to discover them.
- **Canon applicability**: **Canon has partial coverage**: `canon/skills/reference/role-planner.md:32-53` requires a PRD with Questions and assumptions and a human stop, while `canon/skills/reference/brief-clarify.md:1-30` makes unresolved decisions explicit. Canon has no confidence-bearing hypothesis, one-question cadence, explicit statement-of-intent artifact, or dedicated pre-brief command. A concrete target is a planner-side `canon brief interview` or an optional intent section/pass, without weakening the existing approval gate.
- **Effort to lift**: moderate.

### 3.2. Capability map before a multi-capability specification

- **Where**: `vendors/addyosmani-agent-skills/skills/spec-driven-development/SKILL.md:17-43`, `:46-103`; command entry `vendors/addyosmani-agent-skills/commands/spec.toml:1-16`
- **What it does**: A bundled request is first represented as a small table of stable kebab-case module ids, responsibilities, dependencies, and build order. The human reviews that map before each module receives its own spec; each spec then covers objective, commands, project structure, style, testing, boundaries, success criteria, and open questions, and the workflow stops after saving the spec for later approval.
- **Why it's interesting**: It separates boundary/dependency decisions from detailed requirements and prevents a large initiative from becoming one monolithic plan. Stable ids give later plans and commands a selection key rather than relying on filename or prose guessing.
- **Canon applicability**: **Canon has partial coverage, with a conflict in artifact granularity**: `crates/canon-cli/src/brief_template.rs:53-63` has one ordered feature brief containing Intent, Desired outcome, User flow, Step detail, Acceptance, Questions, and Out of scope; `canon/skills/reference/role-planner.md:32-53` requires one subject/change brief and flow ids. Canon does not model an approved capability graph or recurse into module briefs. Canon's strict flow/acceptance structure should remain authoritative; the target is an optional `capabilities`/dependency layer in `canon context` or a planner note that links multiple changes, not a second untracked `SPEC.md` system.
- **Effort to lift**: significant.

### 3.3. Acceptance-shaped, dependency-ordered vertical task slices

- **Where**: `vendors/addyosmani-agent-skills/skills/planning-and-task-breakdown/SKILL.md:25-90`, `:91-160`; command entry `vendors/addyosmani-agent-skills/commands/planning.toml:1-13`
- **What it does**: Planning begins read-only, builds a dependency graph, then slices vertically so each task delivers one complete testable path instead of a horizontal database/API/UI layer. Each task has a description, testable acceptance criteria, verification commands/checks, dependencies, likely files, and a size; checkpoints appear after phases, and tasks larger than roughly five to eight files are split.
- **Why it's interesting**: Canon's planner hands off after the brief, while implementer units and evidence must still turn flow steps into executable work. A normalized task shape and explicit dependency/checkpoint data could make dispatch and handoff more precise without treating a checkbox as proof.
- **Canon applicability**: **Canon is partial**: `canon/skills/reference/role-implementer.md:26-59` defines scenario creation, units, test naming, and evidence binding, and `canon/skills/reference/canon-vocab.md:65-86` provides validated typed task atoms with status and evidence attributes. Canon does not require every task to carry dependency order, file scope, checkpoint, or task-level verification before dispatch. Target `canon-vocab`/typed task atoms and `canon dispatch`, while preserving `canon evidence add` as the proof rather than accepting a plan checkbox.
- **Effort to lift**: moderate.

### 3.4. A written, numeric quality bar with ratchets and lifecycle placement

- **Where**: `vendors/addyosmani-agent-skills/skills/constraint-driven-development/SKILL.md:18-28`, `:65-105`, `:166-220`, `:250-285`; command entry `vendors/addyosmani-agent-skills/commands/constraints.toml:1-33`
- **What it does**: The skill creates `CONSTRAINTS.md` with an always-enforced floor, numeric dimensions tied to exact commands, measured-but-not-yet-enforced values, and exceptions with owner and expiry. It places fast checks in BUILD, related tests at VERIFY, expensive checks at REVIEW/CI, and guards the bar against lowered thresholds, skipped/deleted tests, suppressions, stubs, and unexplained exceptions; it supplies “measure and hold” ratchet defaults when no target exists.
- **Why it's interesting**: It turns quality expectations into durable, reviewable policy instead of agent prose. The guard specifically models a common failure mode in autonomous work: changing the checker or threshold rather than fixing the implementation.
- **Canon applicability**: **Canon has partial coverage, but its policy is a different contract**: `canon/skills/reference/canon-gate.md:1-36` defines evidence coverage, verdicts, staleness, findings, and stable failure classes; `canon/skills/reference/canon-policy.md:65-90` requires human approval of `.canon/policy.yaml` and fails closed on policy drift. Canon does not provide a generic dimension/command/threshold table or a diff-scoped constraint-ratchet guard. Target `canon policy`/`canon gate check` only if the new dimensions can be represented as declared evidence requirements; otherwise **INSPIRE** a companion quality-bar manifest and guard, avoiding a second approval policy that conflicts with `.canon/policy.yaml`.
- **Effort to lift**: significant.

### 3.5. Fresh-context adversarial doubt before a non-trivial decision

- **Where**: `vendors/addyosmani-agent-skills/skills/doubt-driven-development/SKILL.md:29-44`, `:46-104`, `:105-151`
- **What it does**: The agent names a compact claim, extracts only the smallest ARTIFACT and CONTRACT, and sends those (not the claim or the author's reasoning) to a fresh reviewer with an issues-first prompt. Findings are reconciled as contract misread, actionable, trade-off, or noise; the loop stops after trivial findings, three cycles, or user override.
- **Why it's interesting**: It catches wrong direction while correction is cheap and explicitly prevents review bias from the author's conclusion. It also has a bounded lifecycle, unlike an unbounded “ask another agent” convention.
- **Canon applicability**: **Canon has partial coverage**: `canon/skills/reference/canon-review.md:184-189` and `canon/skills/reference/canon-gate.md:151-183` support independent reviewer sessions, signed reviews, and distinct actor/session requirements for in-scope scenarios. Canon's review is primarily post-evidence and scenario/change verdict oriented; it has no pre-implementation claim/contract artifact or bounded in-flight doubt cycle. Target a planner/implementer handoff field or `canon review` mode for pre-build decisions, but do not weaken the existing independent review identity rules.
- **Effort to lift**: moderate.

### 3.6. Context selection and source-backed decisions as reproducible inputs

- **Where**: `vendors/addyosmani-agent-skills/skills/context-engineering/SKILL.md:1-60`, `:61-115`; `vendors/addyosmani-agent-skills/skills/source-driven-development/SKILL.md:1-80`
- **What it does**: Context engineering orders persistent rules, relevant spec sections, source files, errors, and conversation history, then recommends selective loading and restartable session boundaries that preserve accepted scope, task state, files, checks, and unresolved risks. Source-driven development detects exact dependency versions, fetches the relevant official documentation, implements against it, and cites every framework-specific decision, explicitly treating fetched text as untrusted data.
- **Why it's interesting**: It treats context selection and external facts as inputs that should be scoped, versioned, and auditable rather than as an invisible prompt. This is especially useful for agent handoffs and replaying a plan under a different session.
- **Canon applicability**: **Canon has a strong partial/mostly-present analog**: `canon/skills/reference/canon-context.md:38-58` provides immutable repository-relative context packs with path safety and digest verification, while `:86-97` versions prompt inputs in the same registry; `canon/skills/reference/role-implementer.md:46-59` binds evidence to reports/artifacts. Missing pieces are a first-class source citation record and a compact handoff contract for accepted scope/task state. Target `context-pack`, `prompt`, and session/handoff metadata; **SKIP** copying the vendor's untyped “rules file” hierarchy as a parallel source of truth.
- **Effort to lift**: moderate.

## 4. Risks & incompatibilities

- **Effect version**: Not applicable; this surface is Markdown/TOML workflow guidance, not an Effect program.
- **Framework**: No UI framework is involved. The source-driven skill's examples are ecosystem-specific, but the selected surface is framework-neutral.
- **Heavy deps**: The constraint skill recommends many optional external tools (Semgrep, gitleaks, osv-scanner, Lighthouse, axe, dependency-cruiser, Stryker). Adopting the workflow does not require adding them all; a Canon integration should consume declared evidence or policy rather than install a broad toolchain by default.
- **Coupling**: The skills are coupled to a host that can ask interactive questions, save conventionally named Markdown files, and spawn isolated reviewers. `/build auto` additionally assumes clean git state, per-task commits, and a human plan approval (`vendors/addyosmani-agent-skills/_vendor_claude/commands/build.md:19-68`).
- **TOS-violation patterns**: None observed in the reviewed define/plan surface. It does not impersonate provider clients, forge request fingerprints, or evade rate limits.
- **License footnote**: The audit recommends clean-room reimplementation from these behaviors. No license routing decision is needed for a future implementation; any verbatim-lift PR must handle the vendor's license separately.

## 5. Recommended actions

1. **EXTRACT** the explicit intent interview shape (hypothesis, confidence, one question with a guess, outcome/user/success/constraint/out-of-scope restatement) → `canon/skills/reference/role-planner.md` plus a new planner command/reference — it fills the pre-brief discovery gap while preserving human approval as the only scope gate.
2. **INSPIRE** the capability-map and stable module-id concept → `canon context`/typed change planning — it can expose dependency direction before several related changes are authored, but must not create an untracked parallel `SPEC.md` authority.
3. **EXTRACT** task fields for acceptance, verification, dependencies, files, and bounded size → `canon-vocab` task atoms and dispatch metadata — it makes implementer handoffs actionable while evidence remains the only completion proof.
4. **INSPIRE** the numeric constraint/ratchet and weakened-bar guard → `canon policy`/`canon gate check` integration or a companion manifest — it gives teams a durable quality bar, but must remain distinct from and approved alongside Canon's evidence policy.
5. **INSPIRE** the bounded ARTIFACT + CONTRACT adversarial pre-review → planner/implementer handoff or a pre-build review record — it complements, rather than replaces, Canon's distinct-session post-evidence review.
6. **EXTRACT** selective context and immutable source-input handling → `canon context-pack`, `canon prompt`, and session handoff metadata — Canon already has digest-backed packs and prompts, so lift the reproducibility contract rather than the vendor's file-name conventions.
7. **SKIP** the vendor's mandatory repository-root `SPEC.md`, `tasks/plan.md`, `tasks/todo.md`, and `CONSTRAINTS.md` as Canon-native artifacts → `canon/skills/reference/role-planner.md` and `canon-plan-import.md` — Canon already owns `.canon/changes/<slug>/brief.md`, typed task records, scenario specs, policy, and evidence; duplicating files would create competing sources of truth.
8. **SKIP** `/build auto`'s per-task commit and “single approval” semantics → Canon's approved brief/policy, evidence, independent review, and ship gate — Canon's approval is deliberately tied to the brief digest and policy, and its gate needs scenario/evidence/review state rather than a git-commit convention (`canon/skills/reference/canon-gate.md:284-302`).

## 6. Citations

- `vendors/addyosmani-agent-skills/skills/interview-me/SKILL.md:29-85` — confidence-bearing hypothesis, one-question loop, explicit restatement, and stop rule.
- `vendors/addyosmani-agent-skills/skills/idea-refine/SKILL.md:35-74` — divergent/convergent/sharpen phases and one-page output.
- `vendors/addyosmani-agent-skills/skills/spec-driven-development/SKILL.md:17-43` — capability map, stable ids, dependency direction, and gated phases.
- `vendors/addyosmani-agent-skills/skills/spec-driven-development/SKILL.md:46-103` — assumptions, six-area spec, boundaries, and approval stop.
- `vendors/addyosmani-agent-skills/skills/planning-and-task-breakdown/SKILL.md:25-90` — dependency graph, vertical slices, task shape, and verification.
- `vendors/addyosmani-agent-skills/skills/constraint-driven-development/SKILL.md:65-105` — `CONSTRAINTS.md` shape, command-backed numbers, and exceptions.
- `vendors/addyosmani-agent-skills/skills/constraint-driven-development/SKILL.md:166-220` — lifecycle placement, diff-scoped checks, and weakened-bar detection.
- `vendors/addyosmani-agent-skills/skills/doubt-driven-development/SKILL.md:46-104` — ARTIFACT + CONTRACT extraction, adversarial review, reconciliation, and bounded cycles.
- `vendors/addyosmani-agent-skills/skills/context-engineering/SKILL.md:1-60` — context hierarchy and restartable session boundary.
- `vendors/addyosmani-agent-skills/commands/build.toml:1-79` — command-level plan approval, clean baseline, task loop, and autonomous mode.
- `vendors/addyosmani-agent-skills/_vendor_claude/commands/ship.md:1-94` — parallel specialist fan-out and synthesized go/no-go with rollback.
- `canon/skills/reference/role-planner.md:32-53` — Canon's current brief content, check, approval, and human-stop contract.
- `canon/skills/reference/brief-shape.md:11-23` — Canon's marker-based brief structure and blocker rules.
- `crates/canon-cli/src/brief_template.rs:53-63` — Canon's seven ordered PRD sections.
- `crates/canon-cli/src/brief_check.rs:1-50` — Canon's blocker/advisory model, coverage rules, and waiver semantics.
- `canon/skills/reference/canon-context.md:38-58` — immutable, repository-relative context packs and verification.
- `canon/skills/reference/canon-gate.md:284-302` — Canon's hard stop before evidence when a brief is not human-approved.

[Some lines truncated to 768 chars. Use agent://AuditADefinePlan:raw:3-3 to read line 3 whole]
