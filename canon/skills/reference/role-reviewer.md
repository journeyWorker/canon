# Role: reviewer

You are the independent second look. Declare the role with
`CANON_ROLE=reviewer` (or `--as reviewer`); `--role` on `review add` then
defaults to `reviewer`.

## Commands

Every line is complete; fill the `<placeholders>`. `<root-id>` is the `id` of
a `specs.roots[]` entry in `canon.yaml` (`root` by default).

```bash
canon brief check --change-id <slug> --as reviewer --json
canon gate check --as reviewer --json                 # once, before review add
canon finding add --change-id <slug> --round <n> --seq <n> --severity <blocker|should-fix|note> --reviewer <me> --actor-id <me> --session-id <session> --summary "<defect>" --scenario-id <id> --scenario-id <id> --as reviewer --json   # one --scenario-id per scenario the defect is about
canon finding list --change-id <slug> --open --as reviewer --json
canon finding close --change-id <slug> --round <n> --seq <n> --disposition fixed --resolution-sha <sha> --as reviewer --json
canon review add --project-id <root-id> --scenario-id <id> --scenario-id <id> --reviewer <me> --actor-id <me> --role reviewer --session-id <session> --pin <sha> --upstream-ref <ref> --as reviewer --json
canon review add --project-id <root-id> --scenario-ids-from - --reviewer <me> --actor-id <me> --role reviewer --session-id <session> --pin <sha> --upstream-ref <ref> --as reviewer --json   # ids on stdin, one per line
canon review add --project-id <root-id> --change-id <chore-slug> --reviewer <me> --actor-id <me> --role reviewer --session-id <session> --pin <sha> --upstream-ref <ref> --as reviewer --json   # a chore: one change-level review of its invariants and checks against the diff
canon divergence accept --project-id <root-id> --scenario-id <id> --reason "<why the gap stays>" --tracking <issue-url-or-ref> [--expiry YYYY-MM-DD] --reviewer <me> --actor-id <me> --session-id <session> --as reviewer --json
canon artifacts verify --full --as reviewer --json    # the files the evidence and reviews bind are in the artifact store; refuses when an object store is unreachable
canon query --kind evidence --history --as reviewer --json   # every version, not only the current record
```

`review add` needs `--upstream-ref <ref>` or `--original-spec-ref <path>`;
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

Run `gate check` once, before `review add`: its `unreviewed-promotion`
violations are the subjects and scenarios that need your review. Do not loop
it. After `review add`, read its `meta.next` instead of calling `gate check`
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

Passing is not enough: read each bound test against its scenario's Then
steps. Record a finding (`finding add`) when a check pins behavior the
scenario does not require, or skips a Then step the scenario states: such a
test passes without proving the scenario. Do this before you sign.

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
  Close a finding once it is fixed.
- Sign off in one batch with `review add`, all or nothing. It writes
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
  review add --project-id <root-id> --change-id <slug> --reviewer <me> --role
  reviewer --pin <sha> --upstream-ref <ref> …`. It signs the chore's
  invariants and checks against the diff and pins its brief, so editing the
  brief stales it. The chore also needs evidence recorded after its approval
  (the implementer's `--all-scenarios` run) before it ships.

Reference: `reference/canon-review.md`.
