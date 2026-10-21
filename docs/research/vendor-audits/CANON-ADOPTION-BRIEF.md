# Canon adoption brief for the vendor-audit patterns (2026-10-21)

Phase 7 (map) of the vendor-audit research. Inputs: `docs/research/vendor-audits/00-cross-vendor-patterns.md` (15 merged patterns X1-X15 with phase-6 revalidation notes) and the three per-vendor `00-synthesis.md` files. Every "Canon current-state" line below was re-read against the working tree at commit `900db9fb` (Canon 0.24.1); where the phase-6 note was incomplete or wrong it says so. Items marked `[INFERENCE]` were not exercised and need a behavior test before they are relied on.

Backlog at the time of writing (`gh issue list --state open`, `gh pr list`): open issues #14, #15, #16, #17, #18, #19, #20, #39, #54, #74, #75, #76 (all labelled `watch`, some also `recipe`) and #96 (unlabelled bug: adopting a draft change blocks evidence). **There are no open pull requests.** Closed issues that overlap this work and constrain it: #45/#47 (brief check, refinement passes), #48 (`tasks.md` becomes an agent work note), #49 (chore kind), #51 (trace-checked protocol evals), #72/#87 (re-verification after a fixed blocker), #73 (reviewer checks the checks), #85 (one name per concept).

Canon rules that every sketch below respects: one name per concept (0.24); every refusal carries a concrete `hint`; records stay backward compatible (new fields optional, absent from the wire when unset); no carry-forward of earlier records into new ones; no shims or aliases.

Effort scale: trivial (text edit, minutes), low (text plus one fixture), moderate (Rust change with scenarios and evidence, or a script with tests), significant (new subsystem or cross-harness work).

## 1. Summary table

| ID | Pattern | Revalidated verdict | Canon state | Change kind | Effort | Suggested bundle |
| --- | --- | --- | --- | --- | --- | --- |
| X1 | Fail-first proof for fixes | REFINED | partial (history exists, no guidance) | skill text + eval | trivial text, moderate eval | 0 + A |
| X2 | Reviewer proof standards | REFINED | partial (one axis shipped by #73) | skill text | trivial | A |
| X3 | Anti-rationalization / red-flag blocks; advice-vs-mechanism map | REFINED | missing (role blocks); partial (mechanism split is implicit) | skill text | low | A |
| X4 | Harness capability matrix + spawn contract | CONFIRMED | partial (provider projection only) | skill text now; CLI/record later | trivial text; moderate later | A (text), C (attestation) |
| X5 | `skills install`/`check` prune and report dropped files | CONFIRMED | partial (legacy Codex files only) | CLI | moderate | B |
| X6 | Skill-text authoring rules + reachability check | REFINED | partial (knowledge checker exists, not wired to CI) | repo process + script | trivial text, low-moderate script | 0 |
| X7 | Dogfood eval methods (pressure predicates, strict grader contract, ablation) | REFINED | partial (trace checker and paired scoreboard exist) | eval | moderate; ablation significant | 0 (ablation: C) |
| X8 | Reasoned finding dispositions | CONFIRMED | missing | CLI + record | moderate | B |
| X9 | Brief durability (current vs desired, no stale paths) | REFINED | partial | skill text + dogfood fixture | trivial-low | A |
| X10 | Implementation units as flow-step slices with a verify command | REFINED | partial | skill text | trivial | A |
| X11 | Expand/contract guidance for migration chores | REFINED | partial (chore kind exists, no migration guidance) | skill text + template hint | trivial text; low template | A (text), B (template) |
| X12 | Long-run record and continuation packet | REFINED | partial (Handoff and dispatch `Run` exist) | skill text now; CLI/record later | trivial now; significant later | A (text), C (CLI) |
| X13 | Repo-specific verifier recipe | REFINED | missing | recipe | significant | C (under #20) |
| X14 | Scope bar and durable rejection records | REFINED | partial (`watch` label is the bar) | repo process | trivial | 0 (optional) |
| X15 | Intent-to-first-command routing table | CONFIRMED | partial (route-by-task exists) | skill text | trivial | A |

Honest leverage ranking, highest first: **X8** (only item that adds data canon cannot reconstruct later, and #18 depends on it), **X5** (a silent failure that grows as canon deletes text), **X1+X2** (cheapest answer to the failure classes canon has already seen: #73, #39), **X7** (without it no skill-text change in this list can be shown to work), **X9/X10/X11** (small prose edits), **X4** (prose part only). Low leverage for canon as built: **X3** (unmeasured as a technique), **X15**, **X12** until #54 has demand, **X14** (no declined proposals exist yet), **X13** (large, execution-side, blocked on #20's own precondition).

## 2. Per-pattern analysis

### X1. Fail-first proof for fixes

**Canon current-state**
- The implementer requires a scenario per bug fix but no red run first: `canon/skills/reference/role-implementer.md:40-41` ("Every bug fix adds or extends a scenario"); after-fix handling is only about re-recording the suite (`role-implementer.md:93-100`).
- Evidence is attribution, not execution proof; `--ref` may name a command that never ran (`crates/canon-cli/src/evidence.rs:18-39`).
- Earlier versions are retained: the ledger is latest-only with an archive (`canon/skills/SKILL.src.md:63-66`), and `canon query --kind evidence --history` reads ledger plus archive (`crates/canon-cli/src/query.rs:394-428`).
- Not in the phase-6 note: a report with a failing case already yields a `divergent` verdict automatically on `--all-scenarios` (`canon/skills/reference/canon-gate.md:482-484`), and `faithful` over a failed case is refused (`canon-gate.md:437-438`). So a red run bound as evidence before the fix and superseded by the green run leaves a red-then-green pair in `--history` with no new field. `[INFERENCE]` that this works end to end on a `building` subject without a transient gate or status side effect.
- Conflict: `divergent` is documented as "the external limit" and is what a reviewer may accept as a tracked gap (`role-implementer.md:78-81`; `role-reviewer.md:78-90`). Using it for "was red" overloads one name (0.24 rule) unless the text says plainly that a red run is superseded, never accepted.

**Gap or overlap:** (b) partial, extend. Mechanism exists; guidance and an eval check do not.

**Preconditions:** none for the text. The ordering check in the eval needs X7's ordered predicates. The `[INFERENCE]` above needs a behavior test first (record a failing report on a `building` subject, run `gate check` and `status`, then record the passing report and read `--history`).

**Conflicting / complementary work:** #87 (closed, shipped 0.24.1) already scopes post-fix re-verification by `scenario_ids`; X1 is its upstream complement. #14/#39: because evidence is self-attested, fail-first stays advisory; the reviewer can only inspect history, not trust it. #18 (scoreboard) could later count fix findings with a prior non-faithful record; not now.

**Adoption sketch**
1. `canon/skills/reference/role-implementer.md`, step 1 or 3: for a bug fix, bind the failing run first (`canon evidence add --scenario-id <id> --report <fmt>:<path> …` on the failing report), then fix, then record the passing run. State that the red record is superseded by the green one and is never an accepted gap. If no executable repro is possible, say what the closest check is in `--ref`.
2. `canon/skills/reference/role-reviewer.md` "Check the checks" (`:71-76`): for a fix, read `canon query --kind evidence --history --scenario-id <id>` and record a finding when no earlier non-faithful version exists and no closest repro is named. Advisory wording only; no gate.
3. Add a dogfood trace case (X7): seeded bug-fix prompt; predicate "an `evidence add` with a failing report precedes the first code write that lands in git".
4. Test: the behavior test above, then a dogfood run on omp. Do not add a `failed_before` field in this release (self-attested, so it adds a field without adding trust).

### X2. Reviewer proof standards

**Canon current-state**
- Already shipped: independent re-run of claimed checks (`role-reviewer.md:63-69`) and "read each bound test against its scenario's Then steps", with a finding for tests that pin unrequired behavior or skip a Then step (`role-reviewer.md:71-76`, from closed #73). This covers one of the five axes (spec-fidelity of the test).
- Review pins scenario digest and evidence verdict plus check identity (`canon/skills/reference/canon-review.md:234-271`; `crates/canon-gate/src/freshness.rs`).
- Canon states that attestations are claims it does not validate (`canon-review.md:7-13`). No defect-class vocabulary, falsifiability probe, blast-radius fact, or separate diff-versus-spec read exists.

**Gap or overlap:** (b) partial, extend. Do not import a five-axis mandate; extend the existing "Check the checks" section.

**Preconditions:** none.

**Conflicting / complementary work:** #18 (review-quality scoreboard) wants categories for findings; defect-class names written here are candidate categories, but do not add a finding `class` field (no population to score yet). #39 matters: a self-review under a new label makes every reviewer standard moot. #73 is the closed predecessor; reconcile wording with it rather than adding a second section.

**Adoption sketch**
1. `role-reviewer.md:71-76`: replace the two-sentence section with named ways a passing test proves nothing (tautological expected value, asserts implementation detail, never reaches the real path or side effect, asserts only the absence of an error, mocks the unit under test). Keep it under ~12 lines.
2. Optional probe: "when a check looks weak, name the one-line change to the code that should make it fail; run it if cheap". Wording says optional; no record change.
3. Add a blast-radius line to the review summary template in `canon-review.md` (what else the change touches that the checks do not exercise), as prose in `--summary`/`finding add --summary`, not a new field.
4. Separate pass: say that the PRD read (`role-reviewer.md:52-61`) and the diff read are two passes; a finding names which one it came from.
5. Test: skill-text change verified by a dogfood pressure case (X7) that seeds a tautological test and expects a finding before sign-off.

### X3. Anti-rationalization / red-flag / verification blocks; advice-versus-mechanism map

**Canon current-state**
- Role references are procedure plus `NEVER` rules, no excuse-to-reality tables or red-flag lists (`role-implementer.md:33-100`, `role-reviewer.md:52-90`).
- Advice versus mechanism is already explicit in places: gate-checked requirements (`canon-gate.md:18-33`), advisory prose rules (`crates/canon-cli/src/brief_check.rs:10-25`, advisories `prose.*` at `:91-114`). It is not collected in one map.
- #39 is the concrete observed failure (self-review under a new label).

**Gap or overlap:** (a) gap for role blocks, (b) partial for the map.

**Preconditions:** X14's scope bar as a discipline: write a row only for a failure canon has actually observed (dogfood eval findings, #39, #72, #77, #78). No row from vendor prose alone.

**Conflicting / complementary work:** #39/#14 (rows about self-review). Placement matters for cost: `SKILL.src.md` is loaded in every session, role files only for their role.

**Adoption sketch**
1. Add a short "Excuses that fail" block (3-5 rows, each: the thought, the reality, the command or refusal that shows it) to each of the four `role-*.md`, sourced from observed failures. Implementer: "the test is slow, record `faithful`" (refused at `canon-gate.md:437-438`). Reviewer: "I will relabel the session" (#39). Planner: "the human is unavailable, assume the approval" (`role-planner.md:64-73`). Orchestrator: "edit the policy so the gate passes" (`role-orchestrator.md:58-71`).
2. Put the advice-versus-mechanism map (≤8 rows: rule, where it is enforced, or "advisory") in `canon/skills/reference/canon-gate.md`, not in `SKILL.src.md`. This departs from the phase-6 note (which places it in `SKILL.src.md`) because the shared core is paid on every session and a map is reference material.
3. One sentence in `role-orchestrator.md`: a correction repeated across runs belongs in a scenario, policy, or gate, not more prose.
4. Test: skill-text change; verified only through a dogfood pressure case (X7). Be explicit that the technique is unmeasured; if two dogfood rounds show no predicate change, remove the blocks (X6 rule: delete no-op prose).

### X4. Harness capability matrix and orchestrator spawn contract

**Canon current-state**
- `ProviderDescriptor` has name, projection root and detection markers only (`crates/canon-cli/src/skills.rs:93-104`); selection is flag, config, lock, then detection (`skills.rs` `select_providers`).
- The orchestrator says to spawn each role and start the reviewer as a separate process with a real session id (`role-orchestrator.md:31-56`). There is no consolidated spawn brief (goal, scope, ownership, verification), no worktree-ownership rule, and no per-provider fallback.
- `--session-id` is a label (#14). Codex had no subagent tool when #39 was filed (Codex CLI 0.160.1); that fact may be stale.

**Gap or overlap:** (b) partial. Prose contract and table now; attestation later.

**Preconditions:** the attestation half needs a harness that exposes its real session id (#14, #39, #20). Nothing in canon can create that.

**Conflicting / complementary work:** #14, #39, #15, #16 (all say canon only records what a runtime reports). #96: its batch-planning scenario makes the spawn contract's scope-ownership line relevant (a planner adopting drafts into a live subject freezes the reviewers' evidence). #20 recipes are where per-harness hooks live.

**Adoption sketch**
1. New section in `role-orchestrator.md` (or one new `reference/` file linked from it): a capability table with columns provider, in-harness subagent tool, separate-process launch command for the reviewer, ask-user tool, hooks, and an explicit "skipped" column. Each row dated with the harness version it was checked against. The four rows are exactly the providers in `skills.rs:93-135`.
2. Spawn contract as prose: each spawn names the goal, the scope it owns (paths or scenarios), what it must not touch, the command that proves it done, and its own `--actor-id`/`--session-id`. Fits the existing "Give each a unit" line (`role-orchestrator.md:48-49`).
3. Do not add capability fields to `ProviderDescriptor` yet; text tables do not need Rust and drift is bounded by the date column.
4. Later (bundle C, watch): a dispatch or session field carrying a harness-reported session id, only when a harness hands one over.
5. Test: skill-text change; the table is checked by a dogfood run on a non-omp harness (the #39 reproduction on Codex) that shows the reviewer launched as a separate process.

### X5. `skills install`/`check` prune and report dropped files

**Canon current-state**
- `canonical_bundle` collects only the current `SKILL.src.md`, `reference/**`, `scripts/**` (`crates/canon-cli/src/skills.rs:535-549`). The install lock already records a per-file hash map (`CanonicalManifest`, `skills.rs:771-794`).
- `install_canonical` removes only proven legacy Codex files, via `legacy_codex_removals` restricted to the old `.codex/skills` projection (`skills.rs:1054-1068`, call at `:1391-1450`).
- `check` iterates only current bundle files; a dropped file can make `manifest_ok` false but is not named (`skills.rs:1482-1526`; output at `crates/canon-cli/src/main.rs:2290-2315`: statuses, remnants, outdated; JSON `statuses[]`, `remnants`, `manifestOk`).
- Consequence (confirmed by reading, not run): a reference file deleted from the source remains in every projection until a human deletes it, and canon names no orphan.

**Gap or overlap:** (b) partial, extend. The lock's file map makes this a generalization of existing code, not a new mechanism.

**Preconditions:** must land before any release that deletes or renames a `reference/*.md` file. Bundle A must therefore only edit existing files, and X6's prune rule (delete no-op text) must not delete a file before this ships.

**Conflicting / complementary work:** none open. Related shipped context: 0.17 D5 moved Codex to `.agents` (the reason `legacy_codex_removals` exists). One-name rule: reuse the word `remnant` for a file canon wrote and no longer ships; do not introduce a second term (`orphan`) in user-visible text. `[INFERENCE]` the existing `remnants` field can carry both kinds with a state string; check against `main.rs:2299-2312` before adding a field.

**Adoption sketch**
1. `skills.rs`: replace `legacy_codex_removals` with a removal set computed as `previous.files` keys not in the new manifest, selected provider's projection paths only, and only where the on-disk hash equals the recorded hash (a modified file is kept and reported, never deleted). The existing re-check-hash-at-removal (`remove_file_if_hash`) carries over.
2. `check`/`doctor`: report each such file as `remnant` with the fix command (`canon skills install`), exit via the existing drift code. JSON: additive state value only.
3. Unselected-provider files stay out of scope for deletion (`doctor` already reports stray projections without deleting, `SKILL.src.md:140-142`).
4. Scenarios under `.canon/specs/kind=feature/area=skills/`: dropped and unmodified (removed), dropped and modified (kept, reported), dropped for an unselected provider (untouched), idempotent re-run. Each refusal path names the hint.
5. Test: Rust behavior tests plus the canon-on-canon scenarios; no dogfood eval needed (deterministic).

### X6. Skill-text authoring rules with a reachability check; routing eval

**Canon current-state**
- One umbrella skill with lazy references (`canon/skills/README.md:1-18`); only `SKILL.src.md`, `reference/**`, `scripts/**` are projected (`skills.rs:535-549`), so `canon/skills/README.md` is a maintainer-facing file that costs agents nothing.
- Routing is by hand in `canon/skills/SKILL.src.md:156-169` and in `role-planner.md:49-56` (the five brief passes are linked only from there).
- A checker already exists: `scripts/check-knowledge.py` (module docstring at `:1-10`) verifies mapped paths, projection sources and Markdown link resolution against `canon/knowledge-index.json`. Reading that index: it lists 16 of the 17 topic references and omits `canon/skills/reference/canon-policy.md`, the four `role-*.md`, and the five `brief-*.md`. Neither it nor `scripts/test-check-knowledge.py` is run by any workflow (`.github/workflows/*.yml` run only `scripts/check-release-workflow-safety.py`).
- `canon skills check` validates projections, not links inside the bundle (`skills.rs:1482-1526`).

**Gap or overlap:** (b) partial. Extend `check-knowledge.py`; do not write a second checker or a Rust command. Addy's TF-IDF/rank-1 routing eval does not apply to a one-skill catalog (phase-6 verdict stands).

**Preconditions:** none. A CI hook is the part that makes it real.

**Conflicting / complementary work:** #15 (skill resolution can only be observed by a harness); a static reachability check is orthogonal and cheap. X5 (do not let X6's pruning delete files first).

**Adoption sketch**
1. `canon/skills/README.md`: add 6-8 authoring rules from Matt's `writing-for-agents`: one trigger per pointer, a checkable completion criterion for each role step, single source of truth (a rule lives in one file, others link), delete prose that changes no behavior, no paths/line numbers that will rot.
2. `scripts/check-knowledge.py`: add a "reachable" check: every file under `canon/skills/reference/` is linked from `SKILL.src.md` or a `role-*.md`, and every relative link inside `canon/skills/**` resolves. Fail with the file name and the linking-file suggestion (hint-shaped message).
3. Add the missing reference entries to `canon/knowledge-index.json` or scope the check to links only; do not do both.
4. Wire `python3 scripts/check-knowledge.py` and its test into `.github/workflows/test.yml`.
5. Test: add fixture cases to `scripts/test-check-knowledge.py` (orphan file, broken target); the script's own tests are the behavior test.

### X7. Dogfood eval methods

**Canon current-state**
- `evals/dogfood/trace_check.py` parses omp and Codex logs without executing them and enforces pre-brief writes, question-turn writes, and role separation (`evals/dogfood/trace_check.py:526-638`, tests `evals/dogfood/test_trace_check.py:17-118`). Fixtures live in `evals/dogfood/fixtures/`.
- A paired baseline/candidate scoreboard exists for the corpus eval (`evals/run_eval.py:619-645`) and strict paired promotion rules for strategies (`canon/skills/reference/canon-learn.md:25-45`).
- The audit record is documented as canon records, never generates (`canon-gate.md:240-271`). The per-expectation result schema of an external evaluator is not specified.
- Dogfood evals are run by hand (`docs/research/2026-10-19-dogfood-eval-12.md`) and analyzed by the trace checker; `test_trace_check.py` is not in CI.
- Missing: named ordered predicates with line-referenced failures, pressure fixtures (deadline, authority, sunk cost), strict per-expectation result validation with raw-invalid retention, skill/no-skill arms.

**Gap or overlap:** (b) partial, extend.

**Preconditions:** none. Everything else in bundle A needs this to be measurable, so it goes first.

**Conflicting / complementary work:** #51 (closed; the original trace-checked evals issue). #18 and #15 (a scoring population and skill resolution arrive through eval infrastructure). Open question for the owner: ablation is research, never a ship gate (phase-6 verdict).

**Adoption sketch**
1. `evals/dogfood/trace_check.py`: add a small predicate table (name, matcher over the existing `session["calls"]`/`session["writes"]` events, ordering) and report failures with `file:line` via the existing `location` fields (`:540`). First predicates: `red-before-code-write` (X1), `verify-command-named-per-unit` (X10), `reviewer-session-distinct` (exists in spirit at the role-separation check), `no-assumed-approval` (exists).
2. New fixtures in `evals/dogfood/fixtures/` as `*.jsonl` per predicate, pass and fail, with matching `test_trace_check.py` cases (the existing pattern: one fixture, one test).
3. Strict grader contract for any external evaluator that feeds `--kind audit`: document in `canon-gate.md` near `:240-271` the per-expectation shape (integer id, `passed`, non-blank evidence, derived counters) and keep raw invalid output. Docs only; no canon command validates it (canon records, never runs).
4. Add `python3 -m unittest` for `evals/dogfood` to `.github/workflows/test.yml`.
5. Ablation (skill vs no skill): do not build. File a `watch` issue; revisit only if a skill-text change cannot be judged by its predicates.

### X8. Reasoned finding dispositions

**Canon current-state**
- `Finding` holds `severity`, private `disposition`, `reviewer`, `summary`, private `resolution_sha`, `introduced_by`, `file_ref`, `scenario_ids`; no reason (`crates/canon-model/src/records.rs:1747-1803`). `rejected()`/`deferred()` clear `resolution_sha` and retain nothing else (`records.rs:1965-1976`).
- `finding close` takes disposition, resolution sha, actor, role, session (`crates/canon-cli/src/finding.rs:705-728`, `run_close` at `:759`); `DISPOSITION_VALUES` at `:335`; list JSON fields at `:1056`.
- The reviewer docs define the dispositions without asking for a rationale (`canon/skills/reference/canon-review.md:125-132`).
- Not in the phase-6 note: `Finding::is_disposition_transition_of` requires every field except `disposition`/`resolution_sha` to be byte-equal (`records.rs:2009-2042`), and `canon_gate::promote` re-derives it (`finding.rs` module doc around `:192-195`). A `reason` that arrives with the close is a changed field, so the predicate must be extended or the close will be refused at promote. `FindingWire` (`records.rs:1826-1850`) has no `deny_unknown_fields` (no match in the file), so older readers ignore a new key.
- Naming precedent: `--reason` is already used by `canon divergence accept` and `canon subject status`. Use `--reason`, not `--rationale`/`--why`.

**Gap or overlap:** (a) gap.

**Preconditions:** none. Schema regeneration per the `state-model` skill (JOIN_SPINE/schema outputs) comes with any `canon-model` change.

**Conflicting / complementary work:** #18 (review-quality scoreboard needs dispositions with causes to separate "declined as wrong" from "deferred for cost"). #14/#39: reasons are self-attested like the rest of the finding. Phase-6's "downgraded" is not a canon disposition and severity does not move in a transition (`records.rs:2031`); keep it out.

**Adoption sketch**
1. `records.rs`: add `reason: Option<String>` to `Finding` and `FindingWire`, `skip_serializing_if = "Option::is_none"`, `present_value` like its siblings. Coherence: allowed only with `rejected`/`deferred` (mirror the `resolution_sha` pairing at `:2053-2067`); old records without it still read. Extend `is_disposition_transition_of` to let `reason` move with `disposition`. Constructors `rejected(reason)`/`deferred(reason)` or a `with_reason`, never a public mutable field.
2. `finding.rs`: `--reason` on `finding close` and `finding add`, required (non-blank) for `rejected` and `deferred` on the write path, refused with a hint for `fixed`/`open`. New codes follow the `FINDING_*` family (`finding.rs:570`, `:573`): `FINDING_REASON_REQUIRED`, `FINDING_REASON_UNEXPECTED`, each with `.hint(...)` and `.field("--reason")`. Add `reason` to `LIST_FIELDS` and the list JSON.
3. `canon/skills/reference/canon-review.md:125-132` and `role-reviewer.md:17`: command lines show `--reason`; define what a good reason says (one line, names the evidence or the follow-up).
4. Report panel and `canon schema` output pick the field up through their generated paths; regenerate schema and the version conformance cases (`conformance/regenerate.sh`).
5. Test: scenarios `finding.close.NN` and `finding.add.NN` in `.canon/specs/kind=feature/area=finding/` (required reason, refusal hints, old-record read, transition accepted at promote, reopen clears the reason), with evidence and an independent review, as 0.24.1 did for #87.

### X9. Brief durability: current versus desired behavior, no stale paths

**Canon current-state**
- PRD sections: Intent, Desired outcome, User flow, Step detail, Acceptance, Questions, Out of scope (`crates/canon-cli/src/brief_template.rs:55-63`, hints at `:132-138`). Intent's hint is "what problem this solves, for whom, and why now"; nothing asks for current behavior or interfaces.
- `brief check` rules (`brief_check.rs:91-114`) have no current-state or path rule; paths are tolerated (`role-planner.md:45-48` allows a `Source: <path>` line).
- Briefs are read by implementer and reviewer sessions that did not write them, and the reviewer already reads the brief itself (`role-reviewer.md:55-57`), so durability matters.

**Gap or overlap:** (b) partial.

**Preconditions:** none.

**Conflicting / complementary work:** #45/#47 (shipped brief checks and refinement passes): put the guidance in an existing pass rather than a sixth one. Phase 6 correctly warns against a hard path checker; also `RULES` is a closed list of 22 that waivers name, so a new rule id is a visible contract change.

**Adoption sketch**
1. `canon/skills/reference/role-planner.md` step 2 (`:39-48`): state "Intent says what the product does today, in behavior words; Desired outcome says what is true after".
2. Same file: describe interfaces by behavior (inputs, outputs, errors), and keep implementation paths and line numbers out of normative sections; provenance lines are the exception.
3. `canon/skills/reference/brief-harden.md`: add one checklist line (current behavior stated; no rot-prone paths in normative sections).
4. Template hint for Intent (`brief_template.rs:132` and the Korean hint at `:145`) is optional; skip it unless the text-only change fails the eval, because it must change in both languages together.
5. Test: dogfood brief fixture (X7): a prompt where the relevant code moves between planning and building; predicate "brief Intent names current behavior; no path outside a `Source:` line in normative sections".

### X10. Implementation units cut along flow steps, each with a verify command

**Canon current-state**
- `role-implementer.md:48-50` says only "give each unit its own actor id and session id"; `SKILL.src.md:53` says "split the work". Scenarios are tied to flow steps (`role-implementer.md:35-38`). `tasks.md` is an optional note nothing gates on (`role-planner.md:33-35`; closed #48).
- Brief check already enforces flow-step coverage by scenario and a failure scenario per step (`brief_check.rs:10-25`).

**Gap or overlap:** (b) partial. A task ledger with blocking edges would duplicate flow/scenario coverage; skip it.

**Preconditions:** none. X7's `verify-command-named-per-unit` predicate is how it is measured.

**Conflicting / complementary work:** #48 (closed) made `tasks.md` an agent note; X10 must not revive it as a record. #96 (batch-adoption problem) is a reason to keep units inside one change.

**Adoption sketch**
1. `role-implementer.md` step 2 (units): a unit is one flow step's complete path (sees, does, becomes) that can be verified on its own; name the command that proves it; size it to one session.
2. Add the same sentence to `SKILL.src.md:53`, kept to one line.
3. No new records, no commit-per-slice or feature-flag rule (Addy's vertical-slice details assume a shared trunk; canon's ledger already attributes per unit).
4. Test: dogfood predicate (X7) on a three-step brief: each unit's session names a verify command.

### X11. Expand/contract guidance for migration chores

**Canon current-state**
- Chores are limited to refactor, dependency, build, CI, infra changes that alter nothing observable (`role-planner.md:75-87`); sections intent, category, invariants, checks, out of scope (`brief_template.rs:104-110`); invariants forbid behavior, public interface, data or output changes (`brief_template.rs:140`). `brief_check` enforces shape only (`brief_check.rs:10-25`, chore rules `:91-114`).
- A migration that changes data shape or a public interface is therefore not a chore; it is a feature change.
- No migration-phase guidance anywhere.

**Gap or overlap:** (b) partial. Mind (d): Addy's expand/dual-write/backfill/cutover assumes data and interface changes that the chore invariants prohibit.

**Preconditions:** none for text. The template hint is Rust and bilingual.

**Conflicting / complementary work:** #49 (closed, chore kind); #77/#78 (chore reopening and divergent evidence); open #96 (a multi-phase migration will adopt several changes into a live subject, which hits #96 directly).

**Adoption sketch**
1. `role-planner.md` after `:87`: a paragraph, "A migration is cut as expand, migrate, contract, one change each". Expand and contract of a behavior-preserving internal structure are chores; any phase that changes data, a public interface or output is a feature change with a PRD brief.
2. For such a feature, the brief's Acceptance names the down path, the backfill's safety condition, and puts the destructive step in its own later change.
3. Template (bundle B, optional): extend the `invariants` and `checks` hints at `brief_template.rs:140-141` (English) and `:143-144` (Korean) with one clause each, e.g. "a migration phase lists its down path here". No new section; do not change `CHORE_SECTIONS`.
4. Do not wait for #96: write the guidance so each phase is its own subject-adopting change only after the previous shipped.
5. Test: skill-text; dogfood case with a rename-column style request expecting three changes and the first-phase brief naming a down path. Template change: the existing brief template conformance cases, re-blessed.

### X12. Long-run record and continuation packet

**Canon current-state**
- `Handoff` is a typed queue record, `pending → in-progress → done|abandoned`, with chain, parent, sequence, roles and a domain-template body (`crates/canon-model/src/handoff.rs:295-345`; vocabulary in `canon/skills/reference/canon-vocab.md:85-112` for the body templates). It is not a continuation packet.
- `canon dispatch` persists a replayable `Run` manifest with guidance snapshot, task and parent edges, terminal close (`crates/canon-cli/src/dispatch.rs:1-35`; `canon/skills/reference/canon-retrieve.md`). No decision journal, no pause/resume checkpoint.
- #54 states a journal is "not needed now" until review spans separate runner processes.

**Gap or overlap:** (b) partial. Mostly low leverage today.

**Preconditions:** the CLI half waits for #54's trigger.

**Conflicting / complementary work:** #54 (watch), #19 (adaptive monitoring), #14/#39 (sessions). Phase-6's `dispatch pause|resume` would add a second continuation concept next to `Handoff` and `meta.next`; one-name-per-concept argues against it before there is demand.

**Adoption sketch**
1. Now (text): in `role-orchestrator.md` (resume paragraph near `:23-29`) add pointer-not-copy and redaction rules for any note one session leaves for another: reference artifacts and record ids instead of pasting, name the next command, name what was verified versus claimed, redact secrets.
2. When a Handoff body is used for continuation, say in `canon-vocab.md` that `summary` carries the next command and verified claims. No schema change.
3. Later (bundle C, not scheduled): `canon dispatch` checkpoint fields. Comment on #54 with the shape instead of building it.
4. Test: text only; one dogfood resume case (kill the session mid-change, restart with `canon status` and the note) is optional and belongs to X7's later work.

### X13. Repo-specific verifier recipe

**Canon current-state**
- The workflow contract exists: scenario, test-run evidence, independent review, gate transition (`SKILL.src.md:50-57`); batch evidence, artifact binding, e2e re-run awareness (`role-implementer.md:54-81`, `role-reviewer.md:100-106`).
- `skills install` projects only the shared skill (`SKILL.src.md:133-154`). Nothing generates launch/doctor/drive/evidence/cleanup instructions or a user-facing feature map per repo.
- #20 requires 2-3 recipes used by hand first. The repo currently has one hand-synced execution recipe (`orca.yaml`, updated by chore commits #89-#95); that is one data point, not three.

**Gap or overlap:** (a) gap, deliberately deferred.

**Preconditions:** #20's own gate (2-3 hand-used recipes), plus X7 so a recipe's proof-of-one-journey can be checked.

**Conflicting / complementary work:** #20 (direct), #14/#15/#16 (per-harness hooks come with recipes), #19.

**Adoption sketch**
1. Do not build it. Comment on #20: pstack's Launch/Doctor/Drive/Evidence/Cleanup sections plus a feature map keyed to canon scenario ids are the candidate recipe shape.
2. When a second and third hand-used recipe exist, write the generator as an execution-side recipe file (copied into the consuming repo, user-owned), never a canon record kind or core command.
3. Feature-map entries reference scenario ids; drift is detectable by comparing against `canon query --kind scenario`.
4. Test: n/a now; a recipe is accepted only when one mapped journey runs end to end and its evidence passes `canon artifacts verify --full`.

### X14. Scope bar and durable rejection records

**Canon current-state**
- The `watch` label is already the admission bar: "do not build until a real need or a runtime partner appears" (label list via `gh label list`); the 12 open `watch` issues each state their trigger (#14-#20, #39, #54, #74-#76).
- The `wontfix` and `invalid` labels exist but no closed issue carries either (63 issues total at the time of writing). So no proposal has been declined yet, and the phase-6 recommendation to "add a closed-issue label" is already satisfied.
- No `CONTRIBUTING.md`, no issue templates (`.github/` holds workflows only). Release history with issue links is in `README.md`.
- Canon's own decisions are findings on adopted changes (`canon-review.md:125-132`), not intake decisions.

**Gap or overlap:** (c) mostly has. Smallest useful change only.

**Preconditions:** none.

**Conflicting / complementary work:** the backlog-management preference is already "keep as issues, build reasonable ones first"; X14 is that preference written down. X3 uses the same bar for its rows.

**Adoption sketch**
1. Skip building anything until the first proposal is declined.
2. At that point close it with `wontfix` and a one-line reason comment, and add "search closed `wontfix` issues first" (`gh issue list --state closed --label wontfix`) as a short paragraph in `README.md`.
3. For consuming repos, no canon feature: a proposal that is declined is a closed issue in that repo, outside canon's records.
4. Test: none.

### X15. Intent-to-first-command routing table

**Canon current-state**
- `SKILL.src.md:156-169` ("Route by task") already routes by topic and role to reference files; `SKILL.src.md:50-57` is the loop; `canon status` and `meta.next` give state-derived next commands (`crates/canon-cli/src/next.rs:1-30`); `canon schema` describes parser shape (`crates/canon-cli/src/schema.rs:1-27`). The description line already states the trigger (every build/fix request in a repo with `canon.yaml`, `SKILL.src.md:3`).
- Missing: a request-intent row ("fix a bug", "add a feature", "pure refactor", "review someone's change", "resume") naming the first command and role.

**Gap or overlap:** (b) partial, mostly overlapping with Route by task. A second table in the shared core would split one concept across two places; extend the existing list instead.

**Preconditions:** none.

**Conflicting / complementary work:** #20 (playbooks); #96 (the "plan more work on a live subject" row should warn about adoption before it points at `change new`). Do not alter `status` or `meta.next`.

**Adoption sketch**
1. `SKILL.src.md:156-169`: add 5-6 intent bullets at the head of "Route by task": bug fix (planner or existing change, then `scenario new --case failure`), new feature, behavior-preserving refactor (chore), review request (reviewer role), resume (`canon status`), upgrade (`canon migrate --dry-run`). Each bullet: first command and the reference file.
2. Skip `canon help workflow`; `canon schema` and `status` already answer it for a CLI user, and a third discovery surface adds a name to maintain.
3. Test: skill-text; dogfood prompt set (one per intent) with predicate "first canon command matches the row". This is the only item here where the effect may be undetectable in practice; if the eval shows no first-command errors on the baseline, drop the rows.

## 3. Proposed bundles

Ordering: **0** first (it provides the measurement), then **A**, then **B**. **C** is not scheduled; it is a list of issue comments.

### Bundle 0: measure first (eval and repo process; no canon version bump)

- **Items:** X7 (pressure predicates and strict audit-result docs, new fixtures), X6 (authoring rules in `canon/skills/README.md`; reachability checker in `scripts/check-knowledge.py`; CI wiring for it and for `evals/dogfood` tests), X14 (only the one-line decision convention when the first declined proposal arrives; otherwise skip), the X1 behavior test and trace fixture.
- **Risk:** low. No shipped binary or skill-bundle change; the one new risk is CI time and a newly red `check-knowledge.py` on first wiring (it has never run in CI, so expect fixes to the index).
- **Dogfood evidence it worked:** `python3 -m unittest` for `evals/dogfood` and the knowledge checker run in `.github/workflows/test.yml`; a seeded orphan reference file fails the checker with a hint; a replayed old eval log flags a missing red-before-fix with `file:line`.
- Canon rule check: no records, no CLI names touched.

### Bundle A: proof, standards and unit text (skill text only; patch-sized release)

- **Items:** X1 (implementer/reviewer text), X2, X3 (role blocks and the map in `canon-gate.md`), X4 (capability table and spawn contract prose in `role-orchestrator.md`), X9, X10, X11 (planner text only), X12 (pointer-not-copy and redaction text), X15 (rows folded into "Route by task").
- **Scope limits:** no new files under `canon/skills/reference/` unless the capability table cannot fit `role-orchestrator.md`; no deleted files (X5 first); every new row or rule must cite an observed failure (X14 bar); shared-core growth in `SKILL.src.md` capped at about 10 lines.
- **Mechanics:** edit sources, run `canon skills install` so the committed projections under `.claude/`, `.agents/`, `.omp/`, `.pi/` and the lock follow (the 0.24.0 and 0.24.1 commits did the same), `canon skills check` clean, README release section with the dogfood result.
- **Risk:** medium, and it is behavioral rather than structural: more prose can lower compliance with existing rules, and some of it is unmeasured technique (X3, X15). Mitigation: keep each addition in the role file loaded only by that role; remove blocks that the dogfood eval shows change nothing.
- **How the dogfood eval shows it worked:** one fixture repo, one seeded scenario set, run on omp (and the X4 case on Codex), two runs for the 0.24.1 skill and two for the candidate skill, same prompts. Predicates evaluated by `trace_check.py` (bundle 0): red evidence precedes the first landed code write; a tautological seeded test draws a reviewer finding before sign-off; each unit names a verify command; the brief's Intent states current behavior; the reviewer is a distinct launched session on Codex. A predicate counts only when it flips from fail to pass between arms in both candidate runs; with n=2 this is a smoke signal, not a statistic, and the README acceptance text must say so.

### Bundle B: reasons and dropped files (CLI and records; minor release)

- **Items:** X8 (`reason` on `Finding`, `--reason` on `finding close`/`finding add`, refusals with hints, list/report/schema output), X5 (prune and report dropped managed files in `skills install`/`check`/`doctor`), X11's template hint in both languages (optional).
- **Risk:** medium-low. Both changes are additive: `reason` is optional on the wire (older readers ignore an unknown key: no `deny_unknown_fields` in `crates/canon-model/src/records.rs`), and `skills check` JSON gains a state value only. The two real risks are `is_disposition_transition_of` and the `promote` re-derivation (a close with a reason must still promote), and deleting files from a user's projection (kept only when the hash matches the lock).
- **Order inside the release:** X5 can land first and alone as a patch if X8's schema regeneration and conformance re-bless are slower.
- **How a dogfood eval shows it worked:** a reviewer run in which one finding is closed `rejected` without `--reason` hits `FINDING_REASON_REQUIRED` with a runnable hint and recovers in one call (`trace_check.py` recovery depth, already counted); a repeat of that run on a repo upgraded across a release that dropped a reference file ends with `canon skills check` clean and no stale file on disk. Deterministic behavior tests cover the rest.

### Bundle C: not scheduled (issue comments, no code)

- **Items:** X13 (comment on #20 with the recipe shape; one hand-used recipe exists), X12 CLI half (comment on #54 with `dispatch pause|resume` as the shape, trigger unchanged), X4 attestation (comment on #14/#39: the session-id field waits for a harness that reports one), X7 ablation (new `watch` issue).
- **Why unscheduled:** each is blocked on a precondition canon cannot satisfy alone (a harness-reported session id; 2-3 hand-used recipes; a multi-process review demand). Adding them as `watch` comments records the shape and keeps the backlog honest.
- **Not proposed:** a routing eval with TF-IDF and rank-1 floors (one skill, nothing to collide with), Addy's lexical-collision threshold, `failed_before` metadata on evidence, `canon help workflow`, a `tasks.md` dependency ledger, a second finding disposition ("downgraded").

### Where #96 interacts

#96 (adopting a draft change into a live subject blocks evidence for all existing scenarios) is unrelated to any one pattern but touches X4 (spawn contract: planners and reviewers run in parallel), X10 (units stay in one change), X11 (multi-phase migrations adopt several changes) and X15 (the "plan more work" row). Fix #96, or write the planner text around it, before Bundle A's X11 and X15 text is relied on; Bundle A should not describe a workflow #96 currently breaks.
