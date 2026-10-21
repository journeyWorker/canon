# Surface — m-engineering

**Vendor paths**: `vendors/mattpocock-skills/skills/engineering/README.md`, `vendors/mattpocock-skills/skills/engineering/*/SKILL.md`, and supporting files under `vendors/mattpocock-skills/skills/engineering/`
**Scale**: 20 engineering skill directories plus supporting templates and references; approximately 20 primary `SKILL.md` files and several companion documents. LOC was not mechanically measured.
**Primary language(s)**: Markdown, Bash templates, and embedded TypeScript examples.
**Canon analog (if any)**: `canon/skills/SKILL.src.md` and `canon/skills/reference/role-*.md` — both describe agent workflows, but canon persists scope, scenarios, evidence, reviews, findings, and ship state in a typed ledger.

## 1. Overview

The `m-engineering` surface is a collection of composable engineering workflows rather than one end-to-end lifecycle. It covers planning, issue decomposition, implementation, TDD, diagnosis, code review, research, architecture, domain modeling, prototyping, setup, retrospectives, and human-only operational wizards. The skills are split into user-invoked process commands and model-invoked discipline references, with `ask-matt` acting as a router over the available flows. The surface deliberately delegates project-specific process ownership to the repository's issue tracker, glossary, ADRs, and configured conventions instead of defining a universal persistence model. Canon, by contrast, owns a repository-local approval, specification, evidence, independent-rev…

## 2. Entry points & architecture

- **Entry file(s)**:
  - `vendors/mattpocock-skills/skills/engineering/README.md:1-39` — catalogs the user-invoked and model-invoked engineering skills.
  - `vendors/mattpocock-skills/skills/engineering/ask-matt/SKILL.md:1-7` — routes users to the appropriate flow.
  - `vendors/mattpocock-skills/skills/engineering/ask-matt/SKILL.md:12-30` — describes the main idea-to-ship flow and its branches.
- **Core abstractions**:
  - **Skill router** — `ask-matt` maps situations to a main flow, bug/debugging on-ramp, planning on-ramp, architecture-health path, or standalone discipline.
  - **Spec** — `to-spec` turns an existing conversation into a tracker-published specification with problem, solution, stories, implementation decisions, testing decisions, and scope.
  - **Tracer-bullet ticket** — `to-tickets` creates independently verifiable vertical slices with explicit blocking edges.
  - **Feedback loop** — `diagnosing-bugs` requires a red-capable, deterministic command before hypotheses or fixes.
  - **Two-axis review** — `code-review` reviews a fixed-point diff separately for standards and specification fidelity.
  - **Deep module / seam** — `codebase-design` supplies vocabulary and heuristics for interfaces, adapters, depth, locality, leverage, and testability.
  - **Human-only wizard** — `wizard` turns manual provisioning or migration procedures into staged scripts while refusing to automate agent-performable work.
- **Cross-package dependencies**: No vendor package imports. Skills depend on the configured issue tracker, repository glossary and ADR layout, tracker-specific setup documents, and available agent skill/tool invocation mechanisms.
- **External runtime dependencies**: Git and the configured issue-tracker CLI or local Markdown convention; optional browser/dashboard tooling for wizards; project-specific test runners and typecheck commands. No framework or SDK is required by the skill pack itself.

## 3. Pattern catalog

### 3.1. Explicit separation between planning, implementation, and review

- **Where**: `vendors/mattpocock-skills/skills/engineering/ask-matt/SKILL.md:12-30`; `vendors/mattpocock-skills/skills/engineering/implement/SKILL.md:8-17`; `vendors/mattpocock-skills/skills/engineering/code-review/SKILL.md:6-23`
- **What it does**: The router defines a main flow from grilling and specification through tickets or direct implementation, then TDD and code review. `implement` explicitly invokes TDD, runs incremental and final checks, invokes code review, and commits. `code-review` pins a fixed point and separates standards review from specification review, running the two axes in parallel.
- **Why it's interesting**: This gives agents a clear phase boundary and prevents implementation from being treated as the whole process. It also preserves two different review questions: “does this meet the requested behavior?” and “does this follow the repository's standards?”
- **Canon applicability**: **Canon has it, but with a stronger persisted model.** `canon/skills/SKILL.src.md:39-53` defines planner, implementer, reviewer, and orchestrator phases; `canon/skills/reference/role-reviewer.md:1-15` requires an independent reviewer; `canon/skills/reference/canon-review.md:1-21` models findings and reviews as ledger records. Canon does not currently expose a separate standards/spec review axis as a named review structure; the review record is primarily tied to scenarios, evidence, and the upstream brief.
- **Effort to lift**: moderate.

### 3.2. Tracer-bullet tickets with dependency edges and a ready frontier

- **Where**: `vendors/mattpocock-skills/skills/engineering/to-tickets/SKILL.md:3-11`; `vendors/mattpocock-skills/skills/engineering/to-tickets/SKILL.md:25-40`; `vendors/mattpocock-skills/skills/engineering/to-tickets/SKILL.md:42-67`
- **What it does**: A plan or conversation is decomposed into narrow, complete vertical slices. Each ticket declares the tickets that block it; wide mechanical refactors use an expand–contract sequence instead of pretending to be vertical slices. The user approves granularity and edges before publication, and work proceeds from the currently unblocked frontier. The tracker implementation may use native dependency links or local `Blocked by` fields.
- **Why it's interesting**: Canon has units and scenario-level records, but it does not appear to have a first-class task graph for implementation work. This pattern makes concurrency and dependency reasoning explicit without conflating a task checklist with behavior coverage.
- **Canon applicability**: **Canon is partial.** `canon/skills/reference/role-planner.md:1-18` creates subjects, changes, and briefs; `canon/skills/reference/role-implementer.md:1-15` creates features and scenarios; `canon/skills/SKILL.src.md:45-52` requires units but does not define ticket dependency edges. `crates/canon-cli/src/change.rs:2-5` explicitly says flow coverage replaces a task checklist, while `crates/canon-cli/src/change.rs:93-103` writes and parses a brief rather than a task graph. A task graph could target change planning or unit dispatch without replacing scenario coverage.
- **Effort to lift**: significant.

### 3.3. Public-interface testing with pre-agreed seams and vertical red-green slices

- **Where**: `vendors/mattpocock-skills/skills/engineering/tdd/SKILL.md:13-26`; `vendors/mattpocock-skills/skills/engineering/tdd/SKILL.md:28-38`; `vendors/mattpocock-skills/skills/engineering/tdd/tests.md:1-35`
- **What it does**: Tests are written against public interfaces and observable behavior, not private implementation details or internal mocks. Before writing tests, the agent proposes seams and asks the user to confirm which seams are in scope, including what each seam catches and misses. The loop is one failing test, one minimal implementation, then refactoring later; expected values must come from an independent source of truth rather than recomputing the implementation.
- **Why it's interesting**: Canon binds test reports to scenarios and checks whether cases are present, but this pattern improves the quality of the test itself. It provides a concrete criterion for distinguishing scenario evidence that actually proves behavior from a test that merely exercises an implementation detail.
- **Canon applicability**: **Canon is partial.** `canon/skills/reference/role-implementer.md:39-58` requires tests named by scenario id and evidence bound to reports or artifacts. `canon/skills/reference/role-reviewer.md:45-56` tells the reviewer to compare each bound test with the scenario's Then steps and record a finding when the test skips or overclaims them. However, `canon/skills/reference/role-planner.md:22-31` defines acceptance and failure paths but does not require a pre-agreed seam note or an independent expected-value rule. A concrete target is the implementer evidence guidance and reviewer “check the checks” step.
- **Effort to lift**: moderate.

### 3.4. Red-capable diagnosis loop before hypothesis or fix

- **Where**: `vendors/mattpocock-skills/skills/engineering/diagnosing-bugs/SKILL.md:18-66`; `vendors/mattpocock-skills/skills/engineering/diagnosing-bugs/SKILL.md:68-92`; `vendors/mattpocock-skills/skills/engineering/diagnosing-bugs/SKILL.md:100-138`
- **What it does**: Diagnosis starts by building and already running one tight command that drives the actual bug and asserts the exact symptom. The loop is tightened for speed, signal sharpness, and determinism before reproduction and minimization. Only then does the agent generate 3–5 falsifiable hypotheses, instrument one variable at a time, write a regression test at the correct seam, and clean up tagged debugging instrumentation.
- **Why it's interesting**: It prevents plausible but ungrounded diagnosis and makes the initial failure itself a durable artifact. It also explicitly treats the absence of a correct test seam as an architectural finding rather than papering over it with a shallow test.
- **Canon applicability**: **Canon is partial.** `canon/skills/reference/role-reviewer.md:43-56` requires the reviewer to run the checks and inspect whether they prove the scenario; `canon/skills/reference/role-implementer.md:35-38` requires a scenario for every bug fix. But `crates/canon-cli/src/evidence.rs:31-39` says evidence provides attribution and audit, not verification, and `crates/canon-cli/src/evidence.rs:69-78` states that `evidence add` does not execute the referenced command or test. Canon therefore records and reviews claimed evidence but does not require a red-capable reproduction loop before implementation.
- **Effort to lift**: moderate.

### 3.5. Durable briefs with explicit current state, desired state, interfaces, acceptance, and scope

- **Where**: `vendors/mattpocock-skills/skills/engineering/triage/AGENT-BRIEF.md:3-5`; `vendors/mattpocock-skills/skills/engineering/triage/AGENT-BRIEF.md:28-37`; `vendors/mattpocock-skills/skills/engineering/triage/AGENT-BRIEF.md:39-67`
- **What it does**: The agent brief is the authoritative contract for an AFK agent, including both issue and PR surfaces. It names current behavior, desired behavior, key interfaces, independently verifiable acceptance criteria, and explicit out-of-scope items. It deliberately avoids file paths and line numbers because those are expected to become stale.
- **Why it's interesting**: This is a practical handoff format that keeps implementation durable while still naming behavioral and interface contracts. The explicit current/desired distinction is useful for review and for identifying scope creep.
- **Canon applicability**: **Canon has it, with a different lifecycle.** `canon/skills/reference/role-planner.md:20-37` defines the PRD brief sections: intent, outcome, flow, detail, acceptance, questions, and out of scope. `canon/skills/reference/role-reviewer.md:31-42` requires the reviewer to read that brief and compare scenarios against it. `crates/canon-cli/src/brief_template.rs:1-12` and `crates/canon-cli/src/brief_template.rs:54-67` implement a structured brief and fixed section order. The gap is not the brief itself; it is a distinct durable handoff view for an already-partially-built PR or review round.
- **Effort to lift**: trivial to moderate.

### 3.6. Process ownership is deliberately delegated to repository conventions and external systems

- **Where**: `vendors/mattpocock-skills/skills/engineering/setup-matt-pocock-skills/SKILL.md:7-15`; `vendors/mattpocock-skills/skills/engineering/to-spec/SKILL.md:8-19`; `vendors/mattpocock-skills/skills/engineering/wayfinder/SKILL.md:7-25`; `vendors/mattpocock-skills/skills/engineering/wizard/SKILL.md:3-10`
- **What it does**: The pack does not own a universal issue database, lifecycle ledger, or approval record. Setup discovers and records the repository's issue tracker, labels, glossary, and ADR layout; `to-spec` publishes to that configured tracker; wayfinder treats the tracker as the canonical map and stops when the route is clear; wizard handles only actions that genuinely require a human. The skills repeatedly say to consult the configured tracker or repository documents rather than inventing a second process.
- **Why it's interesting**: This is a deliberate non-ownership pattern, not an omission by accident. It keeps the skills portable across GitHub, GitLab, local Markdown, and other workflows, while using structured documents and tracker relationships as the process substrate.
- **Canon applicability**: **Canon conflicts in ownership philosophy but can adopt the boundary.** `canon/skills/SKILL.src.md:1-8` explicitly makes canon the shared core for every build/change/fix in a `canon.yaml` repository, and `canon/skills/SKILL.src.md:13-34` owns approval, policy, and brief semantics. `crates/canon-cli/src/brief.rs:28-35` says canon never edits an external plan and instead maintains its own `brief.md`; this is a strong, intentional divergence from the vendor's tracker-first ownership. The useful target is documenting where canon intentionally owns the process and where it delegates execution details to external trackers, test runners, or human procedures.
- **Effort to lift**: trivial for documentation; significant for adopting tracker-native task graphs.

### 3.7. Primary-source research and architecture/domain decision records

- **Where**: `vendors/mattpocock-skills/skills/engineering/research/SKILL.md:3-12`; `vendors/mattpocock-skills/skills/engineering/codebase-design/SKILL.md:8-12`; `vendors/mattpocock-skills/skills/engineering/codebase-design/SKILL.md:62-66`; `vendors/mattpocock-skills/skills/engineering/domain-modeling/SKILL.md:40-70`
- **What it does**: Research delegates reading to a background agent, requires primary sources, and writes one cited Markdown artifact. Codebase design supplies a stable vocabulary for deep modules and seams, while domain modeling challenges terminology, tests it against scenarios and code, updates the glossary inline, and reserves ADRs for hard-to-reverse, surprising trade-offs.
- **Why it's interesting**: Canon has a strong execution ledger but comparatively little prescribed structure for research notes, vocabulary decisions, or architecture findings that precede a brief. These practices could improve planner inputs and make review findings more reusable.
- **Canon applicability**: **Canon is partial.** `canon/skills/reference/role-planner.md:20-37` supports questions, assumptions, and out-of-scope decisions in the brief. `canon/skills/reference/canon-context.md` and `canon/skills/reference/canon-retrieve.md` provide context and retrieval mechanisms, but no equivalent requirement says that external research must use primary sources and cite claims, or that domain-term changes should be recorded separately from implementation scope. A target could be planner guidance or a non-coverage research artifact convention, not a new scenario lane.
- **Effort to lift**: moderate.

## 4. Risks & incompatibilities

- **Effect version**: Not applicable. The surface is Markdown guidance and Bash/TypeScript examples rather than an Effect or Rust library.
- **Framework**: No UI framework dependency. The prototype skill permits a single HTML file or project-native UI route, but this is guidance rather than a framework runtime.
- **Heavy deps**: No heavy transitive dependencies in the skill pack itself. Runtime cost comes from whichever tracker CLI, browser automation, test runner, or project tooling the consuming repository uses.
- **Coupling**:
  - `to-spec`, `to-tickets`, `triage`, and `wayfinder` depend on setup-provided issue-tracker configuration.
  - `to-spec`, TDD, domain modeling, and architecture skills depend on repository-local glossary and ADR conventions when present.
  - `implement` and `implement-spec` assume a Skill tool or equivalent subagent mechanism.
  - `wizard` depends on the supplied `template.sh` library and should not be generalized into an agent automation mechanism.
- **TOS-violation patterns**: None observed. The pack discusses external APIs, credentials, and issue trackers, but does not impersonate provider clients, forge request fingerprints, or evade rate limits.
- **License footnote**: No license routing decision is made here. Canon's default remains clean-room reimplementation from this audit rather than verbatim copying.

## 5. Recommended actions

1. **EXTRACT** the “check the checks” rule and public-interface evidence criteria → `canon/skills/reference/role-reviewer.md` and `canon/skills/reference/canon-review.md` — reviewers should explicitly record when a bound test does not prove every scenario Then step, including tautological or implementation-coupled checks.
2. **EXTRACT** the red-capable diagnosis criterion → `canon/skills/reference/role-implementer.md` and the bug-fix scenario authoring guidance — require or recommend a reproducible, symptom-specific command before a bug fix is treated as well-grounded.
3. **INSPIRE** tracer-bullet task graphs and explicit blocking edges → planner/orchestrator references and a future `canon` task or dispatch surface — canon's scenario corpus proves behavior, but implementation units could benefit from a separate dependency-aware frontier.
4. **INSPIRE** the two-axis standards/spec review distinction → `canon review` documentation and review record vocabulary — preserve canon's scenario/evidence pins while allowing a review to state whether it checked scope fidelity, repository standards, or both.
5. **INSPIRE** durable current/desired behavior and interface sections → `brief-shape.md` or `brief-polish.md` — add guidance that makes handoff contracts explicit without adding stale file-path requirements.
6. **INSPIRE** primary-source research and domain-decision conventions → planner context/retrieval documentation and optional research artifacts — make research provenance and terminology decisions reusable without counting them as product evidence.
7. **SKIP** adopting the vendor's tracker as canon's source of truth → `canon/skills/SKILL.src.md` and `crates/canon-cli/src/brief.rs` — canon intentionally owns an internal brief, ledger, and gate while allowing external plans to be imported by reference.
8. **SKIP** treating `implement`'s commit requirement as a canon gate invariant → `canon/skills/reference/role-implementer.md` — canon's evidence and review records pin commits or upstream refs where needed, but a universal commit workflow would over-couple canon to a particular branch/PR process.
9. **SKIP** copying the wizard's manual credential and dashboard automation → outside canon's process core — it is intentionally human-only operational assistance, not scope/evidence/review behavior.

## 6. Citations

- `vendors/mattpocock-skills/skills/engineering/README.md:1-39` — complete engineering skill catalog and user/model invocation split.
- `vendors/mattpocock-skills/skills/engineering/ask-matt/SKILL.md:12-30` — main idea-to-ship flow, branches, and handoffs.
- `vendors/mattpocock-skills/skills/engineering/to-tickets/SKILL.md:25-40` — tracer-bullet tickets, blocking edges, and expand–contract refactors.
- `vendors/mattpocock-skills/skills/engineering/to-tickets/SKILL.md:42-67` — user approval of ticket granularity and tracker-specific publication/frontier behavior.
- `vendors/mattpocock-skills/skills/engineering/tdd/SKILL.md:13-38` — public-interface tests, pre-agreed seams, red-green loop, and anti-patterns.
- `vendors/mattpocock-skills/skills/engineering/diagnosing-bugs/SKILL.md:18-66` — tight, deterministic, red-capable feedback loop requirement.
- `vendors/mattpocock-skills/skills/engineering/diagnosing-bugs/SKILL.md:90-120` — falsifiable hypotheses, one-variable instrumentation, and correct-seam regression tests.
- `vendors/mattpocock-skills/skills/engineering/code-review/SKILL.md:6-23` — fixed-point diff review and separate Standards/Spec axes.
- `vendors/mattpocock-skills/skills/engineering/triage/AGENT-BRIEF.md:3-37` — durable authoritative handoff with acceptance and out-of-scope boundaries.
- `vendors/mattpocock-skills/skills/engineering/wayfinder/SKILL.md:7-25` — deliberate planning-only ownership boundary and tracker-backed decision map.
- `vendors/mattpocock-skills/skills/engineering/research/SKILL.md:3-12` — primary-source research and cited Markdown artifact.
- `vendors/mattpocock-skills/skills/engineering/codebase-design/SKILL.md:8-12,62-66` — deep-module vocabulary, deletion test, seam discipline, and locality.
- `vendors/mattpocock-skills/skills/engineering/domain-modeling/SKILL.md:40-70` — glossary challenge, inline terminology updates, and ADR threshold.
- `canon/skills/SKILL.src.md:13-34` — canon-owned approval, policy, and brief lifecycle.
- `canon/skills/SKILL.src.md:39-53` — canon's planner, implementer, reviewer, orchestrator working loop.
- `canon/skills/reference/role-planner.md:20-37` — brief structure, approval stop, and questions/out-of-scope handling.
- `canon/skills/reference/role-implementer.md:35-58` — scenario-linked tests and evidence binding.
- `canon/skills/reference/role-reviewer.md:31-56` — brief/scenario review and “check the checks” responsibility.
- `canon/skills/reference/canon-review.md:1-21` — Findings and Reviews as separate attestations.
- `crates/canon-cli/src/evidence.rs:31-39,69-78` — evidence as attribution/audit rather than command execution or verification.
- `crates/canon-cli/src/brief.rs:28-35` — canon's deliberate choice not to edit external plans.

[Some lines truncated to 768 chars. Use agent://AuditMEngineering:raw:10-10 to read line 10 whole]
