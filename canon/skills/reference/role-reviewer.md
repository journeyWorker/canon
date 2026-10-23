# Role: reviewer

You are the independent second look. Declare the role with
`CANON_ROLE=reviewer` (or `--as reviewer`); `--role` on `review scenario`,
`review change` and `finding add` then defaults to `reviewer`. The human's
acts refuse you (`HUMAN_ONLY`): `brief approve`, `policy approve` and
`divergence accept`. Name what the human has to decide in your report; never
look for a flag that grants it.

## Commands

Every line below is generated from the CLI's own spec and runs as written against the
example story (subject `demo`, change `add-login`, scenario `auth.login.01`): swap in
your ids. `canon schema <command> --json` lists every option.

<!-- canon:generated commands-reviewer begin -->
```bash
canon brief check                                                                                                                     # Read the brief against the rules
canon brief show                                                                                                                      # Show where the change stands and the next one to three steps; never edits
canon completion zsh                                                                                                                  # Write the shell completion script for canon (bash, zsh or fish)
canon context                                                                                                                         # Show what you can author here: record kinds, fields, enums, policies
canon divergence add auth.login.01 --actor-id alice --role reviewer                                                                   # Add a divergence candidate, staged with no run_seq yet (commit it with `canon gate promote --kind divergence`, or in the same run with --promote / `write
canon divergence defer auth.login.01 --actor-id alice --reason "waiting on the vendor" --expiry 2030-01-01T00:00:00Z --role reviewer  # Directly record a deferred divergence (requires --reason/--expiry)
canon divergence list --as-of 2030-01-01                                                                                              # List the current divergence burn-down state
canon divergence resolve auth.login.01 --actor-id alice --role reviewer                                                               # Directly record a resolved divergence
canon finding add --change-id add-login --round 1 --seq 2 --severity should-fix --actor-id alice --summary "Token is logged"          # Record one code-review finding (promoted in the same run under `write
canon finding close add-login --round 1 --seq 1 --disposition rejected --reason "works as designed"                                   # Move an already-committed finding's disposition (promoted in the same run under `write
canon finding list --change-id add-login                                                                                              # List a change's committed findings at their latest version
canon format .canon/specs                                                                                                             # Validate a spec/artifact corpus against canon's format
canon gate check                                                                                                                      # Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
canon query --kind evidence                                                                                                           # Read stored records across every storage tier
canon retrieve --for-role reviewer --domain dev --limit 20                                                                            # Fetch role-scoped strategy guidance
canon review change add-login --actor-id alice --role reviewer --original-spec-ref docs/prd.md                                        # Write one attributed Review record for a chore change directly to the ledger (exactly one provenance ref required)
canon review scenario auth.login.01 --actor-id alice --role reviewer --original-spec-ref docs/prd.md                                  # Write one attributed Review record per scenario directly to the ledger, nothing to promote (exactly one provenance ref required)
canon schema evidence add                                                                                                             # Describe canon's own commands
canon skills show canon --full                                                                                                        # Print a guide built into this binary (markdown on stdout); the installed SKILL.md is a stub that points here
canon status                                                                                                                          # Show where this repo stands: subjects by status, their gaps, and the next commands
```
<!-- canon:generated commands-reviewer end -->

`review scenario` needs exactly one of `--upstream-ref <ref>` or
`--original-spec-ref <path>`; add `--artifact <path>` (repeatable) for
user-visible behavior. `--rev` is the commit you reviewed (default `HEAD`;
any git revision, a short sha included). With `write.promote: manual`,
follow `finding add` and `finding close` with `canon gate promote --as
reviewer --json`. `canon schema review scenario --json` lists every option.

The forms you reach for most (the story's ids; swap in yours):

```bash
canon review scenario auth.login.01 auth.login.02 --actor-id reviewer-1 --session-id session-7 --original-spec-ref docs/prd.md
canon review scenario auth.login.01 --actor-id reviewer-1 --session-id session-7 --review-outcome indeterminate --original-spec-ref docs/prd.md
canon review change add-login --actor-id reviewer-1 --session-id session-7 --original-spec-ref docs/prd.md
canon finding add --change-id add-login --round 1 --seq 2 --severity blocker --actor-id reviewer-1 --session-id session-7 --summary "No rate limit" --scenario-id auth.login.01 --scenario-id auth.login.02 --introduced-rev HEAD~3
canon finding close add-login --round 1 --seq 2 --disposition fixed --resolution-rev HEAD
canon finding close add-login --round 1 --seq 3 --disposition rejected --reason "works as designed"
canon finding list --change-id add-login --open
```

Several findings at once are one `--input` array (the keys are option names
in camelCase):

```text
canon finding add --input '[{"changeId":"add-login","round":1,"seq":3,"severity":"note","actorId":"reviewer-1","summary":"Token is logged"},{"changeId":"add-login","round":1,"seq":4,"severity":"should-fix","actorId":"reviewer-1","summary":"Error text leaks the user name"}]'
```

A review pins what you saw: the scenario's digest plus the verdict
and checks (report format and matched case names, or artifact digests) of its
latest evidence per role, not which record held them. It stays current when
the implementer records the same verdict from the same checks again, and
goes stale when the verdict or the checks differ or the scenario changes.
A review of code that is not committed yet is allowed: the record carries
`codeTree` and `worktreeDirty: true`, and it still covers the commit made
from that working tree while the code did not change in between. A review of
code that changed afterwards does not. Reviews written before 0.23 do not
count until `canon migrate` re-pins them (an advisory names it); that is
the orchestrator's step, not yours.

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

1. `brief check`: the brief must have no blockers (exit 1 means it has some).
2. Read the brief itself, `.canon/changes/<slug>/brief.md`: intent, the flow
   `F1`, `F2`, …, acceptance and failure path per step, out of scope. You do
   not refine the brief; a brief defect is a finding, not an edit.
3. Check the scenarios against it: every flow step has a scenario that
   proves it, every step has a failure scenario, each scenario's steps match
   the step's acceptance, and nothing builds what the brief puts out of
   scope.

## Run the checks yourself

Run the build, the tests and the app the evidence claims to cover, on the
code you review: the commit `--rev` names, or the working tree when it is
not committed yet. Compare the results with the evidence records. Never sign
what you could not run: a reviewer that cannot run the tests says so and
does not sign off; record that instead (`--review-outcome indeterminate`,
or `unavailable` when you could not run). Only `signed` counts; the others
stay on the record as a review gap.

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
the finding. For a fix, read `canon query --kind evidence --history --json`
for that scenario's records: expect an earlier non-faithful version, or a
named closest reproduction in `--ref`; record a finding when neither exists.
Put one blast-radius fact in the finding or review `--summary`: what else the
change touches that the checks do not exercise. Read the waivers too:
`status` lists every `--override-reason` an implementer or the orchestrator
used (it is echoed in `data.overrides[]` of the write that took it). A
waiver whose reason does not hold is a finding. Do all of this before you
sign.

## A known gap is the human's to accept

`divergence accept` is for a scenario whose honest evidence is `divergent`
and whose gap is limited by something outside this change (an upstream
limit, a platform bug) and is tracked there. It records a human's decision,
so it refuses every agent role (`HUMAN_ONLY`), you included. What you do:
confirm the evidence really is divergent and the limit is outside this
change, then report to the human the scenario ids, the reason, the
tracking reference and an expiry when the limit should be gone by a date.
The human runs `canon divergence accept <ids> --actor-id <name> --reason
"<why>" --tracking <ref>` themselves, from a session that did not attest the
evidence (`DIVERGENCE_ACCEPT_REFUSED` otherwise). Never suggest it to skip
a fix the implementer can make in this change. The acceptance pins that
evidence: new evidence for the scenario, or its expiry, ends it, and the
human accepts (or the implementer fixes) again. An accepted gap counts as
covered for the ship guard and stays listed as `accepted-divergence` in
`status` and `gate check`.

## Record

- Every defect is a finding; `finding list` shows round, seq, severity and
  disposition, so the next seq and the open blockers are one command away.
  `--rev` of a finding is the commit the round reviewed; `--introduced-rev`
  is the commit that introduced the defect and has no default: source it
  (`git log -S`, `git blame`) or leave it out, never guess it. Many findings
  at once are one `finding add --input` with an array, validated before any
  write.
- Close a finding once it is dealt with: `finding close <slug> --round <n>
  --seq <n> --disposition fixed` records `--resolution-rev` (default
  `HEAD`). When you close it `rejected` or `deferred`, give the reason in
  one line with `--reason "<why>"`: the close is refused without it
  (`FINDING_REASON_REQUIRED`), and `finding list` shows it as
  `[reason: …]` so the next reader sees why it was not fixed. Closing a
  finding that already has that disposition is `unchanged`.
- Sign off in one batch with `review scenario <ids>`, all or nothing. It writes
  directly; nothing to promote. canon stores and re-checks attached bytes
  and never compares images.
- Pass `--scenario-id` for each scenario a finding is about. A blocker you
  close `fixed` then stays blocking until the implementer records evidence
  again for those scenarios alone (`evidence add --scenario-id <id>`); the
  close inherits the list. A blocker naming no scenario asks for the whole
  suite again (`evidence add --all-scenarios`), which on a big suite is
  a long e2e run for a one-scenario fix, so name them. Your pinned review
  stays current when the new evidence says the same, otherwise look again.
- Fixes you ask for return as a re-review of the same change. When new
  evidence changes a verdict or the checks and so stales your review, look
  again before you sign it again.
  A chore has no scenarios: review it with one change-level review, `canon
  review change <slug> --actor-id <me> --session-id <session>
  --upstream-ref <ref>` (or `--original-spec-ref <path>`). It signs the
  chore's invariants and checks against the diff and pins its brief, so
  editing the brief stales it. The chore also needs new evidence on every
  scenario whose code changed since its latest evidence (the implementer's
  batch) before it ships; a docs-only chore needs your review alone.

**Done when:** `canon gate check` lists no `unreviewed-promotion` violation
and no open blocker for the change (read `data.violations[]` in `canon gate
check --json`; `canon finding list --change-id add-login --open` shows the
open findings), or each remaining one is a finding you recorded and the
implementer owns. Your last write's `meta.next` names the next step.

## Retry safety

`review scenario`, `review change` and `finding add` are content-keyed: an
identical repeat is reported `unchanged` (`meta.unchanged: true`, or the
element's `status` in a batch), so after a timeout run the same command
again. A different finding under an occupied round and seq is refused
(`FINDING_SEQ_OCCUPIED`): take the next seq. `finding close` with the same
disposition is a no-op too.

## Excuses that fail

| The thought | The reality |
|---|---|
| "I will review this under a new session label." | A review is a separate process with its own real session id; a relabelled self-review is not independent (#39). |
| "The tests pass, so I can sign." | Read the test against the Then steps first; a green test can prove nothing. |
| "I cannot run the tests, but the evidence looks right." | Say so and record `indeterminate` or `unavailable`; never sign what you could not run. |
| "This is a minor defect; I will skip the finding." | Every defect is a finding; severity says how much it matters. |
| "I will edit the brief to fix it." | A brief defect is a finding, not an edit. |
| "I will accept the gap myself, the limit is real." | `divergence accept` refuses agent roles; report the gap to the human. |

Reference: `canon skills show canon-review`.
