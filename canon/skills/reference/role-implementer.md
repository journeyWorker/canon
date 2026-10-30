# Role: implementer

You build inside an approved scope: scenarios, code, tests and evidence.
Declare the role with `CANON_ROLE=implementer` (or `--as implementer`); `--role`
on what you record then defaults to `implementer`.
Build nothing before the brief is approved: `canon evidence add` refuses an
unapproved brief's scenarios. If it is not approved, say so and wait; the
planner and the human own that. The only waiver is `--override-reason
"<why>"`, for the case where you were told to attest while the brief waits for
the human's answer: canon records it on the evidence, echoes it in
`data.overrides[]` and lists it in `status` for the human. Never use it to
skip the wait yourself. `brief approve`, `policy approve` and `divergence
accept` are the human's; they refuse you (`HUMAN_ONLY`) unless they record an
answer the human gave in the conversation (`--relayed-via`, the planner's
procedure: `canon skills show role-planner`). You never run them without that
answer, and never remove or unset your role (`env -u CANON_ROLE`, dropping
`--as`) to get past the refusal.

## Commands

Every leaf of this role is below with its full syntax line (every option the leaf
has) and an example generated from the CLI's own spec, which runs as written against
the example story (subject `demo`, change `add-login`, scenario `auth.login.01`): swap
in your ids. Use only the options on a line; never guess one. Ask `canon schema
<command> --json` only for a leaf this block does not list, or after a `USAGE` refusal.

<!-- canon:generated commands-implementer begin -->
Every leaf takes `--as <ROLE>` and `--json`; `--repo <DIR>` too, except `completion`, `schema`, `skills show`. `--input <JSON|FILE|->` runs a leaf from one JSON document (an array is a batch) and is only on the lines ending `[--input]`. Its document keys are the option names in camelCase, singular even when the option repeats (the value is then an array), and `operands` holds the operands. `--fields <FIELDS>` is a comma list; camelCase and snake_case both match.
In a syntax line `<X>` is a value, `[x]` optional, `a | b` a choice, `(a | b)` exactly one is required, `[--x <V>]*` may repeat, `<X>+` is one or more, `[--a --b <V>]` go together. An option that is not on the line does not exist: it is refused (`USAGE`).

```text
# Write the shell completion script for canon (bash, zsh or fish)
canon completion <SHELL>
```
```bash
canon completion zsh
```
```text
# Show what you can author here: record kinds, fields, enums, policies
canon context [--fields <FIELDS>]
```
```bash
canon context
```
```text
# Create an immutable context pack from a repository-local JSON manifest
canon context-pack add --manifest <FILE> [--input]
```
```bash
canon context-pack add --manifest .canon/context-manifest.json
```
```text
# Show a context pack after verifying its manifest and content objects
canon context-pack show <PACK_ID>
```
```bash
canon context-pack show sha256:0000000000000000000000000000000000000000000000000000000000000000
```
```text
# Verify a context pack's manifest and immutable content objects
canon context-pack verify <PACK_ID>
```
```bash
canon context-pack verify sha256:0000000000000000000000000000000000000000000000000000000000000000
```
```text
# Add a divergence candidate, staged with no run_seq yet
canon divergence add <SCENARIO_ID> --actor <ACTOR_ID> [--project <PROJECT_ID>] [--rev <REV>] [--state <open|still-divergent|resolved|deferred>] [--reason <TEXT>] [--expiry <TIMESTAMP>] [--round <N>] [--session <SESSION_ID>] [--detail <TEXT>] [--role <ROLE>] [--promote | --no-promote] [--input]
```
```bash
canon divergence add auth.login.01 --actor alice --role reviewer
```
```text
# Directly record a deferred divergence (requires --reason/--expiry)
canon divergence defer <SCENARIO_ID> --actor <ACTOR_ID> --reason <TEXT> --expiry <TIMESTAMP> [--project <PROJECT_ID>] [--rev <REV>] [--round <N>] [--session <SESSION_ID>] [--role <ROLE>] [--input]
```
```bash
canon divergence defer auth.login.01 --actor alice --reason "waiting on the vendor" --expiry 2030-01-01T00:00:00Z --role reviewer
```
```text
# List the current divergence burn-down state
canon divergence list [--as-of <TIMESTAMP>] [--limit <N> | --all] [--cursor <CURSOR>] [--fields <FIELDS>]
```
```bash
canon divergence list --as-of 2030-01-01
```
```text
# Directly record a resolved divergence
canon divergence resolve <SCENARIO_ID> --actor <ACTOR_ID> [--project <PROJECT_ID>] [--rev <REV>] [--round <N>] [--session <SESSION_ID>] [--detail <TEXT>] [--role <ROLE>] [--input]
```
```bash
canon divergence resolve auth.login.01 --actor alice --role reviewer
```
```text
# Record one attributed EvidenceRecord for a task or scenario
canon evidence add [--task <TASK_ID>] [--project <PROJECT_ID>] [--class <CLASS>] [--ref <TEXT>] [--verdict <faithful|not-applicable|divergent>] [--summary <TEXT>] [--command-result <TEXT>] [--scenario <SCENARIO_ID>] [--run <RUN_ID>] [--surface-ref <PATH_OR_EFFECT>]* [--actor <ACTOR_ID>] [--role <ROLE>] [--session <SESSION_ID>] [--approver <NAME>] [--approval-role <human>] [--signature-file <FILE>] [--approval-at <TIMESTAMP>] [--rev <REV>] [--artifact <PATH>]* [--report <FORMAT:PATH>]* [--report-case <NAME>]* [--all-scenarios] [--max-artifact-mib <N>] [--override-reason <TEXT>] [--promote | --no-promote] [--input]
```
```bash
canon evidence add --scenario auth.login.01 --class test-run --ref "npm test" --role implementer
```
```text
# Emit exact bytes for external `ssh-keygen -Y sign -n canon-approval-v1`
canon evidence approval-payload --approver <NAME> --approval-at <TIMESTAMP> [--task <TASK_ID>] [--project <PROJECT_ID>] [--scenario <SCENARIO_ID>] [--run <RUN_ID>] [--rev <REV>] [--surface-ref <PATH_OR_EFFECT>]*
```
```bash
canon evidence approval-payload --scenario auth.login.01 --project root --approver alice --approval-at 2030-01-01T00:00:00Z
```
```text
# Record an external evaluator's re-audit of one scenario (class `audit`; the verdict follows --audit-result; canon never runs the evaluator)
canon evidence audit <SCENARIO_ID> --ref <TEXT> --evaluator <ID> --evaluator-version <TEXT> --evaluator-digest <DIGEST> --audit-result <confirmed|refuted|indeterminate> [--project <PROJECT_ID>] [--summary <TEXT>] [--command-result <TEXT>] [--run <RUN_ID>] [--actor <ACTOR_ID>] [--role <ROLE>] [--session <SESSION_ID>] [--rev <REV>] [--artifact <PATH>]* [--report <FORMAT:PATH>]* [--report-case <NAME>]* [--max-artifact-mib <N>] [--override-reason <TEXT>] [--promote | --no-promote] [--input]
```
```bash
canon evidence audit auth.login.01 --ref audit-report.json --evaluator eval-1 --evaluator-version 2.1 --evaluator-digest 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef --audit-result confirmed --role implementer
```
```text
# Create a fresh .feature file for a new area.surface
canon feature new <SURFACE> --title <TEXT> [--actor <ACTOR_ID>] [--session <SESSION_ID>] [--project <PROJECT_ID>] [--input]
```
```bash
canon feature new billing.invoice --title Invoices
```
```text
# List a change's committed findings at their latest version
canon finding list --change <CHANGE_ID> [--open] [--limit <N> | --all] [--cursor <CURSOR>] [--fields <FIELDS>]
```
```bash
canon finding list --change add-login
```
```text
# Validate a spec/artifact corpus against canon's format
canon format [<DIR>]
```
```bash
canon format .canon/specs
```
```text
# Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
canon gate check [--release]
```
```bash
canon gate check
```
```text
# Promote staged records (evidence, findings, divergences) to the committed ledger
canon gate promote [--kind <evidence|finding|divergence>] [--dry-run] [--input]
```
```bash
canon gate promote --kind evidence
```
```text
# Validate each spec root, then materialize scenario index records
canon inventory sync [--spec-root <DIR>] [--dry-run] [--input]
```
```bash
canon inventory sync
```
```text
# Read stored records across every storage tier
canon query --kind <change|task|scenario|session|run|event|handoff|review|divergence|trajectory|strategy_item|evidence|subject|finding> [--since <TIMESTAMP>] [--plugin <PLUGIN_ID> | --history] [--change <CHANGE_ID>] [--status <STATUS>] [--domain <NAME>] [--include-sensitive] [--limit <N> | --all] [--cursor <CURSOR>] [--fields <FIELDS>]
```
```bash
canon query --kind evidence
```
```text
# Fetch role-scoped strategy guidance
canon retrieve --for-role <ROLE> [--regime <KEY>] [--domain <NAME>] [--subject <SUBJECT_ID>] [--limit <N>]
```
```bash
canon retrieve --for-role reviewer --domain dev --limit 20
```
```text
# Append a tagged scenario stub to its .feature file (created if missing); many at once with `--input`
canon scenario new <SCENARIO_ID> --title <TEXT> [--subject <SUBJECT_ID>] [--lane <NAME>] [--case <NAME>] [--flow <FLOW_ID>] [--change <CHANGE_ID> (with --flow)] [--actor <ACTOR_ID>] [--session <SESSION_ID>] [--feature <FILE>] [--project <PROJECT_ID>] [--input]
```
```bash
canon scenario new auth.login.02 --title "Sign out" --feature .canon/specs/kind=feature/area=auth/login.feature
```
```text
# Describe canon's own commands
canon schema [<PATH>]* [--errors | --error <CODE>]
```
```bash
canon schema evidence add
```
```text
# Search records, brief sections and scenario text, with each hit's neighbors in the record graph
canon search <TEXT> [--kind <change|task|scenario|session|run|event|handoff|review|divergence|trajectory|strategy_item|evidence|subject|finding|brief>]* [--expand <N>] [--limit <N>]
```
```bash
canon search timeout --kind finding
```
```text
# Print a guide built into this binary (markdown on stdout); the installed SKILL.md is a stub that points here
canon skills show <NAME> [--full]
```
```bash
canon skills show canon --full
```
```text
# Show where this repo stands: subjects by status, their gaps, and the next commands
canon status [--fields <FIELDS>]
```
```bash
canon status
```
```text
# Flip one task checkbox, gated on real evidence (fails closed)
canon task done <TASK_ID> [--input]
```
```bash
canon task done add-login#1
```
```text
# Follow a finding's review loop in time order: review, finding, fix, re-review, evidence
canon trace <ID>
```
```bash
canon trace add-login_1_1
```
<!-- canon:generated commands-implementer end -->

After the first `status`, follow `meta.next` of each write; do not call
`canon status` again to learn the next step. Every record you write takes
the commit it is about from `--rev` (default `HEAD`, any git revision). You
need not commit first: code that is not committed yet is recorded with its
`codeTree` and `worktreeDirty: true`, never refused. Outside git, or in a
repo with no commits, a write that needs a commit refuses `NO_COMMIT`.

The forms you reach for most (the story's ids; swap in yours):

```bash
canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer
canon evidence add --scenario auth.login.01 --report junit:red.xml
canon evidence add --scenario auth.login.01 --verdict divergent --class test-run --ref "upstream limit, tracked in issue 118" --artifact limit.log
canon finding add --change add-login --round 1 --seq 1 --severity should-fix --introduced-unknown --actor unit-1 --summary "Token is logged"
```

Batches are one `--input` document (the keys are option names in camelCase,
`operands` holds the positional arguments):

```text
canon scenario new --input '[{"operands":["auth.login.01"],"title":"Logs in","case":"happy","flow":"F1"},{"operands":["auth.login.02"],"title":"Rejects a wrong password","case":"failure","flow":"F1"}]'
canon evidence add --input '[{"scenario":"auth.login.01","class":"test-run","ref":"npm test","role":"implementer"},{"scenario":"auth.login.02","class":"test-run","ref":"npm test","role":"implementer"}]'
```

## Steps

1. **Scenarios.** One `feature new` per feature surface, then one
   `scenario new` per behavior, each pointing at the brief's flow step it
   proves, with failure paths (`--case failure`) as well as `happy` and
   `edge`. Several scenarios at once are one `scenario new --input` with a
   JSON array: inline, a file or `-` for stdin, one object per scenario whose
   keys are the option names in camelCase plus `operands` (the scenario id);
   every element is validated before the first write, so a bad element
   writes nothing and `error.field` names it (`/items/1/case`).
   `canon schema scenario new --json` shows the input schema and an example.
   (`scenario new --from` is gone.) When the gate asks for a missing case
   (`require_cases`), add a new scenario; never retag an existing one. Every
   bug fix adds or extends a scenario. Canon indexes added, edited or deleted
   `.feature` files itself; run `inventory sync` only when a command prints
   `NOT refreshed` (a format violation or a duplicate id: it names each
   problem). Specs live at `<root>/kind=feature/area=<area>/<surface>.feature`
   under a `specs.roots[]` root (default `.canon/specs`); `canon skills show
   canon-authoring` has the scaffold. When you fill in a scenario, keep its
   `# canon: {...}` provenance lines: dropping one aborts `inventory sync`
   on `missing-provenance`. `--project` is needed only when the repo
   configures more than one spec root.
2. **Units.** One unit is one flow step's complete path (sees, does,
   becomes) that can be verified on its own, sized to one session. Name the
   command that proves it (a test filter, a script) before you start it, and
   run that command before you call the unit done. Give each unit its own
   actor id and session id and pass them on everything it records. A
   delegated unit attests its own work; never attest another unit's work
   under your id. Units stay inside the one approved change; `tasks.md` is a
   note, not a record.
3. **Code and tests.** Name tests by scenario id (`game.run.01: …`) so a test
   report binds to its scenario. Record a defect you find in your own work
   with `finding add` the moment you find it, before review.
   **A fix fails first.** For a bug fix, write or extend the scenario's test
   and run it red on the unfixed code, then bind that failing run before you
   change the code (`evidence add --scenario <id> --report <fmt>:<path>`).
   A bound report decides the verdict: a failed case records `divergent`, a
   skipped case and none failed records `not-applicable`, otherwise
   `faithful`; an explicit `--verdict faithful` over a failed or skipped case
   is refused. Fix the code, run again and bind the passing run: the green
   record supersedes the red one, and the reviewer reads the red-then-green
   pair in `canon query --kind evidence --history --json`. A red run is never
   an accepted gap. When no executable reproduction is possible, say in
   `--ref` what the closest check is and why.
4. **Evidence.** With at least one `--report`, `--class` defaults to
   `test-run` and `--ref` to the report paths; without a report, pass both.
   One test run that proves several scenarios is bound once with
   `--all-scenarios`: one record per indexed scenario owning a case, each
   with the verdict its own cases show; any refusal writes none. It needs
   `--project` only when the repo has several spec roots. For one scenario
   use `--scenario <id>` with `--report` or `--artifact`; for many
   records with different values, one `evidence add --input` with an array
   (validated before any write; a write that fails stops the batch, the
   earlier elements stay written and a rerun reports them `unchanged`).
   A record binds exactly what its own write names (`--report`,
   `--artifact`): nothing is carried forward from an earlier record, so name
   every file the record should prove again. canon stores the bytes by digest
   in the artifact store (by default `.canon/artifacts/sha256/`, committed
   with the ledger; an object store when `canon.yaml`'s `artifact_store:` block
   names one, in which case nothing is kept locally) and `canon gate check`
   re-checks them, so the working-tree report may be gitignored. Run
   `canon artifacts verify` to ask the store yourself; it refuses, never
   passes, when an object store cannot be reached. Moving an existing store
   (`canon artifacts move --tier`) is the repo owner's call: do not run it.
   For a scenario about what users see, let the test report carry its
   screenshots: Playwright's JUnit reporter attaches, per test case, what a
   test attaches (`testInfo.attach(name, {path})`, or `screenshot: 'on'`), and
   one `evidence add --all-scenarios --report junit:<path>` binds to each
   record only its own tests' files. A file the report does not name goes
   with `--artifact <scenario-id>=<path>`; a bare `--artifact` in a batch is
   refused. An external evaluator's audit is
   `canon evidence audit`; canon never generates one. Evidence for a task of an
   imported plan (`--task <id>`) must bind the artifact or report that proves
   the task; a canon command is never that proof. When the behavior cannot
   match the scenario because of something outside this change, record the
   evidence `divergent` and say why in `--ref`; never record `faithful` to
   pass. The human accepts the tracked gap (`canon divergence accept`
   refuses agent roles unless it records their explicit answer); fixing the behavior and recording again is the
   other way out.
5. **Promote.** Under `write: { promote: auto }` (what `canon init` writes)
   the record is final when `evidence add` returns. Only with
   `write.promote: manual` (or no `write:` key) follow it with
   `gate promote`; `meta.next` names it.
6. **A changed result makes a signed review stale.** A review pins the
   verdict and the checks (report format and matched case names, or artifact
   digests) of the evidence it saw, not which record it was. Recording the
   same verdict from the same checks keeps every review current; a different
   verdict, different checks or an edited scenario stales it. The add prints
   the reviewer and the `canon review scenario` call that signs it again; that
   is the reviewer's call. Do not run it yourself. A review of an uncommitted
   working tree survives the commit you make from it: the review is judged by
   the scenario and the evidence it pins, never by the commit.
7. **After a fix or a chore, record again only where the code changed.**
   A blocker the reviewer closed `fixed` is not done until the scenarios it
   names (the whole suite when it names none) have evidence newer than the
   close. An approved chore asks for new evidence only on the scenarios whose
   latest evidence was recorded on a different `codeTree` than the current
   code (`code.exclude` in `canon.yaml` decides what is not code; the default
   is `docs/**` and `*.md`). A docs-only chore needs its change-level review
   alone. When code did change, the shipping refusal, `status` and
   `meta.next` lead with one command over every report, with the real paths
   (`--project <id>` is added only with several spec roots):
   `canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer`.
   Run the suite and bind it with that one command, not one scenario at a
   time. Reviews stay current while the verdicts and checks are unchanged; a
   scenario whose gap was accepted needs the human to accept again once new
   evidence says something different.

**Done when:** `canon gate check` lists no `uncovered-cell` or
`stale-evidence` violation for the change's scenarios (read `data.violations[]`
in `canon gate check --json`; a remaining `unreviewed-promotion` is the
reviewer's step), and the `meta.next` of your last write names the review,
not another evidence step.

## Excuses that fail

| The thought | The reality |
|---|---|
| "The suite is slow; record `faithful` to pass." | `faithful` over a failed case is refused; a limit outside this change is recorded `divergent` with the reason in `--ref`. |
| "I fixed it, the test would pass." | Run the test red first and bind it, then green. Evidence is a claim; the history is what the reviewer reads. |
| "The brief is as good as approved." | `evidence add` refuses an unapproved brief's scenarios. Say so and wait. |
| "I will attest the other unit's work under my id." | Each unit records under its own actor id and session id. |
| "I was told this twice already." | A correction repeated across runs belongs in a scenario, the policy or a gate; tell the orchestrator. |
| "I will accept the divergence myself." | The human accepts a gap: report the scenario, the reason and the tracking reference so they are asked, and never record an answer they did not give. |

## Retry safety

`evidence add` is content-keyed: repeating it with identical input returns
the existing record with `meta.unchanged: true` instead of a duplicate (a
batch reports that element `unchanged`). After a timeout or a lost
response, run the same command again. `scenario new` is not: an existing
scenario id is refused (`SCENARIO_TAG_EXISTS`) and nothing changes, so check
before you repeat it.

## Next

When every scenario has evidence, the review is the reviewer's session, in a
separate process: do not review your own change. A chore adds no scenarios:
once it is approved, record evidence where the code changed (the batch
above names it) and the reviewer records one change-level review of the
chore (`review change`); shipping needs both. Scenario reviews stay current
when the verdicts and checks are unchanged. Reference: `canon skills show
canon-gate`.
