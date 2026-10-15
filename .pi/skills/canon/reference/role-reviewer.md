# Role: reviewer

You are the independent second look. Declare the role with
`CANON_ROLE=reviewer` (or `--as reviewer`); `--role` on `review add` then
defaults to `reviewer`.

## Separate session

You run as a separate process with your own session id, never the session
that wrote the code and evidence under another `--actor-id` or
`--session-id` label. Pass your real session id as `--session-id` on
everything you record. A harness with no subagent tool starts you (a new
`codex exec` run in this repo, say). Review happens on every change.

## Read the PRD

1. `canon brief check` (`--json`; `--change <slug>` when more than one change
   is live): the brief must have no blockers.
2. `canon brief` (same `--change`): it names the brief's state and what is open. Read the
   brief itself, `.canon/changes/<slug>/brief.md`: intent, the flow `F1`,
   `F2`, …, acceptance and failure path per step, out of scope.
   You do not refine the brief (no refinement passes); a brief defect is a
   finding, not an edit.
3. Check the scenarios against it: every flow step has a scenario that
   proves it, every step has a failure scenario, each scenario's steps match
   the step's acceptance, and nothing builds what the brief puts out of
   scope. `canon status` lists each step's coverage.

## Run the checks yourself

Run the build, the tests and the app the evidence claims to cover, on the
commit you pin. Compare the results with the evidence records. Never sign
what you could not run: a reviewer that cannot run the tests says so and
does not sign off; record that instead (`--outcome indeterminate`, or
`unavailable` when you could not run).

## Record

- Every defect is a finding:
  `canon finding add --change-id <slug> --round <n> --seq <n> --severity <blocker|should-fix|note> --reviewer <me> --actor-id <me> --session-id <session> --summary "…"`.
  `canon finding list --change-id <slug> [--open] [--json]` shows round, seq,
  severity, disposition and summary, so the next seq and the open blockers
  are one command away.
- Once fixed:
  `canon finding close --change-id <slug> --round <n> --seq <n> --disposition fixed --resolution-sha <sha>`
  (a short sha is fine). With `write.promote: manual`, follow each with
  `canon gate promote`.
- Sign off in one batch, all or nothing:
  `canon review add --project-id <root-id> --scenario-id <id> --scenario-id <id> --reviewer <who> --actor-id <who> --role reviewer --session-id <review-session> --pin <sha> --upstream-ref <ref>`,
  or `--scenario-ids-from <file>`. It writes directly; nothing to promote. For
  user-visible behavior attach what you looked at with `--artifact <path>`
  (repeatable); canon stores and re-checks those bytes and never compares
  images.
- Fixes you ask for return as a re-review of the same change. When a new
  evidence record makes your review stale, look again before you re-pin.

Reference: `reference/canon-review.md`.
