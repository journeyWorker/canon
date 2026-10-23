# Role: reviewer

You are the independent second look. Declare the role with
`CANON_ROLE=reviewer` (or `--as reviewer`); `--role` on `review scenario` then
defaults to `reviewer`.

## Commands

Every line below is generated from the CLI's own spec and runs as written against the
example story (subject `demo`, change `add-login`, scenario `auth.login.01`): swap in
your ids. `canon schema <command> --json` lists every option.

<!-- canon:generated commands-reviewer begin -->
```bash
canon brief check                                                                                                                                                                    # Read the brief against the rules: blocker and advisory findings (exit 0 clean, 2 blockers, 1 error)
canon brief show                                                                                                                                                                     # Show where the change stands and the next one to three steps; never edits
canon completion zsh                                                                                                                                                                 # Write the shell completion script for canon (bash, zsh or fish)
canon context                                                                                                                                                                        # Show what you can author here: record kinds, fields, enums, policies
canon divergence add auth.login.01 --sha 0123456789abcdef0123456789abcdef01234567 --actor-id alice --role reviewer                                                                   # Add a divergence candidate, staged with no run_seq yet (commit it with `canon gate promote --kind divergence`, or in the same run with --promote / `write
canon divergence defer auth.login.01 --sha 0123456789abcdef0123456789abcdef01234567 --actor-id alice --reason "waiting on the vendor" --expiry 2030-01-01T00:00:00Z --role reviewer  # Directly record a deferred divergence (requires --reason/--expiry)
canon divergence list --as-of 2030-01-01                                                                                                                                             # List the current divergence burn-down state
canon divergence resolve auth.login.01 --sha 0123456789abcdef0123456789abcdef01234567 --actor-id alice --role reviewer                                                               # Directly record a resolved divergence
canon finding add --change-id add-login --round 1 --seq 2 --severity should-fix --actor-id alice --summary "Token is logged"                                                         # Record one code-review finding (promoted in the same run under `write
canon finding close add-login --round 1 --seq 1 --disposition rejected --reason "works as designed"                                                                                  # Move an already-committed finding's disposition (promoted in the same run under `write
canon finding list --change-id add-login                                                                                                                                             # List a change's committed findings at their latest version
canon format .canon/specs                                                                                                                                                            # Validate a spec/artifact corpus against canon's format
canon gate check                                                                                                                                                                     # Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
canon query --kind evidence                                                                                                                                                          # Read stored records across every storage tier
canon retrieve --for-role reviewer --domain dev --limit 20                                                                                                                           # Fetch role-scoped strategy guidance
canon review change add-login --actor-id alice --pin abc123 --role reviewer --original-spec-ref docs/prd.md                                                                          # Write one attributed Review record for a chore change directly to the ledger (exactly one provenance ref required)
canon review scenario auth.login.01 --actor-id alice --pin abc123 --role reviewer --original-spec-ref docs/prd.md                                                                    # Write one attributed Review record per scenario directly to the ledger, nothing to promote (exactly one provenance ref required)
canon schema evidence add                                                                                                                                                            # Describe canon's own commands
canon status                                                                                                                                                                         # Show where this repo stands: subjects by status, their gaps, and the next commands
```
<!-- canon:generated commands-reviewer end -->

`review scenario` needs `--upstream-ref <ref>` or `--original-spec-ref <path>`;
add `--artifact <path>` (repeatable) for user-visible behavior. A short
`<sha>` is fine. With `write.promote: manual`, follow `finding add` and
`finding close` with `canon gate promote --as reviewer --json`.

A scenario review pins what you saw: the scenario's digest plus the verdict
and checks (report format and matched case names, or artifact digests) of its
latest evidence per role, not which record held them. It stays current when
the implementer records the same verdict from the same checks again, and
goes stale when the verdict or the checks differ or the scenario changes.
Reviews written before 0.23 do not count until `canon migrate` re-pins them
(an advisory names it); that is the orchestrator's step, not yours.

Run `gate check` once, before `review scenario`: its `unreviewed-promotion`
violations are the subjects and scenarios that need your review. Do not loop
it. After `review scenario`, read its `meta.next` instead of calling `gate check`
or `status` again.

## Separate session

You run as a separate process with your own session id, never the session
that wrote the code and evidence under another `--actor-id` or
`--session-id` label. Pass your real session id as `--session-id` on
everything you record. A harness with no subagent tool starts you (a new
`codex exec` run in this repo, say). Review happens on every change.

## Read the PRD

1. `brief check`: the brief must have no blockers.
2. Read the brief itself, `.canon/changes/<slug>/brief.md`: intent, the flow
   `F1`, `F2`, …, acceptance and failure path per step, out of scope. You do
   not refine the brief; a brief defect is a finding, not an edit.
3. Check the scenarios against it: every flow step has a scenario that
   proves it, every step has a failure scenario, each scenario's steps match
   the step's acceptance, and nothing builds what the brief puts out of
   scope.

## Run the checks yourself

Run the build, the tests and the app the evidence claims to cover, on the
commit you pin. Compare the results with the evidence records. Never sign
what you could not run: a reviewer that cannot run the tests says so and
does not sign off; record that instead (`--outcome indeterminate`, or
`unavailable` when you could not run).

## Check the checks

Passing is not enough. Read each bound test against its scenario's Then
steps, in two separate passes, and say which pass a finding came from:

1. **Spec fidelity.** The brief's step and the scenario against the test
   (the PRD read above). Finding: the check pins behavior the scenario does
   not require, or skips a Then step it states.
2. **Standards.** The diff and the test against the code it claims to prove.
   Finding: the test passes without proving anything. The usual ways:
   - the expected value is computed by the code under test (tautology);
   - it asserts an implementation detail (a call count, a private name), not behavior;
   - it never reaches the real path or the real side effect (a file written, a record stored, a request sent);
   - it asserts only that no error happened;
   - it mocks the unit it is meant to test.

When a check looks weak, you may name the one-line change to the code that
should make it fail and run it if it is cheap; if the test stays green, record
the finding. For a fix, read `canon query --kind evidence --history
--scenario-id <id>`: expect an earlier non-faithful version, or a named
closest reproduction in `--ref`; record a finding when neither exists. Put
one blast-radius fact in the finding or review `--summary`: what else the
change touches that the checks do not exercise. Do all of this before you
sign.

## Accept a known gap

`divergence accept` is for a scenario whose honest evidence is `divergent`
and whose gap is limited by something outside this change (an upstream
limit, a platform bug) and is tracked there: give the reason and the
`--tracking` reference, and an `--expiry` when the limit should be gone by
a date. Never use it to skip a fix you can make in this change. It is your
step, from a session that did not attest the evidence
(`DIVERGENCE_ACCEPT_REFUSED` otherwise). The acceptance pins that evidence:
new evidence for the scenario, or its expiry, ends it, and you accept (or
the implementer fixes) again. An accepted gap counts as covered for the
ship guard and stays listed as `accepted-divergence` in `status` and
`gate check`.

## Record

- Every defect is a finding; `finding list` shows round, seq, severity and
  disposition, so the next seq and the open blockers are one command away.
  Close a finding once it is dealt with. When you close it `rejected` or
  `deferred`, give the reason in one line with `--reason "<why>"`: the close
  is refused without it (`FINDING_REASON_REQUIRED`), and `finding list` shows
  it as `[reason: …]` so the next reader sees why it was not fixed.
- Sign off in one batch with `review scenario`, all or nothing. It writes
  directly; nothing to promote. canon stores and re-checks attached bytes
  and never compares images.
- Pass `--scenario-id` for each scenario a finding is about. A blocker you
  close `fixed` then stays blocking until the implementer records evidence
  again for those scenarios alone (`evidence add --scenario-id <id> …`); the
  close inherits the list. A blocker naming no scenario asks for the whole
  suite again (`evidence add --all-scenarios`), which on a big suite is
  a long e2e run for a one-scenario fix, so name them. Your pinned review
  stays current when the new evidence says the same, otherwise look again.
- Fixes you ask for return as a re-review of the same change. When new
  evidence changes a verdict or the checks and so stales your review, look
  again before you re-pin.
  A chore has no scenarios: review it with one change-level review, `canon
  review change <slug> --project-id <root-id> --actor-id <me> --role
  reviewer --pin <sha> --upstream-ref <ref>`. It signs the chore's
  invariants and checks against the diff and pins its brief, so editing the
  brief stales it. The chore also needs evidence recorded after its approval
  (the implementer's `--all-scenarios` run) before it ships.

## Excuses that fail

| The thought | The reality |
|---|---|
| "I will review this under a new session label." | A review is a separate process with its own real session id; a relabelled self-review is not independent (#39). |
| "The tests pass, so I can sign." | Read the test against the Then steps first; a green test can prove nothing. |
| "I cannot run the tests, but the evidence looks right." | Say so and record `indeterminate` or `unavailable`; never sign what you could not run. |
| "This is a minor defect; I will skip the finding." | Every defect is a finding; severity says how much it matters. |
| "I will edit the brief to fix it." | A brief defect is a finding, not an edit. |

Reference: `reference/canon-review.md`.
