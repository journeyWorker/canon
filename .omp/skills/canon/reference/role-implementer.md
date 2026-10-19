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
canon scenario new <area>.<surface>.<nn> --subject-id <subject-id> --flow <F-id> --case <happy|edge|failure> --title "<title>" --as implementer --json
canon scenario new --from - --as implementer --json     # many at once: NDJSON on stdin, one {"id","title","subject-id","flow","case",…} per line
canon evidence add --project-id <root-id> --all-scenarios --report <junit|cucumber|swift-testing>:<path> --kind test-run --role implementer --ref "<test command>" --actor-id <unit> --session-id <session> --as implementer --json
canon evidence add --project-id <root-id> --scenario-id <id> --artifact <path> --kind test-run --role implementer --ref "<what it shows>" --actor-id <unit> --session-id <session> --as implementer --json
canon evidence add --project-id <root-id> --scenario-id <id> --verdict divergent --artifact <path> --kind test-run --role implementer --ref "<the external limit, and where it is tracked>" --actor-id <unit> --session-id <session> --as implementer --json   # honest divergent evidence
canon artifacts verify --full --as implementer --json  # bound files are in the artifact store (omit --full to trust the store's size and digest metadata); refuses when an object store is unreachable
canon finding add --change-id <slug> --round <n> --seq <n> --severity <blocker|should-fix|note> --reviewer <unit> --actor-id <unit> --session-id <session> --summary "<defect>" --as implementer --json
canon inventory sync --as implementer --json          # only after a write printed NOT refreshed
canon query --kind <kind> --history --as implementer --json   # earlier versions too; without --history only the current record of each key
canon ledger compact --as implementer --json           # only for a pre-0.23 ledger or an interrupted move (add --dry-run to preview)
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
   A record binds exactly what its own write names (`--report`,
   `--artifact`): nothing is carried forward from an earlier record, so name
   every file the record should prove again. canon stores the bytes by digest
   in the artifact store (by default `.canon/artifacts/sha256/`, committed
   with the ledger; an object store when `canon.yaml`'s `artifact_store:` block
   names one, in which case nothing is kept locally) and `canon gate check`
   re-checks them, so the working-tree report may be gitignored. Run
   `canon artifacts verify` to ask the store yourself; it refuses, never
   passes, when an object store cannot be reached. Moving an existing store
   (`canon artifacts migrate`) is the repo owner's call: do not run it.
   For a scenario about what users see, let the test report carry its
   screenshots: Playwright's JUnit reporter attaches, per test case, what a
   test attaches (`testInfo.attach(name, {path})`, or `screenshot: 'on'`), and
   one `evidence add --all-scenarios --report junit:<path>` binds to each
   record only its own tests' files. A file the report does not name goes
   with `--artifact <scenario-id>=<path>`; a bare `--artifact` in a batch is
   refused. An external evaluator's audit is
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
6. **A changed result makes a signed review stale.** A review pins the
   verdict and the checks (report format and matched case names, or artifact
   digests) of the evidence it saw, not which record it was. Recording the
   same verdict from the same checks keeps every review current; a different
   verdict, different checks or an edited scenario stales it. The add prints
   the reviewer and the `canon review add … --pin <sha>` that re-pins it; that
   is the reviewer's call. Do not run it yourself.
7. **After a fix or a chore, record the whole suite again.** A blocker the
   reviewer closed `fixed`, and an approved chore, are not done until every
   evidenced scenario has evidence newer than the close or the approval:
   run the suite and bind it with `canon evidence add --all-scenarios …`
   (the shipping refusal names the scenarios and this command). Reviews stay
   current while the verdicts and checks are unchanged; a scenario whose gap
   was accepted is accepted again by the reviewer once new evidence says
   something different.

## Retry safety

Repeating `evidence add` with identical input returns the existing record
with `meta.unchanged: true` instead of a duplicate. After a timeout or a lost
response, run it again.

## Next

When every scenario has evidence, the review is the reviewer's session, in a
separate process: do not review your own change. A chore adds no scenarios:
once it is approved, run the suite and bind it with `--all-scenarios`;
shipping needs that evidence newer than the chore's approval, plus the
reviewer's one change-level review of the chore (`review add --change-id`).
Scenario reviews stay current when the verdicts and checks are unchanged.
Reference: `reference/canon-gate.md`.
