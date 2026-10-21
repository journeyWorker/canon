# Surface — p-reasoning

**Vendor paths:** `vendors/cursor-plugins/pstack/skills/architect`, `vendors/cursor-plugins/pstack/skills/how`, `vendors/cursor-plugins/pstack/skills/why`, `vendors/cursor-plugins/pstack/skills/figure-it-out`, `vendors/cursor-plugins/pstack/skills/in-your-own-words`, `vendors/cursor-plugins/pstack/skills/sharpen`, `vendors/cursor-plugins/pstack/skills/teach`, `vendors/cursor-plugins/pstack/skills/recall`, `vendors/cursor-plugins/pstack/skills/reflect`, `vendors/cursor-plugins/pstack/skills/ketchup`, `vendors/cursor-plugins/pstack/skills/bro`, `vendors/cursor-plugins/pstack/skills/unslop`, `vendors/cursor-plugins/pstack/skills/no-comments`, `vendors/cursor-plugins/pstack/skills/technical-writing`, `vendors/cursor-plugins/pstack/skills/typescript-best-practice…
**Scale:** 18 primary `SKILL.md` files, plus supporting references; LOC not measured in this audit
**Primary language(s):** Markdown, with TypeScript guidance and shell-command examples
**Canon analog (if any):** `canon/skills/SKILL.src.md`, `canon/skills/reference/role-planner.md`, `canon/skills/reference/brief-polish.md`, `canon/skills/reference/canon-learn.md` — canon already has an approval-gated brief, prose advisories, plan ingestion, and role-namespaced strategy memory, but not all of the vendor's authoring and reasoning workflows.

## 1. Overview

This surface is a collection of agent workflows for architecture design, codebase explanation, investigation, planning, writing, context recovery, and durable learning. The skills are prompt-level procedures rather than executable libraries. Several skills compose other skills: `architect` uses `how`, `why`, and `arena`; `teach` combines `how` and `why`; `figure-it-out` combines design, verification, delegation, and decision logging; `automate-me` combines transcript mining, skill authoring, and prose cleanup.

The strongest reusable mechanisms are explicit design artifacts, a staged hypothesis-and-verification loop, detailed prose rules, and separate working-memory and durable-learning workflows. The surface also contains task-specific or environment-specific skills, such as `make-bot-ui`, `typescript-best-practices`, and `no-comments`, that are not direct canon candidates.

Canon already covers the PRD brief, human approval, scenario alignment, evidence, review, and strategy promotion. The main gaps are design artifacts that precede implementation, richer prose checks than the current phrase and length rules, and a user-facing workflow for turning work history into candidate lessons before canon's evidence-backed strategy promotion.

## 2. Entry points & architecture

- **Entry files:** Each requested skill has its own `SKILL.md`. The main workflow entry points are `vendors/cursor-plugins/pstack/skills/architect/SKILL.md:1-10`, `vendors/cursor-plugins/pstack/skills/figure-it-out/SKILL.md:8-10`, `vendors/cursor-plugins/pstack/skills/recall/SKILL.md:1-6`, and `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:1-8`.
- **Core abstractions:**
  - `architect` design sketch: types, signatures, module boundaries, alternatives, and rationale before implementation (`vendors/cursor-plugins/pstack/skills/architect/SKILL.md:7-10`).
  - `figure-it-out` phase workflow: frame, design, run, log, and verify an auditable multi-step task (`vendors/cursor-plugins/pstack/skills/figure-it-out/SKILL.md:9-10`, `vendors/cursor-plugins/pstack/skills/figure-it-out/SKILL.md:38-52`).
  - `how` and `why`: separate architectural behavior tracing from historical or motivational investigation (`vendors/cursor-plugins/pstack/skills/how/SKILL.md:8-20`, `vendors/cursor-plugins/pstack/skills/why/SKILL.md:1-10`).
  - `recall` and `reflect`: reconstruct recent context or mine a transcript for durable skill changes (`vendors/cursor-plugins/pstack/skills/recall/SKILL.md:1-6`, `vendors/cursor-plugins/pstack/skills/reflect/SKILL.md:1-12`).
  - `unslop` and `technical-writing`: prose lint rules and a layered documentation standard (`vendors/cursor-plugins/pstack/skills/unslop/SKILL.md:1-8`, `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:1-8`).
- **Cross-package dependencies:** The skills refer to sibling pstack skills including `arena`, `poteto-mode`, `show-me-your-work`, `create-skill`, `interrogate`, `prove-it-works`, and several principle skills. They also refer to Cursor Tasks, worktrees, MCPs, and project transcripts.
- **External runtime dependencies:** Cursor agent orchestration, Task subagents, configured model names, MCP servers, git worktrees, shell utilities, and optional provider-specific integrations. The core writing and planning patterns do not require a runtime library.

## 3. Pattern catalog

### 3.1. Design sketch plus alternative synthesis before implementation

- **Where:** `vendors/cursor-plugins/pstack/skills/architect/SKILL.md:7-10`, `vendors/cursor-plugins/pstack/skills/architect/SKILL.md:22-47`, `vendors/cursor-plugins/pstack/skills/architect/SKILL.md:78-79`; `vendors/cursor-plugins/pstack/skills/architect/references/rationale-template.md:14-19`; `vendors/cursor-plugins/pstack/skills/architect/references/runner-prompt.md:1-8`
- **What it does:** `architect` creates a design artifact before implementation. The artifact contains a usage sketch, types, signatures, module boundaries, and prose rationale. For non-trivial work, it grounds the design with `how` and `why`, produces at least two structurally different candidates through `arena`, and records which candidate became the base, which pieces were grafted, and what was rejected.
- **Why it's interesting:** Canon's brief captures product intent, flow, acceptance, questions, and scope, but it does not require a code-ownership or type-shape artifact before implementation. Canon's scenario and evidence model can prove behavior while still allowing an implementation to lock in the wrong module boundary or data shape.
- **Canon applicability:** **Partial.** Canon has a human-approved PRD brief and optional external plan ingestion, but the planner reference does not define a design-sketch artifact containing signatures, ownership, alternatives, or a synthesis decision. `canon/skills/reference/role-planner.md:39-56` defines the brief and five prose or alignment passes; `canon/skills/reference/canon-plan-import.md:120-133` explicitly ignores architecture prose in imported Superpowers plans.
- **Effort to lift:** Moderate.

### 3.2. Falsifiable phase workflow with per-unit verdicts and a decision trail

- **Where:** `vendors/cursor-plugins/pstack/skills/figure-it-out/SKILL.md:8-23`, `vendors/cursor-plugins/pstack/skills/figure-it-out/SKILL.md:27-42`, `vendors/cursor-plugins/pstack/skills/figure-it-out/SKILL.md:48-52`
- **What it does:** `figure-it-out` turns a large task into five named phases. Phase A defines a falsifiable completion predicate, quantified scope, and rigor level. Phase B writes the phase list for human review. Phase C treats each unit as an experiment with a hypothesis, smallest change, real-artifact measurement, and one of `VERIFIED`, `NOT VERIFIED`, or `INCONCLUSIVE`. Phase D records the run, and Phase E checks the predicate and encodes recurring corrections as gates, lints, checks, or scripts.
- **Why it's interesting:** Canon has a strong lifecycle and evidence gate, but its brief and unit model do not provide a standard artifact for the run's hypothesis, baseline, per-unit verdict, or decision trail. This pattern would make long-running orchestration easier to resume and audit without replacing scenario evidence.
- **Canon applicability:** **Partial.** Canon already requires a brief before work, scenario-linked evidence, independent review, and a state transition guarded by `canon gate check` (`canon/skills/SKILL.src.md:1-8`). `canon/skills/reference/canon-gate.md:3-6` separates evidence coverage from verdict status. The missing part is a first-class run plan or decision-log record that captures hypotheses, baselines, and inconclusive units. Canon's `canon context-pack` can preserve inputs, but it does not represent the execution decision trail (`canon/skills/reference/canon-context.md:19-38`).
- **Effort to lift:** Significant.

### 3.3. Layered prose standard that separates document mode from sentence rules

- **Where:** `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:1-8`, `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:21-48`, `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:50-92`; `vendors/cursor-plugins/pstack/skills/unslop/SKILL.md:1-8`, `vendors/cursor-plugins/pstack/skills/unslop/SKILL.md:20-33`
- **What it does:** `technical-writing` first chooses a Diátaxis mode: tutorial, how-to, reference, or explanation. It then applies sentence-level rules for active voice, conditions before instructions, sentence length, one action per instruction, explicit referents, consistent terminology, and concrete codebase vocabulary. `unslop` adds stable rule ids for AI vocabulary, filler, over-compression, em dashes, vague attribution, passive voice, and abstract metaphors. The rules preserve meaning and allow exceptions when strict compliance would make a sentence worse.
- **Why it's interesting:** This is a more complete prose model than a phrase blacklist. It distinguishes document purpose from sentence style and gives an agent a rewrite procedure rather than only a warning. The stable rule ids also make findings citeable across revisions.
- **Canon applicability:** **Partial.** Canon's brief prose checker already emits `prose.evaluative`, `prose.connective`, `prose.contrast`, `prose.colloquial`, `prose.degree`, `prose.sentence-length`, `prose.paragraph-length`, and `prose.key-sentence` advisories (`crates/canon-cli/src/brief_check.rs:17-22`, `crates/canon-cli/src/brief_check.rs:105-113`). `canon/skills/reference/brief-polish.md:3-19` tells the planner to replace evaluative or degree words, split long text, and mark one key sentence per normative section. It does not classify the brief's document mode or check the broader unslop and technical-writing rules such as vague referents, passive voice, terminology drift, or instructions with multiple actions.
- **Effort to lift:** Moderate.

### 3.4. Prose linting as advisory, scoped, explainable findings rather than authorship judgment

- **Where:** `vendors/cursor-plugins/pstack/skills/unslop/SKILL.md:8-33`; `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:1-8`, `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:50-92`; `vendors/cursor-plugins/pstack/skills/why/SKILL.md:13-20`
- **What it does:** The vendor separates prose cleanup from factual investigation. `unslop` identifies concrete language patterns and preserves intended meaning. `technical-writing` defines the audience and document mode before applying style rules. `why` explicitly requires confidence separation and source citations, so a prose rewrite cannot silently turn an inference into a fact.
- **Why it's interesting:** Canon's current brief prose rules already avoid treating clean prose as an authorship or quality verdict, but the vendor adds a useful distinction between style findings and epistemic claims. This matters for briefs, review reports, and generated dashboards where polished prose can otherwise make weak evidence appear authoritative.
- **Canon applicability:** **Has the core; partial for broader artifacts.** `crates/canon-cli/src/brief_check.rs:17-22` says prose findings are advisories and never an authorship verdict. `canon/skills/reference/brief-polish.md:25-30` says a clean prose run is not a claim that the brief is good. However, the same contract is not expressed as a reusable checker for review reports, strategy bodies, imported plans, or evidence summaries. The report documentation warns that generated claims must remain verbatim to computed data (`canon/skills/reference/canon-report-dashboard.md:17-20`), but there is no general prose quality pass for those artifacts.
- **Effort to lift:** Moderate.

### 3.5. Context reconstruction from recent work plus shared records, with explicit scope and status

- **Where:** `vendors/cursor-plugins/pstack/skills/recall/SKILL.md:1-6`, `vendors/cursor-plugins/pstack/skills/recall/SKILL.md:11-24`; `vendors/cursor-plugins/pstack/skills/recall/SKILL.md:27-35`
- **What it does:** `recall` rebuilds working context from two records: the agent's prior chat history and a shared record of user reports, fixes, incidents, and current symptoms. It pins the workspace, topic, and time window before searching. It returns a fixed capsule with current state, threads, recurring problems, and one next move, and it verifies surfaced branches, tickets, and artifacts against live state.
- **Why it's interesting:** Canon records work in subjects, changes, scenarios, evidence, findings, reviews, runs, and strategy trajectories, but an agent entering an existing subject still lacks a standard read-only context capsule. A bounded recall workflow could reduce repeated discovery and make handoffs more useful without adding unverified claims to the ledger.
- **Canon applicability:** **Missing as a user-facing workflow; partial as storage.** Canon has `canon status`, role-specific next steps, and context packs, but `canon/skills/SKILL.src.md:54-67` only directs an agent to inspect status and load the role reference. `canon/skills/reference/canon-context.md:19-38` provides immutable input packs, not a recent-work summary. `canon/skills/reference/canon-session-ingest.md:206-207` explicitly excludes task/context prose and raw event detail from ingested metadata, so canonical recall cannot be reconstructed from session metadata alone.
- **Effort to lift:** Significant.

### 3.6. Reflection that routes accepted learnings to structural enforcement

- **Where:** `vendors/cursor-plugins/pstack/skills/reflect/SKILL.md:1-12`, `vendors/cursor-plugins/pstack/skills/reflect/SKILL.md:20-36`; `vendors/cursor-plugins/pstack/skills/automate-me/SKILL.md:1-12`, `vendors/cursor-plugins/pstack/skills/automate-me/SKILL.md:36-60`
- **What it does:** `reflect` mines the active transcript through judgment, tooling, and divergent reviewers, synthesizes Accepted, Rejected, and Backlog findings, then moves anything better enforced by a lint, metadata flag, or runtime check out of the prose-edit queue. `automate-me` mines repeated user behavior across transcript slices, asks the user to confirm missing intent, clusters findings, and authors or updates a mode skill only after evidence is cross-checked.
- **Why it's interesting:** This is a practical front end for learning. It distinguishes one-off advice from repeated patterns, asks for explicit confirmation before changing shared instructions, and routes recurring corrections into structure instead of relying on memory.
- **Canon applicability:** **Partial.** Canon's `canon learn` is stronger on evidence and promotion but starts after verdict trajectories exist. `canon/skills/reference/canon-learn.md:3-21` defines warm trajectories and durable strategies, and `canon/skills/reference/canon-learn.md:23-45` requires an external evaluation and paired evidence for promotion. Canon has no standard command that takes a transcript or review finding, clusters repeated corrections, and emits a candidate strategy or a structural-enforcement recommendation before promotion.
- **Effort to lift:** Significant.

### 3.7. Composed explanation workflows separate behavior, motivation, and teaching

- **Where:** `vendors/cursor-plugins/pstack/skills/how/SKILL.md:8-20`, `vendors/cursor-plugins/pstack/skills/how/SKILL.md:31-36`; `vendors/cursor-plugins/pstack/skills/teach/SKILL.md:1-12`, `vendors/cursor-plugins/pstack/skills/teach/SKILL.md:27-40`; `vendors/cursor-plugins/pstack/skills/in-your-own-words/SKILL.md:1-6`, `vendors/cursor-plugins/pstack/skills/sharpen/SKILL.md:1-13`
- **What it does:** `how` traces runtime flow and ownership. `why` investigates motivation and records null evidence categories. `teach` composes both into a plain explanation tailored to the reader. `in-your-own-words` and `sharpen` provide small, bounded transformations for restating goals or deepening a task without silently expanding its scope.
- **Why it's interesting:** Canon's roles separate planning, implementation, review, and orchestration, but the references do not provide a standard explanation artifact that distinguishes observed behavior, historical rationale, inference, and unanswered questions. This distinction would help planners and reviewers understand existing code without treating an explanation as an approved scope change.
- **Canon applicability:** **Partial.** Canon already requires the planner to state intent, outcome, flow, acceptance, assumptions, and out-of-scope material (`canon/skills/reference/role-planner.md:39-56`). It also requires assumptions to remain explicit when the human cannot answer (`canon/skills/reference/role-planner.md:70-73`). It does not define a reusable `how`/`why` explanation record or a citation contract for architecture and rationale notes.
- **Effort to lift:** Moderate.

## 4. Risks & incompatibilities

- **Cursor-specific orchestration:** Many workflows assume Cursor Task subagents, configured model names, MCP discovery, and Cursor transcript locations. A clean-room canon implementation should describe provider-neutral records and adapters rather than copy those paths or model-selection instructions.
- **Unbounded transcript access:** `recall`, `reflect`, and `automate-me` read chat history and shared records. A canon implementation needs explicit repository, subject, time-window, and privacy boundaries. Canon's session-ingest contract deliberately excludes task/context prose and raw events from metadata (`canon/skills/reference/canon-session-ingest.md:206-207`).
- **Unverified narrative learning:** Reflection and transcript mining can produce plausible but unsupported lessons. They must remain candidate inputs until tied to canon verdict trajectories, evidence, or an explicit human approval. `canon learn promote` already requires paired evaluation and signed approval (`canon/skills/reference/canon-learn.md:23-45`).
- **Planning artifact drift:** A design sketch could become a second source of truth beside the approved brief. Canon should bind design artifacts to a change digest and treat them as implementation guidance, not as a replacement for the brief or acceptance scenarios.
- **Prose rule overreach:** `unslop` contains subjective language rules such as vocabulary and metaphor avoidance. Canon's current checker correctly makes prose findings advisories and supports scoped waivers (`crates/canon-cli/src/brief_check.rs:17-22`; `canon/skills/reference/brief-polish.md:16-19`). Any extension should preserve advisory severity and explainable findings.
- **Framework and task-specific skills:** `make-bot-ui` is tied to a specific webhook and tailnet setup. `typescript-best-practices` is language-specific. `no-comments` assumes a comment-review subagent and interactive approval. These are not general canon features.
- **TOS-violation patterns:** No requested skill in this surface directs agents to impersonate Anthropic, Google, or OpenAI clients, forge request fingerprints, or evade provider limits.
- **License footnote:** The audit recommends clean-room reimplementation from the mechanisms described here. No vendor file is a verbatim-lift target.

## 5. Recommended actions

1. **INSPIRE** the `architect` design-sketch and `figure-it-out` phase artifacts for `canon/skills/reference/role-planner.md` and a future `canon design` or `canon plan` record — preserve the approved brief as the scope authority while adding optional type/module ownership, alternatives, baseline, hypothesis, and decision-trail fields.
2. **INSPIRE** the `architect` alternative synthesis for `canon/skills/reference/canon-plan-import.md` and future plan records — imported architecture prose should either be explicitly mapped into a typed design artifact or be reported as ignored, rather than silently disappearing as it does today (`canon/skills/reference/canon-plan-import.md:124-133`).
3. **EXTRACT** the vendor's stable prose-rule structure into `crates/canon-cli/src/brief_check.rs` as additional advisory rules — add only rules with deterministic findings, section scope, matched text, and waiver support; target the existing `prose.*` namespace rather than creating a second prose checker.
4. **INSPIRE** the `technical-writing` document-mode distinction for `canon/skills/reference/brief-polish.md` — let the planner identify whether a brief section is normative instruction, explanation, or context before applying style advice, without making mode selection a ship blocker.
5. **SKIP** copying `unslop` as a broad subjective gate — canon already has explainable advisory prose rules and scoped waivers, while an unbounded vocabulary blacklist would create false positives and policy churn.
6. **INSPIRE** `recall`'s bounded context capsule for `canon status` or a future read-only `canon context recall` command — target `canon/skills/reference/canon-subject.md` and `crates/canon-cli/src/status.rs`, with explicit subject, change, role, time-window, and source provenance.
7. **EXTRACT** the `reflect` routing rule into the `canon learn` design — a candidate lesson that can be enforced by schema, a gate, a lint, or a record invariant should be routed there before strategy promotion; target `canon/skills/reference/canon-learn.md` and `crates/canon-cli/src/learn.rs`.
8. **INSPIRE** `automate-me`'s repeated-evidence threshold for candidate lessons — require observations across multiple independent runs or trajectories before distilling a strategy, then keep canon's existing paired evaluation and signed approval as the activation gate.
9. **INSPIRE** `how`/`why`/`teach` separation for reviewer and planner reports — target `canon/skills/reference/canon-review.md` and `canon/skills/reference/role-reviewer.md` with separate fields for observed behavior, rationale evidence, inference, and unknowns.
10. **SKIP** `make-bot-ui`, `typescript-best-practices`, and `no-comments` as canon features — they are environment-, language-, or workflow-specific and do not improve canon's brief, evidence, review, or learning contracts.
11. **SKIP** treating `in-your-own-words`, `bro`, `ketchup`, and `sharpen` as ledger records — they are response transformations. They may inform agent guidance, but they do not establish product requirements, evidence, review, or durable strategy validity.

## 6. Citations

- `vendors/cursor-plugins/pstack/skills/architect/SKILL.md:7-10` — design sketches contain types, signatures, module boundaries, and rationale before implementation.
- `vendors/cursor-plugins/pstack/skills/architect/SKILL.md:22-47` — grounding, alternative candidates, arena synthesis, and checkpoint behavior.
- `vendors/cursor-plugins/pstack/skills/figure-it-out/SKILL.md:8-10` — the workflow itself is the pre-code deliverable.
- `vendors/cursor-plugins/pstack/skills/figure-it-out/SKILL.md:27-42` — baseline, hypothesis, real-artifact verification, and explicit verdict states.
- `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:21-48` — Diátaxis modes and document-purpose separation.
- `vendors/cursor-plugins/pstack/skills/technical-writing/SKILL.md:50-92` — active voice, instruction structure, sentence limits, and terminology rules.
- `vendors/cursor-plugins/pstack/skills/unslop/SKILL.md:8-33` — stable prose rules for filler, AI vocabulary, passive voice, vague references, and over-compression.
- `vendors/cursor-plugins/pstack/skills/recall/SKILL.md:1-6` — recall's two-record context reconstruction contract.
- `vendors/cursor-plugins/pstack/skills/recall/SKILL.md:11-35` — bounded search scope, live-state verification, and capsule output.
- `vendors/cursor-plugins/pstack/skills/reflect/SKILL.md:1-12` — parallel transcript review and synthesis.
- `vendors/cursor-plugins/pstack/skills/reflect/SKILL.md:20-36` — Accepted, Rejected, Backlog routing and structural-enforcement check.
- `crates/canon-cli/src/brief_check.rs:17-22` — canon's current prose advisory contract and non-authorship semantics.
- `crates/canon-cli/src/brief_check.rs:105-113` — current `prose.*` rule identifiers.
- `crates/canon-cli/src/brief_check.rs:725-740` — current phrase and sentence or paragraph thresholds.
- `canon/skills/reference/brief-polish.md:3-19` — canon's prose-polish scope and rewrite guidance.
- `canon/skills/reference/role-planner.md:39-56` — canon's approved brief structure and optional refinement passes.
- `canon/skills/reference/canon-plan-import.md:120-133` — imported plan mapping that ignores architecture prose.
- `canon/skills/reference/canon-learn.md:3-21` — warm trajectories and durable strategies.
- `canon/skills/reference/canon-learn.md:23-45` — paired evaluation and signed approval for strategy promotion.
- `canon/skills/reference/canon-context.md:19-38` — immutable context-pack mechanism.
- `canon/skills/reference/canon-session-ingest.md:206-207` — session-ingest metadata excludes task/context prose and raw event detail.
- `canon/skills/reference/canon-retrieve.md:1-5` — retrieval is advisory and fail-soft.
- `canon/skills/SKILL.src.md:1-8` — canon's brief, scenario, evidence, review, and transition loop.

[Some lines truncated to 768 chars. Use agent://AuditPReasoning:raw:3-3 to read line 3 whole]
