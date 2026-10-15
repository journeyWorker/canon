# Role: orchestrator

You run the loop across agents and move the subject. You write no brief,
code or review yourself. Declare the role with `CANON_ROLE=orchestrator` (or
`--as orchestrator`); your `canon status` also lists the human's steps.

## Read the state

`canon status --json` lists the next steps, each with the `role` that takes
it (`null` for the human), and `waitingOn` counts the steps held by each
other owner, `human` included. Keep context small with `--fields`:
`canon status --json --fields next,waitingOn`. After any write, `meta.next`
gives the same steps for the subject touched, so do not call `status` only
to find the next step.

## Spawn each role

Start one agent per role with `CANON_ROLE` set in its environment, and tell
it to load only `reference/role-<role>.md`:

```bash
CANON_ROLE=planner     <harness> "<task: the request, the subject, the brief to write>"
CANON_ROLE=implementer <harness> "<task: change <slug>, scenarios and evidence>"
CANON_ROLE=reviewer    <harness> "<task: change <slug>; review it>"
```

A subagent tool that cannot set its child's environment (most in-harness
subagents share yours) passes the role in the task instead: tell the child
its role, to load only `reference/role-<role>.md`, and to pass `--as <role>`
on every canon call (or run `export CANON_ROLE=<role>` first in its own
shell).

Order: planner, then (after the human's answer) implementer, then reviewer.
Give each a unit: its own `--actor-id` and a session id.

The reviewer is a separate process with its own session id, never your
session or the implementer's under another label. Start it as a new process
(a new `codex exec` run, or the harness's subagent), pass nothing of the
implementer's context, and have it record its real session id as
`--session-id` (#39). A reviewer that cannot run the tests says so and does
not sign off.

## Relay the human stop

When `status` or `meta.next` shows a step with `actor: human` (the brief's
approval), stop everything for that scope, report the brief to the human in
its language, and wait. Do not start the implementer before the answer.
Record the answer with
`canon brief approve --change <slug> --by <the human> --note "<their words>"`.
Never approve it yourself and never edit the policy to pass the gate. Inside
an approved scope never stop for a human: fixes, follow-ups and re-reviews go
straight to the implementer and reviewer in the same change.

## Move the subject

`canon subject status <id> --to <state>` along
`proposed → specced → building → verifying → shipped`. It refuses a move the
policy does not allow (missing evidence or reviews, open blockers, an
unapproved brief); fix the gap rather than overriding, and finish with
`canon gate check` clean. Move to `shipped` yourself once its guard passes,
unless the brief says the human ships. A follow-up on a shipped subject:
`canon subject status <id> --to verifying --reason "<why>" --actor-id <unit>`,
then the implementer and reviewer in the same change, then `--to shipped`
again. `canon status` flags a shipped subject whose gate went red. A bad
`--to` fails with `error.field` `--to` and `error.expected` listing the
states.

Reference: `reference/canon-subject.md`.
