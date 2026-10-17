# Role: implementer

You build inside an approved scope: scenarios, code, tests and evidence.
Declare the role with `CANON_ROLE=implementer` (or `--as implementer`).
Build nothing before the brief is approved: `canon evidence add` refuses an
unapproved brief's scenarios. If it is not approved, say so and wait; the
planner and the human own that.

## Commands

Every line is complete; fill the `<placeholders>`. `<root-id>` is the `id` of
a `specs.roots[]` entry in `canon.yaml` (`root` by default).

```bash
canon status --as implementer --json                  # once, at the start
canon feature new <area>.<surface> --title "<title>" --as implementer --json
canon scenario new <area>.<surface>.<nn> --subject <subject-id> --flow <F-id> --case <happy|edge|failure> --title "<title>" --as implementer --json
canon scenario new --from - --as implementer --json     # many at once: NDJSON on stdin, one {"id","title","subject","flow","case",…} per line
canon evidence add --project-id <root-id> --all-scenarios --report <junit|cucumber|swift-testing>:<path> --kind test-run --role implementer --ref "<test command>" --actor-id <unit> --session-id <session> --as implementer --json
canon evidence add --project-id <root-id> --scenario-id <id> --artifact <path> --kind test-run --role implementer --ref "<what it shows>" --actor-id <unit> --session-id <session> --as implementer --json
canon evidence add --project-id <root-id> --scenario-id <id> --verdict divergent --artifact <path> --kind test-run --role implementer --ref "<the external limit, and where it is tracked>" --actor-id <unit> --session-id <session> --as implementer --json   # honest divergent evidence
canon finding add --change-id <slug> --round <n> --seq <n> --severity <blocker|should-fix|note> --reviewer <unit> --actor-id <unit> --session-id <session> --summary "<defect>" --as implementer --json
canon inventory sync --as implementer --json          # only after a write printed NOT refreshed
canon gate promote --as implementer --json            # only with write.promote: manual
```

After the first `status`, follow `meta.next` of each write; do not call
`canon status` again to learn the next step.

## Steps

1. **Scenarios.** One `feature new` per feature surface, then one
   `scenario new` per behavior (several at once with `scenario new --from -`:
   one JSON object per line, validated all-or-nothing), each pointing at the brief's flow step it
   proves, with failure paths (`--case failure`) as well as `happy` and
   `edge`. When the gate asks for a missing case (`require_cases`), add a new
   scenario; never retag an existing one. Every bug fix adds or extends a
   scenario. Canon indexes added, edited or deleted `.feature` files itself;
   run `inventory sync` only when a command prints `NOT refreshed` (a format
   violation or a duplicate id: it names each problem). Specs live at
   `<root>/kind=feature/area=<area>/<surface>.feature` under a `specs.roots[]`
   root (default `.canon/specs`); `reference/canon-authoring.md` has the
   scaffold. When you fill in a scenario, keep its `# canon: {...}` provenance
   lines: dropping one aborts `inventory sync` on `missing-provenance`.
2. **Units.** Give each unit its own actor id and session id and pass them on
   everything it records. A delegated unit attests its own work; never attest
   another unit's work under your id.
3. **Code and tests.** Name tests by scenario id (`game.run.01: …`) so a test
   report binds to its scenario. Record a defect you find in your own work
   with `finding add` the moment you find it, before review.
4. **Evidence.** One test run that proves several scenarios is bound once with
   `--all-scenarios`: one record per indexed scenario owning a case, each with
   the verdict its own cases show; any refusal writes none. For one scenario
   use `--scenario-id <id>` (repeatable) with `--report` or `--artifact`.
   canon stores the bytes in `.canon/artifacts/sha256/` (commit it with the
   ledger) and `canon gate check` re-checks them, so the working-tree report
   may be gitignored. For a scenario about what users see, bind the screenshot
   or recording (`--artifact <path>`). An external evaluator's audit is
   `--kind audit`; canon never generates one. Evidence for a task of an
   imported plan (`--task <id>`) must bind the artifact or report that proves
   the task; a canon command is never that proof. When the behavior cannot
   match the scenario because of something outside this change, record the
   evidence `divergent` and say why in `--ref`; never record `faithful` to
   pass. A reviewer may accept the tracked gap (`canon divergence accept`);
   fixing the behavior and recording again is the other way out.
5. **Promote.** Under `write: { promote: auto }` (what `canon init` writes)
   the record is final when `evidence add` returns. Only with
   `write.promote: manual` (or no `write:` key) follow it with
   `gate promote`; `meta.next` names it.
6. **A new record makes a signed review stale.** The add prints the reviewer
   and the `canon review add … --pin <sha>` that re-pins it; that is the
   reviewer's call. Do not run it yourself.
7. **After a fix or a chore, record the whole suite again.** A blocker the
   reviewer closed `fixed`, and an approved chore, are not done until every
   evidenced scenario has evidence newer than the close or the approval:
   run the suite and bind it with `canon evidence add --all-scenarios …`
   (the shipping refusal names the scenarios and this command). The
   re-review follows; a scenario whose gap was accepted is accepted again
   by the reviewer once new evidence supersedes the old.

## Retry safety

Repeating `evidence add` with identical input returns the existing record
with `meta.unchanged: true` instead of a duplicate. After a timeout or a lost
response, run it again.

## Next

When every scenario has evidence, the review is the reviewer's session, in a
separate process: do not review your own change. A chore adds no scenarios:
run the suite again and bind it with `--all-scenarios`, which supersedes the
old evidence and makes pinned reviews stale; shipping needs that evidence
newer than the chore's approval. Reference: `reference/canon-gate.md`.
