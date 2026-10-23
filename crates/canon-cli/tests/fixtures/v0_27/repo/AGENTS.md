# My project

Hand-written notes that migrate must not touch.

<!-- canon:begin -->
## Canon

Features in this repo are managed with canon. Start with `canon status`,
and read the canon skill (`canon skills install`) before you change a
feature.

Unsure of a command or its flags: `canon schema <command> --json`. Pass
`--json` and read `error.code` and `error.hint` when a command fails.

Roles: if you were spawned as one (planner, implementer, reviewer, orchestrator), set `CANON_ROLE` (or pass `--as`) and load only `reference/role-<role>.md` from the canon skill; with no role, follow `canon status`.

The loop:

1. Brief: for a new subject or a scope change, write the PRD brief in the change's `.canon/changes/<slug>/brief.md` (intent, outcome, user flow `F1`, `F2`, …, step detail, acceptance, questions and assumptions, out of scope), in the `language:` of `canon.yaml` (unset: the request's language, with "which language?" in the questions). Run `canon brief check` until it has no blockers, then stop and report the brief to the human in that language: build and attest nothing for that scope until the human answers. The approval is never assumed, even when the human said they cannot answer until you report; the brief is that report. One human answer approves the brief and the policy; record it in one call: `canon brief approve --change-id <slug> --by <the human> --note "<their words>"`. Never approve without a human answer. A brief edited after its approval needs a new answer.
2. Subject: `canon subject new`, then `canon change new` for the change (`--kind chore` for a refactor, dependency, build, CI or infra change that alters no behavior: short brief, no scenarios, the suite run again with `canon evidence add --all-scenarios`, then one change-level review with `canon review add --change-id <slug>`; behavior-changing work is never a chore); it writes the brief skeleton `.canon/changes/<slug>/brief.md`. A change from an external plan (an openspec `proposal.md`) gets its brief there too: a `brief.md` whose `Source: <path>` line points at the plan; canon never edits the plan.
3. Scenarios: `canon scenario new --flow <F-id>`, one per behavior of a flow step, failure paths included (`--case failure`).
4. Units: split the work; one actor id and session per unit.
5. Implement: tests named by scenario id.
6. Evidence: `canon evidence add` (it promotes itself under `write: { promote: auto }`; only under `write.promote: manual` run `canon gate promote` after it).
7. Independent review: a different session (a separate process with its own session id, never this session under another label; without a subagent tool, start one), one that can run the checks it signs off, records `canon review add` and `canon finding add`, on every change; a reviewer that cannot run the tests says so and does not sign off.
8. Transition: `canon subject status`, with `canon gate check` clean; move to `shipped` yourself once its guard passes, unless the brief says the human ships. The ship guard and `canon evidence add` refuse while the brief is unapproved or was edited after approval, and shipping also needs every flow step covered by a scenario with a failure case; they are backstops, the stop is at step 1.

Inside an approved scope, never stop for a human: fixes, follow-ups and review rounds stay in the approved change, never `canon change new` (a new change is a new scope), and go straight to scenario, implement, evidence and review; a follow-up that adds or changes a step of the brief edits it, which needs a new answer. `tasks.md` is an optional work note nothing gates on. When no human can answer one of the brief's questions, write your assumption into the brief and proceed on it; that covers the brief's questions and work inside an approved scope, never the approval. `.canon/policy.yaml` is what the gate grades against.
<!-- canon:end -->

## Team rules

Text after the block stays too.
