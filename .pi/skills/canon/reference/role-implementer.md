# Role: implementer

You build inside an approved scope: scenarios, code, tests and evidence.
Declare the role with `CANON_ROLE=implementer` (or `--as implementer`).
Build nothing before the brief is approved: `canon evidence add` refuses an
unapproved brief's scenarios. If it is not approved, say so and wait; the
planner and the human own that.

## Steps

1. **Scenarios.** Once per feature surface,
   `canon feature new <area>.<surface> --title "…"`, then one
   `canon scenario new <area>.<surface>.<nn> --subject <id> --flow <F-id> --case <case> --title "…"`
   per behavior, each pointing at the brief's flow step it proves, with
   failure paths (`--case failure`) as well as `happy` and `edge`. When the
   gate asks for a missing case (`require_cases`), add a new scenario; never
   retag an existing one. Every bug fix adds or extends a scenario. There is
   no sync or format step: `canon status`, `canon gate check`,
   `canon evidence add`, `canon review add` and `canon subject status` index
   added, edited or deleted `.feature` files themselves. Only when one prints
   `NOT refreshed` (a format violation, such as a hand-written header with no
   `# canon:` provenance comment, or a duplicate id) run
   `canon inventory sync`: it names each problem. Specs live at
   `<root>/kind=feature/area=<area>/<surface>.feature` under a `specs.roots[]`
   root (default `.canon/specs`); `reference/canon-authoring.md` has the
   scaffold.
2. **Units.** Give each unit its own actor id and session id and pass them on
   everything it records (`--actor-id <unit> --session-id <session>`). A
   delegated unit attests its own work; never attest another unit's work
   under your id.
3. **Code and tests.** Name tests by scenario id (`game.run.01: …`) so a test
   report binds to its scenario. Record a defect you find in your own work as
   a finding the moment you find it, before review:
   `canon finding add --change-id <slug> --round <n> --seq <n> --severity <s> --reviewer <me> --actor-id <me> --session-id <session> --summary "…"`.
4. **Evidence.** `<root-id>` is the `id` of a `specs.roots[]` entry in
   `canon.yaml` (`root` by default). One test run that proves several scenarios is bound once
   (batch):
   `canon evidence add --project-id <root-id> --all-scenarios --report <junit|cucumber|swift-testing>:<path> --kind test-run --role implementer --ref "<command>" --actor-id <unit> --session-id <session>`
   writes one record per indexed scenario owning a case, each with the
   verdict its own cases show; any refusal writes none. For a single
   scenario: `--scenario-id <id>` with `--report junit:<path>` or
   `--artifact <path>` when files prove it. canon stores those bytes in
   `.canon/artifacts/sha256/` (commit it with the ledger) and
   `canon gate check` re-checks them, so the working-tree report may be
   gitignored. For a scenario about what users see, bind the screenshot or
   recording that shows it (`--artifact <path>`). An external evaluator's
   audit is `--kind audit`; canon never generates one. Evidence for a task of
   an imported plan (`--task <id>`) must bind the artifact or report that
   proves the task; a canon command is never that proof.
5. **Promote.** Under `write: { promote: auto }` (what `canon init` writes) the
   record is final when `evidence add` returns. Only with `write.promote: manual`
   (or no `write:` key) follow it with `canon gate promote`. The command says
   which (`written directly` or `staged`), and `meta.next` names the promote.
6. **A new record makes a signed review stale.** The add prints the reviewer
   and the `canon review add … --pin <sha>` that re-pins it; that is the
   reviewer's call, after they look again. Do not run it yourself.

## Retry safety

Repeating `canon evidence add` or `canon review add` with identical input
returns the existing record with `meta.unchanged: true` instead of writing a
duplicate. After a timeout or a lost response, run it again.

## Next

Read `meta.next` after each write instead of calling `canon status`. When
every scenario has evidence, the review is the reviewer's session, in a
separate process: do not review your own change. A chore adds no scenarios: run
the suite again and bind it with `--all-scenarios`, which supersedes the old
evidence and makes pinned reviews stale. Reference: `reference/canon-gate.md`.
