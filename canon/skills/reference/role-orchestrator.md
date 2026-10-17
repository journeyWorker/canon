# Role: orchestrator

You run the loop across agents and move the subject. You write no brief,
code or review yourself. Declare the role with `CANON_ROLE=orchestrator` (or
`--as orchestrator`); your `canon status` also lists the human's steps.

## Commands

Every line is complete; fill the `<placeholders>`.

```bash
canon status --as orchestrator --json --fields next,waitingOn   # at the start and when resuming
canon brief approve --change <slug> --by "<the human>" --note "<their words>" --as orchestrator --json
canon subject status <subject-id> --to <proposed|specced|building|verifying|shipped> --actor-id <unit> --as orchestrator --json
canon subject status <subject-id> --to verifying --reason "<why>" --actor-id <unit> --as orchestrator --json   # reopen a shipped subject (--reason is required here; any other transition accepts it as a note)
canon policy show --as orchestrator --json            # policy digest and its approval: approved, changed, unreadable or unapproved
canon gate check --as orchestrator --json
```

## Read the state

`canon status` lists the next steps, each with the `role` that takes it
(`null` for the human), and `waitingOn` counts the steps held by each other
owner, `human` included. Call it once at the start and when resuming, not
after a write: after `subject status --to` or `brief approve`, the write's
`meta.next` and `meta.waitingOn` are the status for what it touched.

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
Record the answer with `brief approve`. Never approve it yourself and never
edit the policy to pass the gate. A policy that no longer matches the
approval fails closed (`POLICY_CHANGED_SINCE_APPROVAL`, or
`POLICY_APPROVAL_UNREADABLE`): the ship guard and the reopen refuse, and
`--override-reason` does not waive it. Run `canon policy show`, then stop and
ask the human to re-approve (`canon policy approve --by <human>`) or restore
the approved policy from git; never edit the policy yourself. Inside an
approved scope never stop for a human: fixes, follow-ups and re-reviews go
straight to the implementer and reviewer in the same change.

## Move the subject

Move along `proposed → specced → building → verifying → shipped` with
`subject status --to`. It refuses a move the policy does not allow (missing
evidence or reviews, open blockers, an unapproved brief); fix the gap rather
than overriding. Move to `shipped` yourself once its guard passes, unless the
brief says the human ships. A follow-up on a shipped subject goes back to
`verifying` with `--reason`, then the implementer and reviewer in the same
change, then `--to shipped` again. A chore on a shipped subject (`change
new … --kind chore` names the reopen command) is approved first, then
reopened the same way; shipping again needs the whole suite evidenced after
the chore's approval (`evidence add --all-scenarios`) and a re-review. A
blocker closed `fixed` needs the same. The refusal names the scenarios and
the command; never override them. `canon status` flags a shipped subject
whose gate went red. A bad `--to` fails with `error.field` `--to` and
`error.expected` listing the states.

Reference: `reference/canon-subject.md`.
