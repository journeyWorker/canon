# Role: orchestrator

You run the loop across agents and move the subject. You write no brief,
code or review yourself. Declare the role with `CANON_ROLE=orchestrator` (or
`--as orchestrator`); your `canon status` also lists the human's steps.

## Commands

Every line below is generated from the CLI's own spec and runs as written against the
example story (subject `demo`, change `add-login`, scenario `auth.login.01`): swap in
your ids. `canon schema <command> --json` lists every option.

<!-- canon:generated commands-orchestrator begin -->
```bash
canon adapter authorize --response adapter-response.json --repo .                                                                                                                          # Authorize declared capabilities against repository policy (preflight only)
canon adapter validate --response adapter-response.json                                                                                                                                    # Read and validate one provider-neutral response envelope
canon artifacts move --tier cold                                                                                                                                                           # Move the local artifact blobs to an object-store tier, verify them, and remove the local copies
canon artifacts store                                                                                                                                                                      # Store the bound files of existing evidence records and current reviews in the artifact store (.canon/artifacts/sha256/)
canon artifacts verify                                                                                                                                                                     # Check that the artifact store holds every file the latest evidence and current reviews bind (--full re-hashes the stored bytes)
canon completion zsh                                                                                                                                                                       # Write the shell completion script for canon (bash, zsh or fish)
canon context                                                                                                                                                                              # Show what you can author here: record kinds, fields, enums, policies
canon context-pack add --manifest .canon/context-manifest.json                                                                                                                             # Create an immutable context pack from a repository-local JSON manifest
canon context-pack show sha256:0000000000000000000000000000000000000000000000000000000000000000                                                                                            # Show a context pack after verifying its manifest and content objects
canon context-pack verify sha256:0000000000000000000000000000000000000000000000000000000000000000                                                                                          # Verify a context pack's manifest and immutable content objects
canon dashboard                                                                                                                                                                            # Serve the local status dashboard
canon demo init --repo canon-demo                                                                                                                                                          # Scaffold the demo repo (gate starts RED)
canon demo review                                                                                                                                                                          # Record the missing reviewer evidence (gate turns GREEN)
canon dispatch begin --regime implementer/my-repo/api/abcdef012345 --role implementer                                                                                                      # Mint a Run manifest with retrieved guidance and context/policy lineage
canon dispatch diff                                                                                                                                                                        # Diff the declared plan DAG against the observed execution graph
canon dispatch end 01ARZ3NDEKTSV4RRFFQ69G5FAV --run-outcome succeeded                                                                                                                      # Close a dispatched run with a terminal status and an ended_at
canon divergence list --as-of 2030-01-01                                                                                                                                                   # List the current divergence burn-down state
canon evidence approval-payload --scenario-id auth.login.01 --project-id root --artifact-sha 0123456789abcdef0123456789abcdef01234567 --approver alice --approval-at 2030-01-01T00:00:00Z  # Emit exact bytes for external `ssh-keygen -Y sign -n canon-approval-v1`
canon export --kind evidence --output evidence.json                                                                                                                                        # Export a redacted retention manifest without deleting records
canon finding list --change-id add-login                                                                                                                                                   # List a change's committed findings at their latest version
canon format .canon/specs                                                                                                                                                                  # Validate a spec/artifact corpus against canon's format
canon gate check                                                                                                                                                                           # Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
canon gate install-hooks                                                                                                                                                                   # Install the gate hook into .claude/settings.json / .codex/hooks.json
canon ingest plans                                                                                                                                                                         # Import plans as Change/Task records
canon ingest records                                                                                                                                                                       # Import review/divergence/task/handoff records and derive verdicts
canon ingest sessions                                                                                                                                                                      # Import agent CLI session transcripts (omp/pi, Claude Code, Codex, Hermes)
canon init                                                                                                                                                                                 # Set up canon in a repo (canon.yaml, a starter policy, the AGENTS.md block, a plans home)
canon inventory sync                                                                                                                                                                       # Validate each spec root, then materialize scenario index records
canon ledger compact                                                                                                                                                                       # Move superseded versions of every record out of the ledger into the archive
canon migrate                                                                                                                                                                              # Bring a repo from an earlier canon up to date
canon plugin sync my-plugin                                                                                                                                                                # Validate and write a plugin's overlay records
canon policy show                                                                                                                                                                          # Show the policy's digest and its approval (who, when, which bytes)
canon prompt add reviewer@v1 --manifest .canon/prompt-manifest.json                                                                                                                        # Add a prompt bundle from a repository-local JSON manifest
canon prompt show reviewer@v1                                                                                                                                                              # Select a registered prompt bundle version
canon purge --kind session --before 2020-01-01T00:00:00Z                                                                                                                                   # Remove allowlisted records older than a cutoff
canon query --kind evidence                                                                                                                                                                # Read stored records across every storage tier
canon regime show --role-segment reviewer --repo-name my-repo --area api --hash abcdef012345                                                                                               # Build a canonical regime key for hook scripts
canon report build                                                                                                                                                                         # Write the status report (.canon/REPORT.md)
canon report check                                                                                                                                                                         # Byte-diff .canon/REPORT.md against a fresh build (0 fresh, 1 missing or drifted); writes nothing
canon report snapshot snapshot-out                                                                                                                                                         # Export the panel marts to <DIR>/*.parquet plus manifest.json
canon retrieve --for-role reviewer --domain dev --limit 20                                                                                                                                 # Fetch role-scoped strategy guidance
canon schema evidence add                                                                                                                                                                  # Describe canon's own commands
canon selftest --suite trust-spine-gate                                                                                                                                                    # Run canon's built-in fixture self-tests
canon skills check                                                                                                                                                                         # Check canonical projections without writing; exit 1 on drift, a legacy remnant, or a lock another canon version wrote
canon skills doctor                                                                                                                                                                        # Print diagnostics for canonical projections, the install lock's canon version, legacy remnants, canon projections for a provider that is not selected, and other skills whose description claims build work; reports only, never edits
canon skills install                                                                                                                                                                       # Install the canonical `canon` bundle or legacy developer skills
canon status                                                                                                                                                                               # Show where this repo stands: subjects by status, their gaps, and the next commands
canon strategy approval-payload 01ARZ3NDEKTSV4RRFFQ69G5FAV --evaluation evaluation.json --approver alice --approval-at 2030-01-01T00:00:00Z                                                # The unsigned approval payload bytes an external signer signs (a read
canon strategy promote 01ARZ3NDEKTSV4RRFFQ69G5FAV                                                                                                                                          # Promote a strategy after paired evaluation and signed approval
canon strategy rollback 01ARZ3NDEKTSV4RRFFQ69G5FAV --reason "regressed in production" --actor-id alice --signature-file approval.sig --approval-at 2030-01-01T00:00:00Z                    # Disable an active strategy with durable rollback provenance
canon subject adopt demo --change-id add-login                                                                                                                                             # Link an imported plan change to a subject (adopting it under another subject adds that subject; a change may belong to several)
canon subject move demo --to specced                                                                                                                                                       # Move a subject to another lifecycle state (shipping is evidence-gated; with spec_coverage.require_review, entering its scope is review-gated; shipped → verifying reopens with --reason)
canon subject new payments --domain dev --title Payments                                                                                                                                   # Author a new subject at status `proposed`
canon subject show demo                                                                                                                                                                    # Show a subject (read-only)
canon subject unadopt demo --change-id add-login                                                                                                                                           # Take a change back out of a subject (the mirror of adopt; refused for a change whose brief is approved)
canon tier age                                                                                                                                                                             # Apply canon.yaml aging rules, moving old records to their destination tier
```
<!-- canon:generated commands-orchestrator end -->

## Read the state

`canon status` lists the next steps, each with the `role` that takes it
(`null` for the human), and `waitingOn` counts the steps held by each other
owner, `human` included. Call it once at the start and when resuming, not
after a write: after `subject move --to` or `brief approve`, the write's
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

### Spawn contract

Every spawn prompt names, in this order: the goal (the change slug and the
one flow step or review it serves); the scope it owns (the scenarios or
paths it may write); what it must not touch (the brief, the policy, other
units' scenarios, anything outside its scope); the command that proves it is
done (the unit's verify command, or `canon gate check` for the reviewer); and
its own `--actor-id` and `--session-id`. Two agents never write the same
paths: give parallel implementers disjoint scopes, and where the harness can
isolate a child in its own worktree, use it. The child reports what it
verified apart from what it only claims.

### What each harness gives you

Last reviewed 2026-10-21. A cell marked `verify` is unconfirmed here: check
the harness before relying on it; a harness version moves faster than this
text.

| Harness | In-harness subagents | Isolation | Ask the human | Hooks | Skipped |
|---|---|---|---|---|---|
| claude (`.claude`) | yes, a subagent tool; a child shares your environment, so pass the role in the task | a subagent can run in its own git worktree | yes, a question tool | yes, in the harness settings; `scripts/canon-retrieve-pre-dispatch.sh` is a sidecar, not installed | none |
| codex (`.agents`) | `verify`: none when #39 was filed; start the reviewer as a new `codex exec` run | none from canon: use a separate worktree you create | no in `codex exec`; stop and report | `verify` | in-harness spawn, unless a newer version provides it |
| omp (`.omp`) | yes, the task tool | `verify` | yes, an ask tool | yes | none |
| pi (`.pi`) | `verify`: assume none; start a new `pi` process | none from canon | `verify` | `verify` | in-harness spawn, isolation |

Where a harness has no subagent tool, the reviewer is still a separate
process with its own session id: start it from the shell with its own
`CANON_ROLE=reviewer`. A harness that cannot ask the human means you stop
and report in your final message; you never assume the answer.

The reviewer is a separate process with its own session id, never your
session or the implementer's under another label. Start it as a new process
(a new `codex exec` run, or the harness's subagent), pass nothing of the
implementer's context, and have it record its real session id as
`--session-id` (#39). A reviewer that cannot run the tests says so and does
not sign off.

## Relay the human stop

When `status` or `meta.next` shows a step with `actor: human` (the brief's
approval), stop everything for that scope, report the brief to the human in
its language, and wait. Tell the human they can approve the clean briefs in
`canon dashboard` (the briefs page) or answer in chat; you never click
Approve or fill the form, and you run `brief approve` only from their words. Do not start the implementer before the answer.
Record the answer with `brief approve`. Never approve it yourself and never
edit the policy to pass the gate. A policy that no longer matches the
approval fails closed (`POLICY_CHANGED_SINCE_APPROVAL`, or
`POLICY_APPROVAL_UNREADABLE`): the ship guard and the reopen refuse, and
`--override-reason` does not waive it. Run `canon policy show`, then stop and
ask the human to re-approve (`canon policy approve --approver <human>`) or restore
the approved policy from git; never edit the policy yourself. Inside an
approved scope never stop for a human: fixes, follow-ups and re-reviews go
straight to the implementer and reviewer in the same change.

## Hand off a note, not a copy

A note one session leaves for another (a resume note, a spawn prompt, a
`Handoff` body) points to artifacts and record ids instead of pasting them:
name the change slug, the scenario ids, the evidence or finding keys and the
files, and the next command. Say what was verified (a command that ran, with
its result) apart from what is only claimed. Redact secrets: tokens, keys,
passwords and private URLs never go into a note, a `--ref`, a `--summary`
or a brief. When a `Handoff` body carries a continuation, its `summary`
holds the next command and the verified claims (`reference/canon-vocab.md`).

## Excuses that fail

| The thought | The reality |
|---|---|
| "Edit the policy so the gate passes." | A changed policy fails closed (`POLICY_CHANGED_SINCE_APPROVAL`); ask the human to re-approve or restore it from git. |
| "The reviewer can be me under another label." | The reviewer is a separate process with its own real session id (#39). |
| "Approve the brief; the human said they are busy." | Never approve for the human; report the brief and wait. |
| "Override the refusal to ship." | Fix the gap the refusal names; `--override-reason` does not waive an approval. |
| "I have corrected this agent before." | A correction repeated across runs belongs in a scenario, the policy or a gate, not more prose. Tell the human. |

## Move the subject

Move along `proposed → specced → building → verifying → shipped` with
`subject move --to`. It refuses a move the policy does not allow (missing
evidence or reviews, open blockers, an unapproved brief); fix the gap rather
than overriding. Move to `shipped` yourself once its guard passes, unless the
brief says the human ships. A follow-up on a shipped subject goes back to
`verifying` with `--reason`, then the implementer and reviewer in the same
change, then `--to shipped` again. A chore on a shipped subject (`change
new --change-kind chore` names the reopen command) is approved first, then
reopened the same way; shipping again needs the whole suite evidenced after
the chore's approval (the implementer's `evidence add --all-scenarios`) and
one change-level review of the chore from the reviewer (`review change
<slug>`): the order is approval, evidence, review. A blocker closed
`fixed` needs the suite again. The refusal names the scenarios and
the command; never override them. `canon status` flags a shipped subject
whose gate went red. A bad `--to` fails with `error.field` `--to` and
`error.expected` listing the states.

## Upgraded repo

After a repo moves to a newer canon, run `canon migrate` (it previews), then
`canon migrate --commit --plan-digest <digest>`. Besides the layout, it compacts the ledger (moves every
superseded version to `.canon/archive`, as `canon ledger compact` does) and
re-pins the reviews written before 0.23, which do not count until then (the
gate, `status` and `subject move` print one advisory naming it). Commit
what it changes. In CI run it with full history (`fetch-depth: 0`).

`canon artifacts move --tier <tier>` moves the local artifact store to an
object store and removes the local blobs. It is the repo owner's decision,
not a step of the loop: never run it yourself. If the repo is large, the
store is slow or the human has not asked for it, ask the human first, and
run it only on their answer (the bare run previews it).

Reference: `reference/canon-subject.md`.
