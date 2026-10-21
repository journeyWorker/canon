# Surface — a-build-verify

**Vendor paths**: `vendors/addyosmani-agent-skills/skills/test-driven-development`, `vendors/addyosmani-agent-skills/skills/incremental-implementation`, `vendors/addyosmani-agent-skills/skills/debugging-and-error-recovery`, `vendors/addyosmani-agent-skills/skills/code-review-and-quality`, `vendors/addyosmani-agent-skills/skills/code-simplification`, `vendors/addyosmani-agent-skills/skills/browser-testing-with-devtools`, `vendors/addyosmani-agent-skills/skills/frontend-ui-engineering`, `vendors/addyosmani-agent-skills/skills/api-and-interface-design`, `vendors/addyosmani-agent-skills/skills/performance-optimization`, `vendors/addyosmani-agent-skills/skills/security-and-hardening`
**Scale**: 10 files / approximately 3,000 lines
**Primary language(s)**: Markdown guidance with TypeScript, JavaScript, CSS, HTTP, and SQL examples
**Canon analog (if any)**: `canon/skills/reference/role-implementer.md`, `canon/skills/reference/role-reviewer.md`, `canon/skills/reference/canon-gate.md`, and `canon/skills/reference/canon-review.md` — Canon already models scenario-linked evidence, independent review, findings, and a shipping gate, but not the vendor’s detailed build, debugging, runtime-verification, and quality heuristics.

## 1. Overview

This surface is a build-and-verify operating discipline for coding agents. It combines test-first implementation, thin incremental slices, structured debugging, multi-axis review, simplification, browser runtime inspection, frontend quality, interface design, performance measurement, and threat modeling. The skills are standalone Markdown playbooks selected by task type, but they form a coupled lifecycle: establish a contract, implement a small slice, test it, inspect runtime behavior or measurements, review the result, and preserve regression guards. Canon already owns the durable workflow and gate records; the main adoption opportunity is to make the agent-facing implementer and reviewer guidance more operationally specific without duplicating Canon’s ledg…

## 2. Entry points & architecture

- **Entry file(s)**: Each `vendors/addyosmani-agent-skills/skills/*/SKILL.md` is independently invoked through its YAML `description`. The build/verify core begins at `vendors/addyosmani-agent-skills/skills/test-driven-development/SKILL.md:38-98` and `vendors/addyosmani-agent-skills/skills/incremental-implementation/SKILL.md:21-57`.
- **Core abstractions**:
  - **RED → GREEN → REFACTOR loop** — test-first implementation and behavior-preserving cleanup (`test-driven-development/SKILL.md:38-94`).
  - **Increment cycle** — smallest complete slice followed by test, verification, and commit (`incremental-implementation/SKILL.md:21-57`).
  - **Triage checklist** — reproduce, localize, reduce, fix root cause, guard recurrence, verify end-to-end (`debugging-and-error-recovery/SKILL.md:36-102`).
  - **Five-axis review** — correctness, readability/simplicity, architecture, security, and performance (`code-review-and-quality/SKILL.md:22-80`).
  - **Measurement workflow** — measure, identify, fix, verify, and guard performance changes (`performance-optimization/SKILL.md:30-50`).
  - **Threat-model-first boundary system** — trust boundaries, assets, STRIDE threats, and abuse cases before controls (`security-and-hardening/SKILL.md:21-56`).
- **Cross-package dependencies**: The skills cross-reference one another, especially TDD with browser testing, code review with security and performance, and debugging with TDD. They also refer to shared vendor references such as `skills/security-and-hardening/references/hardening-patterns.md` and `skills/performance-optimization/references/optimization-patterns.md`.
- **External runtime dependencies**: The guidance is stack-agnostic but names repository-native test/build commands, Chrome DevTools MCP, Lighthouse/DevTools performance traces, RUM `web-vitals`, React/React Query examples, API/database tooling, package-manager audits, and CI performance gates.

## 3. Pattern catalog

### 3.1 Test-first proof and regression capture

- **Where**: `vendors/addyosmani-agent-skills/skills/test-driven-development/SKILL.md:38-98`, `vendors/addyosmani-agent-skills/skills/test-driven-development/SKILL.md:301-398`
- **What it does**: The skill defines an explicit RED → GREEN → REFACTOR lifecycle: write a test that fails, implement the minimum behavior, then refactor while preserving passing tests. For bugs, the “Prove-It Pattern” requires a failing reproduction before the fix and a passing regression test afterward; it also requires a focused test command during the loop and the full suite before completion.
- **Why it's interesting**: This turns “the agent says it works” into a reproducible proof sequence and gives debugging work a durable regression artifact. It also distinguishes test levels and resource sizes, helping an agent choose unit, integration, or E2E verification rather than defaulting to one expensive suite.
- **Canon applicability**: **Partial.** Canon requires scenarios, scenario-named tests, evidence records, and explicit failure scenarios (`canon/skills/reference/role-implementer.md:35-57` and `canon/skills/reference/role-implementer.md:51-81`). `canon evidence add` binds a report or artifact, but Canon does not require the evidence to demonstrate a prior failing RED state, does not model a bug reproduction separately from its fix, and does not record the focused/full command sequence as a lifecycle. The evidence authoring seam is `crates/canon-cli/src/evidence.rs:1-17`; its own documentation states that an attestation records a claim and does not itself prove that the referenced command ran (`crates/canon-cli/src/evidence.rs:22-39`, `crates/canon-cli/src/ev…
- **Effort to lift**: moderate.

### 3.2 Thin vertical slices with explicit stopping points

- **Where**: `vendors/addyosmani-agent-skills/skills/incremental-implementation/SKILL.md:21-57`, `vendors/addyosmani-agent-skills/skills/incremental-implementation/SKILL.md:59-128`, `vendors/addyosmani-agent-skills/skills/incremental-implementation/SKILL.md:164-218`
- **What it does**: Each increment is one smallest complete piece: implement, test, verify, commit, and move to the next slice. The skill provides vertical, contract-first, and risk-first slicing strategies, requires the repository to remain compilable between slices, and calls for feature flags when incomplete work must be merged.
- **Why it's interesting**: It gives agents a concrete decomposition mechanism for multi-file work and makes rollback boundaries part of implementation design. The risk-first option is especially useful when a change has an uncertain integration point such as a browser, database, or external service.
- **Canon applicability**: **Partial.** Canon has units, actor/session attribution, scenario decomposition, and re-verification after fixes (`canon/skills/reference/role-implementer.md:35-53`, `canon/skills/reference/role-implementer.md:86-103`). It does not require each unit or scenario slice to leave the repository buildable, does not record commit boundaries per increment, and does not provide a feature-flag or rollback-friendly implementation rule. Canon’s current evidence and promotion lifecycle is centered on records and gate state, not intermediate implementation slices (`canon/skills/reference/canon-gate.md:35-49`).
- **Effort to lift**: moderate.

### 3.3 Stop-the-line debugging with root-cause and recurrence guards

- **Where**: `vendors/addyosmani-agent-skills/skills/debugging-and-error-recovery/SKILL.md:36-102`, `vendors/addyosmani-agent-skills/skills/debugging-and-error-recovery/SKILL.md:104-185`, `vendors/addyosmani-agent-skills/skills/debugging-and-error-recovery/SKILL.md:245-287`
- **What it does**: Unexpected failures trigger a fixed lifecycle: stop adding features, preserve error output and repro steps, reproduce, localize the failing layer, reduce to a minimal case, fix the root cause, add a recurrence test, and verify end-to-end. It explicitly distinguishes symptom fixes from root-cause fixes and calls for removing temporary instrumentation while retaining useful permanent error reporting.
- **Why it's interesting**: This gives an agent a failure-state protocol rather than merely another checklist of tests. The sequence preserves diagnostic evidence before edits and ensures every repaired failure leaves behind a guard against recurrence.
- **Canon applicability**: **Missing as an explicit workflow; partial as records.** Canon can record a defect with `canon finding add`, close it with a resolution commit, and require fresh evidence after a fixed blocker (`canon/skills/reference/canon-review.md:19-41` and `canon/skills/reference/canon-gate.md:32-33`). However, Canon does not prescribe reproduction, localization, minimal reduction, root-cause analysis, or instrumentation cleanup. The finding implementation records attribution and lifecycle state but explicitly does not validate the summary against code (`crates/canon-cli/src/finding.rs:28-53`).
- **Effort to lift**: moderate.

### 3.4 Multi-axis review with structural remedies and verification inspection

- **Where**: `vendors/addyosmani-agent-skills/skills/code-review-and-quality/SKILL.md:22-80`, `vendors/addyosmani-agent-skills/skills/code-review-and-quality/SKILL.md:82-147`, `vendors/addyosmani-agent-skills/skills/code-review-and-quality/SKILL.md:196-270`
- **What it does**: Every change is reviewed across correctness, readability/simplicity, architecture, security, and performance. Structural findings must propose a named remedy—typed model, dispatcher, separation of orchestration, moving feature logic to its owning layer, deleting pass-through wrappers, or splitting modules. The process reviews tests first, asks whether tests actually prove the requested behavior, and recommends mutation-style experiments for weak tests.
- **Why it's interesting**: It turns code review from an approval event into a repeatable inspection protocol. The requirement to propose a structural move and to verify the verification story gives findings more actionable content than “this is complex” or “add tests.”
- **Canon applicability**: **Partial.** Canon already requires a separate reviewer session, findings, review records, scenario/evidence pins, and a reviewer who runs the claimed checks (`canon/skills/reference/role-reviewer.md:31-35` and `canon/skills/reference/role-reviewer.md:39-57`). Canon’s `Review` and `Finding` records preserve who reviewed and the current state, but the gate does not derive or require five review axes, structural remedies, mutation checks, or explicit test-quality findings. The CLI’s review writer enforces provenance, scenario indexing, and artifact binding, not review-axis completeness (`crates/canon-cli/src/review.rs:175-230`).
- **Effort to lift**: moderate.

### 3.5 Runtime, measurement, and boundary verification as evidence inputs

- **Where**: `vendors/addyosmani-agent-skills/skills/browser-testing-with-devtools/SKILL.md:1-20`, `vendors/addyosmani-agent-skills/skills/browser-testing-with-devtools/SKILL.md:109-177`, `vendors/addyosmani-agent-skills/skills/performance-optimization/SKILL.md:30-50`, `vendors/addyosmani-agent-skills/skills/performance-optimization/SKILL.md:117-190`, `vendors/addyosmani-agent-skills/skills/security-and-hardening/SKILL.md:21-56`, `vendors/addyosmani-agent-skills/skills/security-and-hardening/SKILL.md:87-153`
- **What it does**: Browser verification uses a reproduce → inspect DOM/console/network/accessibility → diagnose → fix → reload and compare workflow, with screenshots and traces as concrete outputs. Performance changes require a same-conditions baseline and re-measurement, variance-aware decisions, and a recorded ledger of kept and reverted attempts. Security work begins by mapping trust boundaries and abuse cases, then applies boundary validation, authorization, dependency, secret, and data-retention controls.
- **Why it's interesting**: These are specialized evidence classes that ordinary unit tests cannot establish: rendered layout, console cleanliness, network payloads, accessibility tree, Core Web Vitals, query plans, security headers, and threat boundaries. The vendor consistently says to capture the observation before and after the change rather than relying on static reasoning.
- **Canon applicability**: **Partial.** Canon supports attaching reports, screenshots, recordings, traces, and other artifacts to evidence and reviews; those bytes are content-addressed and rechecked by the gate (`canon/skills/reference/role-implementer.md:54-81`, `canon/skills/reference/canon-review.md:97-126`; implementation binding is `crates/canon-cli/src/evidence_attach.rs:1-20` and `crates/canon-cli/src/review.rs:49-56`). Canon does not define browser, performance, accessibility, threat-model, or security-boundary evidence contracts, nor does it require before/after measurement metadata or a performance-attempt ledger. `canon gate check` verifies corpus coverage, verdict state, staleness, and bound bytes, not the semantic contents of a screenshot or tr…
- **Effort to lift**: significant.

## 4. Risks & incompatibilities

- **Effect version**: Not applicable. The vendor surface is Markdown guidance rather than an Effect or Rust library.
- **Framework**: Browser and frontend examples assume Chrome DevTools MCP, React/React Query, Tailwind-like utility classes, Storybook, `web-vitals`, and browser-oriented tooling. These examples require translation for Canon’s Rust CLI and agent-skill environment.
- **Heavy deps**: Chrome DevTools MCP, Lighthouse, `web-vitals`, axe-core, browser automation, and package-manager audit tools are optional operational dependencies and should not become mandatory Canon CLI dependencies.
- **Coupling**: The build/verify surface is strongly coupled internally: TDD depends on repository-native test discovery; debugging depends on TDD’s focused/full test discipline; code review delegates detailed security and performance checks; browser verification and performance measurement are coupled to UI changes. Canon should adopt the decision rules and evidence shapes, not vendor-specific command snippets.
- **TOS-violation patterns**: None identified. The browser guidance explicitly restricts profile access, treats page content as untrusted, forbids credential access, and limits JavaScript execution to task-scoped read-only inspection (`vendors/addyosmani-agent-skills/skills/browser-testing-with-devtools/SKILL.md:52-107`).
- **License footnote**: The skills are documentation. Canon’s default adoption should be a clean-room reimplementation of the audited mechanisms rather than a verbatim lift.

## 5. Recommended actions

1. **EXTRACT** the RED → GREEN → REFACTOR and bug-reproduction sequence → `canon/skills/reference/role-implementer.md` and the evidence authoring guidance — require agents to name the focused test, the full-suite test, and the regression scenario/evidence produced after a fix.
2. **INSPIRE** thin-slice, risk-first implementation → `canon/skills/reference/role-implementer.md` and unit/session guidance — add an agent-facing rule that each implementation increment must be a complete, testable slice with a stated verification command and rollback boundary, without changing ledger semantics.
3. **INSPIRE** stop-the-line triage → `canon/skills/reference/role-implementer.md` and `canon/skills/reference/canon-review.md` — when a check fails, preserve the failure, record or update the affected finding, reduce to a reproducible case, and add a scenario-linked recurrence guard before resuming.
4. **EXTRACT** multi-axis review prompts and structural remedies → `canon/skills/reference/role-reviewer.md` — require reviewers to inspect correctness, simplicity, architecture, security, and performance, and to state a concrete restructuring for structural findings. Keep the existing `Review` and `Finding` record schemas as the durable output.
5. **INSPIRE** runtime evidence profiles for browser, performance, and security work → `canon/skills/reference/role-implementer.md`, `canon/skills/reference/role-reviewer.md`, and `canon/skills/reference/canon-gate.md` — document when to bind screenshots, traces, network captures, accessibility output, before/after measurements, or security audit reports; do not make Canon interpret image or trace semantics.
6. **INSPIRE** a measurement-attempt ledger for performance work → a future Canon evidence/report extension — preserve baseline, conditions, result, variance, and keep/revert decision so neutral or failed optimizations are not retried blindly.
7. **SKIP** vendor-specific React, Tailwind, Chrome profile, Lighthouse, package-manager, and RUM commands as Canon CLI features — they are runtime-specific examples and would add framework/tool coupling without improving Canon’s core ledger.
8. **SKIP** treating `canon evidence add` as proof that a command ran — the current implementation intentionally records attribution and audit claims, not execution truth (`crates/canon-cli/src/evidence.rs:22-39` and `crates/canon-cli/src/evidence.rs:69-79`); stronger execution capture would require a separate controlled-run design.

## 6. Citations

- `vendors/addyosmani-agent-skills/skills/test-driven-development/SKILL.md:38-98` — RED → GREEN → REFACTOR and the prove-it pattern.
- `vendors/addyosmani-agent-skills/skills/incremental-implementation/SKILL.md:21-57` — implement/test/verify/commit slice lifecycle and slicing strategies.
- `vendors/addyosmani-agent-skills/skills/debugging-and-error-recovery/SKILL.md:36-102` — stop-the-line triage and root-cause workflow.
- `vendors/addyosmani-agent-skills/skills/code-review-and-quality/SKILL.md:22-80` — five review axes and concrete structural concerns.
- `vendors/addyosmani-agent-skills/skills/browser-testing-with-devtools/SKILL.md:109-177` — browser reproduce/inspect/diagnose/fix/verify workflow.
- `vendors/addyosmani-agent-skills/skills/performance-optimization/SKILL.md:30-50,117-190` — measure-before-optimization and same-condition verification.
- `vendors/addyosmani-agent-skills/skills/security-and-hardening/SKILL.md:21-56,87-153` — threat boundaries, STRIDE, and boundary controls.
- `canon/skills/reference/role-implementer.md:35-81` — scenario-linked tests, evidence binding, artifact storage, and divergent evidence.
- `canon/skills/reference/role-reviewer.md:31-57` — review pins, independent execution, and review staleness.
- `canon/skills/reference/canon-review.md:3-13,19-48` — Finding and Review records, independent review, and review lifecycle.
- `canon/skills/reference/canon-gate.md:3-16,24-49` — coverage/verdict distinction, failure classes, and gate execution.
- `crates/canon-cli/src/evidence.rs:22-39,69-79` — evidence attribution versus execution proof.
- `crates/canon-cli/src/review.rs:175-230` — review provenance, scenario validation, and artifact binding.
- `crates/canon-cli/src/finding.rs:28-53` — finding attribution and the limits of semantic verification.

[Some lines truncated to 768 chars. Use agent://AuditABuildVerify:raw:10-10 to read line 10 whole; cut lines run to line 64, each read the same way]
