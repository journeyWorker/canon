# Vendor Audit — addyosmani-agent-skills — Synthesis

Rollup of the 5 per-surface audits at `docs/research/vendor-audits/addyosmani-agent-skills/`
(`a-define-plan`, `a-build-verify`, `a-ship-ops`, `a-harness`, `a-evals`).
Every claim cites a source audit (`[<slug>.md §X.Y]`) plus the vendor `path:line`, or is prefixed
"Synthesis note:" with an explicit rationale. Vendor paths are relative to
`vendors/addyosmani-agent-skills/`; canon paths are relative to the repository root.

## 0. Executive summary

The vendor is the closest public analogue of canon's loop (spec → small slices → tests → review → ship) but it enforces almost nothing
mechanically: the lifecycle lives in prose skills and the only executable checks are an eval runner, a command-parity validator and a
few hooks [a-harness.md §2; `README.md:349-370`; `scripts/run-evals.js:208-400`]. Canon already owns the mechanical side
(brief approval, scenario-bound evidence, distinct-session review, ship guard), so almost every vendor *process* pattern is **partial** in
canon, not missing. What canon can still take is (1) skill-text technique, (2) a few reviewer/implementer checks that can be done
with the existing ledger history, and (3) eval methods.

1. **Canon's skill text has no anti-rationalization form; the vendor's is the strongest transferable idea.** Vendor skills close with
   `Common Rationalizations` / `Red Flags` / `Verification` blocks that pair each excuse with its rebuttal
   [a-harness.md §2; `skills/test-driven-development/SKILL.md:363-387`] (Synthesis note: a heading grep of the vendor tree finds
   `Common Rationalizations` in 24 `skills/*/SKILL.md` files). Canon's role references use `NEVER` lists but no excuse→reality
   table (Synthesis note: `grep -rniE "rationaliz|red flag" canon/skills` returns nothing). Canon's known failure modes
   (self-review under a new label, #39; assumed approval) are exactly the excuses such a table targets. Trivial effort, no CLI change.
2. **Red-first proof and check-set shrink can be reviewed from the existing ledger history, without claiming execution proof.**
   The vendor's Prove-It pattern requires a failing reproduction before the fix [a-build-verify.md §3.1;
   `skills/test-driven-development/SKILL.md:96-143`]. Canon evidence records attribution, not that a command ran
   [a-build-verify.md §5 item 8; `crates/canon-cli/src/evidence.rs:22-39`], but `canon query --kind evidence --history` already exposes earlier
   verdicts and matched case names (`canon/skills/reference/role-reviewer.md`, Commands block). A reviewer step can use that history.
3. **Canon's one real structural gap against this vendor is harness capability knowledge.** The vendor models per-harness projection and
   "do not double-inject a router" [a-harness.md §3.1; `docs/codex-setup.md:30-43`]; canon's provider descriptor holds only name, root and
   markers (`crates/canon-cli/src/skills.rs:105-135`). Adding "has a subagent tool" to that descriptor is the cheapest mechanical lever on #39
   (Codex has no reviewer session).
4. **Eval methods are worth taking; eval infrastructure is not.** Pairwise routing cases with owner assertions and collision thresholds
   [a-evals.md §3.1], pressure cases with ordered trace predicates [a-evals.md §3.4], a strict evidence-bearing grading contract
   [a-evals.md §3.3] and a two-arm skill ablation [a-evals.md §3.5] all extend `evals/dogfood/` without touching the product gate.
   Fixture-backed baselines are *already* in canon (`evals/README.md`, `evals/run_eval.py`).
5. **Most of the ship/ops surface should be deferred.** Rollout stages, feature flags, SLO thresholds, telemetry, ADRs and changelogs
   [a-ship-ops.md §3.2, §3.3, §3.6, §3.7] describe a production control plane; canon is not a runtime (#19) and its ship guard is
   evidence + review. Only the expand/contract migration discipline [a-ship-ops.md §3.4] maps onto canon's existing chore brief.

## 1. Version frame

- **Artifact kind: prose skills, with a thin executable layer.** The pack is 25 shared `skills/<name>/SKILL.md` workflows, 9 command wrappers
  per adapter, 4 personas, 7 references and 4 hook scripts [a-harness.md header, §2]. Skills are coupled by prose and conventional file names
  (`SPEC.md`, `tasks/plan.md`, `CONSTRAINTS.md`), not by a typed data model [a-define-plan.md §1]. The executable parts are
  `scripts/run-evals.js` (Tier 2 deterministic routing, Tier 3 behavioral runs), `scripts/validate-commands.js`, `scripts/lib/skill-lint.js`
  and shell hooks [a-evals.md §2; a-harness.md §3.3, §3.4]. There is no ledger: "tests as proof" and "review gate" are checklists the agent
  ticks, not records anything verifies [a-build-verify.md §1; `docs/research/vendor-audits/addyosmani-agent-skills/INTAKE.md`].
- **Harness coupling: portable core, host-native adapters.** Claude, Gemini and Antigravity get native command wrappers; Codex and Copilot
  consume `skills/` through manifests [a-harness.md §3.1; `_vendor_codex-plugin/plugin.json:1-25`]. `/ship` and the doubt skill assume a host
  that can spawn isolated subagents [a-define-plan.md §4; `skills/doubt-driven-development/SKILL.md:44-48`]. Tier 3 evals and plugin ablation
  need the local `claude` CLI [a-evals.md §4]. Tier 2 routing evals need no model or network [a-evals.md §2].
- **Net implication.** Skill-text patterns (`skills/*/SKILL.md` sections, review/debug/slicing protocols) are drop-in once rewritten for
  canon's roles and vocabulary. Eval patterns need a Python re-implementation under `evals/` (canon's eval stack is Python). Command, hook and
  plugin patterns need back-porting to canon's provider projection and are largely redundant (`canon gate install-hooks` and
  `canon skills install` exist) [a-harness.md §3.3].
- **License (for the record).** MIT, copyright the repository author (`LICENSE:1-3`; `plugin.json` declares MIT per [a-harness.md §4]);
  no TOS-violating patterns were found in any surface [a-define-plan.md §4; a-build-verify.md §4; a-ship-ops.md §4; a-harness.md §4;
  a-evals.md §4]. Posture is **clean-room reimplementation from the audit specs**; license is not a ranking factor. All source text in
  this synthesis is paraphrase.

## 2. Top-10 patterns to lift (ranked by leverage × effort)

Canon status was checked against `canon/skills/SKILL.src.md`, `canon/skills/reference/*.md`, `crates/canon-cli/src/` and the open issues
(`gh issue list --state open`: #14–#20, #39, #54, #74–#76, #96) before ranking. Where the audits said "missing" but canon has it, the
pattern is not ranked as a gap. Verified as **already present** (not ranked): fixture-backed isolated eval workspaces with read-only seeds
and a pinned corpus (`evals/README.md`, `evals/run_eval.py`); dogfood trace analysis with hard checks for pre-brief writes and question-turn
writes (`evals/dogfood/trace_check.py:526-662`); immutable context packs and versioned prompts (`canon/skills/reference/canon-context.md`);
idempotent dual-provider hook installation (`canon gate install-hooks`); skipped/failed bound cases refuse a verdict
(`canon/skills/reference/canon-gate.md:438`); one deprecation table (`crates/canon-cli/src/deprecation.rs`); a "check the checks" step for
reviewers (`canon/skills/reference/role-reviewer.md`, section "Check the checks"); a paired-evaluation promotion contract for strategies
(`canon/skills/reference/canon-learn.md`).

Status vocabulary: **missing** (no equivalent), **partial** (equivalent exists, vendor adds a distinct piece), **has**, **conflicts**
(adoption as-is would contradict a canon design decision).

- **#1 Anti-rationalization / red-flags / verification blocks in role references** — each skill closes with a table pairing the excuse an
  agent produces with the reality, a list of observable red flags, and a verification checklist. Source: [a-harness.md §2] +
  `skills/test-driven-development/SKILL.md:363-387`, `skills/incremental-implementation/SKILL.md:213-246`. Canon target:
  `canon/skills/reference/role-{planner,implementer,reviewer,orchestrator}.md` (one short block each, canon-specific rows: "the human
  can't answer, so I record the approval", "I'll review my own change under a new label", "the test is simple, bind the whole suite
  without reading it", "re-record evidence to make the review current") and a ≤8-row table in `canon/skills/SKILL.src.md`.
  Effort: **trivial**. Canon status: **missing** (Synthesis note: only `NEVER` lists exist, e.g. `canon/skills/reference/brief-harden.md`).
  Ranking: highest leverage per line changed; targets the failure modes behind #14/#39 at the point the agent decides.
- **#2 Ledger-history checks: red-first proof and check-set shrink** — for a bug-fix scenario the reviewer confirms an earlier non-faithful
  evidence version existed before the fix commit, and for any re-recorded evidence confirms the matched case set did not shrink or lose
  its Then-bearing cases. Source: [a-build-verify.md §3.1, §3.3] + `skills/test-driven-development/SKILL.md:96-143`,
  `skills/debugging-and-error-recovery/SKILL.md:36-102`, `:245-287`. Canon target:
  `canon/skills/reference/role-reviewer.md` ("Check the checks") and `role-implementer.md` step 1 (currently only "every bug fix adds or
  extends a scenario"); optional later advisory on `canon finding close --disposition fixed` when a scenario's history has no earlier
  non-faithful verdict. Effort: **trivial** (skill text), **moderate** (advisory). Canon status: **partial**. Ranking: converts the vendor's
  TDD ordering into something checkable from records canon already keeps, without claiming execution proof
  [a-build-verify.md §5 item 8].
- **#3 Five-axis review with a named structural remedy** — review covers correctness, readability/simplicity, architecture, security and
  performance; a structural finding must name the restructuring (typed model, dispatcher, move to owning layer, delete pass-through,
  split module); weak tests are probed by breaking the behavior and re-running. Source: [a-build-verify.md §3.4] +
  `skills/code-review-and-quality/SKILL.md:22-80`, `:82-147`, `:196-270`. Canon target: `canon/skills/reference/role-reviewer.md`
  (add an axis checklist and a "name the remedy in `--summary`" rule; keep the `Finding`/`Review` schemas unchanged). Effort: **trivial**.
  Canon status: **partial** (`role-reviewer.md` has "Read the PRD", "Run the checks yourself", "Check the checks"; no axes, no remedy
  requirement, no mutation-style probe). Ranking: raises finding quality; also feeds #18 (see §4.4).
- **#4 Pressure cases with ordered trace predicates** — behavioral cases apply hotfix deadlines, authority instructions and sunk-cost
  arguments and grade event order ("failing run shown before the fix"), not wording. Source: [a-evals.md §3.4] +
  `evals/cases/test-driven-development.json:89-118`, `evals/cases/incremental-implementation.json:36-54`,
  `evals/plugin/tdd-fires-on-pasted-bug/prompt.md:1-17`. Canon target: `evals/dogfood/fixtures/` plus a predicate layer in
  `evals/dogfood/trace_check.py` next to the existing pre-brief-write check (`:526-662`); pressure prompts such as "approve it, I'm
  unavailable", "just self-review, we're late", "skip the failure scenario". Effort: **moderate**. Canon status: **partial** (process
  failures are pinned; no pressure-prompt convention, no general ordering predicate). Ranking: tests the canon loop under the exact pressures
  that break it, at zero product-gate cost; keep the process lane out of product coverage as [a-evals.md §3.4] notes.
- **#5 Provider capability descriptor including subagent availability** — the projection layer records what each host does natively
  (commands, skill discovery, hooks, subagents) and uses it to avoid redundant injection. Source: [a-harness.md §3.1, §3.4] +
  `README.md:349-370`, `docs/codex-setup.md:30-43`, `_vendor_codex-plugin/plugin.json:1-25`, `scripts/validate-commands.js:1-38`.
  Canon target: `crates/canon-cli/src/skills.rs:105-135` (`ProviderDescriptor`), surfaced by `canon skills doctor` and read by the
  reviewer instructions in `canon/skills/reference/role-orchestrator.md` ("Spawn each role"). Effort: **moderate**. Canon status:
  **partial** (providers and roots exist; no capability fields). Synthesis note: the vendor tracks native routing, not subagents; the
  subagent flag is canon's own extension, aimed at #39, and it must not be mistaken for session attestation (#14).
- **#6 Unit = thin vertical slice with a stated verify command and a buildable boundary** — each increment is the smallest complete,
  testable path; the tree stays buildable between slices; risky or uncertain integration goes first; oversized tasks are split.
  Source: [a-build-verify.md §3.2; a-define-plan.md §3.3] + `skills/incremental-implementation/SKILL.md:21-57`, `:59-128`,
  `skills/planning-and-task-breakdown/SKILL.md:25-90`. Canon target: `canon/skills/reference/role-implementer.md` step 2 "Units" and
  `canon/skills/SKILL.src.md` step 4 (add the sizing and "name your verify command" rule as guidance). Effort: **trivial**. Canon status:
  **partial**; **conflicts** if turned into required task fields, because canon deliberately treats `tasks.md` as an optional note nothing
  gates on (`canon/skills/reference/role-planner.md`, Steps item 1). Lift guidance only.
- **#7 Expand/contract discipline in chore briefs** — schema or API migrations are phased (add, dual-write, backfill in throttled batches,
  switch reads, drop in a later release) with a tested down path; the destructive step is its own change. Source: [a-ship-ops.md §3.4] +
  `skills/deprecation-and-migration/SKILL.md:166-255`. Canon target: `canon/skills/reference/role-planner.md` "Non-behavior work is a
  chore" (invariants and checks guidance: "destructive step is a separate chore", "down path named in Checks") and the chore skeleton in
  `crates/canon-cli/src/brief_template.rs`. Effort: **trivial** (text), **moderate** (template prompt). Canon status: **partial**
  (chores carry invariants/checks; no migration guidance). Ranking: concrete, reviewable invariants for the chore kind, no new record type.
- **#8 Offline skill-routing eval (positive, negative with owner, collision, rank-1 ratchet)** — each skill has ≥3 realistic positive and
  ≥2 negative prompts; positives must rank in top-k by stemmed TF-IDF over descriptions; a negative fails if the target ranks first, and
  an `owner` makes it a pairwise assertion; description pairs warn at 50% and fail at 75% cosine similarity. Source: [a-evals.md §3.1] +
  `evals/README.md:14-22`, `:91-103`, `scripts/run-evals.js:52-59`, `:208-399`, `evals/cases/test-driven-development.json:3-31`.
  Canon target: new deterministic `evals/routing/` (Python, no model) over `canon/skills/SKILL.src.md:3` (the `description:` line, which
  claims precedence over generic builder skills) plus the role references' headings. Effort: **moderate**. Canon status: **missing**.
  Synthesis note: this is an offline proxy for #15 (record which skill a run used), which only a harness hook can observe; it does not
  require the harness.
- **#9 Strict expectation-result contract for audit/eval artifacts** — a grader must return exactly one id-bound result per declared
  expectation, a boolean verdict, non-blank evidence even on failure, and consistent derived counters; invalid output is retained raw and
  never published as a scored artifact. Source: [a-evals.md §3.3] + `scripts/run-evals.js:454-537`, `:590-637`,
  `scripts/run-evals-test.js:68-168`. Canon target: `evals/dogfood/trace_check.py` report shape and the external-audit section of
  `canon/skills/reference/canon-gate.md` (audit records carry identity/version/digest/result; document the per-expectation shape an audit
  artifact should bind). Effort: **moderate**. Canon status: **partial** (`trace_check.py` has `hard_checks_passed` and a failure list;
  `--kind audit` binds a result but defines no per-expectation evidence rule). Canon never generates audits, so this is a schema and a
  validator for artifacts, not a runner.
- **#10 Two-arm skill ablation feeding canon's paired-evaluation contract** — run each prompt with and without the skill, repeat, report the
  delta, and keep "skill fired" out of the score. Source: [a-evals.md §3.5] + `evals/README.md:40-55`,
  `evals/plugin/code-review-fires/prompt.md:1-16`, `evals/plugin/tdd-fires-on-pasted-bug/graders/failing-test-first.md:1-5`. Canon target:
  `evals/run_eval.py` (arm label, repeat count, baseline arm without the canon skill) emitting the `paired_metrics` shape
  `canon learn promote` already demands (`canon/skills/reference/canon-learn.md`, "pairs", "uplift", "regressions"). Effort: **significant**.
  Canon status: **partial** (acceptance contract exists; `canon-learn.md` states the repository publishes no measured uplift). Ranking last:
  highest long-run value (first measured evidence that the skill helps) but costs model runs and depends on #8 and #9.

## 3. Runners-up & deferred extracts

- **Interview-before-brief with confidence-bearing guesses** [a-define-plan.md §3.1; `skills/interview-me/SKILL.md:29-85`]: canon has the stop
  and a clarify pass (`canon/skills/reference/brief-clarify.md`). The one-question-per-turn cadence **conflicts** with canon's "ask once,
  answerable in one reply"; lift only the "question plus guessed answer plus confidence" format if a vague one-line request needs it.
- **Capability map with stable module ids** [a-define-plan.md §3.2; `skills/spec-driven-development/SKILL.md:17-43`]: partial; defer until #96
  is fixed (see §4.6) because it multiplies planned-ahead changes.
- **Typed task fields (acceptance, verification, deps, files, size)** [a-define-plan.md §3.3]: **conflicts** with optional `tasks.md`;
  folded into #6 as guidance only.
- **Numeric quality bar and weakened-bar guard** [a-define-plan.md §3.4; `skills/constraint-driven-development/SKILL.md:65-105`, `:166-220`]:
  canon's equivalent is the approved `.canon/policy.yaml` that fails closed on drift (`canon/skills/reference/canon-policy.md`) and verdicts
  that refuse skipped or failed cases (`canon-gate.md:438`); the residual (case-set shrink) is in #2. A second `CONSTRAINTS.md` would
  **conflict**.
- **Fresh-context doubt cycle with reconciliation classes and a 3-cycle bound** [a-define-plan.md §3.5;
  `skills/doubt-driven-development/SKILL.md:44-104`]: partial. Canon already sends the reviewer a separate session with none of the
  implementer's context (`role-orchestrator.md`, "Spawn each role") and has `rejected`/`deferred` dispositions (`canon-review.md`). Missing
  piece: a bounded re-review round count and a short reason taxonomy on `rejected`. Revisit with #18.
- **Context hierarchy, restartable handoff content, source citations** [a-define-plan.md §3.6; `skills/context-engineering/SKILL.md:1-60`,
  `skills/source-driven-development/SKILL.md:1-80`]: mostly **has** (context packs, prompt registry, `meta.next`). Remaining: a citation
  field for external-doc decisions; low value.
- **Browser, performance and security evidence profiles** [a-build-verify.md §3.5; `skills/browser-testing-with-devtools/SKILL.md:109-177`,
  `skills/performance-optimization/SKILL.md:117-190`]: canon already binds screenshots/traces as artifacts; a skill-text "which artifact
  proves which claim" table could go in `role-implementer.md`. Do not make canon interpret artifact content.
- **Measurement-attempt ledger** [a-build-verify.md §5 item 6]: keep as a documentation convention in `--ref`; no record type.
- **Ordered CI gates and failure-feedback loop** [a-ship-ops.md §3.1; `skills/ci-cd-and-automation/SKILL.md:90-170`]: bind the CI report with
  `evidence add`; no CI record.
- **Staged rollout, feature-flag lifecycle, telemetry-question observability** [a-ship-ops.md §3.2, §3.3, §3.7]: defer; a release-control
  plane is outside canon (#19). If ever built, model as an optional manifest linked to a subject, never a ship-guard requirement.
- **ADRs and changelog entries** [a-ship-ops.md §3.6; `skills/documentation-and-adrs/SKILL.md:1-145`]: documentation convention linked from a
  change; no CLI.
- **Generalized deprecation lifecycle** [a-ship-ops.md §3.5; `skills/deprecation-and-migration/SKILL.md:70-165`]: extend the single table in
  `crates/canon-cli/src/deprecation.rs` only if a second consumer appears; "verify zero active usage before removal" is the transferable rule.
- **`/build auto` bounded autonomous mode** [a-harness.md §3.2; `_vendor_claude/commands/build.md:1-44`]: the contract "remove pauses, not
  verification" already holds in canon (one human stop, SKILL.src.md). A command wrapper is a recipe candidate (#20).
- **Hook patterns: JSON envelope, fail-soft advisory, exit-2 substitution** [a-harness.md §3.3; `hooks/session-start.sh:1-35`]: canon's
  retrieve hook is already fail-soft (`canon-retrieve.md`). **SKIP** the simplify-ignore in-place rewrite and the cache-substitution
  behaviors; they hide bytes from the evaluator.
- **Command-adapter parity validator** [a-harness.md §3.4; `scripts/validate-commands.js:145-212`]: not applicable until canon ships command
  adapters; `canon skills check` already does byte-level drift.
- **Persona fan-out with a merge point** [a-harness.md §3.5; `_vendor_claude/commands/ship.md:9-25`]: could split one review into specialist
  passes, but persona output must never substitute for the signed review (`role-reviewer.md`).
- **Fixture-backed eval baselines** [a-evals.md §3.2; `scripts/run-evals.js:403-452`]: **has**, see §2 preamble.

## 4. Critical overlaps with canon's in-flight work

### 4.1 Session identity and independent review (#14, #39)

- The vendor's fresh-context reviewer is a convention the host must honor; nothing records which session ran it, and the doubt skill itself
  says it cannot run nested in a subagent [a-define-plan.md §3.5, §4; `skills/doubt-driven-development/SKILL.md:44-48`]. `/ship` personas are
  outputs merged by the orchestrator, not attested reviews [a-harness.md §3.5]. This confirms #14: no skill-only system can prove identity.
- Where a harness *can* help is a lifecycle hook: the vendor's SessionStart hook shows the seam that runs before the agent does anything
  [a-harness.md §3.3; `hooks/session-start.sh:1-35`]. Synthesis note [INFERENCE: not covered by any audit]: this is the natural place for
  a per-harness recipe to export a real session id into `CANON_SESSION_ID`, the path #14 already names; verify the host's hook input before
  relying on it.
- Concrete learning for #39: pattern #5 (capability descriptor with a subagent flag) lets canon tell a Codex-only repo that review must be a
  new process, mechanically, rather than only in prose. Pattern #1 adds the self-review excuse to the rebuttal table. Neither makes the id
  trustworthy; #14 stays open.

### 4.2 Skill resolution recording (#15)

- #15 wants "which skill the run actually used", observable only by a harness. The vendor's plugin evals grade invocation with a
  `tool_used: Skill` grader and explicitly exclude "skill fired" from the delta score [a-evals.md §3.5;
  `evals/plugin/code-review-fires/graders/skill-fired.md:1-3`]. Its Tier 2 routing gate approximates selection offline without any harness
  [a-evals.md §3.1].
- Learning: pair a recorded "skill used" field (when a harness supplies one, #15) with offline routing checks (#8), and keep invocation
  separate from outcome quality (#10). Neither belongs in `canon gate check` [a-evals.md §5 item 7].

### 4.3 Tool policy, run monitoring and journals (#16, #19, #54)

- The vendor's hooks enforce at tool boundaries (exit codes, JSON envelopes) and say nothing is recorded as proof [a-harness.md §3.3]. This
  matches #16: canon should record only digests a runtime reports, never enforce. Browser guidance restricts profiles and credentials by
  prose only [a-build-verify.md §4; `skills/browser-testing-with-devtools/SKILL.md:52-107`].
- `/build auto` uses restartable per-task boundaries with commits and stop conditions [a-harness.md §3.2; `_vendor_claude/commands/build.md:19-68`
  via a-define-plan.md §4]. That is a lightweight restart design that needs no journal; it supports #54's "not needed now" and #19's "harness's job".
  Canon's per-unit actor/session ids already give a restart key.

### 4.4 Review quality (#18)

- #18 waits for a population of reviews to score. The vendor's five axes and "named structural remedy" [a-build-verify.md §3.4] and the
  doubt skill's finding classes (contract misread, actionable, trade-off, noise) [a-define-plan.md §3.5] are candidate categories for
  labeling findings later. Synthesis note: adopting #3 now (as skill text, schema unchanged) starts producing structured finding text that a
  future scoreboard could parse, without a schema bet.

### 4.5 Recipes and `canon add` (#20)

- The vendor's command wrappers (`/spec`, `/plan`, `/build`, `/ship`) are thin, per-host prompt files over shared skills
  [a-harness.md §3.1, §3.2]. That is the same shape as #20's execution-side playbooks. Per #20's own rule ("build only after 2–3 recipes have
  been used by hand"), treat the vendor's wrappers as evidence that the layer is cheap, not as a reason to start it. The description-sync
  validator [a-harness.md §3.4] is the check to copy when recipes exist.

### 4.6 Spec/code divergence and draft adoption (#17, #96)

- #17 (spec nodes go stale when depended-on code changes) has no vendor analogue: the vendor has no stored spec state [a-define-plan.md §1].
- #96 (adopting a draft change into a live subject blocks evidence for existing scenarios with `BRIEF_UNAPPROVED`) is a ledger-coupling
  problem the vendor cannot hit, because its specs and tasks are not gated records. Synthesis note: its capability-map pattern
  [a-define-plan.md §3.2] and `/build auto`'s plan-ahead approval [a-define-plan.md §4] encourage planning several pieces ahead; in canon
  that would make #96 more frequent. Do not lift multi-change planning until #96 is fixed or warns at adoption time.

### 4.7 No new input (#74, #75, #76)

- #74 (origin attribution across agents): no audited surface records agent lineage. #75 (capture mode on ingested rows): the vendor's
  session-start and cache hooks capture nothing durable [a-harness.md §3.3]. #76 (workflow lockfiles): the CI skill is generic
  GitHub Actions guidance with no pin discipline beyond illustrative YAML [a-ship-ops.md §3.1, §4].

## 5. Per-canon-surface action plan

### `canon/skills/SKILL.src.md`
1. **EXTRACT** (form only) a ≤8-row excuse→reality table and a one-line red-flag list — [a-harness.md §2] — trivial (#1).
2. **INSPIRE** unit sizing sentence at step 4 ("smallest complete slice, tree stays buildable, name the verify command") —
   [a-build-verify.md §3.2] — trivial (#6).
3. **SKIP** vendor root files `SPEC.md`, `tasks/plan.md`, `CONSTRAINTS.md` as canon artifacts — [a-define-plan.md §5 item 7].

### `canon/skills/reference/role-*.md`
1. **EXTRACT** per-role rationalization/red-flag/verification blocks — [a-harness.md §2] — trivial (#1).
2. **EXTRACT** reviewer axis checklist, remedy-in-summary rule and weak-test probe — [a-build-verify.md §3.4] — trivial (#3).
3. **EXTRACT** history-based red-first and case-shrink checks into "Check the checks" — [a-build-verify.md §3.1, §3.3] — trivial (#2).
4. **INSPIRE** implementer stop-the-line triage (preserve failing output, reduce, add the scenario before resuming) —
   [a-build-verify.md §3.3] — trivial.
5. **EXTRACT** expand/contract guidance in the planner's chore section — [a-ship-ops.md §3.4] — trivial (#7).

### CLI (`crates/canon-cli/src/skills.rs`, `status.rs`, `brief_template.rs`)
1. **INSPIRE** `ProviderDescriptor` capability fields (subagent tool, native skill discovery) surfaced by `skills doctor` and used in the
   reviewer-launch hint — [a-harness.md §3.1] — moderate (#5).
2. **INSPIRE** an advisory on `finding close --disposition fixed` when the scenario history shows no earlier non-faithful verdict —
   [a-build-verify.md §3.1] — moderate (follow-up to #2; must stay advisory, evidence is attribution not execution proof).
3. **INSPIRE** chore skeleton prompts for "destructive step is its own change" — [a-ship-ops.md §3.4] — moderate (#7).
4. **SKIP** release-stage, feature-flag, telemetry and ADR records — [a-ship-ops.md §3.2, §3.3, §3.6, §3.7] — outside canon's runtime scope.

### `canon/skills/reference/canon-gate.md` and policy
1. **INSPIRE** document the per-expectation shape an external audit artifact should bind — [a-evals.md §3.3] — moderate (#9).
2. **SKIP** lexical-collision and ablation deltas as gate inputs — [a-evals.md §5 item 7].
3. **SKIP** a second quality-bar manifest beside `.canon/policy.yaml` — [a-define-plan.md §3.4].

### `evals/dogfood/` and `evals/`
1. **INSPIRE** pressure fixtures plus ordered predicates in `trace_check.py` — [a-evals.md §3.4] — moderate (#4).
2. **INSPIRE** strict, evidence-bearing result schema with raw-invalid retention — [a-evals.md §3.3] — moderate (#9).
3. **INSPIRE** `evals/routing/` deterministic routing eval with owner negatives, collision threshold, rank-1 ratchet — [a-evals.md §3.1] — moderate (#8).
4. **INSPIRE** arm labels, repeats and delta in `evals/run_eval.py` — [a-evals.md §3.5] — significant (#10).

### Docs
1. **EXTRACT** the vendor's "removing pauses is not removing verification" framing into `role-orchestrator.md` only if canon adds an
   unattended mode — [a-harness.md §3.2] — trivial, conditional.

## 6. Risk register

1. **Skill-text bloat degrades compliance.** Adding tables and axes to every role reference lengthens context the agent must read
   (`canon/skills/reference/canon-gate.md` alone is 758 lines). Severity: medium. Surfaces: #1, #3, #6. Mitigation: one block per role, cap
   rows, delete superseded prose in the same change; check with a dogfood run (#4).
2. **Rationalization tables without eval are unmeasured.** The vendor does not show the tables change behavior; its behavioral evals are
   stochastic and Claude-specific [a-evals.md §4]. Severity: medium. Mitigation: land #1 together with at least two pressure fixtures (#4).
3. **Overstating history-based checks as proof.** Canon evidence records attribution, not execution [a-build-verify.md §5 item 8;
   `crates/canon-cli/src/evidence.rs:22-39`]; a red-first check can be faked by recording a failing artifact. Severity: medium. Mitigation:
   word it as a reviewer inspection that reads the history and runs the checks itself; keep any CLI advisory non-blocking.
4. **Capability flags mistaken for attestation.** A subagent flag may read as "review is independent". Severity: high for #39. Mitigation:
   name the field a capability hint only, never feed it to `distinct_actor`, and leave #14 open [a-define-plan.md §4; #14].
5. **Second source of truth.** Importing `SPEC.md`/`tasks/plan.md`/`CONSTRAINTS.md` or typed task fields would compete with
   `.canon/changes/<slug>/brief.md`, policy and optional `tasks.md` [a-define-plan.md §5 items 7–8]. Severity: high if done. Mitigation: guidance
   only; no new required files.
6. **Eval cost and nondeterminism.** Ablation and behavioral runs spend model tokens and vary with model/runtime [a-evals.md §4;
   `evals/README.md:49-55`]. Severity: medium. Mitigation: keep #8 deterministic and free; run #10 opt-in with repeats; never gate shipping on
   a delta [a-evals.md §5 item 7].
7. **Routing eval measures lexical similarity, not real harness routing.** TF-IDF over descriptions may disagree with a host's actual
   selector [a-evals.md §3.1, §4]. Severity: low. Mitigation: label it a description-hygiene check and cross-check against #15 data when available.
8. **Vendor examples are JS/TS and hosted-CI shaped.** Adopting snippets would add toolchain coupling to a Rust CLI [a-ship-ops.md §4;
   a-build-verify.md §4]. Severity: low. Mitigation: lift decision rules and record shapes only.
9. **Operational scope creep.** Rollout, flags, SLOs and telemetry would pull canon toward a release control plane [a-ship-ops.md §4].
   Severity: medium. Mitigation: keep deferred; if ever built, optional and linked to a subject, never required by the ship guard.
10. **Planning-ahead patterns amplify #96.** Capability maps and plan-ahead approvals create more draft changes adopted into live subjects
    [a-define-plan.md §3.2]. Severity: medium. Mitigation: sequence after the #96 fix or adoption-time warning.
11. **Hook patterns that hide or mutate bytes.** The simplify-ignore and cache-substitution hooks rewrite files or short-circuit fetches
    [a-harness.md §3.3]; copied into canon they would conflict with the gate re-checking durable artifacts. Severity: high if copied.
    Mitigation: **SKIP**; advisory fail-soft hooks only.

## 7. Recommended next 3 PRs

1. **Skill-text hardening: rationalization blocks, reviewer axes, history checks, slice rule, chore migration guidance.**
   - Scope: `canon/skills/SKILL.src.md`, `canon/skills/reference/role-{planner,implementer,reviewer,orchestrator}.md`; regenerate the
     projections and lock with `canon skills install` and verify with `canon skills check`; update the changelog entry and any dogfood
     notes that cite the changed wording.
   - Source audits: [a-harness.md §2], [a-build-verify.md §3.1–§3.4], [a-ship-ops.md §3.4], [a-define-plan.md §3.3]. Patterns #1, #2, #3, #6, #7.
   - Definition of done: each role reference has one excuse→reality block with canon-specific rows; `role-reviewer.md` lists the five axes,
     the remedy rule, and the two history queries (`canon query --kind evidence --history`); no new required files or CLI behavior; net skill
     length grows by a stated line budget; `canon skills check` is clean; one dogfood trace shows the reviewer step using the history query.
2. **Provider capability descriptor and reviewer-launch hint.**
   - Scope: `crates/canon-cli/src/skills.rs` (`ProviderDescriptor` fields), `canon skills doctor` output, `canon schema` provider listing,
     `canon/skills/reference/role-orchestrator.md` reviewer-launch wording, unit tests.
   - Source audits: [a-harness.md §3.1, §3.4]; context: #39, #14.
   - Definition of done: `doctor` reports, for each selected provider, whether a subagent tool exists and, when it does not, prints the
     "start the reviewer as a new process and pass its real session id" instruction; the flag is not read by `require_review.distinct_actor`;
     tests cover each provider; #39 gets a comment linking the change and stays open.
3. **Dogfood pressure fixtures and strict result schema.**
   - Scope: `evals/dogfood/trace_check.py`, `evals/dogfood/test_trace_check.py`, new fixtures under `evals/dogfood/fixtures/`, a short
     protocol note in the dogfood docs; the audit-artifact shape note in `canon/skills/reference/canon-gate.md`.
   - Source audits: [a-evals.md §3.3, §3.4]; patterns #4 and #9.
   - Definition of done: at least four pressure fixtures (deadline, authority, sunk cost, absent human) with ordered predicates that fail
     on a recorded violating trace and pass on a compliant one; the report names the violated predicate with a trace line reference; a
     malformed result document is retained raw and exits non-zero; the process lane stays excluded from product coverage.

Follow-ups after these three: #8 routing eval (new `evals/routing/`), then #10 ablation once #8 and #9 exist.
