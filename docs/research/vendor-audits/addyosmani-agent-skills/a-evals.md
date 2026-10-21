# Surface — a-evals

**Vendor paths**: `vendors/addyosmani-agent-skills/evals/cases`, `vendors/addyosmani-agent-skills/evals/fixtures`, `vendors/addyosmani-agent-skills/evals/plugin`, `vendors/addyosmani-agent-skills/scripts/run-evals.js`, `vendors/addyosmani-agent-skills/scripts/run-evals-test.js`  
**Scale**: 26 case files, 28 fixture directories/files, 6 plugin cases, 2 runner/test files, 1 protocol README, approximately 3,500 LOC across the evaluated surface  
**Primary language(s)**: JSON, Markdown, JavaScript (Node.js)  
**Canon analog (if any)**: `evals/dogfood/trace_check.py` and `evals/dogfood/test_trace_check.py` — Canon already analyzes recorded agent traces with fixture-backed pass/fail checks, while this surface adds catalog routing, behavioral expectations, and plugin ablation patterns.

## 1. Overview

This surface is a three-tier evaluation system for agent skills. Tier 1 is structural validation elsewhere in the repository; Tier 2 deterministically evaluates skill routing and catalog hygiene; Tier 3 runs a skill against a fixture-backed or dialogue prompt and grades the resulting trace. A separate Claude Code plugin suite evaluates natural-language routing and response deltas with and without the whole plugin. The surface is coupled to the vendor's `skills/` catalog and Claude/Claude Code runtimes, but its case and fixture formats are independently understandable.

## 2. Entry points & architecture

- **Entry file(s):** `vendors/addyosmani-agent-skills/scripts/run-evals.js:208-400` — loads skill frontmatter and per-skill JSON cases, runs deterministic trigger/schema/fixture/collision checks, and enforces an optional rank-1 floor. `vendors/addyosmani-agent-skills/scripts/run-evals.js:539-669` — runs opt-in behavioral evaluations through headless `claude`, grades traces, persists results, and cleans workspaces.
- **Core abstractions:**
  - `trigger.positive` / `trigger.negative` case arrays — realistic routing prompts, top-k expectations, and optional owner skills for pairwise negative checks (`vendors/addyosmani-agent-skills/evals/README.md:57-95`).
  - `evals[]` — skill-creator-shaped behavioral records with `id`, `kind`, `prompt`, `expected_output`, `files`, and `expectations[]` (`vendors/addyosmani-agent-skills/evals/README.md:64-95`).
  - `materializeWorkspace` / `initializeWorkspace` — copies fixture paths into a fresh workspace, applies optional `.eval/working-tree.patch`, initializes a deterministic local Git baseline, and returns the execution root (`vendors/addyosmani-agent-skills/scripts/run-evals.js:403-452`).
  - `parseGrading` / `persistGradingOutcome` — validates one verdict per expectation, requires nonblank evidence, derives summary counters, and preserves invalid raw grader output rather than publishing it (`vendors/addyosmani-agent-skills/scripts/run-evals.js:454-537`).
  - Plugin case directory — `prompt.md` frontmatter constrains turns/tools and `graders/` combines `tool_used`, `regex`, and `llm` graders (`vendors/addyosmani-agent-skills/evals/README.md:40-51`).
- **Cross-package dependencies:** The runner imports only its local `scripts/lib/skill-lint.js`; cases refer to the sibling `skills/` catalog and `evals/fixtures/`. Plugin evaluation depends on the root plugin manifest's `experimental.evals` scope.
- **External runtime dependencies:** Node.js built-ins and the `claude` CLI for Tier 3; Claude Code's `claude plugin eval` for plugin cases. Tier 2 has no network or model dependency.

## 3. Pattern catalog

### 3.1 Layered deterministic routing gate with pairwise negatives and collision detection

- **Where:** `vendors/addyosmani-agent-skills/evals/README.md:14-22,91-103`; `vendors/addyosmani-agent-skills/scripts/run-evals.js:52-59,208-399`; `vendors/addyosmani-agent-skills/evals/cases/test-driven-development.json:3-31`
- **What it does:** Each skill must have a same-named case file with at least three positive prompts, two negative prompts, and one behavioral eval. Positive prompts are ranked against all skill descriptions using stemmed TF-IDF and must land within `top_k`; a negative fails if the target ranks first, and an `owner` turns it into a non-vacuous pairwise assertion that the intended skill outranks the target. The runner also warns at 50% and errors at 75% description cosine similarity, then reports a rank-1 rate with an optional `--min-rank1` ratchet.
- **Why it's interesting:** It tests the skill catalog as a routing system rather than treating each skill in isolation. Pairwise negatives and collision thresholds expose false positives and description drift, while a free deterministic tier is suitable for every change and gives a concrete metric before spending model tokens.
- **Canon applicability:** **Partial.** Canon has a deterministic gate over scenario coverage, required cases, evidence, reviews, sampled audits, and false-success (`canon/skills/reference/canon-gate.md:18-33,173-185`), and its authoring vocabulary explicitly treats `happy`/`failure`/`edge` as distinct cases (`canon/skills/reference/canon-authoring.md:54-80`). It does not currently have a catalog-level router eval that ranks role/skill descriptions against positive and owner-bound negative prompts. A concrete target is a new `canon skill eval`/`canon selftest` surface adjacent to `crates/canon-cli/src/gate.rs:1169-1195`, with deterministic case files and a reported rank-1/collision metric; this must remain separate from the product evidence gate so lexical r…
- **Effort to lift:** moderate.

### 3.2 Fixture-backed behavioral runs with reproducible workspace baselines

- **Where:** `vendors/addyosmani-agent-skills/evals/README.md:36-38,57-99`; `vendors/addyosmani-agent-skills/scripts/run-evals.js:403-452,539-604`; `vendors/addyosmani-agent-skills/evals/cases/test-driven-development.json:64-118`; `vendors/addyosmani-agent-skills/evals/fixtures/test-driven-development/README.md:1-20`
- **What it does:** An execution eval declares repository-relative `files[]` fixtures and expectation statements. The runner copies them into a fresh throwaway workspace, applies an optional working-tree patch, initializes Git with fixed identity/configuration, commits a `fixture baseline`, and then runs the agent with explicit edit/bash/read tools. A `dialogue` kind deliberately omits fixtures and grades the conversation instead; the deterministic gate rejects missing fixtures for execution evals and provisional cases.
- **Why it's interesting:** The fixture is not merely prompt context: it is a reproducible initial state that supports real edits, command runs, diffs, and commit-sensitive workflow checks. The distinction between execution and dialogue prevents a conversational result from silently standing in for an implementation artifact.
- **Canon applicability:** **Partial.** Canon already has replayable immutable context packs whose manifest selects repository-relative files and stores their bytes content-addressably (`canon/skills/reference/canon-context.md:40-58`), and dispatch manifests snapshot run inputs for replay rather than refreshing live retrieval (`canon/skills/reference/canon-retrieve.md:79-92`). Canon also binds external artifacts/reports to evidence by digest (`crates/canon-cli/src/evidence_attach.rs:1-13,41-69`). What is missing is a first-class, named “evaluation workspace baseline” that records fixture materialization, setup patch, tool permissions, and executor invocation as one run input. Concrete targets are `canon context-pack` metadata plus a run/evidence artifact man…
- **Effort to lift:** significant.

### 3.3 Trace-first grading with strict, evidence-bearing verdict normalization

- **Where:** `vendors/addyosmani-agent-skills/scripts/run-evals.js:590-637`; `vendors/addyosmani-agent-skills/scripts/run-evals-test.js:68-168,171-285`; `vendors/addyosmani-agent-skills/evals/plugin/tdd-fires-on-pasted-bug/graders/failing-test-first.md:1-5`
- **What it does:** The executor emits verbose `stream-json`; the grader receives the full trace between explicit untrusted-data markers over stdin and is told to judge tool calls, file edits, and command runs rather than prose claims. The parser requires exactly one integer-id-bound result per declared expectation, boolean `passed`, nonblank `evidence`, and internally consistent pass/fail/total counters; invalid output is retained as `.grading.raw.txt` and never published as scored JSON. Plugin graders apply the same principle at smaller scope: a `tool_used: Skill` indicator is paired with an output regex or LLM judge, and the skill-fired signal is excluded from two-arm scoring.
- **Why it's interesting:** It creates a typed boundary between an untrusted model transcript and a scored result. Evidence is mandatory even for failed expectations, IDs bind results to canonical expectation text, and malformed/blank grading cannot accidentally become a green artifact. The plugin rule avoids counting “the skill fired” as proof that the response improved.
- **Canon applicability:** **Has it, but with a boundary.** Canon's dogfood analyzer already parses recorded OMP/Codex JSONL without executing transcript content, separates implementer/reviewer sessions, detects writes before the brief stop and question-only writes, and reports recovery depth (`evals/dogfood/trace_check.py:287-300,522-662`; `evals/dogfood/test_trace_check.py:17-38,82-108`). Canon's evidence model also requires external audit identity/version/digest/result and derives the verdict (`canon/skills/reference/canon-gate.md:240-262`). The gap is not basic trace checking; it is a reusable expectation-to-verdict schema with mandatory per-expectation evidence and raw-invalid retention. A concrete target is `evals/dogfood/trace_check.py`/fixtures for p…
- **Effort to lift:** moderate.

### 3.4 Pressure cases and explicit behavioral pass criteria

- **Where:** `vendors/addyosmani-agent-skills/evals/README.md:36-38,53-55`; `vendors/addyosmani-agent-skills/evals/cases/test-driven-development.json:89-118`; `vendors/addyosmani-agent-skills/evals/cases/incremental-implementation.json:36-54`; `vendors/addyosmani-agent-skills/evals/plugin/tdd-fires-on-pasted-bug/prompt.md:1-17`
- **What it does:** Behavioral cases state an expected outcome and several observable expectations, including ordering (“shown failing before the fix”), invariant examples, full-suite execution, and pressure resistance. Cases intentionally introduce hotfix deadlines, authority instructions, sunk-cost arguments, or pasted code with no mention of tests. The grader scores the artifact and trace against these statements rather than matching a preferred wording.
- **Why it's interesting:** It evaluates whether a skill preserves its process under adversarial incentives, not only whether it can produce a happy-path answer. Ordering and negative criteria make “eventually did the right thing” insufficient when the workflow promises a required sequence.
- **Canon applicability:** **Partial.** Canon scenarios already require observable outcomes and explicit failure paths (`canon/skills/reference/canon-authoring.md:15-18,54-80`), and the dogfood fixtures pin process failures such as pre-brief writes and question-turn writes (`evals/dogfood/test_trace_check.py:26-38`). However, Canon's standard scenario/evidence model does not provide a general pressure-case field or a grader that checks event ordering across a session. Concrete target: extend the dogfood protocol fixture convention (not the product `.feature` vocabulary) with pressure prompts and ordered trace predicates, then bind the resulting report as an artifact to the relevant process scenario. Keep `process` lane scenarios excluded from product coverag…
- **Effort to lift:** moderate.

### 3.5 Plugin ablation and multi-grader delta measurement

- **Where:** `vendors/addyosmani-agent-skills/evals/README.md:40-55`; `vendors/addyosmani-agent-skills/evals/plugin/code-review-fires/prompt.md:1-16`; `vendors/addyosmani-agent-skills/evals/plugin/code-review-fires/graders/skill-fired.md:1-3`; `vendors/addyosmani-agent-skills/evals/plugin/code-review-fires/graders/severity-labels.md:1-3`; `vendors/addyosmani-agent-skills/evals/plugin/tdd-fires-on-pasted-bug/graders/failing-test-first.md:1-5`
- **What it does:** `claude plugin eval` runs each prompt with and without the complete plugin, repeats runs, and reports the response delta. Each case's prompt frontmatter constrains turns/tools; separate graders measure invocation (`tool_used`), lexical output structure (`regex`), and semantic behavior (`llm`). The documented protocol recognizes stochastic invocation and recommends reading the delta/report or setting a threshold rather than treating a single exit code as deterministic truth.
- **Why it's interesting:** The baseline arm distinguishes “the model already does this” from “the skill caused this,” and multiple grader types separate firing, response shape, and actual correctness. This is a practical causal signal for a skill pack, not merely a pass/fail smoke test.
- **Canon applicability:** **Missing, with an analog.** Canon has independent review by a different actor/session and can record external audit metadata, but it does not run a controlled no-skill baseline against a skill-enabled arm or persist a response delta. `canon/skills/reference/canon-review.md:39-55` covers reviewer separation and pinned evidence; `canon/skills/reference/canon-gate.md:240-262` covers external evaluator provenance. Concrete target: an optional research-only `canon audit`/dogfood report schema containing arm labels, run count, evaluator/model/harness, per-expectation outcomes, and delta; record it as `--kind audit` with the evaluator digest. Do not make ablation deltas a default ship gate: model stochasticity and tool/runtime drift make…
- **Effort to lift:** significant.

## 4. Risks & incompatibilities

- **Effect version:** No Effect or other application framework is used. The runner is plain Node.js and the plugin harness requires a particular Claude Code version; reports explicitly note stochastic model behavior, so results are not byte-stable across model/runtime changes (`vendors/addyosmani-agent-skills/evals/README.md:49-55`).
- **Framework:** No UI framework. Plugin cases are Claude Code-specific and cannot be assumed to represent provider-neutral routing.
- **Heavy deps:** The checked-in runner has zero npm dependencies, but Tier 3 and plugin evals spend model tokens and depend on local `claude` binaries. Full traces can be large; the runner uses stdin and bounded buffers (`vendors/addyosmani-agent-skills/scripts/run-evals.js:623-624`).
- **Coupling:** Tier 2 depends on the skill catalog's frontmatter descriptions; Tier 3 depends on the `claude` CLI and fixture layout; plugin cases depend on Claude Code's `experimental.evals` manifest scope and grader semantics.
- **TOS-violation patterns:** None observed. The surface invokes the local `claude` CLI and does not impersonate a provider, forge request fingerprints, or evade rate limits.
- **License footnote:** This audit recommends clean-room reimplementation from the mechanisms and schemas above; no verbatim lift is required.

## 5. Recommended actions

1. **EXTRACT** the strict expectation result contract (integer id binding, one result per expectation, nonblank evidence, derived counters, raw retention on invalid output) → canon `evals/dogfood/trace_check.py` plus a versioned audit-result artifact schema consumed by `canon evidence add --kind audit` — it closes the gap between a trace report and a trustworthy, inspectable scored artifact without letting Canon execute or self-attest the check.
2. **EXTRACT** fixture-backed execution baselines with repository-relative path validation and immutable content digests → canon `canon context-pack` metadata and `crates/canon-cli/src/evidence_attach.rs` — it makes dogfood runs reproducible and binds the exact trace/report inputs to evidence.
3. **INSPIRE** pairwise positive/negative routing cases, owner assertions, collision warnings, and rank-1 ratchets → a future deterministic `canon skill eval`/`canon selftest` catalog check — it tests role/skill dispatch precision separately from product behavior and avoids vacuous negative passes.
4. **INSPIRE** pressure prompts and ordered trace predicates → `evals/dogfood/fixtures/` and the dogfood protocol documents — it tests the canon working loop under authority, time, and sunk-cost pressure while keeping process rules out of product scenario coverage.
5. **INSPIRE** plugin ablation with repeated runs and per-grader deltas → an opt-in research/audit report recorded as `canon evidence add --kind audit` — it measures skill contribution, but should remain advisory because model invocation is stochastic.
6. **SKIP** treating `dialogue` evals as interchangeable with execution evidence → Canon's scenario/evidence gate — Canon should require the artifact type named by the scenario and policy; a conversation-only result must not satisfy a code-behavior scenario.
7. **SKIP** making lexical collision or model delta a shipping blocker by default → `canon gate check` / `canon subject status --to shipped` — Canon's existing evidence, independent review, staleness, and false-success rules are the authoritative release guard, while these vendor metrics are routing or audit signals.

## 6. Citations

- `vendors/addyosmani-agent-skills/evals/README.md:14-38` — three tiers, deterministic routing versus token-cost behavioral runs, fixture and trace semantics.
- `vendors/addyosmani-agent-skills/evals/README.md:40-55` — plugin ablation, grader composition, stochasticity, and report handling.
- `vendors/addyosmani-agent-skills/evals/README.md:57-103` — case schema, positive/negative owner routing, required fixtures, and rank-1/collision metrics.
- `vendors/addyosmani-agent-skills/scripts/run-evals.js:52-59,208-399` — minimum case counts, eval kinds, TF-IDF routing, pairwise owner checks, collision thresholds, and rank-1 ratchet.
- `vendors/addyosmani-agent-skills/scripts/run-evals.js:403-452` — fixture materialization, `.eval/working-tree.patch`, deterministic Git baseline, and workspace initialization.
- `vendors/addyosmani-agent-skills/scripts/run-evals.js:454-537` — grading parser, evidence requirement, summary derivation, and raw invalid-output retention.
- `vendors/addyosmani-agent-skills/scripts/run-evals.js:590-637` — stream-json execution trace, untrusted trace fencing, stdin grading, timeout, and persisted run metadata.
- `vendors/addyosmani-agent-skills/evals/cases/test-driven-development.json:64-118` — concrete invariant, ordering, full-suite, and pressure expectations.
- `vendors/addyosmani-agent-skills/evals/plugin/tdd-fires-on-pasted-bug/prompt.md:1-17` — prompt frontmatter and pasted-bug trigger case.
- `vendors/addyosmani-agent-skills/evals/plugin/tdd-fires-on-pasted-bug/graders/failing-test-first.md:1-5` — LLM grader's observable pass/fail criteria.
- `evals/dogfood/trace_check.py:526-662` — Canon trace analysis output, hard checks, role separation, recovery, naming/deprecation metrics, and scope limits.
- `evals/dogfood/test_trace_check.py:17-38,82-108` — Canon fixture-backed pass/fail tests for roles, pre-brief/question writes, recovery, provider parsing, and malformed calls.
- `canon/skills/reference/canon-context.md:40-58` — immutable context-pack inputs and path safety.
- `canon/skills/reference/canon-gate.md:18-33,240-262` — gate failure classes, false-success, external audit records, and evaluator provenance.
- `canon/skills/reference/canon-review.md:39-55` — independent reviewer/session separation and pinned review state.

[Some lines truncated to 768 chars. Use agent://AuditAEvals:raw:31-31 to read line 31 whole; cut lines run to line 63, each read the same way]
