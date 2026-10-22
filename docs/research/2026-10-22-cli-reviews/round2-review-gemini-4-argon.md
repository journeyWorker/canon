**Verdict: Ready after small fixes.** The 76-leaf normative table (`cli-commands-0.28.md`), the 3-part operand rule, `--rev` defaulting to `HEAD` with `codeTree` (D7b), and the Orca adoptions (D10) resolve all Round 1 blockers and eliminate every command-construction guess across the six agent flows. Before starting S0, fix three contradictions between `plan.md` D12 and the table rows (O13 `--override-reason` role access, O4/O6/O14 unapplied recommendations, and `--all-scenarios` single-root `--project-id` inference) and two T3/D3 mechanical collisions detailed below.

---

## 1. Round 1 Findings Status

| ID / Review | Finding | Status | Where Resolved / Why Not Fully Resolved |
| --- | --- | --- | --- |
| **Gemini B1** | `--commit` collision: boolean safety flag vs valued Git SHA option | **Resolved** | `plan.md` D6, D7b, Renames; `cli-commands-0.28.md` header & rows. `--commit` is exclusively the boolean switch on the 5 destructive leaves; commit options are `--rev` (default `HEAD`), `--resolution-rev`, and `--introduced-rev`. |
| **Gemini B2** | `CANON_ROLE` breaks `canon schema` / `context` when invalid or set | **Resolved** | `plan.md` D3 ("Unchanged by environment"), Renames; `cli-commands-0.28.md` (`schema`, `context`, `skills get`). Invalid `CANON_ROLE` is a warning on those leaves; filtering requires `--as <ROLE>`. |
| **Gemini B3** | Clean cutover of `retrieve --json` and `gate task` silently breaks installed repo hooks | **Resolved** | `plan.md` D8, D9; `cli-commands-0.28.md` (`retrieve`, `gate install-hooks`, `migrate`). Retrieve hook uses `(.data // .)`; `status`/`gate check`/`context` detect `< 0.28` locks/hooks and put `canon migrate` first in `meta.next`; `gate install-hooks` defaults `--command` to `canon task done`. |
| **Gemini B4** | `--input` document-to-argv breaks on `-` conflicts, relative paths, booleans, and batch errors | **Resolved** | `plan.md` D3, D7, D10, T16; `cli-commands-0.28.md` shared options. Specifies `--<kebab>=<value>` single-token serialization, boolean `false` → `--no-*` or omitted, `null` → `INPUT_INVALID_TYPE`, cwd-relative paths, `INPUT_STDIN_CONFLICT`, `INPUT_STDIN_TTY`, 8 MiB / 10,000 item caps (`INPUT_TOO_LARGE`), and pre-write validation with `error.field` pointers. |
| **Gemini S1** | Grammar violations: `artifacts move`, `subject status`, `report` modes, missing `-id`, `<FINDING_KEY>` | **Partly resolved** | `plan.md` D1, Renames; `cli-commands-0.28.md` (`artifacts move`, `subject move`, `report build\|check\|snapshot`, `--task-id`/`--plugin-id`/`--parent-run-id`, `finding close <CHANGE_ID> --round <N> --seq <N>`). **Remaining gap:** `skills get` uses `get`, which `plan.md` D1 still says is "not used" (see New Finding S3). |
| **Gemini S2** | Operands vs `--<noun>-id`: 12 commands have ambiguous primary objects | **Resolved** | `plan.md` D1 (3-part operand rule); `cli-commands-0.28.md` (applied to all 76 leaves, Resolved questions #1–15). |
| **Gemini S3** | Envelope & Exit gaps: `schemaVersion` format, `task done` partial data, `meta.context` paths, exit 1 vs 2 | **Resolved** | `plan.md` D3 (`schemaVersion` = `"1.0"`, relative `meta.context.repo`, `data` preserved on exit 1 checks incl. `task done`), D4 & T5 (`*_NOT_FOUND`, `*_UNREADABLE`, `*_INVALID_ARGUMENT` → exit 2). |
| **Gemini S4** | Safety set (D6): `effect: destructive` mismatches and `purge --kind` possibleValues | **Resolved** | `plan.md` Q1; `cli-commands-0.28.md` (`purge`, `artifacts move`, `migrate`, `ledger compact`, `tier age` all marked `destructive`; `purge --kind` restricted to `{session,run,event,trajectory}`). |
| **Gemini S5** | Spec & Tests T1–T16 gaps: undeclared env vars, per-leaf error codes, casing, hostile IDs, `--dry-run` parity | **Partly resolved** | `plan.md` D3 mandates `camelCase` keys across all reports; T16 tests values starting with `-`. **Remaining gap:** T1–T18 still do not assert that every `std::env::var("CANON_...")` in the workspace is in `spec.global.env`, nor that `--dry-run` matches real validation refusals (see New Finding N1). |
| **Gemini N1–N3** | Harness session env vars (N1), `trace_check.py` coupling (N2), `format` cleanup (N3) | **Partly resolved** | N2 resolved in `plan.md` D9 (`trace_check.py` reads `effect` from spec). N3 partly resolved (`fmt` alias dropped; O6 recommends dropping `--check` and making `[<DIR>]` optional, which D12 accepts, though the `format` row wasn't updated). N1 skipped by design. |
| **Reviewer B1** | Publish one complete, internally consistent grammar | **Partly resolved** | `cli-commands-0.28.md` provides the complete 76-leaf table. **Remaining gap:** D12 accepts O4, O6, and O14, but the table rows and `plan.md` Renames still reflect the pre-O3/O4/O6/O14 text, and D10 vs D12 conflict on O13 (see New Findings B1, B2). |
| **Reviewer B2** | Define one `schemaVersion` grammar | **Resolved** | `plan.md` D3; `cli-commands-0.28.md` Output header & Resolved #28 (`"<major>.<minor>"` per report, starting at `"1.0"`). |
| **Reviewer B3** | Make batch all-or-nothing enforceable | **Resolved** | `plan.md` D7, D10, T16. Pre-validates all items before first write (any validation error writes nothing); mid-batch write failure stops and reports `written`/`failed`/`skipped` in `data.items[]` with exit 1, and content-addressed idempotency makes re-running safe (`unchanged`). |
| **Reviewer B4** | Specify output variants per leaf | **Resolved** | `plan.md` D3; `cli-commands-0.28.md` Output column & Resolved #29 (explicit `report <Name>`, `artifact <media type>`, or `stream` per leaf, plus `--json` behavior). |
| **Reviewer S1** | Decide operands and requiredness per leaf; `--project-id` inference | **Partly resolved** | `plan.md` D1; `cli-commands-0.28.md` all 76 rows & Resolved #27. **Remaining gap:** `evidence add` row still says `--all-scenarios` "requires `--project-id`", contradicting Resolved #27 (see New Finding S1). |
| **Reviewer S2** | Make `--input` lossless and bounded | **Resolved** | `plan.md` D7, D10, T16; `cli-commands-0.28.md` shared options (`--<kebab>=<value>`, 8 MiB / 10k items, `INPUT_STDIN_CONFLICT`, `INPUT_STDIN_TTY`, `--commit`/`--plan-digest` forbidden in `--input`). |
| **Reviewer S3** | Close safety and refusal gaps | **Resolved** | `plan.md` D6, T11, T12; `cli-commands-0.28.md` (`effectNote` on every read with side effects, O7 no audit log on refusal, 5 destructive commands with `PLAN_CHANGED` exit 2). |
| **Reviewer S4** | Make errors and exits mechanical | **Partly resolved** | `plan.md` D3, D4, D10, T5. **Remaining gap:** `field` format has a slash-vs-bracket mismatch between D3 and D7 (`"/items/2/scenarioId"` vs `"items[2].scenarioId"`, see New Finding N2). |
| **Reviewer S5** | Strengthen clean-cutover recovery | **Resolved** | `plan.md` D8, D9 (`.canon/migrate-backup/<timestamp>/`, idempotent `canon migrate`, fixture test with 0.27 hooks/skill/scripts, warning + `meta.next` in `status`/`gate check`/`context`). |
| **Reviewer S6** | Expand T1–T16 | **Resolved** | `plan.md` T1–T18 (added T16 input/batch/retry, T17 commits/`codeTree`, T18 re-verification; T7 checks empty stderr; T5 checks exit 2 error classes). |

---

## 2. New Findings Introduced by Round 2 Revisions

### Blockers

#### B1. Direct contradiction on `--override-reason` between `plan.md` D10, `plan.md` D12, and `cli-commands-0.28.md`
- **Where:** `plan.md` D10 ("Human-only acts") vs `plan.md` D12 ("Per-leaf table decisions: O13 differs") vs `cli-commands-0.28.md` rows `subject move`, `evidence add`, and Open item O13.
- **Why:**
  - `plan.md` D10 states: *"`brief approve`, `policy approve`, `divergence accept` **and the overrides** refuse the agent roles with `retryable: false` and a hint addressed to the human"*.
  - `cli-commands-0.28.md` (`subject move` and `evidence add` rows, plus O13) states: *"`--override-reason` is human-only (D10)"*.
  - `plan.md` D12 states the exact opposite: *"O13 differs: `--override-reason` **stays available to agents** (an implementer attesting while a brief waits on the human is the documented use); every waived refusal is recorded, echoed in `data.overrides[]`, and listed by `status` for the human. The approvals themselves (`brief approve`, `policy approve`, `divergence accept`) refuse the agent roles (D10)."*
  - An implementer or test author cannot implement `evidence add --override-reason`, `evidence audit --override-reason`, or `subject move --override-reason` when D10 and the table say it refuses agent roles while D12 says it must allow them.
- **Fix:** Align `plan.md` D10 and `cli-commands-0.28.md` (`evidence add`, `evidence audit`, `subject move`, and O13) with owner decision D12: remove "and the overrides" from D10's human-only refusal list, and state in the three table rows that `--override-reason` is permitted for agent roles (`implementer` on `evidence add|audit`, `orchestrator` on `subject move`) and echoed in `data.overrides[]`.

#### B2. Owner decisions in D12 (O4, O6, O14) and O3 are not applied to the normative table rows or `plan.md` Renames
- **Where:** `plan.md` D12 and Renames table vs `cli-commands-0.28.md` rows `change new`, `format`, `strategy approval-payload`, `prompt add|show`, and Open items O3, O4, O6, O14.
- **Why:** `plan.md` D12 states: *"O1–O12, O14 and O15 are taken as recommended there."* However, `cli-commands-0.28.md` says *"the table assumes each recommendation"*, when in fact four rows in the table (and `plan.md`) still contain the **un-updated** designs:
  1. **O4 (`change new --kind` vs T3):** O4 recommends *"Rename `change new --kind` to `--change-kind`"* because `--kind` otherwise has two value domains and two meanings (record kind `<KIND>` vs brief template `{feature,chore}`), which fails T3 ("one name → one value name, arity and meaning"). Yet the `change new` row in `cli-commands-0.28.md` still specifies `--kind <KIND>{feature,chore}=feature`!
  2. **O6 (`canon format`):** O6 recommends *"Also drop `--check` and make `[<DIR>]` default to the configured spec roots, so `canon format` works bare like `gate check`."* Yet the `format` row in `cli-commands-0.28.md` still specifies `<DIR> — 1, required` and `--check (a no-op, kept...)`.
  3. **O14 (`strategy approval-payload`):** O14 recommends *"Add `--approval-at <TIMESTAMP>` (required) so both payload leaves are pure reads."* Yet the `strategy approval-payload` row in `cli-commands-0.28.md` lists only `--evaluation <FILE>; --approver <NAME>` under Required options and still says *"The payload embeds the current time, so the output is not repeatable"*.
  4. **O3 (`prompt --bundle-version`):** O3 notes that `prompt add|show` takes `<NAME@VERSION>` as an operand and recommends *"delete the `--bundle-version` row from the plan"*. Yet `plan.md` Renames still has `option | prompt --version | --bundle-version`.
- **Fix:** Update the `change new` (`--change-kind`), `format` (`[<DIR>]`, drop `--check`), and `strategy approval-payload` (add required `--approval-at <TIMESTAMP>`, change idempotency to `replay-safe`) rows in `cli-commands-0.28.md`, and delete the `prompt --version → --bundle-version` row from `plan.md` Renames.

---

### Should-Fix

#### S1. `evidence add --all-scenarios` row contradicts D1, D11, and Resolved #27 on `--project-id`
- **Where:** `cli-commands-0.28.md` (`evidence add` row vs Resolved question #27) and `plan.md` D11.
- **Why:**
  - `plan.md` D1 states: *"`--project-id` is required only when the repo configures more than one spec root, on every command that takes it"*.
  - `plan.md` D11 states that the re-verification hint is `evidence add --all-scenarios --report junit:a.xml --report junit:b.xml` (with no `--project-id`).
  - `cli-commands-0.28.md` Resolved #27 confirms: *"`--all-scenarios` follows the same rule."*
  - Yet the `evidence add` row in `cli-commands-0.28.md` ("Exclusive, dependent, notes") still says: *"it [`--all-scenarios`] requires `--project-id` and at least one `--report`"*. If implemented as written in the row, D11's canonical batch command will fail with exit 2 in single-root repos, and T14 will fail when parsing the D11 hint.
- **Fix:** In the `evidence add` row of `cli-commands-0.28.md`, change *"it requires `--project-id` and at least one `--report`"* to *"it requires at least one `--report` (`--project-id` is needed only when several spec roots exist)"*.

#### S2. Three option names (`--source`, `--snapshot`, `--role`) violate Test T3 ("one name → one value name, arity and meaning")
- **Where:** `cli-commands-0.28.md` (`skills *` vs `ingest plans`; `dashboard` vs `report snapshot`; `evidence/review/finding/divergence` vs `retrieve/dispatch begin`), `plan.md` T3.
- **Why:** Test T3 enforces *"one name → one value name, arity and meaning; vocabulary membership both ways"*. In the 0.28 table:
  1. **`--source` vs `--plan-source`:** `ingest plans --source` was renamed to `--plan-source <DIR>` (D1 Renames), while `skills install|check|doctor` kept `--source <DIR>`. Why rename one and not the other, or if `--source` is in the vocabulary, why was `ingest plans --source` renamed?
  2. **`--role` has two value sets and two meanings:** On attribution commands (`evidence add|audit`, `review scenario|change`, `finding add|close`, `divergence *`), `--role <ROLE>` is optional, defaults to `--as`, and on `evidence add` `possibleValues` in 0.27 was open/attribution, whereas on `dispatch begin` and `retrieve`, `--role <ROLE>` is a required scope selector with no `--as` default. T3's check for "one default and `possibleValues` for closed sets" will trip unless `--role`'s closed set `{planner,implementer,reviewer,orchestrator}` is identical everywhere and T3 allows per-command `required`/`default`.
  3. **`--fields <A,B>` has `<LIST>` vs `<A,B>` value-name mismatch:** In `cli-commands-0.28.md`, the header lists `<LIST>` as a vocabulary placeholder (`--providers <LIST>`), while `status`, `context`, `query`, `export`, `finding list`, and `divergence list` write `--fields <A,B>`. `<A,B>` is not in the placeholder list in `plan.md` D1 or the `cli-commands-0.28.md` header!
- **Fix:**
  1. Use `<LIST>` (or add `<FIELDS>` to the placeholder vocabulary) for `--fields` so T3's placeholder check passes.
  2. Ensure `--role <ROLE>` uses the exact same placeholder `<ROLE>` and closed set `{planner,implementer,reviewer,orchestrator}` across all leaves, and specify in T3 that `required` and `default` may vary per command while `valueName`, `arity`, and `possibleValues` (except `purge --kind` / `gate promote --kind` subsets) are invariant.

#### S3. `skills get` violates `plan.md` D1's verb list ("`update`, `get`, `delete` are not used")
- **Where:** `plan.md` D1 ("Verbs: ... `update`, `get`, `delete` are not used") vs `plan.md` D10 & `cli-commands-0.28.md` (`canon skills get <NAME> [--full]`).
- **Why:** Round 1 caught `artifacts move` violating D1's verb denylist, which was fixed by moving `move` into the allowed verb list. However, Round 2 added `canon skills get <NAME>` (copied from `orca skills get`), while leaving `get` in D1's explicitly banned list (`"update, get, delete are not used"`). Meanwhile, every other read-one-item command in canon uses `show` (`brief show`, `subject show`, `policy show`, `prompt show`, `context-pack show`, `regime show`).
- **Fix:** Either rename `canon skills get <NAME>` to `canon skills show <NAME>` (matching every other noun group in canon—note that Orca itself aliases `skills show` to `skills get` because agents guess `show`), **or** remove `get` from D1's banned verb list and define when `get` is used instead of `show`. (`skills show <NAME>` is strongly preferred for consistency with `prompt show`, `brief show`, `policy show`).

#### S4. `report snapshot <DIR>` stdout in text mode vs D3 artifact rule is underspecified
- **Where:** `plan.md` D3 ("Not wrapped: artifacts and streams...") and `cli-commands-0.28.md` (`report snapshot <DIR>` row).
- **Why:** Every other `artifact` leaf (`export` without `-o`, `evidence approval-payload`, `strategy approval-payload`, `skills get`, `completion`) writes raw artifact bytes to `stdout` in text mode and an envelope to `stdout` in `--json` mode. By contrast, `report snapshot <DIR>` writes 11 files (`10 <mart>.parquet` + `manifest.json`) into `<DIR>`. What does `canon report snapshot <DIR>` write to `stdout` when `--json` is **not** passed, and is `ReportSnapshotReport` a regular `report` on stdout? Marking `report snapshot` as `kind: artifact` in `outputs[]` when stdout is actually human summary text (or `ReportSnapshotReport` in `--json`) will confuse T6/T7.
- **Fix:** Classify `report snapshot <DIR>`'s stdout output as `report ReportSnapshotReport` (just like `export -o <FILE>` or `report build`, which also write files to disk and print a report on stdout), or state explicitly what text mode prints to stdout.

#### S5. `codeTree` computation in D7b/D11 needs explicit handling for `code.exclude` in git tree hashing
- **Where:** `plan.md` D7b ("computed from the working tree with a temporary index (tracked and untracked files, gitignored files and `.canon/` excluded; `git write-tree`)") and D11 ("What counts as code is `code.exclude` in `canon.yaml` (globs, default `docs/**`, `*.md`), applied to `codeTree` everywhere").
- **Why:** D7b says that for a clean commit made after a dirty working tree review, *"the commit made afterwards has the same code tree (computed the same way from the commit)"*. A standard git commit object's `tree` SHA includes `docs/**` and `*.md` (and `.canon/` if any files under `.canon/` are tracked, such as `.canon/changes/*/brief.md`, `.canon/policy.yaml`, `.canon/policy.approval.json`, or `.canon/specs/**`!).
  - If `codeTree` on a dirty worktree strips `.canon/` and `code.exclude` (`docs/**`, `*.md`), then computing `codeTree` on a commit MUST also read the commit's tree into a temporary index, remove `.canon/` and `code.exclude` paths (`git rm --cached`), and run `git write-tree`—it cannot simply use `git rev-parse <rev>^{tree}`.
  - Furthermore, does `codeTree` exclude `.canon/specs/**` (or configured `specs.roots[]`)? If `codeTree` includes `.canon/` exclusion, `.canon/specs/**` is excluded automatically, whereas a repo with `specs.roots` at `specs/` would include `specs/` in `codeTree` unless `specs.roots[]` are also excluded from `codeTree`!
- **Fix:** Specify in D7b/D11 that `codeTree` for both a working tree and a commit `<rev>` is computed by filtering out `.canon/`, all configured `specs.roots[]`, and `code.exclude` globs before `git write-tree`, so that (a) working-tree `codeTree` == post-commit `codeTree`, and (b) editing a `.feature` spec file or brief never changes `codeTree`.

---

### Notes

#### N1. T3/T5/T12 minor test gaps: `CANON_*` env var inventory and `--dry-run` refusal parity
- **Where:** `plan.md` Tests T3, T5, T12.
- **Why:** `plan.md` D2 says `canon.cli.json` contains "global options and env vars", and `cli-commands-0.28.md` mentions `CANON_ROLE`, `CANON_ACTOR`, and `CANON_SKILLS_SOURCE`, while `CANON_BLESS_CLI_SPEC` and `CANON_S3_*` exist in code. None of T1–T18 verifies that `spec.global.env` is complete. Similarly, T12 checks that refused writes leave the tree unchanged, but doesn't verify that `--dry-run` on a bad input refuses with the same error code as a real run.
- **Fix:** Add a check in T2/T3 that every `env::var("CANON_...")` in `crates/` is listed in `canon.cli.json` (or an internal-only allowlist), and a check in T12 that `--dry-run` runs full input validation.

#### N2. `error.field` batch pointer syntax differs between D3/D10 and D7
- **Where:** `plan.md` D3 (`"/items/2/scenarioId"`), `orca-cli-research.md` #8 (`"/items/2/scenarioId"`), vs `plan.md` D7 (`"items[<i>].<key>"`).
- **Why:** D3 says `error.field` is *"an argv name such as `--scenario-id`, or a JSON pointer into `--input` such as `/items/2/scenarioId`"*, whereas D7 says *"reports `error.field = \"items[<i>].<key>\"`"*.
- **Fix:** Pick one format (RFC 6901 JSON Pointer `"/items/2/scenarioId"` from D3 is standard) and use it in both D3, D7, and T16.

---

## 3. Agent Flows Walkthrough (0.28 Syntax)

Walking the 76-leaf table as an agent with only the generated skill and `cli-commands-0.28.md`:

### Flow 1: Write a subject with a brief and scenarios

```bash
# 1. Initialize canon in the current directory (non-interactive via --providers)
canon init --providers omp --language en --json

# 2. Create the subject (operand <SUBJECT_ID> per D1 r1; required --domain, --title)
canon --as planner subject new feature-login --domain web --title "User Login" --json

# 3. Create a feature change under the subject (operand <CHANGE_ID> per D1 r1)
# [NOTE: uses --kind feature per current table row, or --change-kind feature once B2/O4 is applied]
canon --as planner change new login --subject-id feature-login --title "Add user login" --json

# 4. Inspect and check the brief (optional operand [<CHANGE_ID>] per D1 r2; exit 1 = blockers)
canon --as planner brief show login --json
canon --as planner brief check login --json

# 5. Human approves the brief (human-only per D10: agent asks user to run this without --as)
canon brief approve login --approver alice --note "approved as written" --json

# 6. Scaffold scenarios in a single batch via --input (replaces --from per D7)
canon --as planner scenario new --input '[
  {"operands":["auth.login.01"],"title":"Sign in with valid credentials","subjectId":"feature-login","changeId":"login","case":"happy","flow":"F1"},
  {"operands":["auth.login.02"],"title":"Reject invalid password","subjectId":"feature-login","changeId":"login","case":"failure","flow":"F2"}
]' --json

# 7. Sync the spec inventory and transition subject to building
canon --as implementer inventory sync --json
canon --as orchestrator subject move feature-login --to specced --json
canon --as orchestrator subject move feature-login --to building --json
```
- **Guesses required:** **0** (once B2/O4 settles whether `change new` uses `--kind` or `--change-kind`; default is `feature` so omitting the flag works unambiguously today).

---

### Flow 2: Record evidence for several scenarios

```bash
# 1. Build a canonical regime key and begin a dispatched run
canon --as orchestrator regime show --role-segment implementer --repo-name my-repo --area auth --hash abcdef012345 --json
canon --as orchestrator dispatch begin --role implementer --regime implementer/my-repo/auth/abcdef012345 --actor-id impl-agent-1 --task-id login#1 --json

# 2. Record test-run evidence across all scenarios from a bound JUnit report
# (--rev defaults to HEAD and records codeTree + worktreeDirty per D7b; --role defaults from --as implementer)
# [NOTE: omitting --project-id in a 1-root repo per D1/D11/Resolved #27; fix S1 in the table row so this isn't refused]
canon --as implementer evidence add --all-scenarios --class test-run --ref "cargo test" --report junit:target/nextest/default/junit.xml --actor-id impl-agent-1 --session-id sess-impl-1 --run-id 01ARZ3NDEKTSV4RRFFQ69G5FAV --json

# 3. Or record evidence for multiple scenarios via an --input batch (when not using --all-scenarios)
canon --as implementer evidence add --input '[
  {"scenarioId":"auth.login.01","class":"test-run","ref":"cargo test login_01","verdict":"faithful","actorId":"impl-agent-1","sessionId":"sess-impl-1"},
  {"scenarioId":"auth.login.02","class":"test-run","ref":"cargo test login_02","verdict":"divergent","actorId":"impl-agent-1","sessionId":"sess-impl-1"}
]' --json

# 4. Record an external evaluator audit on a scenario (split command `evidence audit <SCENARIO_ID>`)
canon --as implementer evidence audit auth.login.01 --ref audit-report.json --evaluator-id eval-1 --evaluator-version 2.1 --evaluator-digest 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef --audit-result confirmed --actor-id impl-agent-1 --session-id sess-impl-1 --json

# 5. Promote staged evidence (if write.promote is manual) and mark the plan task done
canon --as implementer gate promote --kind evidence --json
canon --as implementer task done login#1 --json

# 6. End the dispatched run (operand <RUN_ID> per D1 r1; --outcome {succeeded,failed})
canon --as orchestrator dispatch end 01ARZ3NDEKTSV4RRFFQ69G5FAV --outcome succeeded --json
```
- **Guesses required:** **0** (assuming S1 is fixed so `--all-scenarios` infers `--project-id` when there is one spec root).

---

### Flow 3: Review scenarios and findings

```bash
# 1. Retrieve role-scoped guidance (enveloped in --json; data is the array)
canon --as reviewer retrieve --role reviewer --domain web --subject-id feature-login --limit 20 --json

# 2. Review scenarios in one call (variadic operand <SCENARIO_ID>...; --rev defaults to HEAD)
canon --as reviewer review scenario auth.login.01 auth.login.02 --actor-id rev-agent-1 --session-id sess-rev-1 --original-spec-ref .canon/changes/login/brief.md --json

# 3. Record a code-review finding (no operand per D1 r3; --role defaults to reviewer; --rev defaults to HEAD)
canon --as reviewer finding add --change-id login --round 1 --seq 1 --severity should-fix --actor-id rev-agent-1 --session-id sess-rev-1 --scenario-id auth.login.02 --summary "Missing rate-limit header on 401" --json

# 4. List open findings for the change
canon --as reviewer finding list --change-id login --open --json

# 5. Close the finding as fixed (operand <CHANGE_ID> + --round + --seq per D1 r2; --resolution-rev defaults to HEAD)
canon --as reviewer finding close login --round 1 --seq 1 --disposition fixed --actor-id rev-agent-1 --session-id sess-rev-1 --json

# 6. If auth.login.02 remains divergent and is an accepted known gap, ask the human to accept it (human-only per D10)
canon divergence accept auth.login.02 --actor-id alice --role reviewer --reason "Rate-limit header deferred to v2" --tracking https://example.com/issues/118 --expiry 2030-01-01 --json
```
- **Guesses required:** **0**.

---

### Flow 4: Ship a subject

```bash
# 1. Inspect the subject and move it to verifying
canon --as orchestrator subject show feature-login --json
canon --as orchestrator subject move feature-login --to verifying --json

# 2. Run the gate check (with --release if cutting a release)
canon --as orchestrator gate check --release --json

# 3. Transition the subject to shipped
canon --as orchestrator subject move feature-login --to shipped --json

# 4. Verify done criterion: gate check exits 0 with data.clean == true and subject show reports status == "shipped"
canon --as orchestrator gate check --json
canon --as orchestrator subject show feature-login --json
```
- **Guesses required:** **0**.

---

### Flow 5: Chore re-verification (D11)

```bash
# 1. Reopen the shipped subject to verifying (--reason is required for shipped -> verifying)
canon --as orchestrator subject move feature-login --to verifying --reason "Chore: refactor auth docs and helper" --json

# 2. Author and approve a chore change
canon --as planner change new login-cleanup --subject-id feature-login --kind chore --title "Clean up auth docs and helper" --json
canon brief approve login-cleanup --approver alice --note "chore approved" --json

# Case A: Docs-only chore (only files matching code.exclude, e.g. docs/** or *.md, changed)
# `codeTree` is unchanged from the last evidence on auth.login.01/.02, so no new scenario evidence is needed (D11).
# Only the change-level review of the chore brief is required:
canon --as reviewer review change login-cleanup --actor-id rev-agent-1 --session-id sess-rev-2 --original-spec-ref .canon/changes/login-cleanup/brief.md --json

# Case B: Code changed (codeTree changed)
# `canon status --json` puts the single batch command in meta.next[].argv (D11):
canon --as implementer evidence add --all-scenarios --class test-run --ref "cargo test" --report junit:target/nextest/default/junit.xml --actor-id impl-agent-1 --session-id sess-impl-2 --json
canon --as reviewer review scenario auth.login.01 auth.login.02 --actor-id rev-agent-1 --session-id sess-rev-2 --original-spec-ref .canon/changes/login/brief.md --json
canon --as reviewer review change login-cleanup --actor-id rev-agent-1 --session-id sess-rev-2 --original-spec-ref .canon/changes/login-cleanup/brief.md --json

# 3. Re-ship the subject
canon --as orchestrator subject move feature-login --to shipped --json
```
- **Guesses required:** **0**.

---

### Flow 6: Destructive `purge` (D6)

```bash
# 1. Run bare `canon purge` to preview the plan (exit 0; prints data.planDigest and data.effects[]; deletes nothing)
canon --as orchestrator purge --kind session --before 2026-01-01 --json

# 2. Read data.planDigest from step 1 and commit the exact plan (refused with PLAN_CHANGED exit 2 if state changed)
canon --as orchestrator purge --kind session --before 2026-01-01 --commit --plan-digest sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef --json
```
- **Guesses required:** **0**.