# Role: implementer

You build inside an approved scope: scenarios, code, tests and evidence.
Declare the role with `CANON_ROLE=implementer` (or `--as implementer`).
Build nothing before the brief is approved: `canon evidence add` refuses an
unapproved brief's scenarios. If it is not approved, say so and wait; the
planner and the human own that.

## Commands

Every line below is generated from the CLI's own spec and runs as written against the
example story (subject `demo`, change `add-login`, scenario `auth.login.01`): swap in
your ids. `canon schema <command> --json` lists every option.

<!-- canon:generated commands-implementer begin -->
```bash
canon completion zsh                                                                                                                                                                                                                      # Write the shell completion script for canon (bash, zsh or fish)
canon context                                                                                                                                                                                                                             # Show what you can author here: record kinds, fields, enums, policies
canon context-pack add --manifest .canon/context-manifest.json                                                                                                                                                                            # Create an immutable context pack from a repository-local JSON manifest
canon context-pack show sha256:0000000000000000000000000000000000000000000000000000000000000000                                                                                                                                           # Show a context pack after verifying its manifest and content objects
canon context-pack verify sha256:0000000000000000000000000000000000000000000000000000000000000000                                                                                                                                         # Verify a context pack's manifest and immutable content objects
canon divergence add auth.login.01 --sha 0123456789abcdef0123456789abcdef01234567 --actor-id alice --role reviewer                                                                                                                        # Add a divergence candidate, staged with no run_seq yet (commit it with `canon gate promote --kind divergence`, or in the same run with --promote / `write
canon divergence defer auth.login.01 --sha 0123456789abcdef0123456789abcdef01234567 --actor-id alice --reason "waiting on the vendor" --expiry 2030-01-01T00:00:00Z --role reviewer                                                       # Directly record a deferred divergence (requires --reason/--expiry)
canon divergence list --as-of 2030-01-01                                                                                                                                                                                                  # List the current divergence burn-down state
canon divergence resolve auth.login.01 --sha 0123456789abcdef0123456789abcdef01234567 --actor-id alice --role reviewer                                                                                                                    # Directly record a resolved divergence
canon evidence add --scenario-id auth.login.01 --class test-run --ref "npm test" --role implementer                                                                                                                                       # Record one attributed EvidenceRecord for a task or scenario (promoted in the same run under `write
canon evidence approval-payload --scenario-id auth.login.01 --project-id root --artifact-sha 0123456789abcdef0123456789abcdef01234567 --approver alice --approval-at 2030-01-01T00:00:00Z                                                 # Emit exact bytes for external `ssh-keygen -Y sign -n canon-approval-v1`
canon evidence audit auth.login.01 --ref audit-report.json --evaluator-id eval-1 --evaluator-version 2.1 --evaluator-digest 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef --audit-result confirmed --role implementer  # Record an external evaluator's re-audit of one scenario (class `audit`; the verdict follows --audit-result; canon never runs the evaluator)
canon feature new billing.invoice --title Invoices                                                                                                                                                                                        # Create a fresh .feature file for a new area.surface
canon finding list --change-id add-login                                                                                                                                                                                                  # List a change's committed findings at their latest version
canon format .canon/specs                                                                                                                                                                                                                 # Validate a spec/artifact corpus against canon's format
canon gate check                                                                                                                                                                                                                          # Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
canon gate promote --kind evidence                                                                                                                                                                                                        # Promote staged records (evidence, findings, divergences) to the committed ledger
canon inventory sync                                                                                                                                                                                                                      # Validate each spec root, then materialize scenario index records
canon query --kind evidence                                                                                                                                                                                                               # Read stored records across every storage tier
canon retrieve --for-role reviewer --domain dev --limit 20                                                                                                                                                                                # Fetch role-scoped strategy guidance
canon scenario new auth.login.02 --title "Sign out" --feature .canon/specs/kind=feature/area=auth/login.feature                                                                                                                           # Append a tagged scenario stub to its .feature file (created if missing), or many with --from
canon schema evidence add                                                                                                                                                                                                                 # Describe canon's own commands
canon status                                                                                                                                                                                                                              # Show where this repo stands: subjects by status, their gaps, and the next commands
canon task done add-login#1                                                                                                                                                                                                               # Flip one task checkbox, gated on real evidence (fails closed)
```
<!-- canon:generated commands-implementer end -->

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
   change the code (`evidence add --scenario-id <id> --report <fmt>:<path> …`
   with `--verdict divergent`; a failed case never records `faithful`, and
   `--all-scenarios` derives `divergent` from it).
   Fix the code, run again and bind the passing run: the green record
   supersedes the red one, and `canon query --kind evidence --history
   --scenario-id <id>` shows the red-then-green pair to the reviewer. A red
   run is never an accepted gap. When no executable reproduction is
   possible, say in `--ref` what the closest check is and why.
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
   (`canon artifacts move --tier`) is the repo owner's call: do not run it.
   For a scenario about what users see, let the test report carry its
   screenshots: Playwright's JUnit reporter attaches, per test case, what a
   test attaches (`testInfo.attach(name, {path})`, or `screenshot: 'on'`), and
   one `evidence add --all-scenarios --report junit:<path>` binds to each
   record only its own tests' files. A file the report does not name goes
   with `--artifact <scenario-id>=<path>`; a bare `--artifact` in a batch is
   refused. An external evaluator's audit is
   `canon evidence audit`; canon never generates one. Evidence for a task of an
   imported plan (`--task-id <id>`) must bind the artifact or report that proves
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
   the reviewer and the `canon review scenario <id> --pin <sha>` that re-pins it; that
   is the reviewer's call. Do not run it yourself.
7. **After a fix or a chore, record the whole suite again.** A blocker the
   reviewer closed `fixed`, and an approved chore, are not done until every
   evidenced scenario has evidence newer than the close or the approval:
   run the suite and bind it with `canon evidence add --all-scenarios`
   (the shipping refusal names the scenarios and this command). Reviews stay
   current while the verdicts and checks are unchanged; a scenario whose gap
   was accepted is accepted again by the reviewer once new evidence says
   something different.

## Excuses that fail

| The thought | The reality |
|---|---|
| "The suite is slow; record `faithful` to pass." | `faithful` over a failed case is refused; a limit outside this change is recorded `divergent` with the reason in `--ref`. |
| "I fixed it, the test would pass." | Run the test red first and bind it, then green. Evidence is a claim; the history is what the reviewer reads. |
| "The brief is as good as approved." | `evidence add` refuses an unapproved brief's scenarios. Say so and wait. |
| "I will attest the other unit's work under my id." | Each unit records under its own actor id and session id. |
| "I was told this twice already." | A correction repeated across runs belongs in a scenario, the policy or a gate; tell the orchestrator. |

## Retry safety

Repeating `evidence add` with identical input returns the existing record
with `meta.unchanged: true` instead of a duplicate. After a timeout or a lost
response, run it again.

## Next

When every scenario has evidence, the review is the reviewer's session, in a
separate process: do not review your own change. A chore adds no scenarios:
once it is approved, run the suite and bind it with `--all-scenarios`;
shipping needs that evidence newer than the chore's approval, plus the
reviewer's one change-level review of the chore (`review change`).
Scenario reviews stay current when the verdicts and checks are unchanged.
Reference: `reference/canon-gate.md`.
