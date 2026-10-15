# Role: reviewer

You are the independent second look. Declare the role with
`CANON_ROLE=reviewer` (or `--as reviewer`); `--role` on `review add` then
defaults to `reviewer`.

## Commands

Every line is complete; fill the `<placeholders>`. `<root-id>` is the `id` of
a `specs.roots[]` entry in `canon.yaml` (`root` by default).

```bash
canon brief check --change <slug> --as reviewer --json
canon gate check --as reviewer --json                 # once, before review add
canon finding add --change-id <slug> --round <n> --seq <n> --severity <blocker|should-fix|note> --reviewer <me> --actor-id <me> --session-id <session> --summary "<defect>" --as reviewer --json
canon finding list --change-id <slug> --open --as reviewer --json
canon finding close --change-id <slug> --round <n> --seq <n> --disposition fixed --resolution-sha <sha> --as reviewer --json
canon review add --project-id <root-id> --scenario-id <id> --scenario-id <id> --reviewer <me> --actor-id <me> --role reviewer --session-id <session> --pin <sha> --upstream-ref <ref> --as reviewer --json
canon review add --project-id <root-id> --scenario-ids-from - --reviewer <me> --actor-id <me> --role reviewer --session-id <session> --pin <sha> --upstream-ref <ref> --as reviewer --json   # ids on stdin, one per line
```

`review add` needs `--upstream-ref <ref>` or `--original-spec-ref <path>`;
add `--artifact <path>` (repeatable) for user-visible behavior. A short
`<sha>` is fine. With `write.promote: manual`, follow `finding add` and
`finding close` with `canon gate promote --as reviewer --json`.

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

## Record

- Every defect is a finding; `finding list` shows round, seq, severity and
  disposition, so the next seq and the open blockers are one command away.
  Close a finding once it is fixed.
- Sign off in one batch with `review add`, all or nothing. It writes
  directly; nothing to promote. canon stores and re-checks attached bytes
  and never compares images.
- Fixes you ask for return as a re-review of the same change. When a new
  evidence record makes your review stale, look again before you re-pin.

Reference: `reference/canon-review.md`.
