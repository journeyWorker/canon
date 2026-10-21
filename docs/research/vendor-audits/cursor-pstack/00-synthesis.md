# Vendor Audit — cursor-pstack — Synthesis

Rollup of the 5 per-surface audits at `docs/research/vendor-audits/cursor-pstack/`
(`p-principles`, `p-mode`, `p-verification`, `p-reasoning`, `p-harness`), taken at
vendor commit `190d287`, pstack plugin version 0.15.29
(`docs/research/vendor-audits/cursor-pstack/README.md:3`,
`vendors/cursor-plugins/pstack/_vendor_cursor-plugin/plugin.json:4`).
Every claim cites a source audit (`[<slug>.md §X.Y]`) plus a vendor `path:line`, or is
prefixed "Synthesis note:" with the canon file I checked. In this file `P/` means
`vendors/cursor-plugins/pstack/`; §2 spells paths out in full.

Canon state was re-checked at HEAD (canon 0.24.1, `crates/canon-cli`, `canon/skills/`,
`gh issue list --state open`) before ranking; where an audit's "missing" claim did not
survive that check, §4.0 says so and the top-10 status reflects HEAD, not the audit.

## 0. Executive summary

1. **pstack is a prose skill pack with almost no enforcement; canon is ahead on gating and
   behind on "what good proof looks like".** The principles, playbooks and reasoning skills
   are Markdown the host loads, with no ledger, gate or exit code behind them
   [p-principles.md §3.5; P/skills/principle-sequence-verifiable-units/SKILL.md:1-24]. The
   transferable value is the proof-quality vocabulary in the verification skills: a check
   that fails before the fix and passes after, a safety fact proven by running code, a
   verifier that has been seen to fail, evidence from the real user path
   [p-verification.md §3.4, §3.5; P/skills/tdd/SKILL.md:9-20,
   P/skills/blast-radius/SKILL.md:15-50]. Synthesis note: canon's reviewer step "Check the
   checks" reads tests against Then steps (`canon/skills/reference/role-reviewer.md:71-76`)
   but never asks whether a test fails without the change. Entries #1-#3 close that with
   skill text.
2. **Four audits converge on one long-run pattern: a checkable finish predicate, one
   decision row per iteration, a discard branch, and a resume note**
   [p-mode.md §3.5; p-harness.md §3.3; p-verification.md §5.6; p-reasoning.md §3.2;
   P/docs/guide/07-overnight.md:18-56]. Canon already owns the predicate (approved brief +
   scenarios) and the durable state (ledger). The residual gap is operational, and it
   belongs with issues #20 (recipes) and #54 (run journal), not in the core CLI (#8).
3. **The Claude Code mapping table with explicit "Skipped in Claude Code" rows is the
   cheapest answer to the open session-identity issues (#14, #39).** It makes lossy
   harness translation visible instead of silent
   [p-harness.md §3.1; P/docs/harness/claude-code.md:3-60]. Canon has no per-harness
   capability table (#4).
4. **Three of the audits' "missing" claims were already present at HEAD** (model/provider
   lineage on runs and reviews, stale-evidence by surface binding, brief section roles).
   See §4.0. Build from this synthesis's status column, not from the audits' prose.
5. **A small record-shape gap shows up across the verification and review audits:
   `finding close --disposition rejected|deferred` carries no reason.** pstack's lead
   judgment records a rationale for every dismissed finding
   [p-verification.md §3.3; P/skills/interrogate/SKILL.md:44-51]. Canon cannot score
   reviewers (#18) or audit a rejection without one (#5).

## 1. Version frame

- **Artifact type.** Prose skills first. Every principle is a `SKILL.md` with
  `disable-model-invocation: true` and a trigger-style description
  [p-principles.md §3.1; P/skills/principle-prove-it-works/SKILL.md:1-5]. Playbooks, reasoning
  skills and the verification-skill generators are likewise Markdown procedures
  [p-mode.md §1; p-verification.md §1; p-reasoning.md §1]. Executable checks exist but are
  few and sit outside the audited prose: Synthesis note: `P/skills/poteto-mode/scripts/`
  holds a plan-document linter (`check-plan.mjs`, 205 lines; required sub-block headings,
  perf items, long-dash/curly-quote/mid-sentence-colon lint, `check-plan.mjs:8-28,78-80`),
  an orchestration store CLI (`orch/store.ts`, 1,677 lines), `worktree-audit.sh`, `watch-pr/`,
  and `P/skills/show-me-your-work/scripts/log.sh`. None of them verifies an agent's work
  claim against a record; `check-plan.mjs` lints the structure of a plan file, comparable to
  `canon brief check` but with no approval or ledger behind it. Verification "enforcement"
  is the agent following instructions [p-principles.md §4, "Advice/enforcement ambiguity"].
- **Harness coupling.** Strong to Cursor: Task subagents, AskQuestion, Custom Modes, `/loop`,
  cloud agents, model slugs and effort tokens [p-mode.md §2, §4]. Claude Code is supported
  through translation tables, not native behavior
  [p-harness.md §3.1; P/docs/harness/claude-code.md:3-60]. The Benny automation needs Slack
  actions, a tracker and a UI control adapter
  [p-harness.md §3.2, §4]. Codex, omp and pi are not addressed. Net: nothing is drop-in. The
  portable unit is the record shape, lifecycle and checklist, not the orchestration code
  [p-verification.md §4, "Coupling"].
- **License.** MIT, copyright holder named in `P/LICENSE:1-3`; `plugin.json` declares
  `"license": "MIT"` (`P/_vendor_cursor-plugin/plugin.json:11`) [p-principles.md §4]. Canon's
  default is clean-room reimplementation from the audit specs, so license is not a ranking
  factor. It becomes load-bearing only for a verbatim-lift PR; none is proposed here.
  No TOS-violation patterns were found in any of the five surfaces
  [p-principles.md §4; p-mode.md §4; p-verification.md §4; p-reasoning.md §4; p-harness.md §4].
- **Net implication.** All ten lifts below are clean-room: skill-text changes, record shapes,
  or recipe designs. Items marked "recipe" follow issue #20's rule (build after 2-3 recipes
  have been used by hand).

## 2. Top-10 patterns to lift (ranked by leverage × effort)

Status is canon's state at HEAD: `missing`, `partial`, `has`, or `conflicts`. Each status
carries a "Synthesis note:" naming the canon file checked.

- **#1 Fail-first regression proof** — for a fix or regression claim, the check must fail
  before the change for the intended reason and pass after; when that is impractical, say
  why and name the closest executable repro. Source: [p-verification.md §3.5, §5.5;
  p-harness.md §3.4] · `vendors/cursor-plugins/pstack/skills/tdd/SKILL.md:9-20,36-42`
  (failing-before run, passing-after run, or the stated reason it was not shown);
  `vendors/cursor-plugins/pstack/automations/benny/skills/reproduce-and-fix-issues/SKILL.md:48-78`
  (same symptom reproduced twice, baseline-versus-patched proof before a draft PR).
  **Canon status: missing.** Synthesis note: `canon/skills/reference/role-implementer.md:39-42`
  says "Every bug fix adds or extends a scenario" and nothing about a pre-fix failing run;
  `ChangeKind` is `Feature | Chore` only (`crates/canon-cli/src/brief_template.rs:77-83`), so
  no fix marker exists for a gate to key on; `EvidenceVerdict` is
  `faithful | not_applicable | divergent` (`crates/canon-model/src/records.rs:1254-1258`) with
  no before-state. **Canon target:** step 1, text in `canon/skills/reference/role-implementer.md`
  (Steps 3-4: bind the pre-fix failing run as an extra `--artifact <scenario-id>=<path>`, not
  `--report`, naming it in `--ref`) and `role-reviewer.md` "Check the checks" (re-run the
  scenario's test on the parent of the fix commit when possible). Step 2, only if a dogfood run
  shows agents skipping step 1: optional typed `failed_before` / `failure_reason` fields on
  `evidence add`, optional by evidence kind so no fake tests are forced
  [p-verification.md §5.8]. **Effort: trivial** (text), moderate (typed fields). **Why #1:**
  it closes the one hole a reviewer cannot see by reading a test, a test that passes with or
  without the fix, for the cost of two paragraphs.

- **#2 Proof standards and a falsifiability probe in the reviewer's "check the checks"** —
  evidence must come from the real user path (not internal setters or test-only endpoints),
  capture the action and the resulting state, verify side effects, check what a "dry-run"
  mode really skips by observation, keep artifacts through cleanup, confirm the timed or
  asserted work actually happened, and show a check failing on a broken variant. Source:
  [p-verification.md §3.1, §3.2, §3.5] ·
  `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:30-31,40`
  (proof standards; "a generated skill that was never executed is a draft");
  `vendors/cursor-plugins/pstack/skills/create-formal-verification-skill/SKILL.md:24,37`
  ("able to fail": break a variant and require the checker to catch it);
  `vendors/cursor-plugins/pstack/skills/benchmark-checklist/SKILL.md:27` ("did it even
  happen?": lazy code and timeouts yield numbers for work that never ran).
  **Canon status: partial.** Synthesis note: the reviewer already runs the checks and reads each
  test against the Then steps (`role-reviewer.md:63-76`); the gate refuses fabrication markers
  and honors external `refuted` audits (`canon/skills/reference/canon-gate.md:31,33`). There
  is no rubric for proof quality or any sensitivity step. **Canon target:**
  `canon/skills/reference/role-reviewer.md` ("Check the checks": four questions: real path,
  state after action, side effects, ran-at-all; plus an optional "break one thing and watch
  the test fail" probe for risky scenarios, recorded as an artifact) and `role-implementer.md`
  Step 4 (screenshots/artifacts must survive teardown). **Effort: trivial.** **Why #2:** same
  seam as #1, text only, and it targets the failure seen in issue #39, where the evidence was
  `curl -I` with no test report bound.

- **#3 Blast-radius safety fact as a reviewer report** — name the one fact the change is safe
  because of, prove it by running real code, rate confidence on a ladder
  (assertion → cited line → failure-path walk → ran script/test → reproduced in the running
  app), list risks and cleared risks, and mark the fact `proven` or `unproven`. Source:
  [p-verification.md §3.4, §5.4] ·
  `vendors/cursor-plugins/pstack/skills/blast-radius/SKILL.md:15-50`. **Canon status: partial.**
  Synthesis note: `review add --artifact` stores digest-bound reviewer artifacts
  (`role-reviewer.md:26-28`, `canon/skills/reference/canon-review.md:150`) and findings carry
  `--file-ref` and scenario joins (`canon-review.md:70-73`); no template or ladder asks for a
  non-local risk claim. **Canon target:** a "Blast radius" section in `role-reviewer.md` with a
  report template in `canon-review.md`; an `unproven` safety fact becomes a `finding add
  --severity should-fix|blocker`, the report rides on `review add --artifact`. No gate change:
  the gate checks that bytes exist and are current, not that the prose is true
  [p-verification.md §3.4]. Typed `safety_facts[]` stays deferred. **Effort: trivial** (text),
  moderate (typed). **Why #3:** non-local risk is what a scenario review misses, and the
  artifact path already exists.

- **#4 Per-harness capability and "skipped" matrix** — explicit tables mapping each tool,
  subagent field, isolation mode and path between harnesses, with a rule that a field with no
  row is dropped and reported, never passed on silently. Source: [p-harness.md §3.1, §5.1;
  p-mode.md §5.8] · `vendors/cursor-plugins/pstack/docs/harness/claude-code.md:3-4,15-27,58-60`
  (`readonly: true` → read-only agent plus "check `git status` afterward"
  (`claude-code.md:21`); `resume` → spawn a new agent (`claude-code.md:27`); "Skipped in Claude
  Code: <step>. <reason>." and "Do not stop, ask, or retry" (`claude-code.md:58-60`)).
  **Canon status: partial.** Synthesis note: projection targets and selection are explicit
  (`canon/skills/SKILL.src.md:133-154`) and `role-orchestrator.md:42-56` handles a subagent tool
  that cannot set the child's environment and the `codex exec` reviewer, but no single
  table says, per harness, how to spawn a role, how to start a separate reviewer process, where
  the real session id comes from, and what to do when a harness cannot (issues #14, #39).
  **Canon target:** new `canon/skills/reference/canon-harnesses.md` (one row per provider in
  `skills.providers`: claude, codex, omp, pi), routed from `SKILL.src.md` "Route by task"
  (`SKILL.src.md:156-169`) and `role-orchestrator.md` "Spawn each role"; later, `canon skills
  doctor` prints unsupported rows. **Effort: moderate** (every cell must be verified against
  the harness or a dogfood trace; unknown cells say "unknown"). **Why #4:** it is the only
  lift that reduces the #39 self-review gap without claiming identity proof.

- **#5 Reasoned finding dispositions** — every dismissed or downgraded finding carries a
  one-line reason and the seat(s) that raised it; nothing is auto-applied. Source:
  [p-verification.md §3.3, §5.3] · `vendors/cursor-plugins/pstack/skills/interrogate/SKILL.md:42-51`
  (`Act on` / `Consider` / `Noted` / `Dismissed`, "with the model(s) that raised it and a
  one-line rationale"). **Canon status: partial.** Synthesis note: dispositions are
  `open|fixed|rejected|deferred` (`canon-review.md:128-132`), but `finding close` takes only
  `--disposition`, `--resolution-sha`, actor/session ids (`canon-review.md:74-75`;
  `crates/canon-cli/src/main.rs:2059`), so a `rejected` close records no reason; the review
  record already stores the reviewer's runtime (`reviewer_provider/model/harness`,
  `records.rs:842-881`). **Canon target:** optional `reason` on the Finding record and
  `--reason` on `canon finding close`, required for `rejected` and `deferred`
  (`crates/canon-cli/src/finding.rs`, `crates/canon-model/src/records.rs`,
  `canon-review.md`, a `finding.close.NN` scenario); older records stay valid because the
  field is additive, as `RunLineage` is (`records.rs:652-664`). **Effort: moderate.** **Why
  #5:** a rejected blocker with no reason is unauditable, and a scoreboard (#18) has nothing to
  score without it.

- **#6 Orchestrator spawn contract** — fresh agent per new unit with a consolidated brief
  (goal, writable paths, forbidden paths, exclusive worktree or branch), the parent verifies
  the real artifacts instead of the worker's summary, and a verifier is separate by session
  and preferably by model family. Source: [p-mode.md §3.3, §5.4; p-harness.md §3.5] ·
  `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:99-101` (own every subagent's
  work, fresh by default); `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/orchestrate.md:21-40`
  (one writer per file; brief fields GOAL, SCOPE with exclusive worktree);
  `vendors/cursor-plugins/pstack/skills/show-me-your-work/SKILL.md:31` (a helper's work thrown
  out because its screenshots were blank). **Canon status: partial.** Synthesis note:
  `role-orchestrator.md:31-56` already requires one agent per role, `CANON_ROLE` or `--as`, and
  a reviewer in its own process with its own session id; it gives no spawn-brief shape, no
  worktree-ownership rule, and cannot compare model families, because the `Evidence` record
  has no model field (`rg 'pub model' crates/canon-model/src/records.rs` finds only
  `RunLineage.model`; reviews carry `reviewer_model`). **Canon target:** `role-orchestrator.md` "Spawn each role" (brief
  fields, owned worktree or disjoint file set, reviewer checks out the `--pin` in a separate
  worktree and leaves it unmodified, `git status` clean afterward, as in
  `claude-code.md:21`) and `role-reviewer.md` "Separate session". Model-family diversity is an
  advisory sentence, never a gate, since it is as much a label as `--session-id` (#14).
  **Effort: trivial.** **Why #6:** cheap, and it gives the orchestrator a mechanical
  post-check where identity cannot be proven.

- **#7 Advice-versus-mechanism map and routing of recurring corrections into structure** —
  each rule states whether a tool enforces it, a review checks it, or it is advice only; a
  correction repeated twice becomes a lint, flag, runtime check or script before it becomes
  more prose. Source: [p-principles.md §3.3, §5.1, §5.2, §5.5; p-reasoning.md §3.6, §5.7] ·
  `vendors/cursor-plugins/pstack/skills/principle-encode-lessons-in-structure/SKILL.md:7-11`
  ("the instruction is the symptom"); `vendors/cursor-plugins/pstack/skills/reflect/SKILL.md:20-36`
  (Accepted / Rejected / Backlog, structural fixes moved out of the prose queue).
  **Canon status: partial.** Synthesis note: enforcement already lives in code, `RULES` with
  severities (`crates/canon-cli/src/brief_check.rs:91-114`), the ten gate failure classes
  (`canon-gate.md:18-33`), and the dogfood `trace_check.py` over recorded tool activity
  (`evals/dogfood/trace_check.py:1`); what is missing is a map from each prose rule in
  `SKILL.src.md` to its enforcer, and a routing step in the strategy path
  (`canon-learn.md:23-45` goes from eligibility straight to paired evaluation). **Canon
  target:** a short `canon/skills/reference/canon-enforcement.md` (rule → refusal code / review
  check / advisory / none) checked against `RULES` and the failure-class table so it cannot
  drift [p-principles.md §4, "Catalog drift"]; one routing paragraph in `canon-learn.md` and
  `canon-authoring.md` (before promoting a strategy, ask whether a scenario, policy field, gate
  class or lint can carry it); recurring dogfood failures go to `trace_check.py` or an issue.
  **Effort: trivial** (text), moderate (drift check). **Why #7:** it stops the core skill from
  growing prose rules that nothing reads, and costs a table.

- **#8 Predicate-first long-run record (decision trail plus resume note), as a recipe** — a
  run states a checkable finish predicate, makes one smallest change per iteration, verifies
  the real artifact, commits on progress or discards on none, appends one decision row
  (`ts, phase, decision, why, evidence, result`), and on pause writes a cold-start resume note.
  Source: [p-mode.md §3.5, §5.5, §5.6; p-harness.md §3.3, §5.3; p-verification.md §5.6;
  p-reasoning.md §3.2] · `vendors/cursor-plugins/pstack/docs/guide/07-overnight.md:18-56`;
  `vendors/cursor-plugins/pstack/skills/show-me-your-work/SKILL.md:11-22,50-53` (append-only,
  evidence is a pointer, a wrong call gets a superseding row);
  `vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/pause-safely.md:1-16`,
  `session-pickup.md:1-16`. **Canon status: partial.** Synthesis note: the finish predicate is
  the approved brief plus scenario coverage (`SKILL.src.md:18-33,50-57`), durable state is the
  latest-only ledger with archive (`SKILL.src.md:63-71`), and dispatch runs have a lifecycle
  (`crates/canon-cli/src/dispatch.rs:1174`); there is no iteration/discard record and no handoff
  note. **Canon target:** a hand-used recipe under issue #20 (a `decisions.tsv` whose
  `evidence` cells point at canon record ids or commits, not prose), not a new record kind;
  revisit a typed log with #54 only if recipes show demand. **Effort: moderate** (recipe),
  significant (record). **Why #8, not higher:** four audits point here, but canon owns the
  predicate and ledger already, and #20's own rule says wait for hand-used recipes.

- **#9 Repo-specific verifier recipe (Launch / Doctor / Drive / Evidence / Cleanup + feature
  map)** — a generator interviews the repo, writes a project-local verifier and a per-feature
  map, then runs one mapped feature end to end before handing over. Source:
  [p-verification.md §3.1, §5.1] ·
  `vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:20-44`.
  **Canon status: missing.** Synthesis note: `canon skills install` projects only the canon
  skill (`SKILL.src.md:133-154`); `evidence add` binds whatever report or artifact it is given
  (`role-implementer.md:54-81`); nothing produces a runnable, repo-specific proof harness.
  **Canon target:** a recipe under issue #20 (the "game evidence presets" and per-harness
  hooks named there are the same shape); the feature map should reference canon scenario ids
  rather than duplicate the index. **Effort: significant.** **Why #9:** highest ceiling for
  evidence quality, but it is a recipe family, and #20 defers recipe work until a few have been
  used by hand. Do #1-#3 first; they show whether weak evidence is a rubric problem or a
  harness problem.

- **#10 Intent-to-first-command routing** — a lookup from the kind of request to the
  workflow, role and first command, including a non-executing "which workflow applies?" answer.
  Source: [p-mode.md §3.1, §3.6, §5.1, §5.7] ·
  `vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:121-125` (matched playbook's steps
  copied into the todo list; a skipped step stays with `skip: <reason>`);
  `vendors/cursor-plugins/pstack/skills/poteto-help/SKILL.md:1-45` (answers "how do I work",
  does not start work). **Canon status: partial.** Synthesis note: "Route by task" is keyed by
  CLI area, not by request intent (`SKILL.src.md:156-169`); the skill's trigger covers
  build/change/fix requests only (`SKILL.src.md:3`); chore versus feature is covered in the
  planner reference (`role-planner.md:75-87`); `canon status` stays authoritative for repo
  state (`SKILL.src.md:100-106`). **Canon target:** an eight-row "Route by intent" table in
  `SKILL.src.md` (feature, fix, chore, resume, review-only, read-only question, setup, upgrade)
  each naming the role reference and first command; `crates/canon-cli/src/init.rs` AGENTS block
  gets the same pointer. A `canon help workflow` command stays deferred. **Effort: trivial**
  (table), moderate (command). **Why #10:** useful and nearly free, but it adds no enforcement
  and must not become a second status engine [p-mode.md §5.9].

## 3. Runners-up & deferred extracts

- **Model / reasoning-budget record on runs** [p-mode.md §3.2, §5.2, §5.3;
  `P/skills/setup-pstack/SKILL.md:15-31`]. Status: partial. Synthesis note: `RunLineage`
  already has `provider` and `model` (`records.rs:652-664`; `canon dispatch begin --provider
  --model`, `dispatch.rs:3-4,501-506`) and reviews record `reviewer_*`; effort/tier and model
  family are absent. Low value: it would be a self-declared label like `--session-id` (#14,
  #15, #16 are "watch" for the same reason).
- **`formal-check` evidence kind (scope, bounds, assumptions, sync commit, mutation ref,
  `in sync | updated | bug | blocked`)** [p-verification.md §3.2, §5.2;
  `P/skills/maintain-formal-verification-skill/SKILL.md:11-24`]. Status: partial. Synthesis
  note: the "pass whose covered code moved" check exists as `stale-evidence` plus
  `--surface-ref` (`canon-gate.md:27,606-611`); the new part (stated scope and assumptions,
  never weaken a property to get green) is text a reviewer can ask for under #2. Defer a typed
  kind until a repo actually uses a model checker.
- **Benchmark validity fields** (runs, range, errors, limiter, work observed, `inconclusive`)
  [p-verification.md §3.5; `P/skills/benchmark-checklist/SKILL.md:19-33`]. Status: missing.
  Canon grades behavior scenarios; keep as a checklist paragraph for performance scenarios under
  #2's text change, not a typed schema [p-verification.md §5.8].
- **Multi-seat review campaign with agreement map** [p-verification.md §3.3;
  `P/skills/interrogate/SKILL.md:42`]. Status: partial. Defer: seats are labels (#14, #39),
  and the useful half (reasons, #5) ships alone.
- **Design sketch before implementation** (types, signatures, module boundaries, two
  alternatives) [p-reasoning.md §3.1, §5.1; `P/skills/architect/SKILL.md:7-10,22-47`]. Status:
  conflicts. Synthesis note: canon's brief is a product-level PRD the human approves once
  (`role-planner.md:39-56`); a code-shape artifact next to it risks a second source of truth
  [p-reasoning.md §4, "Planning artifact drift"]. Keep the "two structurally different
  candidates" step for canon's own high-risk gate and policy changes as maintainer practice,
  not a product record.
- **Map imported-plan architecture prose instead of dropping it** [p-reasoning.md §5.2;
  `P/skills/architect/SKILL.md:78-79`]. Status: partial. Synthesis note: the openspec dialect
  counts `design.md` under `design-doc` (`canon-plan-import.md:113`); the superpowers dialect
  never reads architecture prose and raises no diagnostic (`canon-plan-import.md:133`). The
  audit's "silently disappears" is true only for the second.
- **More deterministic prose advisories** (passive voice, vague referents, multi-action
  instructions, terminology drift) [p-reasoning.md §3.3, §3.4, §5.3;
  `P/skills/unslop/SKILL.md:20-33`, `P/skills/technical-writing/SKILL.md:50-92`]. Status:
  partial. Candidates must have a deterministic matcher, a section scope and waiver support to
  join the `prose.*` ids (`brief_check.rs:105-113`); subjective vocabulary lists are rejected
  [p-reasoning.md §5.5]. Low leverage.
- **Document-mode classification of brief sections** [p-reasoning.md §3.3, §5.4]. Status: has.
  See §4.0.
- **`recall` capsule / `canon context recall`** [p-reasoning.md §3.5, §5.6;
  `P/skills/recall/SKILL.md:1-35`]. Status: missing. Significant; `canon status` covers
  subject state, and a narrative capsule needs transcripts that session ingest deliberately
  excludes (`canon/skills/reference/canon-session-ingest.md:206-207` per the audit).
- **Observed / rationale / inference / unknown labels in reviewer and planner reports**
  [p-reasoning.md §3.7, §5.9; `P/skills/poteto-mode/SKILL.md:113` ("measured, inferred, or
  guess")]. Status: partial. Trivial text for `role-reviewer.md`; fold into #2 or #3 if it fits.
- **Repeated-evidence threshold before distilling a lesson** [p-reasoning.md §5.8;
  `P/skills/automate-me/SKILL.md:36-60`]. Status: has. Synthesis note: `occurrence` mode with
  `n_min` and a contradiction reset already exists (`canon-learn.md:126-153`).
- **Closed handoff vocabulary and immutable-coordinate, fail-closed automation** (Benny)
  [p-harness.md §3.2, §3.4, §5.4, §5.5;
  `P/automations/benny/skills/triage-issue-reports/SKILL.md:10-31`]. Status: partial/skip.
  Synthesis note: reviews already have `signed | indeterminate | unavailable`
  (`role-reviewer.md:63-69`) and ship is fail-closed (`SKILL.src.md:57`); the Slack, tracker and
  Cursor Automations integration is not portable [p-harness.md §5.5].
- **Declarative agent-role metadata (fresh-per-task, parent-owned, report-only)** in provider
  projections [p-harness.md §3.5, §5.6; `P/agents/poteto-agent.md:1-10`,
  `P/agents/comment-sicko.md:1-35`]. Status: partial. Defer with the capability declaration
  that canon records but does not enforce (`canon-context.md:60-84` per the audit); relates to
  #16.
- **Principle citation convention ("name the principle and the decision it changed")**
  [p-principles.md §3.2, §5.3]. Status: skip. It is self-report (the #15 problem) and the audit
  itself says citation is rationale, not proof [p-principles.md §5.7].
- **Frontmatter-as-trigger, lifecycle-scoped descriptions** [p-principles.md §3.1, §3.4, §5.4].
  Status: has. Synthesis note: `SKILL.src.md:1-4` is a trigger contract and the role references
  are lifecycle-scoped (`SKILL.src.md:81-98`).
- **Do not copy:** all 24 principles as separate triggerable skills [p-principles.md §5.6],
  Cursor Custom Mode, `/loop` and model-slug persistence [p-mode.md §5.8], a vendor-style todo
  engine replacing `meta.next` [p-mode.md §5.9], `make-bot-ui`, `typescript-best-practices`,
  `no-comments` [p-reasoning.md §5.10].

## 4. Critical overlaps with canon's in-flight work

Open issues checked with `gh issue list --state open`: #14-#20, #39, #54, #74-#76, #96 (all
`watch` or an open defect; none has a landed design to conflict with).

### 4.0 Corrections to the source audits, from HEAD

Synthesis note: three audit claims were stale or overstated against the files cited.

1. **"Model routing: missing"** [p-mode.md §3.2]. Canon records `provider` and `model` per
   run (`records.rs:652-664`, `dispatch.rs:501-506`) and `reviewer_provider/model/harness` per
   review (`records.rs:842-881`, `canon-review.md:149,278-281`). Missing is only effort/tier,
   and evidence records carry no model. Ranking effect: #6 treats model-family diversity as
   advice.
2. **"Gate should flag a formal-check pass whose sync commit is behind covered code"**
   [p-verification.md §3.2]. `stale-evidence` already degrades a pass whose declared surface
   changed since its `evidence_sha`, and `--surface-ref` binds that surface (`canon-gate.md:27,
   606-611`). Ranking effect: the formal-check kind is a runner-up.
3. **"Canon does not classify document mode"** [p-reasoning.md §3.3]. Brief sections carry a
   `Role` of `normative | context | question` that decides which get the prose rules
   (`crates/canon-cli/src/brief_template.rs:22-26`). Ranking effect: no entry.
   Also, the "silent drop" of plan-import architecture prose is true for one dialect only (§3).

### 4.1 Session identity and independent review (#14, #39, #15)

- pstack makes independence concrete as "fresh agent", "verifier on a different model family",
  and "report-only" agents [p-mode.md §3.3; p-verification.md §3.3; p-harness.md §3.5;
  `P/skills/poteto-mode/SKILL.md:99-101`]. All of it is a label the parent trusts; nothing
  attests identity. This is issue #14's exact limit, and `claude-code.md:27` ("resume → spawn a
  new agent and paste the earlier result") shows the vendor leans on harness-native separation,
  not proof.
- Concrete learnings: (a) record the runtime facts the caller reports, which canon already
  does (`reviewer_*`); (b) do not gate on model family until a harness can attest it (#6 keeps
  it advisory); (c) a post-hoc mechanical control is available where attestation is not: a
  reviewer that must not edit leaves the pinned worktree unchanged, checkable with `git status`
  (`claude-code.md:21`), and the orchestrator can record that check (#6); (d) a per-harness
  table (#4) is where "no subagent tool, so start a separate process and pass its real session
  id" (#39's first option) belongs.
- #15 (which skill was used) has the same shape as pstack's "name the principle and the
  decision changed" [p-principles.md §3.2]; both are self-report. Do not lift the convention.

### 4.2 Recipes and per-harness hooks (#20)

- pstack is the largest available corpus of hand-written recipes for the shapes #20 names:
  an intent-to-playbook registry [p-mode.md §3.1], per-harness mapping
  [p-harness.md §3.1], a project-local verifier generator [p-verification.md §3.1], and a
  predicate-first autonomous loop [p-mode.md §3.5]. #20 says "build only after 2-3 recipes have
  actually been used by hand" and "canon core stays the contract".
- Learnings: keep recipes copy-in Markdown plus one small mechanical check each (pstack's own
  `check-plan.mjs` is that shape, `P/skills/poteto-mode/scripts/check-plan.mjs:8-28`); keep
  `canon status` and `meta.next` as the only state engine [p-mode.md §5.9]; read #8 and #9 as
  recipe designs, and ship the text-only entries (#1-#3, #6, #10) in core now.

### 4.3 Run journal, leases, adaptive monitoring (#54, #19)

- pstack's orchestration store is the same problem space as #54: one writer per durable file,
  append-only records, units/frontier/ledger/inbox, a derived `status.md` never hand-edited,
  and standing orders re-sent on every spawn because "directives decay across resumes"
  [p-mode.md §3.5; `P/skills/poteto-mode/playbooks/orchestrate.md:21-40`]. The decision log
  adds a `start` row per run so a later run knows which rows it did not write
  (`P/skills/show-me-your-work/SKILL.md:42`).
- Learnings: canon's latest-only ledger with archive (`SKILL.src.md:63-71`) already gives
  "new row supersedes, history kept"; if #54 proceeds, take the one-writer-per-file rule and the
  `start`-row attribution, not the TSV store. Liveness, retry and escalation handling
  [p-mode.md §3.5; `P/skills/poteto-mode/playbooks/orchestrate.md:75-125` per the audit] are the harness's job, which is #19's conclusion. Repeating the
  role instruction on every spawn is already canon's rule (`role-orchestrator.md:33-35,42-47`).

### 4.4 Review quality scoreboard and finding triage (#18)

- pstack's lead judgment sorts every finding into four buckets with the raising model and a
  reason, and treats "raised independently by two or more models" as the strongest signal
  [p-verification.md §3.3; `P/skills/interrogate/SKILL.md:42-51`].
- Learnings: a scoreboard (#18) needs a population of dispositions with reasons; canon records
  dispositions but no reason (#5), so the cheapest unblocker is the `--reason` field. Agreement
  across seats is meaningful only when seats are distinct runtimes, which canon cannot attest
  (#14); keep the agreement map deferred (§3).

### 4.5 Policy and capture records (#16, #75, #74)

- Benny's workers are told they hold no Slack credentials and may not write; the boundary is
  declared in prose and enforced by the host [p-harness.md §3.2;
  `P/automations/benny/FOR_AGENTS.md:5-35`]. Canon's capability declaration is also recorded,
  not enforced [p-harness.md §3.2, §3.5]. That matches #16: canon can record a policy digest a
  runtime reports, and nothing more.
- Transcript-reading skills pin workspace, topic and window and forbid globbing across other
  projects' private chats [p-reasoning.md §4, "Unbounded transcript access";
  `P/skills/show-me-your-work/SKILL.md:57`]. That supports #75's direction (capture mode and
  scope should be explicit and recorded with the rows); it adds nothing to the issue's fix.
- #74 (origin attribution across agents): no pstack surface bears on it beyond the per-run
  `start` rows in §4.3.

### 4.6 Spec-to-code staleness (#17)

- The formal-verification lifecycle (covered files, sync commit, `in sync | updated | bug |
  blocked`, "never weaken the property") is a hand-run version of what #17 wants from import
  graphs [p-verification.md §3.2; `P/skills/create-formal-verification-skill/SKILL.md:29`;
  `P/skills/maintain-formal-verification-skill/SKILL.md:11-24`]. Synthesis note: canon's
  `stale-evidence` by `--surface-ref` (`canon-gate.md:27,606-611`) is the existing mechanism;
  the transferable part is the drift-versus-bug triage vocabulary for the remedy text when a
  review goes stale, not new state.

### 4.7 No overlap found

- #76 (workflow lockfiles) and #96 (adopting a draft change blocks evidence for all of a live
  subject's scenarios): none of the five audits touches CI pinning or approval scoping, and no
  entry here should wait on or change those fixes.

## 5. Per-canon-surface action plan

### `canon/skills/SKILL.src.md` (core skill)

1. **INSPIRE** an eight-row "Route by intent" table → `SKILL.src.md` "Route by task" +
   `crates/canon-cli/src/init.rs` AGENTS block — [p-mode.md §3.1, §3.6] — trivial (#10).
2. **INSPIRE** a one-line pointer to the harness matrix and the enforcement map from "Know your
   role" / "Safety contract" → `SKILL.src.md:81-98,171-173` — [p-harness.md §3.1;
   p-principles.md §3.3] — trivial (#4, #7).
3. **SKIP** adding principle-style rules to the always-loaded core; each addition must name its
   enforcer or stay in a role reference [p-principles.md §5.6, §5.7].

### Role references (`canon/skills/reference/role-*.md`)

1. **EXTRACT** fail-first proof for fixes → `role-implementer.md` Steps 3-4, `role-reviewer.md`
   "Check the checks" — [p-verification.md §3.5; p-harness.md §3.4] — trivial (#1).
2. **EXTRACT** proof standards and a falsifiability probe → `role-reviewer.md`,
   `role-implementer.md` Step 4 — [p-verification.md §3.1, §3.2, §3.5] — trivial (#2).
3. **EXTRACT** blast-radius report and safety-fact ladder → `role-reviewer.md` new section —
   [p-verification.md §3.4] — trivial (#3).
4. **EXTRACT** spawn brief fields, owned worktree, post-check of an unmodified reviewer
   worktree → `role-orchestrator.md` "Spawn each role" — [p-mode.md §3.3] — trivial (#6).
5. **INSPIRE** claim labels (measured / inferred / guess) in reviewer reports →
   `role-reviewer.md` "Record" — [p-reasoning.md §3.7] — trivial.

### CLI and record model (`crates/canon-cli`, `crates/canon-model`)

1. **EXTRACT** `finding close --reason` and an additive Finding `reason` field, required for
   `rejected` and `deferred` → `crates/canon-cli/src/finding.rs`, `main.rs`,
   `crates/canon-model/src/records.rs`, `canon-review.md`, conformance scenario — [p-verification.md
   §3.3] — moderate (#5).
2. **INSPIRE** optional typed `failed_before` / `failure_reason` on `evidence add`, only after
   #1's text shows demand → `crates/canon-cli/src/evidence.rs` — [p-verification.md §3.5] —
   moderate.
3. **INSPIRE** `canon skills doctor` printing per-harness unsupported rows → `crates/canon-cli/src/skills.rs`
   — [p-harness.md §3.1] — moderate, after the matrix exists (#4).
4. **SKIP** effort/tier and model-family fields on runs and evidence for now [p-mode.md §5.10];
   revisit if a harness can attest them.

### Gate and review references (`canon-gate.md`, `canon-review.md`, `crates/canon-gate/`)

1. **INSPIRE** a blast-radius report template and the `unproven → finding` rule →
   `canon-review.md` — [p-verification.md §3.4] — trivial (#3).
2. **INSPIRE** drift-versus-bug wording in the `stale-evidence` remedy →
   `canon-gate.md:27`, `crates/canon-gate/src/freshness.rs` — [p-verification.md §3.2] —
   trivial.
3. **SKIP** a typed `formal-check` gate and a benchmark gate; the gate keeps judging bytes and
   currency, not prose truth [p-verification.md §3.4, §5.8].

### Planner and brief (`brief_check.rs`, `brief-*.md`)

1. **INSPIRE** extra `prose.*` advisories only with deterministic matchers, section scope and
   waivers → `crates/canon-cli/src/brief_check.rs:105-113` — [p-reasoning.md §3.3, §5.3] —
   moderate; low priority.
2. **SKIP** a design-sketch artifact beside the brief; if wanted, bind it to the change digest
   and mark it guidance [p-reasoning.md §4].
3. **INSPIRE** mapping or reporting imported architecture prose in the superpowers dialect →
   `canon-plan-import.md:133` — [p-reasoning.md §5.2] — trivial.

### `canon learn` and evals (`canon-learn.md`, `evals/dogfood/`)

1. **EXTRACT** routing of recurring corrections to schema, gate, lint or record invariant
   before strategy promotion → `canon-learn.md`, `canon-authoring.md` — [p-reasoning.md §3.6,
   §5.7; p-principles.md §3.3] — trivial (#7).
2. **INSPIRE** auditing a decision log against the session's tool activity (each row maps to a
   real action, each evidence pointer resolves)
   [`P/skills/show-me-your-work/SKILL.md:55-60`] → `evals/dogfood/trace_check.py` — eval
   method, not a gate — [p-verification.md §3.5, §5.6] — moderate; only with #8.
3. **SKIP** a transcript-mining front end until capture scope is settled (#75)
   [p-reasoning.md §4].

### Harness projections and recipes (`canon skills install`, #20)

1. **EXTRACT** `canon/skills/reference/canon-harnesses.md`, one row per provider — [p-harness.md
   §3.1, §5.1] — moderate (#4).
2. **INSPIRE** a predicate-first long-run recipe with a decision-row format → recipe under #20
   — [p-mode.md §3.5; p-harness.md §3.3] — moderate (#8).
3. **INSPIRE** a verifier-generator recipe tied to scenario ids → recipe under #20 —
   [p-verification.md §3.1] — significant (#9).
4. **SKIP** Cursor Custom Modes, `/loop` and Benny's Slack automation [p-mode.md §5.8;
   p-harness.md §5.5].

## 6. Risk register

1. **Prose mistaken for enforcement.** Several pstack principles say "must" with no tool
   behind them; copying them into canon could imply a rule is gated when it is not
   [p-principles.md §4, "Advice/enforcement ambiguity"]. Severity: high. Surfaces: role
   references, `SKILL.src.md`. Mitigation: #7's enforcement map; every new rule names its
   enforcer or is marked advice; citations are rationale, not proof [p-principles.md §5.7].
2. **Labels standing in for identity.** Fresh agent, model family and session id are
   caller-declared; a model-family gate would be as weak as `distinct_actor` (#14, #39).
   Severity: high. Surfaces: `role-orchestrator.md`, review records, gate. Mitigation: model
   family is advisory text (#6); use post-hoc mechanical checks (unmodified worktree); wait for
   a harness that attests session ids.
3. **A fail-first requirement forcing fake tests or fabricated "failed before" claims.**
   The vendor allows skipping when a test is impractical and prefers "no new test over a bad
   test" (`P/skills/tdd/SKILL.md:11,26`); an agent can also claim a pre-fix failure it never
   ran [p-verification.md §5.8]. Severity: medium. Surfaces: `role-implementer.md`,
   `evidence add`. Mitigation: always optional with a stated-reason escape; the failing run is a
   bound artifact (digest-checked), never a bare claim; keep typed fields optional.
4. **Second source of truth.** A design sketch, a playbook label or a decision TSV could drift
   from the brief and ledger [p-reasoning.md §4; p-mode.md §5.9]. Severity: medium-high.
   Surfaces: `canon status`, brief, run records. Mitigation: no new record kind for #8; a
   decision log points at canon record ids; `canon status` stays the one engine.
5. **Harness-coupled facts rot.** Cursor model slugs, effort tokens, `/loop` and the Claude
   Code field mapping change with host releases [p-mode.md §4; p-harness.md §4]. Severity:
   medium. Surfaces: `canon-harnesses.md`. Mitigation: verify each cell against the harness or a
   dogfood trace, mark unknown cells, date the table, let `skills doctor` report drift later.
6. **Unbounded transcript and history access.** `recall`, `reflect` and `automate-me` read chat
   history; canon's session ingest is metadata-only and excludes task prose
   [p-reasoning.md §4]. Severity: high if built, none if skipped. Surfaces: `canon learn`,
   ingest. Mitigation: skip the transcript front end; any future recall is read-only with
   explicit subject, window and source provenance, tied to #75.
7. **Unverified narrative learning.** Reflection can yield plausible but unsupported lessons
   [p-reasoning.md §4]. Severity: medium. Surfaces: `canon learn`. Mitigation: #7 routes them to
   structure or an issue first; promotion keeps paired evaluation and signed approval
   (`canon-learn.md:23-45`).
8. **Cost and brittleness of heavy verification.** Multi-model interrogation, arena, formal
   checkers and profilers are expensive [p-verification.md §4, "Heavy deps"]. Severity:
   medium. Surfaces: review, evidence. Mitigation: opt-in only, never a default gate; do the
   text lifts first.
9. **Subjective prose rules.** `unslop`'s vocabulary and metaphor rules cause false positives
   and policy churn [p-reasoning.md §4]. Severity: low. Surfaces: `brief_check.rs`. Mitigation:
   advisory severity, deterministic matchers only, waivers.
10. **Audit-versus-HEAD drift.** Three audit claims were stale (§4.0). Severity: medium.
    Surfaces: every action in §5. Mitigation: re-read the cited canon lines before opening each
    PR; the status column here is HEAD at canon 0.24.1.
11. **Trigger overlap and catalog drift** among 24 principles [p-principles.md §4]. Severity:
    low. Surfaces: skills, if principles were adopted wholesale. Mitigation: not adopted (§3).

## 7. Recommended next 3 PRs

1. **PR 1: "skill: proof standards, fail-first for fixes, blast-radius report" (#1, #2, #3)**
   - Scope: `canon/skills/reference/role-implementer.md`, `role-reviewer.md`,
     `canon-review.md`; regenerate projections and the skills lock (`canon skills install`,
     then `canon skills check`).
   - Source audits: [p-verification.md §3.4, §3.5, §3.2; p-harness.md §3.4].
   - Definition of done: the three changes are text only, with no CLI or record change; the
     fail-first paragraph names the escape ("state why and name the closest executable check");
     the reviewer reference lists the four proof questions, the optional falsifiability probe
     and the blast-radius template; `canon skills check` is clean; one dogfood run on a bug-fix
     change shows the pre-fix failing artifact bound with `--artifact` (not `--report`)
     [INFERENCE: how the gate treats an attached failing artifact has not been run].
2. **PR 2: "reference: per-harness capability and skip matrix" (#4, #6)**
   - Scope: new `canon/skills/reference/canon-harnesses.md`; route lines in `SKILL.src.md`
     and `role-orchestrator.md` ("Spawn each role": spawn-brief fields, owned worktree, reviewer
     worktree left unmodified, `git status` check).
   - Source audits: [p-harness.md §3.1, §5.1; p-mode.md §3.3, §5.4].
   - Definition of done: one row per provider in `skills.providers` (claude, codex, omp, pi)
     covering role spawn, separate reviewer process, real session id source, read-only handling
     and the recorded outcome when unsupported; each cell verified against the harness or a
     dogfood trace, otherwise "unknown"; no claim of identity proof; issue #39 updated with the
     table link.
3. **PR 3: "finding close: reasoned dispositions" (#5)**
   - Scope: `crates/canon-model/src/records.rs` (additive optional `reason` on Finding),
     `crates/canon-cli/src/finding.rs` and `main.rs` (`--reason`, required for `rejected` and
     `deferred`), schema and error output, `canon-review.md`, `role-reviewer.md`, a
     `finding.close.NN` scenario with evidence and an independent review, built under canon's
     own loop (human-approved brief first).
   - Source audits: [p-verification.md §3.3, §5.3].
   - Definition of done: closing `rejected` or `deferred` without `--reason` is a usage
     refusal that names the flag; pre-existing findings still deserialize and pass the gate;
     `canon finding list --json` exposes the reason; the change is noted on issue #18 as the
     missing input for a review scoreboard.

Next after these: #7 (enforcement map plus routing paragraph in `canon-learn.md`), then #10
(intent table). #8 and #9 wait for issue #20's hand-used recipes.
