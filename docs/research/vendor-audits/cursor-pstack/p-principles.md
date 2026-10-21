# Surface — p-principles

**Vendor paths**: `vendors/cursor-plugins/pstack/skills/principle-*/SKILL.md`, `vendors/cursor-plugins/pstack/docs/guide/08-principles.md`, `vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json`  
**Scale**: 24 files / approximately 350 lines of Markdown and YAML frontmatter (24 principle packages were observed; the task description says 25, but the vendor index also states 24)  
**Primary language(s)**: Markdown with YAML frontmatter  
**Canon analog (if any)**: `canon/skills/SKILL.src.md`, `canon/skills/reference/role-*.md`, and `crates/canon-cli/src/brief_check.rs` — canon combines always-loaded workflow instructions, role-scoped references, and executable brief/gate checks rather than packaging each principle as a separately triggerable skill.

## 1. Overview

The p-principles surface packages engineering heuristics as one `SKILL.md` per principle directory. Each file has a name, a natural-language `description` containing its trigger conditions and intended behavior, and `disable-model-invocation: true`, so the principle is not independently auto-invoked by the host. `/poteto-mode` reads the principle index at the start of multi-step work, selects applicable principles, and names the selected principle and changed decision in its response. The principles are therefore reusable advice modules coupled to the pstack orchestration layer, not standalone executable checks.

The files range from architecture guidance (`type-system-discipline`, `boundary-discipline`, `model-the-domain`) to execution and verification guidance (`sequence-verifiable-units`, `prove-it-works`, `test-behavior-not-implementation`) and delegation/context guidance (`guard-the-context-window`, `never-block-on-the-human`). Most rules are advisory prose; a smaller subset describes checks, evidence, or structural mechanisms but does not itself run them.

## 2. Entry points & architecture

- **Entry file(s)**:
  - `vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json:1-30` — declares the plugin and exposes `./skills/` as the skill root.
  - `vendors/cursor-plugins/pstack/docs/guide/08-principles.md:1-68` — documents the principle catalog, trigger-by-name behavior, and the relationship to `/poteto-mode`.
  - `vendors/cursor-plugins/pstack/skills/principle-*/SKILL.md:1-5` — each package’s registration metadata; all observed principle packages set `disable-model-invocation: true`.
- **Core abstractions**:
  - **Principle skill package** — one directory and `SKILL.md` containing frontmatter plus concise operational guidance; examples include `principle-type-system-discipline` and `principle-prove-it-works`.
  - **Description trigger** — the frontmatter `description` states when the principle applies and summarizes the decision rule, such as “Apply after completing a task, before declaring done” in `principle-prove-it-works`.
  - **Principle index** — `08-principles.md` groups principles by design/execution, architecture, verification, delegation, and meta concerns; it tells agents to use names to steer rather than invoke principles directly.
  - **Orchestrator/playbook integration** — `/poteto-mode` reads the index and applies selected principles during multi-step playbooks; principle files cross-reference one another by relative path.
- **Cross-package dependencies**:
  - `poteto-mode` and its playbooks consume the principles.
  - `benchmark-checklist`, `typescript-best-practices`, `correct`, `create-verification-skill`, and related skills are referenced as concrete follow-through mechanisms.
  - Several principles reference sibling principles, especially `principle-boundary-discipline`, `principle-encode-lessons-in-structure`, `principle-build-the-lever`, and `principle-prove-it-works`.
- **External runtime dependencies**: Cursor’s skill/plugin loader and agent orchestration. No application framework or compiled runtime dependency is present in the principle files.

## 3. Pattern catalog

### 3.1. Frontmatter descriptions act as trigger contracts, while invocation is disabled

- **Where**: `vendors/cursor-plugins/pstack/skills/principle-type-system-discipline/SKILL.md:1-5`, `vendors/cursor-plugins/pstack/skills/principle-prove-it-works/SKILL.md:1-5`, `vendors/cursor-plugins/pstack/skills/principle-never-block-on-the-human/SKILL.md:1-5`
- **What it does**: Each principle declares a stable `name`, a description that combines trigger conditions with the intended outcome, and `disable-model-invocation: true`. For example, type-system discipline triggers when designing types, reviewing signatures, or writing statically typed code; prove-it-works triggers after task completion and before declaring success; never-block triggers when the agent is tempted to ask for permission on reversible work. The host can discover and route by metadata, but the principle cannot independently run as an automatic skill.
- **Why it's interesting**: This separates **selection metadata** from **instruction content**. The description is not merely a summary: it is the routing vocabulary for choosing a principle, while the body supplies the advice. It also avoids accidental invocation of broad principles on every turn.
- **Canon applicability**: **Partial.** `canon/skills/SKILL.src.md:1-4` has a broad frontmatter trigger for all build/change/fix requests, but canon does not package individual principles with separate trigger metadata. Role references are loaded by declared role (`canon/skills/SKILL.src.md:128-133`), and `canon brief check` is invoked explicitly through `crates/canon-cli/src/main.rs:2180-2183`.
- **Effort to lift**: moderate.

### 3.2. A human-readable index selects principles before execution and records the decision

- **Where**: `vendors/cursor-plugins/pstack/docs/guide/08-principles.md:1-20`, `vendors/cursor-plugins/pstack/docs/guide/08-principles.md:32-68`
- **What it does**: The index says `/poteto-mode` reads the principle index at the start of every multi-step task, applies the principles that the task triggers, and names each applied principle together with the decision it changed. It groups the catalog into execution/design, architecture, verification, delegation, and meta categories, making selection a task-level routing decision rather than a flat list of unrelated rules.
- **Why it's interesting**: The mechanism creates a lightweight audit trail for advisory guidance: a response must identify both the principle and its concrete decision impact. The index also lets playbooks refer to a named rule instead of copying its prose into every workflow.
- **Canon applicability**: **Partial.** Canon already has role-scoped loading and explicit phase routing in `canon/skills/SKILL.src.md:49-58`, and `canon status`/`meta.next` provide executable next-step routing (`canon/skills/SKILL.src.md:108-122`). It lacks a comparable principle index that records which advisory rule changed an implementation or planning decision. Canon’s existing `brief check` output is finding-oriented rather than decision-rationale-oriented (`crates/canon-cli/src/brief_check.rs:1091-1128`).
- **Effort to lift**: moderate.

### 3.3. Principles distinguish advice from mechanisms that can actually prove or prevent a mistake

- **Where**: `vendors/cursor-plugins/pstack/skills/principle-encode-lessons-in-structure/SKILL.md:1-9`, `vendors/cursor-plugins/pstack/skills/principle-build-the-lever/SKILL.md:1-15`, `vendors/cursor-plugins/pstack/skills/principle-prove-it-works/SKILL.md:1-24`
- **What it does**: `encode-lessons-in-structure` says repeated instructions should become a lint, metadata flag, runtime check, or script; only judgment that cannot be encoded should remain prose. `build-the-lever` requires a rerunnable codemod, generator, script, or skill for non-trivial work, with a documented exception for genuinely trivial edits. `prove-it-works` requires direct observation of the real artifact and recommends retaining a deterministic check as a reviewer-rerunnable artifact.
- **Why it's interesting**: The package explicitly identifies the enforcement boundary: prose is appropriate for judgment calls, while recurring or observable invariants should move into executable structure. This prevents a growing agent instruction file from pretending to enforce rules that no tool reads.
- **Canon applicability**: **Partial.** Canon already enforces many structural requirements in `crates/canon-cli/src/brief_check.rs:89-119`, including section order, placeholders, flow coverage, and failure-case coverage; `crates/canon-cli/src/brief_check.rs:595-610` shows placeholder rules producing concrete findings. Canon also has executable evidence and gate checks (`crates/canon-cli/src/gate.rs:105-135`). However, canon does not expose a unified mapping from each prose principle to its enforcing check, advisory, evidence requirement, or “not enforceable” status.
- **Effort to lift**: moderate.

### 3.4. The package uses concise, cross-referenced principles to compose workflow behavior without duplicating full rules

- **Where**: `vendors/cursor-plugins/pstack/skills/principle-sequence-verifiable-units/SKILL.md:1-24`, `vendors/cursor-plugins/pstack/skills/principle-type-system-discipline/SKILL.md:8-16`, `vendors/cursor-plugins/pstack/skills/principle-experience-first/SKILL.md:1-19`
- **What it does**: Principle bodies are short and focused on one decision rule. They refer to sibling principles when a concern crosses boundaries: type-system discipline delegates validation placement to boundary discipline; sequence-verifiable-units identifies prove-it-works and build-the-lever as complements; experience-first distinguishes its target criterion from foundational-thinking’s sequencing role. This keeps the files composable instead of turning each one into a complete workflow.
- **Why it's interesting**: The decomposition provides a reusable vocabulary for planning and review without requiring each playbook to duplicate every rationale. It also clarifies the lifecycle point where a principle applies: before design, during migration, after a task, or during review.
- **Canon applicability**: **Partial.** Canon’s `reference/role-*.md` files already separate planner, implementer, reviewer, and orchestrator responsibilities (`canon/skills/reference/role-planner.md:1-5`, `role-implementer.md:1-5`, `role-reviewer.md:1-5`, `role-orchestrator.md:1-5`), and the brief passes separate shape, clarify, harden, align, and polish concerns (`canon/skills/reference/brief-shape.md:1-7`, `brief-polish.md:1-18`). The missing piece is a small, reusable catalog of cross-role engineering principles with explicit lifecycle triggers.
- **Effort to lift**: trivial to moderate.

### 3.5. Trigger descriptions cover lifecycle and task shape, but the bodies remain non-enforceable unless a linked check exists

- **Where**: `vendors/cursor-plugins/pstack/skills/principle-sequence-verifiable-units/SKILL.md:1-13`, `vendors/cursor-plugins/pstack/skills/principle-make-operations-idempotent/SKILL.md:1-20`, `vendors/cursor-plugins/pstack/skills/principle-exhaust-the-design-space/SKILL.md:1-8`
- **What it does**: Descriptions identify concrete trigger contexts such as migrations, crash/retry-prone commands, or novel architecture/UI decisions. The bodies then prescribe actions: verify each unit before proceeding, test repeated/crashed execution for convergence, or build two or three competing prototypes. None of these files records a machine-readable obligation, an artifact schema, or an exit-code-producing check; compliance depends on the agent or reviewer following the prose.
- **Why it's interesting**: This is a useful boundary for canon adoption. A principle can be valuable as guidance without being mistaken for a gate. The trigger can route attention, while the acceptance/evidence system decides whether the required observable result actually exists.
- **Canon applicability**: **Canon has it for several behaviors, but not as a general principle registry.** `canon/skills/reference/role-implementer.md:45-69` requires evidence to bind named reports/artifacts and explains re-verification after fixes; `canon/skills/reference/role-reviewer.md:36-67` requires reviewers to run claimed checks and inspect whether they prove the scenario. By contrast, design alternatives, idempotence reasoning, and per-unit sequencing remain advisory unless represented as scenarios, reviews, or artifacts. `canon/skills/reference/canon-authoring.md:15-18` explicitly distinguishes observable scenarios from implementation/process rules.
- **Effort to lift**: significant if made fully machine-enforced; trivial if adopted only as metadata and review guidance.

## 4. Risks & incompatibilities

- **Effect version**: not applicable. The surface is Markdown/YAML guidance, not Effect or compiled code.
- **Framework**: no UI or application framework dependency.
- **Heavy deps**: none in the principle files. The plugin manifest points to the skill and agent directories only (`vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json:25-30`).
- **Coupling**: strong coupling to Cursor’s skill loader and `/poteto-mode`. The principles are useful as documentation elsewhere, but their trigger semantics and “name the decision changed” response convention depend on the pstack orchestration layer.
- **Trigger ambiguity**: descriptions are natural language and may overlap. For example, migrations can trigger subtract-before-you-add, outcome-oriented-execution, sequence-verifiable-units, build-the-lever, and prove-it-works. Without a precedence or selection policy, different agents may load different subsets.
- **Advice/enforcement ambiguity**: several principles say “must,” “require,” or “do not,” but no package-level metadata identifies whether a rule is executable, review-only, or advisory. A future canon integration must not imply that loading a principle makes it enforced.
- **Catalog drift**: the task names 25 principles, while the observed directory inventory and vendor index contain 24. A generated or audited catalog should derive its count from the actual package set and flag index/package mismatches.
- **TOS-violation patterns**: none observed. The surface does not impersonate provider clients, forge request fingerprints, or evade rate limits.
- **License footnote**: `vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json:4-6` identifies the package as MIT-licensed. Canon’s default is clean-room reimplementation from the audit specification; no verbatim lift is required.

## 5. Recommended actions

1. **EXTRACT** a principle registry with `name`, `trigger`, `lifecycle`, and `enforcement` fields → `canon/skills/reference/canon-principles.md` — preserve pstack’s reusable vocabulary while making each rule’s trigger and enforcement status explicit.
2. **EXTRACT** the “advice versus mechanism” classification → `canon/skills/reference/canon-principles.md` plus `crates/canon-cli/src/brief_check.rs` rule metadata — every principle should state whether it is enforced by a CLI check, required through evidence/review, or advisory only.
3. **INSPIRE** the index-first selection and decision record → `canon/skills/SKILL.src.md` and role references — when a principle affects planning or review, name the principle and the changed decision in the handoff without pretending that citation is proof.
4. **INSPIRE** lifecycle-scoped descriptions → `canon/skills/reference/role-planner.md`, `role-implementer.md`, `role-reviewer.md`, and `role-orchestrator.md` — attach guidance to phases such as brief authoring, scenario/evidence work, independent review, and shipping rather than loading every rule everywhere.
5. **EXTRACT** the recurring-correction rule into a canon authoring convention → `canon/skills/reference/canon-authoring.md` and `canon/skills/reference/canon-gate.md` — when a repeated failure can be represented as a scenario, policy field, inventory check, or gate violation, encode it there instead of adding another prose-only instruction.
6. **SKIP** copying all 24 principles as independently auto-triggerable canon skills → `canon/skills/SKILL.src.md` — canon’s role-scoped workflow and executable CLI checks already provide the primary routing model; duplicating every principle as an always-available trigger surface would increase loading ambiguity and maintenance cost.
7. **SKIP** treating principle citations as evidence or gate results → `crates/canon-cli/src/brief_check.rs`, `crates/canon-cli/src/gate.rs`, and `canon/skills/reference/canon-authoring.md` — a cited principle is rationale, while scenario evidence, review records, and gate checks are the proof.

## 6. Citations

- `vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json:1-30` — plugin metadata and `skills: "./skills/"` packaging declaration.
- `vendors/cursor-plugins/pstack/docs/guide/08-principles.md:1-20` — principle loading behavior, trigger-by-name model, and decision-reporting expectation.
- `vendors/cursor-plugins/pstack/docs/guide/08-principles.md:32-68` — complete principle taxonomy and cross-reference index.
- `vendors/cursor-plugins/pstack/skills/principle-type-system-discipline/SKILL.md:1-16` — frontmatter trigger contract and boundary/schema advice.
- `vendors/cursor-plugins/pstack/skills/principle-prove-it-works/SKILL.md:1-24` — direct-observation requirement and rerunnable verification artifact.
- `vendors/cursor-plugins/pstack/skills/principle-sequence-verifiable-units/SKILL.md:1-24` — per-unit lifecycle and delivery sequencing.
- `vendors/cursor-plugins/pstack/skills/principle-encode-lessons-in-structure/SKILL.md:1-9` — explicit distinction between structural enforcement and prose judgment.
- `vendors/cursor-plugins/pstack/skills/principle-build-the-lever/SKILL.md:1-15` — rerunnable tool/script requirement and triviality exception.
- `canon/skills/SKILL.src.md:18-58` — canon’s approval gate and eight-step role-based working loop.
- `canon/skills/reference/brief-polish.md:3-19` — normative prose advisories and the boundary between prose quality and blocking checks.
- `canon/skills/reference/canon-authoring.md:15-18,30-52` — observable scenario model and distinction between behavior and process rules.
- `crates/canon-cli/src/brief_check.rs:89-119` — executable rule registry, severities, and coverage-rule classification.
- `crates/canon-cli/src/brief_check.rs:595-610` — concrete placeholder enforcement and finding generation.
- `crates/canon-cli/src/brief_check.rs:673-684` — executable flow-coverage and failure-case checks.
- `crates/canon-cli/src/gate.rs:105-135` — `canon gate check` assembling executable coverage, ledger, staleness, and trust checks.
