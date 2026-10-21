# Surface — m-productivity

**Vendor paths**: `vendors/mattpocock-skills/skills/productivity`, `vendors/mattpocock-skills/skills/misc`, `vendors/mattpocock-skills/skills/in-progress`, `vendors/mattpocock-skills/skills/deprecated`  
**Scale**: 45 files / approximately 1,100 LOC across 20 productivity files, 10 misc files, 16 in-progress files, and 1 deprecated README  
**Primary language(s)**: Markdown skill documents, YAML provider metadata, one shell script, one CommonJS configuration file  
**Canon analog (if any)**: `canon/skills/SKILL.src.md` and `canon/skills/reference/` — Canon already has an enforced brief-to-review loop, role handoffs, evidence, and learning/retrieval references, but not all of the vendor's document-authoring and stateful teaching/workflow conventions.

## 1. Overview

This surface is a collection of agent-skill authoring, productivity, handoff, teaching, questioning, workflow-design, and repository-maintenance playbooks. The productivity directory contains reusable ways to structure agent context, manage human interaction, and pass work between sessions; misc contains concrete setup and migration recipes; in-progress contains experimental writing, workflow, and orchestration patterns; deprecated is explicitly empty apart from its retirement note. The skills are mostly standalone Markdown documents with provider-specific metadata under `agents/`; they are not coupled to a runtime library. Canon's closest analogs are the generated `canon` skill plus its role and reference documents, while the strongest missing overlap is a …

## 2. Entry points & architecture

- **Entry file(s):** `vendors/mattpocock-skills/skills/productivity/README.md:1-14` — indexes the user-invoked productivity skills and explains their reachability; each directory's `SKILL.md` is the actual entry point. `vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:1-8` — routes skill authors to the companion mechanics reference.
- **Core abstractions:**
  - **Context pointer / progressive disclosure** — a trigger-bearing reference that selects out-of-context material for a branch (`vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:10-18,37-44`).
  - **Completion criterion** — a checkable end condition with an explicit demand bar for each step (`vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:47-50`).
  - **Checkpoint / brief / push right** — a delayed human decision with a concise decision-ready summary (`vendors/mattpocock-skills/skills/in-progress/loop-me/SKILL.md:10-23`).
  - **Handoff document** — a compact continuation artifact with suggested skills and references instead of duplicated source material (`vendors/mattpocock-skills/skills/productivity/handoff/SKILL.md:8-14`).
  - **Teaching workspace** — mission, resources, lessons, learning records, and reusable assets as persistent state (`vendors/mattpocock-skills/skills/productivity/teach/SKILL.md:13-21`).
  - **Learning record / feedback loop** — durable records of non-obvious lessons plus immediate practice feedback (`vendors/mattpocock-skills/skills/productivity/teach/SKILL.md:93-110`).
- **Cross-package dependencies:** No vendor package imports. Skills link to sibling Markdown files (`SKILL-MECHANICS.md` and the teaching format files) and use provider metadata under each `agents/` directory.
- **External runtime dependencies:** Claude Code/Codex skill invocation conventions in frontmatter/provider YAML; optional `claude --bg` in `in-progress/claude-handoff/SKILL.md`; otherwise no runtime framework or library.

## 3. Pattern catalog

### 3.1 Trigger-bearing pointers with progressive disclosure

- **Where:** `vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:10-18,37-44`
- **What it does:** A context pointer names out-of-context material and encodes the branches that should trigger it. The document separates always-loaded steps from disclosed reference, then co-locates a concept's definition, rules, and caveats once the pointer is followed; it also identifies sprawl and duplication as failure modes. This is a document lifecycle: define the trigger, disclose branch-specific material, and prune stale or duplicated guidance.
- **Why it's interesting:** Agent skill packs accumulate reference quickly, and a weak pointer can leave required material unreached while an overlong top-level skill buries the steps. Canon already routes from `SKILL.src.md` to role and command references, but the routing contract is mostly a static list rather than a general rule for writing trigger-specific pointers and deciding what stays inline.
- **Canon applicability:** **Canon has it partially.** `canon/skills/SKILL.src.md:55-83` routes by role and topic to `reference/role-*` and `reference/canon-*`, and `canon/skills/reference/canon-authoring.md:1-5` establishes a source-of-truth corpus. Canon lacks a compact authoring convention that requires each pointer to state its trigger branches and explicitly tests whether branch-specific material belongs behind it.
- **Effort to lift:** moderate.

### 3.2 Checkable completion criteria and demand bars

- **Where:** `vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:47-50`
- **What it does:** Every step ends with a completion criterion that distinguishes done from not-done. The criterion has both clarity and demand: it can require exhaustive accounting (for example, every modified model) rather than a vague output such as “produce a change list”; when later steps create premature-completion pressure, the document recommends a real context boundary rather than merely adding prose.
- **Why it's interesting:** This is a concrete anti-premature-completion mechanism for agents. It turns a prose procedure into a lifecycle with observable exit conditions and supplies a rule for when a handoff or subagent boundary is justified, which is directly relevant to Canon's planner/implementer/reviewer phases.
- **Canon applicability:** **Canon has it partially, with a stronger data-backed version.** `canon/skills/reference/role-planner.md:39-54` requires flow steps, acceptance, failure paths, and blocker-free checking; `canon/skills/SKILL.src.md:55-83` requires evidence, independent review, and a clean status transition. Canon does not state the cross-cutting authoring rule that every skill step must name its completion criterion and demand bar, so this can improve skill conventions without replacing scenario/evidence gates.
- **Effort to lift:** trivial.

### 3.3 Delayed human checkpoints with decision-ready briefs

- **Where:** `vendors/mattpocock-skills/skills/in-progress/loop-me/SKILL.md:10-23`
- **What it does:** A workflow distinguishes a trigger, an optional checkpoint, and a brief. “Push right” moves the checkpoint as late as possible so automated work prepares the maximum useful context before asking the human; the checkpoint presents a tight summary of what was produced, why, and where to inspect the asset, rather than dumping raw output. The workflow is intentionally non-structural: it adds AI, checkpoints, or schedules only when the loop requires them.
- **Why it's interesting:** This gives agents a reusable human-in-the-loop interaction shape that is more specific than “ask for approval”: perform maximal preparation, then present one bounded decision. It could reduce noisy interruptions around Canon's approval stop and make stop-point messages more decision-ready without weakening the human's authority.
- **Canon applicability:** **Canon has it, but not under this vocabulary.** `canon/skills/SKILL.src.md:18-33` requires a human-approved brief and forbids scenarios, code, or evidence before approval; `canon/skills/reference/role-orchestrator.md:58-62` requires stopping and reporting the brief at the human step. Canon already has the stronger lifecycle guard, so the reusable gap is a short “push right / brief” presentation convention for handoffs and reports, not another approval mechanism.
- **Effort to lift:** trivial.

### 3.4 Durable session handoff as a bounded continuation artifact

- **Where:** `vendors/mattpocock-skills/skills/productivity/handoff/SKILL.md:8-14` and `vendors/mattpocock-skills/skills/in-progress/claude-handoff/SKILL.md:7-16`
- **What it does:** The handoff skill instructs the current session to write a continuation document outside the workspace, include suggested skills, avoid duplicating existing artifacts, and redact sensitive information. The in-progress variant adds a descriptive background-agent name and passes the summary as a file-backed prompt, creating a concrete handoff lifecycle from capture to fresh-agent launch.
- **Why it's interesting:** Canon has independent sessions, actor/session IDs, and role dispatch, but agents still need a reliable human-readable bridge when a session ends or a role changes. The vendor pattern defines the minimum payload and explicitly prevents handoffs from becoming a second source of truth.
- **Canon applicability:** **Canon has it partially.** `canon/skills/reference/role-orchestrator.md:31-56` specifies one agent per role, separate sessions, and real session IDs; `canon/skills/reference/canon-session-ingest.md:1-20` ingests on-disk transcripts into durable Session/Run/Event records. Canon lacks a user-facing “continuation packet” schema that names next skills and points to existing artifacts while preserving privacy, so this is an opportunity for an optional handoff report rather than a replacement for session ingestion.
- **Effort to lift:** moderate.

### 3.5 Stateful learning workspace with mission, records, and feedback

- **Where:** `vendors/mattpocock-skills/skills/productivity/teach/SKILL.md:13-21,48-55,71-95,101-110`; supporting format `vendors/mattpocock-skills/skills/productivity/teach/LEARNING-RECORD-FORMAT.md:3-5,27-36`
- **What it does:** Teaching persists a mission, resources, lessons, reusable assets, notes, and sequential learning records. Each lesson is a small self-contained unit tied to the mission; the learning record captures non-obvious lessons, prior knowledge, and corrected misconceptions, while an interactive feedback loop gives immediate practice feedback. The workspace therefore has explicit durable state and a rule for when a new record is worth writing.
- **Why it's interesting:** Canon's learn feature stores verdict trajectories and promoted strategies for agent behavior, but it does not model the human's mission, prior knowledge, or pedagogical progress. The vendor's concrete artifact set offers a pattern for making “learn” useful beyond strategy promotion: preserve why a lesson mattered, what changed, and what should be taught next.
- **Canon applicability:** **Canon has a different learning system; this is a missing adjacent capability.** `canon/skills/reference/canon-learn.md:1-20` defines warm trajectories and durable strategies, and `canon/skills/reference/canon-learn.md:23-55,97-119` requires evaluation, approval, provenance, and lifecycle-safe promotion. Those records are operational strategy memory, not learner-facing mission/lesson state. A human-learning extension would need a separate record kind or artifact namespace and must not be conflated with promotion evidence.
- **Effort to lift:** significant.

### 3.6 Invocation policy and router selection

- **Where:** `vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL-MECHANICS.md:5-22`
- **What it does:** Skill frontmatter chooses between model-invoked and user-invoked behavior by trading permanent context load against human cognitive load. A router skill is recommended when user-invoked skills multiply: it names the siblings and when to reach for each, but cannot invoke them itself. This makes invocation and discovery an explicit lifecycle decision rather than an accidental frontmatter default.
- **Why it's interesting:** Canon projects provider-specific skills and has many references, so selection and loading cost are real. A written invocation policy could prevent every reference from becoming always-loaded context and could clarify which references are authoritative routers versus leaf documents.
- **Canon applicability:** **Canon has it partially.** `canon/skills/SKILL.src.md:55-83` already defines a topic router (“Route by task”) and provider projection, while `canon/skills/reference/canon-retrieve.md:44-71` defines an advisory pre-dispatch hook. Canon does not classify references by automatic versus explicit invocation or document the context/cognitive-load tradeoff; adopt the policy as authoring guidance, not as a runtime router that bypasses Canon's role gates.
- **Effort to lift:** moderate.

## 4. Risks & incompatibilities

- **Effect version:** Not applicable. The surface is Markdown/YAML and one shell script; it does not use Effect or a comparable library.
- **Framework:** Not applicable. There is no UI framework or application surface.
- **Heavy deps:** None in the audited files. `claude --bg` is an optional external CLI invocation, not a package dependency.
- **Coupling:** The teaching skills are coupled to their companion format files and workspace layout; writing-for-agents is coupled to its `SKILL-MECHANICS.md`; provider metadata is coupled to the selected harness. The handoff variants are coupled to the availability of a fresh background-agent process and a safe temporary location.
- **TOS-violation patterns:** None observed. The surface does not impersonate provider clients, forge request fingerprints, or evade rate limits.
- **License footnote:** No license routing decision is needed for the clean-room adoption described here. Any future verbatim-lift PR must separately inspect the vendor repository's license.

## 5. Recommended actions

1. **EXTRACT** the context-pointer, progressive-disclosure, and completion-criterion rules into `canon/skills/reference/canon-authoring.md` — they provide concrete authoring checks for keeping role skills short, branch-triggered, and explicitly complete without changing Canon's scenario/evidence model.
2. **INSPIRE** the “push right” and decision-ready brief vocabulary in `canon/skills/SKILL.src.md` and `canon/skills/reference/role-orchestrator.md` — Canon already enforces the human stop, but a concise prepared brief would make the stop-point handoff more consistent.
3. **INSPIRE** a bounded continuation packet in `canon/skills/reference/role-orchestrator.md` and `canon/skills/reference/canon-session-ingest.md` — Canon has session records and separate role sessions, while the vendor adds a privacy-aware, non-duplicating next-agent summary; keep it optional and reference existing artifacts rather than creating a second ledger.
4. **INSPIRE** a learner-facing mission/lesson/learning-record namespace beside, not inside, `canon-learn` — the vendor's durable prior-knowledge and misconception records complement Canon's verdict trajectories, but mixing them with signed strategy promotion would corrupt the meaning of operational evidence.
5. **EXTRACT** the invocation-policy and router rules into the skill authoring conventions, targeting `canon/skills/SKILL.src.md` and provider projection guidance — they turn an existing “Route by task” list into an explicit rule for automatic discovery versus explicit invocation and context cost.
6. **SKIP** the vendor's `setup-pre-commit`, `migrate-to-shoehorn`, and other repository-specific setup recipes for Canon's core — they are TypeScript/Husky or test-fixture conventions unrelated to Canon's Rust CLI and agent-evidence lifecycle.
7. **SKIP** the deprecated surface as an adoption source — `vendors/mattpocock-skills/skills/deprecated/README.md:1-3` explicitly says retired skills are deleted and replaced by named changesets, so it contains no live mechanism to lift.

## 6. Citations

- `vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:10-18` — context pointers encode out-of-context material and branch triggers.
- `vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:37-50` — progressive disclosure and checkable completion criteria.
- `vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:76-80` — single source of truth, environment lookup, relevance, and no-op pruning.
- `vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL-MECHANICS.md:5-22` — model/user invocation tradeoff and router skills.
- `vendors/mattpocock-skills/skills/in-progress/loop-me/SKILL.md:10-23` — trigger, checkpoint, push-right, and decision-ready brief.
- `vendors/mattpocock-skills/skills/productivity/handoff/SKILL.md:8-14` — handoff location, suggested skills, deduplication, and redaction.
- `vendors/mattpocock-skills/skills/in-progress/claude-handoff/SKILL.md:7-16` — background-agent handoff lifecycle and descriptive naming.
- `vendors/mattpocock-skills/skills/productivity/teach/SKILL.md:13-21` — persistent teaching workspace data shape.
- `vendors/mattpocock-skills/skills/productivity/teach/SKILL.md:71-95,101-110` — mission grounding, learning progression, trusted resources, and feedback loop.
- `vendors/mattpocock-skills/skills/productivity/teach/LEARNING-RECORD-FORMAT.md:3-5,27-36` — sequential durable learning records and write criteria.
- `canon/skills/SKILL.src.md:18-44,55-83` — human approval stop, role loop, and reference routing already present in Canon.
- `canon/skills/reference/role-orchestrator.md:25-62` — role ownership, separate sessions, and human-stop relay.
- `canon/skills/reference/canon-session-ingest.md:1-20` — durable session/run/event ingestion analog.
- `canon/skills/reference/canon-learn.md:1-20,23-55,97-119` — Canon's distinct trajectory/strategy learning and signed promotion lifecycle.
- `canon/skills/reference/canon-retrieve.md:44-95` — fail-soft advisory retrieval and replay snapshot semantics.

[Some lines truncated to 768 chars. Use agent://AuditMProductivity:raw:10-10 to read line 10 whole]
