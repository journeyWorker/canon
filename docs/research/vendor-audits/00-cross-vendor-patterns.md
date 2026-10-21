# Cross-vendor pattern list (2026-10-21)

The three per-vendor syntheses (`addyosmani-agent-skills/00-synthesis.md` = A, `mattpocock-skills/00-synthesis.md` = M, `cursor-pstack/00-synthesis.md` = P) rank 30 patterns; several are the same idea. This list merges them into 15 candidates, the unit that phase 6 (revalidate) and phase 7 (map) work on. `A#2` means synthesis A, top-10 entry 2.

| ID | Pattern | Sources |
| --- | --- | --- |
| X1 | Fail-first proof for fixes: a fix's scenario has a failing (non-faithful) run recorded before the passing one | A#2, M#2, P#1 |
| X2 | Reviewer proof standards: named ways a passing test proves nothing, falsifiability probe, check-set shrink, blast-radius safety fact, spec-fidelity read separate from standards (five axes) | A#2, A#3, M#1, M#4, P#2, P#3 |
| X3 | Anti-rationalization / red-flag / verification blocks in role references; advice-versus-mechanism map | A#1, P#7 |
| X4 | Harness capability matrix (subagents, isolation, ask-user, hooks, explicit "skipped") per provider, plus an orchestrator spawn contract; bears on #39 | A#5, P#4, P#6 |
| X5 | `canon skills install`/`check` prune and report files a bundle no longer ships | M#3 |
| X6 | Skill-text authoring rules with a reachability check; offline routing eval for skill descriptions | M#6, A#8 |
| X7 | Dogfood eval methods: pressure cases with ordered trace predicates, a strict expectation-result contract for graders, two-arm ablation / paired runs | A#4, A#9, A#10, M#8 |
| X8 | Reasoned finding dispositions (`finding close` carries why for rejected/deferred/downgraded) | P#5 |
| X9 | Brief durability: no file paths or line numbers in the brief; current versus desired behavior | M#5 |
| X10 | Cut implementation units along flow steps / thin vertical slices with a verify command | A#6, M#7 |
| X11 | Expand/contract guidance for migration chores | A#7 |
| X12 | Long-run record and continuation packet (decision trail, resume note, Handoff record) | P#8, M#10 |
| X13 | Repo-specific verifier recipe (launch / doctor / drive / evidence / cleanup + feature map) | P#9 |
| X14 | Scope bar (observed failure + fit) and durable rejection records for canon's own issues | M#9 |
| X15 | Intent-to-first-command routing table in the skill | P#10 |

## Revalidation (phase 6, 2026-10-21)

One read-only agent per pattern re-read the cited vendor code and canon's current code. Verdicts: CONFIRMED (claim holds), REFINED (shape right, detail corrected), DOWNGRADED, EVICTED.

### X1 — REFINED [revalidated 2026-10-21]

**Vendor reality:** All three vendors prescribe a red→green sequence for fixes: write/run a failing reproduction before changing code, then pass it afterward, with a closest executable repro when impractical (Addy `vendors/addyosmani-agent-skills/skills/test-driven-development/SKILL.md:38-98,301-398`; Matt `vendors/mattpocock-skills/skills/engineering/diagnosing-bugs/SKILL.md:18-66`; Cursor `vendors/cursor-plugins/pstack/skills/tdd/SKILL.md:7-35`). This is primarily workflow guidance, not a trustworthy ledger attestation.

**Canon reality:** The implementer guidance requires every bug fix to add/extend a scenario but does not require a pre-fix failing run (`canon/skills/reference/role-implementer.md:35-42`). Evidence records are explicitly attribution, not execution proof; `--ref` may name a command that never ran (`crates/canon-cli/src/evidence.rs:18-39,69-78`). Canon does retain earlier evidence versions: 0.23 archives superseded `EvidenceRecord`s and `canon query --kind evidence --history` reads ledger plus archive (`crates/canon-store/src/archive.rs:1-10,64-76`; `canon/skills/reference/canon-storage.md:108-125`; `crates/canon-cli/src/query.rs:394-428`). However, the verdict domain is only `faithful | not_applicable | divergent`; `divergent` means an external limit, not “th…

**Corrected claim:** Canon has the history and artifact mechanism to support reviewer inspection, but neither records nor proves that an executable check failed before the fix.

**Concrete change / effort:** Add fail-first and closest-repro guidance to implementer/reviewer skill text, plus a dogfood trace fixture asserting ordering; keep it advisory because evidence is self-attested. Trivial text, moderate eval. Typed `failed_before` metadata would be a later moderate change.

[Some lines truncated to 768 chars. Use agent://RevX1:raw:5-5 to read line 5 whole]

### X2 — REFINED [revalidated 2026-10-21]

**Vendor reality:** The pattern is real as prose guidance, not enforcement. Matt explicitly names tautological, implementation-coupled, and horizontal tests, requiring an independent expected value (`vendors/mattpocock-skills/skills/engineering/tdd/SKILL.md:20-45`). Addy requires a failing bug reproduction, passing regression test, and full-suite run (`vendors/addyosmani-agent-skills/skills/test-driven-development/SKILL.md:96-143`), plus regression-test and end-to-end verification (`vendors/addyosmani-agent-skills/skills/debugging-and-error-recovery/SKILL.md:276-298`). Cursor requires real user-path/action/state/side-effect evidence and a broken-variant sensitivity check (`vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:27-31,40-44`);…

**Canon reality:** Canon already requires independently running claimed checks (`canon/skills/reference/role-reviewer.md:63-69`) and reading bound tests against every Then step (`role-reviewer.md:71-76`). Review pins scenario/spec digests and evidence verdict/check identity (`canon/skills/reference/canon-review.md:316-345`; `crates/canon-gate/src/freshness.rs:69-120`). But attestations remain claims canon does not semantically validate (`canon-review.md:10-13`); no defect-class rubric, falsifiability probe, blast-radius fact, or separate diff/spec-fidelity pass exists.

**Corrected claim:** Add prose proof-quality axes, not a single mandatory “five-axis” or check-shrink mechanism.

**Concrete change / effort:** Extend `role-reviewer.md` and `canon-review.md` with defect classes, real-path/side-effect questions, optional broken-variant probe, blast-radius template, and separate spec-fidelity pass; keep records unchanged. Trivial text effort; moderate only for history-based advisory/eval coverage.

[Some lines truncated to 768 chars. Use agent://RevX2:raw:3-3 to read line 3 whole]

### X3 — REFINED [revalidated 2026-10-21]

**Vendor reality:** Addy’s skills do provide explicit excuse→reality tables, red-flag lists, and verification checklists: `vendors/addyosmani-agent-skills/skills/test-driven-development/SKILL.md:363-387` and `skills/incremental-implementation/SKILL.md:213-246`; the audit reports this pattern across 24 skills (`docs/research/vendor-audits/addyosmani-agent-skills/00-synthesis.md:18-22`). Pstack’s “advice versus mechanism” is narrower: `principle-encode-lessons-in-structure/SKILL.md:7-11` says recurring instructions should become lints/metadata/runtime checks, while `reflect/SKILL.md:20-36` routes lessons to Accepted/Rejected/Backlog. Neither is an executable enforcement map.

**Canon reality:** The “missing” claim is directionally right for role-specific excuse/reality blocks: roles have strong `NEVER`/procedure text but no such tables (`canon/skills/reference/role-implementer.md:39-69`, `role-reviewer.md:61-86`). However, canon already distinguishes advice from mechanisms: evidence/review requirements are executable and gate-checked (`canon/skills/reference/canon-gate.md:18-33`), while prose rules are explicitly advisory (`crates/canon-cli/src/brief_check.rs:17-25`). Open #39 is the concrete self-review failure this guidance could target.

**Corrected claim:** Vendor packs demonstrate useful prose anti-rationalization blocks; Pstack adds a prose rule to route repeatable lessons into structure, not a general advice/mechanism registry. Canon lacks the former and only has the latter implicitly.

**Concrete change / effort:** Add short canon-specific rationalization, red-flag, and verification blocks to each role reference plus a compact ≤8-row map in `SKILL.src.md`; state that repeated corrections belong in scenarios, policy, inventory, or gates. No CLI or record-field change is justified. Rough effort: low/trivial.

### X4 — CONFIRMED [revalidated 2026-10-21]

**Vendor reality.** Addy’s pack separates portable skills from harness-native adapters and avoids double-injecting routing where Codex already discovers skills (`a-harness.md:30-32`; `vendors/addyosmani-agent-skills/docs/codex-setup.md:30-43`). Pstack provides an explicit Cursor-to-Claude capability table covering subagents, isolation, ask-user, model fields, paths, and unsupported features with explicit “Skipped” behavior (`vendors/cursor-plugins/pstack/docs/harness/claude-code.md:8-65`). Its orchestration contract requires parent-owned briefs, fresh workers, exclusive worktrees, real-artifact verification, and a separate verifier (`vendors/cursor-plugins/pstack/skills/poteto-mode/playbooks/orchestrate.md:1-45`; `.../poteto-mode/SKILL.md:90-125`).

**Canon reality.** `skills.rs` models only provider name, projection root, and detection markers (`crates/canon-cli/src/skills.rs:92-135`); selection supports flag/config/lock/detection precedence but no runtime capability metadata (`:749-781`). The role text requires separate role agents and a reviewer process with a real session id (`canon/skills/reference/role-orchestrator.md:31-55`), but has no consolidated spawn-brief schema, worktree ownership, or provider-specific fallback. Issue #39 confirms Codex currently lacks subagent tooling and can pass distinct-session checks using relabeled self-review metadata (`issue://39`).

**Corrected claim.** Canon has provider projection and a partial spawn/session contract, but no per-provider capability matrix or enforceable way to attest that a reviewer was actually a separate process.

**Concrete change.** Add a provider capability table and routed harness reference covering spawn, isolation, ask-user, hooks, and explicit skips. Extend orchestrator prose with GOAL/SCOPE/ownership/verification fields. A future dispatch/session record could carry harness-attested session identity; rough effort is moderate, with the prose-only contract being trivial.

### X5 — CONFIRMED [revalidated 2026-10-21]

**Vendor reality:** Matt Pocock’s pack intentionally deletes retired skills; the deprecated bucket says removals name their replacement (`vendors/mattpocock-skills/skills/deprecated/README.md:1-4`). The audit frames this as a lifecycle endpoint, not a compatibility alias (`docs/research/vendor-audits/mattpocock-skills/m-meta.md:84-92`).

**Canon reality:** `canonical_bundle` enumerates only the current `SKILL.src.md`, `reference/**`, and `scripts/**` files (`crates/canon-cli/src/skills.rs:535-560`). `install_canonical` writes current files, but its only removal set is `legacy_codex_removals`, restricted to the old `.codex/skills/canon*` projection and previous-lock hash matches (`skills.rs:1053-1076`, `1391-1448`). Therefore, if `reference/x.md` disappears from the source, an unchanged projected copy remains on disk.

`check` likewise iterates only current bundle files (`skills.rs:1482-1508`). It compares the lock manifest as a whole (`skills.rs:1510-1513`), so the dropped file can make `manifest_ok` false, but it is not emitted as a missing/orphan path. Human output prints statuses, remnants, and outdated-lock messages only (`main.rs:2300-2303`); JSON exposes only `manifestOk` and current statuses (`main.rs:2304-2312`). It exits with generic `SKILLS_DRIFT`.

**Corrected claim:** A retired current-bundle file persists silently; `skills check` may fail generically through manifest mismatch, but does not report the orphan. Only proven legacy Codex files are pruned/reported.

**Concrete change:** Generalize previous-lock, hash-proven deletion to all dropped managed paths; add orphan diagnostics in `check`/`doctor` and tests for dropped, modified, and unselected-provider files. Rough effort: moderate.

### X6 — REFINED [revalidated 2026-10-21]

**Vendor reality.** Matt’s `writing-for-agents` defines context pointers with explicit trigger branches, one trigger per branch, checkable/exhaustive completion criteria, single-source-of-truth pruning, and deletion of no-op prose (`vendors/mattpocock-skills/skills/productivity/writing-for-agents/SKILL.md:12-18,47-52,78-81`). Addy’s evaluator is genuinely a catalog gate: it requires ≥3 positive/≥2 negative prompts per skill, owner-bound pairwise negatives, stemmed TF-IDF top-k ranking, collision thresholds, and an optional rank-1 floor (`vendors/addyosmani-agent-skills/scripts/run-evals.js:52-59,208-399`; `evals/README.md:91-103`).

**Canon reality.** Canon ships one umbrella `canon` skill with lazy references, not a multi-skill catalog (`canon/skills/README.md:1-18`). Its source routes the 17 topic references and role/brief references manually (`canon/skills/SKILL.src.md:8-11,157-169`); role files have explicit procedures but no uniform completion-criterion authoring rule (`canon/skills/reference/role-planner.md:40-75`). `canon skills check` validates provider projections, not internal reference reachability (`crates/canon-cli/src/skills.rs:1-31`). Open issue #15 confirms actual skill resolution is only observable through future harness metadata, not current canon.

**Corrected claim.** Canon should adopt the authoring rules and a static reference-reachability check; Addy’s offline routing/collision eval does **not** apply as a catalog eval while canon ships one skill.

**Concrete change / effort.** Add concise authoring guidance plus a read-only checker that resolves links from `SKILL.src.md` and role files and fails on orphan references or broken targets. Do not add TF-IDF/rank-1 cases unless canon later ships multiple independently routed skills. Rough effort: **trivial text, moderate checker**.

### X7 — REFINED [revalidated 2026-10-21]

**Vendor reality.** Addy’s runner has explicit behavioral expectations and pressure cases, including ordering (“failing before fix”), sunk-cost and authority pressure (`vendors/addyosmani-agent-skills/evals/README.md:34-55`; `.../evals/cases/test-driven-development.json:64-81`). Its grader enforces one id-bound result per expectation, nonblank evidence, derived counters, and raw retention for invalid output (`.../scripts/run-evals.js:454-537`; `.../scripts/run-evals-test.js:68-168`). Plugin evals perform repeated with/without-plugin arms and separate invocation, lexical, and semantic graders (`.../evals/README.md:40-55`; `.../evals/plugin/.../failing-test-first.md:1-5`). These are real, distinct methods.

**Canon reality.** `trace_check.py` already parses traces without execution and enforces pre-brief/question-turn writes plus role separation (`evals/dogfood/trace_check.py:526-638`; `test_trace_check.py:17-108`), and dogfood evals 7–8 demonstrate fixture-backed acceptance criteria and recovery measurement (`docs/research/2026-10-15-dogfood-eval-7.md:8-12`; `...eval-8.md:9-19`). Canon also already has paired baseline/candidate scoring (`evals/run_eval.py:619-645`) and strict paired promotion requirements (`canon/skills/reference/canon-learn.md:31-43`). Missing: general ordered-pressure predicates, per-expectation audit-result validation/raw-invalid retention, and skill/no-skill ablation.

**Corrected claim.** Canon should add pressure fixtures and strict audit-result validation; reuse its existing paired scoreboard, adding skill/no-skill arms only as opt-in research.

**Concrete change / effort.** Extend `trace_check.py` with named ordered predicates and line-referenced failures; add malformed-result fixtures/tests and document the audit schema in `canon-gate.md` (moderate). Add arm metadata/repeats only if measuring skill contribution (significant; never a ship gate).

### X8 — CONFIRMED [revalidated 2026-10-21]

**Vendor reality:** pstack’s `interrogate` workflow classifies each finding as Act on, Consider, Noted, or Dismissed, preserving model agreement/disagreement and requiring a one-line rationale; it does not auto-apply decisions (`vendors/cursor-plugins/pstack/skills/interrogate/SKILL.md:42-51`; `docs/research/vendor-audits/cursor-pstack/p-verification.md §3.3`). The synthesis accurately identifies rationale as the durable distinction, though “downgraded” is vendor terminology rather than a Canon disposition.

**Canon reality:** `Finding` stores `open|fixed|rejected|deferred`, plus severity, reviewer, summary, and resolution SHA, but no reason field (`crates/canon-model/src/records.rs:1591-1605,1747-1790`). `finding close` accepts only disposition, resolution SHA, actor, role, and session (`crates/canon-cli/src/main.rs:1137-1170`; `crates/canon-cli/src/finding.rs:705-728`). Rejected/deferred constructors clear the resolution SHA but retain no rationale (`records.rs:1955-1968`). The review guide defines rejected as “considered and declined” and deferred as “accepted, but postponed,” without requiring explanation (`canon/skills/reference/canon-review.md:125-139`).

**Corrected claim:** Canon has durable finding dispositions and transition attribution, but rejected/deferred transitions carry no rationale or source-seat information, limiting review-quality scoring (#18).

**Concrete change / effort:** Add additive optional `reason` to `Finding`; add `--reason` to `finding close`, required for `rejected` and `deferred`; expose it in JSON/list output; update schema, reviewer skill/docs, and add a close scenario/evidence. Pre-existing records must deserialize unchanged. Rough effort: **moderate**.

### X9 — REFINED [revalidated 2026-10-21]

**Vendor reality:** Matt’s agent brief is an AFK issue/PR contract: it explicitly separates current and desired behavior, names interfaces, requires independently verifiable acceptance, and excludes file paths/line numbers because implementation structure changes (`vendors/mattpocock-skills/skills/engineering/triage/AGENT-BRIEF.md:3-5,14-17,28-37,47-68`). This is real, but scoped to handoff durability rather than a general PRD format.

**Canon reality:** Canon’s PRD has explicit desired outcome, flow, detail, acceptance, questions, and out-of-scope sections (`crates/canon-cli/src/brief_template.rs:54-67`; `canon/skills/reference/role-planner.md:20-37`). It does not require an explicit current-state or interface contract, and `brief_check` has no rule for either (`crates/canon-cli/src/brief_check.rs:91-114`). It also does not prohibit paths: planner guidance permits a `Source: <path>` line for external plans (`canon/skills/reference/role-planner.md:29-32`).

**Corrected claim:** Canon has durable scope/acceptance briefs, but lacks explicit current-state/interface guidance and a separate durable handoff view; “no paths/line numbers” should be guidance for implementation descriptions, not an absolute prohibition.

**Concrete change:** Update brief guidance to require current behavior in Intent, preserve desired behavior in Outcome, and describe interfaces behaviorally; warn against implementation paths/line numbers except provenance. Avoid a new record field or hard path checker. Add one dogfood brief fixture/eval for current-vs-desired and stale-path guidance. Rough effort: trivial–moderate.

### X10 — REFINED [revalidated 2026-10-21]

**Vendor reality:** Addy’s incremental loop requires implement → test → verify → commit per slice and prefers complete vertical paths, with contract-first/risk-first alternatives (`vendors/addyosmani-agent-skills/skills/incremental-implementation/SKILL.md:21-57`, `:59-128`). Its planning skill explicitly contrasts horizontal layers with end-to-end slices and requires acceptance, verification, and scope per task (`vendors/addyosmani-agent-skills/skills/planning-and-task-breakdown/SKILL.md:25-90`). Matt’s ticket guidance adds narrow, independently verifiable tracer bullets, fresh-context sizing, and blocking edges—but explicitly treats wide refactors as expand–migrate–contract (`vendors/mattpocock-skills/skills/engineering/to-tickets/SKILL.md:25-78`).

**Canon reality:** `role-implementer.md` currently says only “split the work,” with per-unit actor/session attribution; scenarios are tied to flow steps and failure cases (`canon/skills/reference/role-implementer.md:35-58`). `role-planner.md` explicitly says `tasks.md` is optional and non-gating (`canon/skills/reference/role-planner.md:20-37`). Native changes write no `tasks.md`; existing files are merely read for task evidence (`canon/skills/reference/canon-subject.md:362-400`). Thus canon is partial, not missing; a task dependency ledger would conflict with flow/scenario coverage.

**Corrected claim:** Add guidance that each implementation unit SHOULD be one complete flow-step slice, independently verifiable with a named command and sized for one context; do not make `tasks.md`, blocking edges, commits, feature flags, or buildability required records.

**Concrete change:** two prose paragraphs in `role-implementer.md`/`SKILL.src.md`; optionally a dogfood trace case checking unit decomposition and verify-command naming. Rough effort: trivial (text), low (eval).

### X11 — REFINED [revalidated 2026-10-21]

**Vendor reality:** The migration skill genuinely prescribes expand → dual-write → batched backfill → read cutover → stop old writes → later drop, with tested down paths, throttling, non-blocking indexes, and optional feature flags (`vendors/addyosmani-agent-skills/skills/deprecation-and-migration/SKILL.md:180-231`). Its sibling guidance reinforces thin, independently verifiable slices (`vendors/addyosmani-agent-skills/skills/incremental-implementation/SKILL.md:21-57`), while the pack routes migration/deprecation and shipping skills for operational work (`vendors/addyosmani-agent-skills/README.md:281-286`). The audited claim is substantively correct.

**Canon reality:** Canon has only the generic chore mechanism: chores are limited to refactor/dependency/build/CI/infra work that changes nothing observable (`canon/skills/reference/role-planner.md:75-85`); the template requires only intent, category, invariants, checks, and out-of-scope (`crates/canon-cli/src/brief_template.rs:103-143`). Crucially, invariants explicitly prohibit behavior, public-interface, data, or output changes (`brief_template.rs:139-141`), and the checker enforces only nonempty prose/category/section shape (`crates/canon-cli/src/brief_check.rs:546-583`). The open issue list contains no migration-specific issue.

**Corrected claim:** Expand/contract is valuable guidance for behavior-preserving infrastructure migrations, but schema/API migrations that change data or public interfaces are not automatically chores; Canon currently lacks migration-phase guidance and a migration-specific contract.

**Concrete change / effort:** Add planner guidance and a chore-template hint distinguishing eligible behavior-preserving migrations from feature changes; require phases, down path, backfill safety, and separate destructive cleanup in the brief’s checks when applicable. No new record type or mechanical gate is justified initially. Rough effort: **trivial–moderate** (documentation/template text).

### X12 — REFINED [revalidated 2026-10-21]

**Vendor reality:** pstack’s overnight contract requires a checkable predicate, isolated worktree, one smallest change per iteration, real-artifact verification, commit/discard branching, and a decision row; pause/pickup adds a checkpoint and cold-start resume note (`vendors/cursor-plugins/pstack/docs/guide/07-overnight.md:18-56`; `skills/poteto-mode/playbooks/{pause-safely,session-pickup,autonomous-run}.md:1-18`). Its orchestration store adds durable units/frontier/ledger/decisions, but is prose/harness machinery rather than a portable record (`skills/poteto-mode/playbooks/orchestrate.md:21-40`). Matt’s Handoff packet names suggested skills, references artifacts instead of copying, and redacts secrets (`vendors/mattpocock-skills/skills/productivity/handoff/…

**Canon reality:** `Handoff` is a typed queue/state-machine record with `pending → in-progress → done|abandoned`, chain/parent/sequence, roles, and a domain-template body—not a continuation packet (`crates/canon-model/src/handoff.rs:295-345,398-433`; `canon/skills/reference/canon-vocab.md:88-108`). `canon dispatch` already persists a replayable `Run` manifest, guidance snapshot, task/parent edges, and terminal close (`crates/canon-cli/src/dispatch.rs:1-35,699-756,1000-1035`; `canon/skills/reference/canon-retrieve.md:80-93`). It has no decision journal or pause/resume checkpoint. Issue #54 is explicitly open/watch and says durable journaling is “not needed now.”

**Corrected claim:** Canon has typed handoffs and durable dispatch manifests, but neither is a generic long-run decision trail or privacy-aware continuation packet; #54 defers that until multi-process review creates demand.

**Concrete change/effort:** Now: add pointer-not-copy and redaction rules to handoff/orchestrator skill text (trivial). Later: add `dispatch pause|resume` with checkpoint/next-command/verified-claims fields and append-only decision rows, plus resume evals (significant; moderate for a recipe-only implementation).

[Some lines truncated to 768 chars. Use agent://RevX12:raw:3-3 to read line 3 whole]

### X13 — REFINED [revalidated 2026-10-21]

**Vendor reality:** Confirmed. pstack’s generator interviews the repository for surface, launch, drive, observation, and isolation, then writes executable Launch/Doctor/Drive/Evidence/Cleanup/Helpers instructions (`vendors/cursor-plugins/pstack/skills/create-verification-skill/SKILL.md:7-36`). It seeds a user-facing feature map and runs one mapped journey end to end, preserving evidence after cleanup (`.../SKILL.md:38-44`). Maintenance source-checks every feature and performs a live pass over all mapped features (`.../maintain-verification-skill/SKILL.md:7-36`).

**Canon reality:** The “missing” label is too absolute. Canon already has the workflow contract—scenario creation, test-run evidence, independent review, and gate transition (`canon/skills/SKILL.src.md:50-57`)—plus report/artifact binding and all-scenario evidence (`canon/skills/reference/role-implementer.md:54-81`), and explicitly recognizes long e2e re-runs (`canon/skills/reference/role-reviewer.md:104-106`). However, `canon skills install` only projects the shared Canon skill (`canon/skills/SKILL.src.md:137-153`); no repo-specific launch doctor, drive harness, cleanup recipe, or feature map is generated.

**Corrected claim:** Canon has scenario/evidence/e2e guidance, but lacks a generated, executable repository-verifier contract and maintained user-feature map.

**Concrete change:** Under open issue #20 (which requires 2–3 hand-used recipes first), add an execution-side recipe/generator—not a core record kind—that interviews the repo, writes Launch/Doctor/Drive/Evidence/Cleanup plus a feature map whose entries reference Canon scenario IDs, and proves one journey before handoff. Add maintenance guidance/eval for map drift and cleanup-preserved artifacts. Rough effort: significant.

### X14 — REFINED [revalidated 2026-10-21]

**Vendor reality:** Matt Pocock’s repo requires both a real observed failure and philosophical fit before an issue remains open (`vendors/mattpocock-skills/SCOPE.md:7-18`). Prior rejections are durable Markdown records with rationale and request history (`vendors/mattpocock-skills/.out-of-scope/new-skills.md:1-24`; `harness-name-collisions.md:1-27`), and unclear issues receive `needs-info` before timed closure (`SCOPE.md:33-35`).

**Canon reality:** The synthesis overstated the missing scope gate. Canon’s own backlog already uses the `watch` label as an admission bar and tracks `next`/`recipe` work through GitHub issues (`docs/research/vendor-audits/mattpocock-skills/00-synthesis.md:242-252`). Canon also has an explicit human approval boundary for each change (`canon/skills/SKILL.src.md:16-31`) and durable finding dispositions (`crates/canon-cli/src/finding.rs:171-200,329-335`). However, findings are defects in an adopted change, not intake decisions; there is no repository-local “decided no” record. The repo documents issue-linked decisions in release history (`README.md:193-215`), but has no `CONTRIBUTING.md` or issue-template policy in the checked tree.

**Corrected claim:** Canon has the observed-need/scope bar operationally through its issue labels and backlog; the smaller gap is a durable, discoverable convention for declined canon ideas and consuming-repo proposals.

**Concrete change / effort:** Add a closed-issue label plus a one-line decision entry/link in `README.md` or docs, and teach contributors to search it before reopening proposals. **Trivial.** Do not add a new record kind or `.canon/policy/` directory unless a repository needs mechanical intake enforcement.

### X15 — CONFIRMED [revalidated 2026-10-21]

**Vendor reality.** pstack has an explicit task-class playbook catalog covering investigation, bug fix, feature, autonomous run, orchestration, and related lifecycles (`vendors/cursor-plugins/pstack/skills/poteto-mode/SKILL.md:121-149`; audit `p-mode.md:145-190`). Its playbook contract preserves matched steps, including skipped steps with reasons (`vendors/.../poteto-mode/SKILL.md:121-125`). `poteto-help` is a separate non-executing router for choosing a skill or workflow (`vendors/.../poteto-help/SKILL.md:1-20`; audit `p-mode.md:3.6`).

**Canon reality.** The synthesis is right that Canon's `SKILL.src.md` routes primarily by role and CLI area, not by incoming request intent (`canon/skills/SKILL.src.md:156-169`). Planner guidance distinguishes feature work from chores, but does not provide a general intent-to-first-command registry (`canon/skills/reference/role-planner.md:75-87`). `canon status` and write-returned `meta.next` already provide authoritative, state-derived continuation commands (`crates/canon-cli/src/status.rs:1-40`; `crates/canon-cli/src/next.rs:1-16`). `canon schema` describes parser shape, effects, exits, roles, and examples, not user-intent routing (`crates/canon-cli/src/schema.rs:1-27,60-90,272-299`). Open issue #20 concerns recipes/playbooks, but no issue specifically imp…

**Corrected claim.** Canon has state/role/command routing, but lacks an explicit request-intent → workflow/reference → first-command table and non-executing workflow-help answer.

**Concrete change.** Add an eight-row table to `SKILL.src.md`, mirrored in generated onboarding. Optionally add read-only `canon help workflow`; do not alter `status` or `meta.next`. Effort is trivial for documentation, moderate for the command and eval coverage.

[Some lines truncated to 768 chars. Use agent://RevX15:raw:5-5 to read line 5 whole]

