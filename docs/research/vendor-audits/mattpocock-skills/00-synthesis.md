# Vendor Audit — mattpocock-skills — Synthesis

Rollup of the 3 per-surface audits at `docs/research/vendor-audits/mattpocock-skills/`
(`m-engineering.md`, `m-productivity.md`, `m-meta.md`). Every claim cites a
source audit (`[<slug>.md §X.Y]`) plus the vendor `path:line`, or is prefixed
"Synthesis note:" with an explicit rationale. Vendor paths are relative to
`vendors/mattpocock-skills/` unless written in full; canon paths are relative
to the repository root. Vendor snapshot: `5beb35b` (shallow clone, read-only).

Canon's state was re-checked for this synthesis (not taken from the audits):
`canon/skills/SKILL.src.md`, `canon/skills/reference/*.md`,
`crates/canon-cli/src/{skills,init,brief_check,evidence}.rs`,
`crates/canon-cli/tests/`, `evals/dogfood/`, and `gh issue list --state open`
(open: #14–#20, #39, #54, #74–#76, #96). Each Top-10 entry carries a canon
status: **missing** / **partial** / **has** / **conflicts**.

## 0. Executive summary

The pack is a set of small prose skills, deliberately *not* a process. Its README
says why: frameworks that "own the process" "take away your control and make bugs
in the process hard to resolve" (`README.md:15-17`) [m-engineering.md §3.6].
There is no ledger, no approval record, no gate; the issue tracker, glossary and
ADRs are the substrate (`skills/engineering/wayfinder/SKILL.md:21-25`) and a new
skill is rejected if it "composes from existing skills"
(`.out-of-scope/new-skills.md:5-9`) [m-meta.md §3.1]. Canon is the opposite kind
of artifact. The most consequential findings:

1. **What the "don't own the process" design argues against canon.** Three
   charges are fair and one is already visible in canon's own tracker.
   (a) *Process bugs are hard to resolve.* Issue #96 is exactly this: adopting a
   not-yet-approved change into a live subject makes every scenario of that
   subject unrecordable (`BRIEF_UNAPPROVED`) and nothing warns at adoption. The
   vendor keeps planning artifacts (maps, decision tickets) non-gating, "plan,
   don't do" (`skills/engineering/wayfinder/SKILL.md:11-13`) [m-engineering.md §3.6].
   (b) *Every skill is permanent maintenance surface*
   (`.out-of-scope/new-skills.md:7`) [m-meta.md §3.1]; canon's reference corpus is
   26 files, 5,218 lines (Synthesis note: `wc -l canon/skills/reference/*.md`). New
   guidance must replace or sit behind a trigger, not accrete. (c) *Skill-land
   enforcement fails open*: the vendor's frozen git-guardrail hook skill collected
   bypass reports (`.out-of-scope/frozen-misc-skills.md:15-18`) [m-meta.md §3.6];
   canon's gate is the right place for enforcement, and any script it ships must
   say whether it can block (`canon/skills/reference/canon-retrieve.md:5` already
   does: it never blocks).
2. **Where canon should hold its line.** The vendor has nothing that replaces
   these, and its own flows show why: *independent review* (its `implement` calls
   `code-review` in the same session, `skills/engineering/implement/SKILL.md:15`,
   and `code-review` runs both axes as sub-agents of the caller,
   `skills/engineering/code-review/SKILL.md:11,60`, which is self-review under
   another label, the #14/#39 failure); the *human-approved, hash-pinned brief*
   (`implement` accepts "a spec or set of tickets" with no approval record,
   `skills/engineering/implement/SKILL.md:7`); *evidence bound by digest* (the
   `pr` skill's before/after evidence is prose in a PR body,
   `skills/engineering/pr/SKILL.md:19-22,158-164`) [Synthesis note: `pr` is not
   covered by any audit; read directly]; and *fail-closed refusals with stable
   codes* over advice. Conversely, canon should concede the tracker, ticket graph,
   glossary/ADR and commit-workflow layers to the repo's own systems (canon already
   does for `tasks.md`: "an optional work note nothing gates on",
   `canon/skills/reference/role-planner.md:34`) and keep execution-side playbooks
   user-owned (#20).
3. **Mechanical review quality is the largest transferable gain.** The vendor names
   the defect classes a passing test can hide (tautological, implementation-coupled,
   horizontal) and the criterion that a diagnosis starts from a loop that goes red
   on *this* bug (`skills/engineering/tdd/SKILL.md:30-32`;
   `skills/engineering/diagnosing-bugs/SKILL.md:57-66`) [m-engineering.md §3.3, §3.4].
   Canon's reviewer already has a "Check the checks" step
   (`canon/skills/reference/role-reviewer.md:71-77`) but covers only two of those
   classes, and canon's evidence is attribution, not execution
   (`crates/canon-cli/src/evidence.rs:69-78`) [m-engineering.md §3.4]. Top-10
   #1, #2, #4.
4. **One real defect surfaced by the retirement audit.** The vendor deletes
   retired skills and names the replacement (`skills/deprecated/README.md:1-4`)
   [m-meta.md §3.6]. Canon's `skills install` writes the current bundle and
   removes only proven legacy-Codex files (`crates/canon-cli/src/skills.rs:1421-1434`);
   `check` iterates only the current bundle (`skills.rs:1482-1520`). Synthesis note
   (read from code, not exercised): a reference file dropped from a later bundle
   would stay in `.claude|.agents|.omp|.pi/skills/canon/reference/` unreported, and
   an agent can still load it. Top-10 #3.
5. **Adopt the checks, not the human stops.** The vendor's seam confirmation
   (`skills/engineering/tdd/SKILL.md:22-24`) and ticket-granularity quiz
   (`skills/engineering/to-tickets/SKILL.md:50-56`) put a human inside the work.
   Canon's rule is one approval per scope and no stop inside it
   (`canon/skills/SKILL.src.md:34-44`). Every lift below routes the same content
   into the brief's Questions and assumptions or into a reviewer check.

## 1. Version frame

- **Artifact kind.** Almost entirely prose skills: 38 `SKILL.md` files, 26 of
  them user-invoked only (Synthesis note: `find skills -name SKILL.md`;
  `grep -l disable-model-invocation`). Executable material is seven small files
  (`scripts/{list-skills.sh,link-skills.sh,sync-plugin-version.mjs}`, a wizard
  template, a HITL loop template, a git-guard hook in the frozen `misc/` bucket,
  a dependency-cruiser config), and none of them gates agent work; the only
  check-mode script verifies package/plugin version agreement
  (`scripts/sync-plugin-version.mjs:1-44`) [m-meta.md §3.4]. Canon ships a CLI,
  typed records and a gate; the vendor ships advice. Drop-in surface is therefore
  skill text and eval method, not mechanisms.
- **Harness coupling.** Moderate and explicit: Claude Code frontmatter
  `disable-model-invocation`, Codex `policy.allow_implicit_invocation: false` in
  `agents/openai.yaml` (`skills/engineering/README.md:3-5`), the "Skill tool"
  (`skills/engineering/implement/SKILL.md:11,15`), sub-agent and worktree
  assumptions (`skills/engineering/implement-spec/SKILL.md:27-36`), optional
  `claude --bg` [m-productivity.md §4]. Canon projects one model-invoked skill to
  four providers and records no invocation metadata, so none of this transfers
  as mechanism; the *reasoning* about context versus cognitive load does
  (`skills/productivity/writing-for-agents/SKILL-MECHANICS.md:5-22`)
  [m-productivity.md §3.6].
- **License.** MIT, copyright 2026 the author (`LICENSE:1-3`; `package.json:9`)
  [m-meta.md §4]. Canon's posture stays clean-room reimplementation from the audit
  spec; license is not a ranking factor and matters only for a verbatim-lift PR.
- **Net implication.** Skill-text and eval-method lifts are drop-in (rewrite in
  canon's wording). Anything that implies state (task graph, scope records,
  changesets, teaching workspace) needs a design decision and is mostly deferred
  or declined below.

## 2. Top-10 patterns to lift (ranked by leverage × effort)

Ranking is for canon: a mechanical check or record shape that closes an observed
gap in the loop outranks advice canon would only restate.

- **#1 Name the ways a passing test proves nothing** — status: **partial**.
  Extend the reviewer's "Check the checks" with the vendor's defect classes:
  *tautological* (expected value recomputed the way the code computes it),
  *implementation-coupled* (mocks internals, asserts through a side channel,
  breaks on a behavior-preserving refactor), and *horizontal* (assertions written
  for imagined behavior); require the expected value to come from an independent
  source (literal, worked example, the scenario's Then). Source: [m-engineering.md
  §3.3] + `skills/engineering/tdd/SKILL.md:30-32`, `skills/engineering/tdd/tests.md:1-35`.
  Canon target: `canon/skills/reference/role-reviewer.md:71-77` (add the classes
  as the finding checklist), `canon/skills/reference/role-implementer.md` step 3
  (write expected values from the scenario, not from the code),
  `canon/skills/reference/canon-review.md` (finding text examples). The seam
  confirmation is *not* lifted as a human stop: the seam goes in the brief's
  Questions and assumptions or the scenario's Then steps. Effort: trivial.
  Rationale: canon's guard against green-but-empty evidence is exactly this step,
  and it currently names only "pins behavior the scenario does not require" and
  "skips a Then step".
- **#2 A fix starts from a red run of its own scenario** — status: **partial**.
  For every fix, the implementer runs the scenario's test before the change and
  keeps the failing report; a red-capable command asserts the user's exact symptom,
  is deterministic and agent-runnable. Source: [m-engineering.md §3.4] +
  `skills/engineering/diagnosing-bugs/SKILL.md:18-22,57-66`; the same before/after
  shape is `skills/engineering/pr/SKILL.md:21,164` [Synthesis note: `pr` read
  directly]. Canon target: `canon/skills/reference/role-implementer.md:40` ("Every
  bug fix adds or extends a scenario": add "and its test fails first; bind that
  failing report with `--artifact <scenario-id>=<path>` on the first evidence
  record") and a reviewer check in `role-reviewer.md` (open the red artifact, or
  `canon query --kind evidence --history`). Open design point: recording the red
  run as `--verdict divergent` would overload that verdict, which
  `role-implementer.md` defines as an external limit; prefer the artifact route.
  Eval method: a dogfood fixture where the "fix" test passes without a red run
  (`evals/dogfood/fixtures/`). Effort: trivial (text), moderate (eval fixture).
  Rationale: closes "evidence records a claim, never executes it" for the fix path
  without a new record kind.
- **#3 Prune retired files from projections and say so in `doctor`** — status:
  **partial**. On install, remove files the *previous* lock proves canon wrote
  (hash match) that the new bundle no longer contains; `check`/`doctor` report an
  unproven leftover as `orphan` and never delete it; release notes name the
  replacement for any retired reference. Source: [m-meta.md §3.6, §5.7, §5.9] +
  `skills/deprecated/README.md:1-4`, `.out-of-scope/frozen-misc-skills.md:3-11`;
  canon gap read at `crates/canon-cli/src/skills.rs:1421-1434` (only
  `legacy_codex_removals`) and `:1482-1520` (check iterates `files` only)
  [Synthesis note]. Canon target: `crates/canon-cli/src/skills.rs`
  (generalize the hash-proof removal already written for the legacy Codex
  projection), `canon skills doctor` output, a `skills.install.<nn>` scenario with
  a bundle that drops a file. Effort: moderate. Rationale: a stale reference is
  silent wrong guidance in the one place canon steers the agent; the mechanism
  and symlink protections already exist.
- **#4 Diff-level spec-fidelity pass, kept separate from standards** — status:
  **partial**. Add to the reviewer's "Read the PRD": read the pinned diff for
  (a) requirements missing or partial, (b) behavior in the diff nobody asked for,
  (c) code that looks implemented but is wrong, quoting the brief line per finding;
  report spec fidelity and repository standards as separate passes and never fold
  one into the other's severity. Source: [m-engineering.md §3.1, §5.4] +
  `skills/engineering/code-review/SKILL.md:68-72,78-89`. Canon target:
  `canon/skills/reference/role-reviewer.md:52-61` (today it checks scenarios
  against the brief, not the diff against it), `canon-review.md`. Canon already
  pins the commit (`--pin <sha>`), so the vendor's "fixed point resolves and the
  diff is non-empty" precondition (`code-review/SKILL.md:21-23`) is **has**. No new
  `--axis` field: canon's severities and `--scenario-id` binding carry it.
  Effort: trivial. Rationale: scope creep is currently reviewable only through
  scenarios, so unrequested behavior with no scenario is invisible.
- **#5 Brief durability: no file paths or line numbers, current versus desired
  behavior, independently verifiable acceptance** — status: **missing** for paths
  and current/desired, **has** for acceptance and out of scope. Source:
  [m-engineering.md §3.5, §5.5] + `skills/engineering/triage/AGENT-BRIEF.md:9-17,
  28-37,41-68`. Canon target: `canon/skills/reference/brief-harden.md` Inspect list
  (text first), then an advisory in `crates/canon-cli/src/brief_check.rs` beside the
  `prose.*` rules. Synthesis note: canon's brief is hash-pinned at approval and an
  edit needs a new human answer (`canon/skills/SKILL.src.md:30-31`), so a stale
  path in an approved brief is not harmless prose: fixing it re-opens the human
  stop. That is the extra reason canon needs this rule beyond the vendor's.
  Effort: trivial (text), moderate (advisory). Rationale: protects the one human
  approval from avoidable invalidation.
- **#6 Skill-text authoring rules, with a reachability check** — status:
  **partial**. Rules to write down: a pointer states its trigger branches, one
  trigger per branch; every step ends on a checkable and exhaustive completion
  criterion; one source of truth per meaning, and do not cache what `--help` or
  `canon schema` answers; delete a sentence that does not change behavior against
  the default. Source: [m-productivity.md §3.1, §3.2, §5.1] +
  `skills/productivity/writing-for-agents/SKILL.md:12-18,47-52,78-81`. Canon
  target: a short "authoring rules" section in `canon/skills/README.md`
  (Synthesis note: not `canon-authoring.md`, which the audit names but which is the
  `.feature` corpus guide, `canon/skills/reference/canon-authoring.md:1-8`), plus a
  check in `scripts/check-knowledge.py` that every `reference/*.md` is routed from
  `SKILL.src.md` or a role file. Canon's brief passes already end in a "Verify"
  section (`canon/skills/reference/brief-clarify.md`), so only the role references
  lack explicit completion criteria. Tension to keep: the vendor says do not cache
  lookups (`SKILL.md:79`), but canon's role files repeat full command lines on
  purpose to avoid `canon schema` round trips (0.18 "fewer calls",
  `canon/skills/SKILL.src.md:107-110`); keep that and apply the rule to prose only.
  Effort: trivial (text), moderate (check). Rationale: five thousand lines of
  reference is where sediment accrues.
- **#7 Cut units along flow steps; blocking order lives in `tasks.md`, not in a
  record** — status: **partial**. Guidance for step 4 "Units": a unit is one
  vertical slice (one flow step with its failure path), verifiable on its own and
  small enough for one fresh context; list "blocked by" in the optional
  `tasks.md`; for a wide mechanical refactor use expand, migrate in batches,
  contract. Source: [m-engineering.md §3.2, §5.3] +
  `skills/engineering/to-tickets/SKILL.md:27-40,65`,
  `skills/engineering/implement-spec/SKILL.md:13,34`. Canon target:
  `canon/skills/SKILL.src.md:53` (step 4 says only "Split the work"),
  `canon/skills/reference/role-orchestrator.md` ("Spawn each role"). A dependency
  graph as a ledger record **conflicts** with `role-planner.md:34` and
  `crates/canon-cli/src/change.rs:2-5` (flow coverage replaces a task checklist);
  do not build it. Effort: trivial. Rationale: the brief's flow steps are already
  vertical slices; the guidance costs two sentences.
- **#8 Settle "is this sentence a no-op" by running the document** — status:
  **partial** (eval harness exists, no ablation). Method: a sentence earns its
  place if removing it changes recorded agent behavior; two people disagreeing
  about a no-op disagree about the model's default, "settle it by running the
  document" (`skills/productivity/writing-for-agents/SKILL.md:81`). Source:
  [m-productivity.md §3.1 and §6 (lines 76-80 cited; the test is line 81)].
  Canon target: `evals/dogfood/trace_check.py` and its fixtures: add a paired-run
  comparison for a proposed deletion in `SKILL.src.md` or a role file (same task,
  with and without the sentence, judged by trace checks such as skipped review or
  premature approval). Pair with the vendor's *negation* warning
  (`SKILL.md:74`): canon's text is dense with "never"; keep prohibitions that a
  refusal code backs (`BRIEF_CHANGED_SINCE_APPROVAL`, `UNKNOWN_ROLE`) and rewrite
  the rest as the positive target (Synthesis note: judged by sampling
  `SKILL.src.md:15-48`, not measured). Effort: moderate. Rationale: it turns the
  recurring "trim the skill" argument into an experiment.
- **#9 Scope bar and rejection records, issue-first** — status: **partial**.
  Admission needs an observed failure from a real session *and* fit with prior
  decisions; each rejected concept is one file with rationale and prior requests
  (`SCOPE.md:7-12,33-35`; `.out-of-scope/new-skills.md:1-24`) [m-meta.md §3.1].
  Synthesis note: canon's own backlog already runs this bar through the `watch`
  label ("do not build until a real need or a runtime partner appears", checked
  with `gh issue list`; #14–#20, #54, #74–#76). What is missing is the durable
  "decided no" for consuming repos and for canon's own declined ideas. Recommended
  scope: a convention (closed issue label plus one line in `README.md`), **not** a
  new record kind or `.canon/policy/` directory; revisit only if a repo asks to
  gate proposals mechanically. Effort: trivial (convention), significant (record
  kind: not recommended). Rationale: keeps the backlog honest without a second
  policy layer.
- **#10 Continuation packet: reuse the Handoff record, add the two rules** —
  status: **partial**. Rules from the vendor: name the suggested next skills,
  reference existing artifacts instead of copying them, redact secrets
  (`skills/productivity/handoff/SKILL.md:8-14`) [m-productivity.md §3.4, §5.3].
  Canon already has typed handoff bodies (`handoff-dev|design|content|test` with
  `verification-steps`, `canon/skills/reference/canon-vocab.md:88-110`) and
  ingested sessions (`canon-session-ingest.md`). Canon target: a sentence in the
  `canon-vocab.md` handoff section and `role-orchestrator.md` ("Spawn each role")
  on pointer-not-copy and redaction. Do not adopt the vendor's storage (a
  `$TMPDIR` file outside the workspace, `handoff/SKILL.md:8`): it is untracked and
  would become the second source of truth the audit warns about. Effort: trivial.
  Rationale: cheap, and it keeps handoffs inside the ledger.

## 3. Runners-up & deferred extracts

- **Decision-ready stop report ("push right")**: do maximum preparation, then ask
  once with a tight brief (`skills/in-progress/loop-me/SKILL.md:22-23`)
  [m-productivity.md §3.3]. Canon **has** most of it
  (`canon/skills/reference/brief-clarify.md`: "questions a human can answer in one
  reply"); add one line to `role-planner.md` step 4 that each open question carries
  a proposed default. Trivial.
- **Merge-danger call in a PR body** (one-way or two-way door, blast radius,
  `skills/engineering/pr/SKILL.md:166-170`). Synthesis note: no audit covers `pr`.
  Candidate for the chore brief's invariants (an irreversible chore step) rather
  than a new field; defer until a repo asks.
- **Classify a recurring review miss before writing a rule**: a mechanical
  violation gets a deterministic check, a judgement call gets standards prose
  (`skills/engineering/retro/SKILL.md:19,29-35`). Synthesis note: not audited;
  matches canon's "honest gates" direction (0.22). Use it as the triage rule when a
  finding class recurs (brief-check rule or gate rule versus skill text).
- **Retro as a lens over ingested sessions** (navigation, automated checks, tool
  economy, information access; `retro/SKILL.md:15-23`). Synthesis note: canon
  already ingests transcripts (`canon-session-ingest.md`); a read-only report over
  them is plausible but is an eval-tooling idea, issue-only.
- **Glossary with "avoid" terms** [m-meta.md §3.2, §5.2]: partially **has**.
  `canon/skills/reference/canon-authoring.md` ("Vocabulary", term / is / is NOT)
  already uses the pattern for area, surface and scenario;
  `crates/canon-cli/tests/one_name.rs:1-50` enforces one flag spelling mechanically
  and `evals/dogfood/trace_check.py` flags deprecated spellings in real runs
  (fixture `naming_failures.jsonl`). A static lint that skill text only uses
  current flags is the open piece; low leverage while trace checks exist. Note
  `canon-vocab.md` is typed-vocabulary authoring, not a glossary
  (`canon-vocab.md:1-8`), so the audit's target there is a poor fit.
- **Two-axis review as a record field** [m-engineering.md §5.4]: skip the field,
  keep the discipline (#4).
- **Promotion manifest and version sync** [m-meta.md §3.4]: canon ships one skill
  and the install lock already records canon version and source commit
  (`crates/canon-cli/src/skills.rs:16-20`); nothing to promote. Defer.
- **Changesets as release-intent records** [m-meta.md §3.5, §5.6]: real value is
  machine-checkable release-note completeness, but it adds a per-change artifact to
  a one-binary release flow. Issue-only (`watch`).
- **Single install narrative across routes** [m-meta.md §3.3]: install prose is
  repeated in `README.md`, `canon/skills/README.md`, `init.rs`; trivial dedupe,
  low priority.
- **Primary-source research artifacts** [m-engineering.md §3.7,
  `skills/engineering/research/SKILL.md:3-12`]: planner guidance at most; no record.
- **Skill description cost**: always-loaded text is paid every turn
  (`writing-for-agents/SKILL.md:22-27`) [m-productivity.md §3.6]. Canon's
  `description` is long (`canon/skills/SKILL.src.md:3`) but is tuned to beat
  generic builder skills; measure with #8 before trimming.
- **Mapped triage labels** [m-meta.md §3.7]: belongs with #20 recipes; shared
  skill plus a repo-local mapping file (`docs/agents/triage-labels.md:3-15`) is the
  right layering for any tracker adapter.
- **Do not lift (conflicts or out of domain)**: tracker as source of truth
  [m-engineering.md §3.6; `crates/canon-cli/src/brief.rs:28-35`]; seam and
  granularity confirmation as human stops (see §0 item 5); a mandatory commit step
  (`implement/SKILL.md:17`) [m-engineering.md §5.8]; the human-only wizard
  [m-engineering.md §5.9]; the teaching workspace and learner records
  [m-productivity.md §3.5, §5.4] (human-learning state is not canon's domain and
  would blur `canon-learn` promotion evidence); `link-skills.sh` symlinks and
  editable copies (`.out-of-scope/frozen-misc-skills.md:9`) [m-meta.md §5.4],
  because canon's projection is a read-only copy with drift checks.

## 4. Critical overlaps with canon's in-flight work

### #14 session identity, #39 self-review without a reviewer session, #54 run journal

Synthesis across [m-engineering.md §3.1, §3.6] and [m-productivity.md §3.4]. The
vendor never has an independent reviewer: `implement` ends by calling `code-review`
(`skills/engineering/implement/SKILL.md:15`), and that skill spawns its two axes
as sub-agents of the caller (`skills/engineering/code-review/SKILL.md:11,60`). It
also states that stopping runaway sub-agents is the harness's job, not the
skill's (`.out-of-scope/subagent-recursion.md:5-9`). Learning for canon: (1) no
vendor mechanism to lift; the problem is generic and canon's `watch/recipe`
posture on #14/#39 matches the vendor's reasoning that session facts belong to the
harness. (2) Hold the wording in `role-reviewer.md:44-50` ("never the session that
wrote the code under another label"). (3) For #54, the vendor's multi-agent shape
is git worktrees, one integration branch, a merger sub-agent and the tracker's
ready frontier, with no journal or lease
(`skills/engineering/implement-spec/SKILL.md:27-40`): coordination by existing
state is enough until review spans runner processes, which supports leaving #54 on
watch.

### #15 which skill a run used, #16 tool-operation decisions, #19 run monitoring

Synthesis across [m-productivity.md §3.6, §3.3] and [m-meta.md §3.6]. The vendor's
invocation split decides *how* a skill gets reached (user types it, or model
fires it; `writing-for-agents/SKILL-MECHANICS.md:5-22`), so a #15 record that
only stores "skill X ran" loses the distinction that matters for audit: user-typed
versus model-invoked. Its chains of "call the Skill tool with `tdd`"
(`implement/SKILL.md:11,15`) leave tool events in a transcript, so some of #15 may
be recoverable from ingested sessions rather than from a harness hook
([INFERENCE]: not checked against `canon-session-ingest.md`'s event fields).
For #16, the vendor's frozen guard hook shows enforcement written as a skill
script failing open (`.out-of-scope/frozen-misc-skills.md:15-18`): keep canon at
"record what a runtime reports" and, for any script it ships, state whether it
can block. #19's trigger / checkpoint / brief vocabulary
(`loop-me/SKILL.md:10-23`) maps onto `actor: human` steps in `meta.next`; no new
mechanism.

### #20 recipes and `canon add`

Synthesis across [m-meta.md §3.1, §3.7], [m-engineering.md §3.6]. The vendor is
effectively a recipe set owned by the user: skills are copied (editable) rather
than depended on, a new skill is declined when it composes from existing ones
(`.out-of-scope/new-skills.md:5-9`), and repo-specific facts (tracker, labels,
glossary layout) live in a mapping file the setup skill writes without touching
the shared skill (`skills/engineering/setup-matt-pocock-skills/SKILL.md:22-36`).
Learning: this is the shadcn-style evidence #20 is waiting for, and it supports
#20's own gate ("build only after 2–3 recipes have been used by hand"). If recipes
arrive, copy the shape "shared skill + repo-local mapping", and apply the
observed-failure bar (#9) to what becomes a recipe.

### #96 adopting a draft change freezes evidence for existing scenarios

Synthesis across [m-engineering.md §3.6] and [m-meta.md §3.7]. This is the
vendor's central objection made concrete: a process-owned artifact (an unapproved
brief) gates unrelated work. The vendor keeps planning non-gating
(`wayfinder/SKILL.md:11-13`; tickets and specs are tracker items that do not block
`implement` on other tickets, `to-tickets/SKILL.md:65`). Learning, without
prescribing the fix: the issue's first option (scope the approval requirement to
the scenarios the change adds or modifies) is the one consistent with "planning
must not gate maintenance"; whichever option ships, `change new` / `subject adopt`
into a `building|verifying|shipped` subject should state the consequence at the
time (the issue reports that only a trailing hint mentions it). Treat any new
gating artifact in this audit's lifts (none of the Top-10 adds one) as needing the
same adoption-time warning.

### #17 spec and code divergence, #18 review scoreboard

Synthesis across [m-engineering.md §3.5, §3.1]. #17: path-free briefs (Top-10 #5)
leave nothing at the brief level for a code-dependency graph to stale; staleness
stays at scenario level, which canon already pins (`role-reviewer.md` pin
paragraph). #18: the spec versus standards separation (#4) gives a stratification
key for a later scoreboard, but #18's blocker (no population) is unchanged.

### #74, #75, #76

No vendor evidence bears on origin attribution, capture-policy recording or
workflow lockfiles; nothing to add.

### Audit corrections found during synthesis

- [m-engineering.md §5.1] recommends extracting "check the checks"; canon already
  has the step (`canon/skills/reference/role-reviewer.md:71-77`). The real gap is
  the named defect classes and the independent expected value (Top-10 #1).
- [m-productivity.md §5.1, §5.5] target `canon-authoring.md` for skill-authoring
  rules; that file guides the `.feature` corpus
  (`canon/skills/reference/canon-authoring.md:1-8`). Target is
  `canon/skills/README.md` (Top-10 #6).
- [m-meta.md §3.6] says canon lacks a bundle-level retirement policy; the stronger
  finding is the missing orphan handling in install/check (Top-10 #3).
- [m-engineering.md §3.1] frames the review axes as missing structure; canon's
  gap is the diff-level read, not a field (Top-10 #4).

## 5. Per-canon-surface action plan

### `canon/skills/SKILL.src.md` and `canon/skills/README.md`

1. **EXTRACT** unit-slicing guidance for step 4 (one flow step per unit; blocked-by
   in `tasks.md`) → `SKILL.src.md:53` — [m-engineering.md §3.2] — trivial.
2. **EXTRACT** authoring rules (pointer triggers, completion criteria, single
   source, no-op test) → `canon/skills/README.md` — [m-productivity.md §3.1, §3.2]
   — trivial.
3. **INSPIRE** positive-target rewrite of non-guardrail prohibitions, validated by
   paired runs → `SKILL.src.md` — [m-productivity.md §3.1] +
   `writing-for-agents/SKILL.md:74,81` — moderate.

### Role references (`role-planner|implementer|reviewer|orchestrator.md`)

1. **EXTRACT** tautological / implementation-coupled / horizontal checklist →
   `role-reviewer.md:71-77`, `role-implementer.md` step 3 — [m-engineering.md
   §3.3] — trivial.
2. **EXTRACT** red-first run for fixes, bound as an artifact →
   `role-implementer.md:40`, `role-reviewer.md` — [m-engineering.md §3.4] —
   trivial.
3. **EXTRACT** diff-level spec-fidelity read, separate from standards →
   `role-reviewer.md:52-61` — [m-engineering.md §3.1] — trivial.
4. **INSPIRE** proposed default per open question in the human-stop report →
   `role-planner.md` step 4 — [m-productivity.md §3.3] — trivial.
5. **INSPIRE** pointer-not-copy and redaction rule for handoffs →
   `role-orchestrator.md` "Spawn each role" — [m-productivity.md §3.4] — trivial.

### Brief passes and `crates/canon-cli/src/brief_check.rs`

1. **EXTRACT** no paths or line numbers, current versus desired behavior →
   `brief-harden.md` Inspect — [m-engineering.md §3.5] — trivial.
2. **INSPIRE** an advisory rule for file-path or line-number references in
   normative sections → `brief_check.rs` (`prose.*` family) — [m-engineering.md
   §3.5] — moderate.

### CLI: `canon skills install|check|doctor` (`crates/canon-cli/src/skills.rs`)

1. **EXTRACT** hash-proven pruning of files the previous lock wrote and the new
   bundle drops; `orphan` state for unproven leftovers in `check`/`doctor` →
   `skills.rs:1421-1434,1482-1520` — [m-meta.md §3.6, §5.9] — moderate.
2. **INSPIRE** release-note rule: a retired reference names its replacement →
   `README.md` release notes — [m-meta.md §5.7] — trivial.

### Evals (`evals/dogfood/`)

1. **INSPIRE** planted-defect fixtures (tautological test; fix without a red run)
   that the reviewer must catch → `evals/dogfood/fixtures/` — [m-engineering.md
   §3.3, §3.4] — moderate.
2. **INSPIRE** paired-run ablation for skill-text deletions →
   `evals/dogfood/trace_check.py` — [m-productivity.md §3.1] — moderate.

### `crates/canon-cli/src/init.rs` (AGENTS block)

1. **INSPIRE** keep the block short and pointer-only; the loop is already
   duplicated between `init.rs:211-` and `SKILL.src.md:13-57`, held together by
   phrase assertions (`crates/canon-cli/tests/skills_install.rs:722-760`)
   [Synthesis note; m-productivity.md §3.1 single-source rule]. Any new rule from
   this audit goes in the skill, not the block — trivial.

### Repository backlog (`gh issue`, `README.md`)

1. **INSPIRE** closed-issue convention for declined ideas, referenced from
   `README.md`; no new record kind → [m-meta.md §3.1] — trivial.
2. **INSPIRE** comment on #96 with the "planning must not gate maintenance"
   learning and on #20 with the "shared skill + repo-local mapping" shape —
   [m-engineering.md §3.6; m-meta.md §3.7] — trivial.

## 6. Risk register

1. **Lifting human confirmation steps** (seam sign-off, ticket quiz) breaks
   canon's one-approval rule — medium. Surfaces: role references, `SKILL.src.md`.
   Mitigation: route the content into the brief's Questions and assumptions or a
   reviewer check [m-engineering.md §3.2, §3.3].
2. **Reference-corpus bloat**: every added rule is permanent maintenance, and the
   corpus is already 5,218 lines — high. Surfaces: all references. Mitigation: each
   addition replaces text or sits behind a trigger; gate additions on an observed
   failure (#9) and test deletions with #8 [m-meta.md §3.1;
   m-productivity.md §3.1].
3. **Reviewer-judgement rules cannot be asserted mechanically**, so #1, #4 decay
   unless exercised — medium. Mitigation: planted-defect dogfood fixtures; keep the
   phrase assertions in `skills_install.rs` in step.
4. **Red-run evidence overloads `divergent`** or invents a verdict — medium.
   Surfaces: `evidence.rs`, `role-implementer.md`. Mitigation: artifact route
   only; decide before any record-shape change [m-engineering.md §3.4].
5. **Orphan pruning deletes a user file** — high if done loosely. Surfaces:
   `skills.rs`. Mitigation: remove only on exact recorded-hash match, report
   anything else, keep the existing symlink protections
   (`skills.rs:1417-1419`).
6. **Scope records become a second policy layer** (explicit over-engineering risk)
   — medium. Mitigation: issue-first; no new record kind or `.canon/` directory
   without a consuming repo asking [m-meta.md §3.1].
7. **A process-owned artifact gates unrelated work** (#96 class) — high.
   Mitigation: adoption-time consequence message; prefer non-gating draft state
   for anything planning-only [m-engineering.md §3.6].
8. **Audit inaccuracies copied forward** (items in §4 "Audit corrections") —
   low–medium. Mitigation: use this synthesis's targets, not the audit's.
9. **Vendor volatility**: `in-progress/` is experimental and `misc/` is frozen
   (`.out-of-scope/frozen-misc-skills.md:3-9`) [m-meta.md §3.6]; cite a pinned
   commit and do not derive canon rules from `in-progress/` alone — low.
10. **Harness-specific mechanics** (`disable-model-invocation`, `openai.yaml`,
    Skill tool, `claude --bg`) do not exist across canon's four providers — low.
    Mitigation: lift reasoning, not mechanism [m-productivity.md §4].
11. **License**: MIT; clean-room posture removes it as a factor — low.

## 7. Recommended next 3 PRs

Each follows canon's own loop in this repository (brief, `skills.install.<nn>`-style
scenarios, evidence, independent review).

1. **Reviewer: name the ways a passing test proves nothing, and read the diff.**
   - Scope: `canon/skills/reference/role-reviewer.md` (lines 52-61, 71-77),
     `canon/skills/reference/role-implementer.md` (step 3),
     `canon/skills/reference/canon-review.md`; a phrase assertion beside
     `crates/canon-cli/tests/skills_install.rs:722-760`; one planted-defect
     fixture (a tautological test) under `evals/dogfood/fixtures/` with a
     `trace_check.py` expectation that a finding is recorded before `review add`.
   - Sources: [m-engineering.md §3.1, §3.3] (Top-10 #1, #4).
   - Done when: the reviewer reference lists tautological, implementation-coupled
     and horizontal tests as finding grounds and states the independent-expected-
     value rule; the reviewer reads the pinned diff for missing, unrequested and
     wrong behavior, quoting the brief line; the fixture run records a finding
     before sign-off; `canon skills check` is clean after reinstall; no new CLI
     field or human stop is added.
2. **Fixes start red.**
   - Scope: `canon/skills/reference/role-implementer.md:40`,
     `canon/skills/reference/role-reviewer.md`,
     `canon/skills/reference/brief-harden.md` (current versus desired behavior, no
     paths or line numbers), optionally the `brief_check.rs` advisory as a
     separate commit; a dogfood fixture where a fix test passes with no red run.
   - Sources: [m-engineering.md §3.4, §3.5] (Top-10 #2, #5).
   - Done when: the implementer text requires the failing report to be bound as an
     artifact on the first evidence record for a fix; the reviewer checks it via the
     artifact or `canon query --kind evidence --history`; no new verdict value;
     the brief pass names the path/line rule (and the advisory, if shipped, has a
     closed rule id and a test that waivers still work).
3. **`skills install` prunes what a bundle no longer ships.**
   - Scope: `crates/canon-cli/src/skills.rs` (`install_canonical`, `check`,
     `doctor`), `crates/canon-cli/tests/skills_install.rs`, README release note
     naming the behavior.
   - Sources: [m-meta.md §3.6, §5.7, §5.9] (Top-10 #3).
   - Done when: a bundle that drops `reference/x.md` removes the projected file on
     the next install only when its bytes match the previous lock's recorded hash;
     a modified leftover is reported as `orphan` by `check` and `doctor` and kept;
     symlink protections hold; the install lock lists no dropped file; a scenario
     covers drop, modified leftover and an unselected provider.
