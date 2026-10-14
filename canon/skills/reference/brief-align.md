# Brief pass: align

Check that scenarios cover the brief's flow, and that the brief still says
what the scenarios prove. Run it after scenarios exist, or when the check
reports `brief.flow-uncovered` or `brief.flow-no-failure`, which gate
shipping, not approval.

## Scope

The flow steps of the current change against the `.feature` scenarios that
carry `@flow:<change>.<F-id>` (written by `canon scenario new --flow <F-id>`).
A chore adds no scenarios and has no flow: this pass does not apply to it.

## Inspect

- `canon status`: each flow step with its scenarios and their evidence and review state; steps with no scenario or no failure scenario are flagged.
- `canon brief check --json`: the coverage findings per step.
- Each step has a `--case failure` scenario for the failure path the Acceptance names, plus `happy` and `edge` where the step has them.
- A scenario's title and steps say what the Acceptance line says. A scenario that proves more than the brief claims means the brief is thin; a brief claim with no scenario is a gap.

## NEVER

- Never retag an existing scenario's case or flow to satisfy coverage: add a new scenario with `canon scenario new --flow`.
- Never edit the brief to match a scenario that drifted; scenario-led brief edits are a scope change and need a new human answer.
- Never write scenario ids into the brief.
- Never report a step covered because a scenario exists: covered means indexed, and evidence and review are checked separately by the gate.

## Verify

`canon brief check` no longer lists the coverage rules; `canon status` shows every step with a failure scenario.

## Next

Implement, evidence and review (loop steps 5 to 7). If the brief had to change, the human answers again.

## Invariants (every brief pass)

- Never invent policy. An edit that could change what the gate demands, who approves, or what ships goes to the brief's Questions and assumptions, not into the text as a decision.
- Explaining writes nothing. Asked what a finding means or what a pass would do, answer in chat and leave the brief untouched.
- No pass requires another as a prerequisite. Run the one the findings or the human point at, in any order.
- A pass closes only the findings it rechecked: rerun `canon brief check` after the edit and report a finding as closed only when it is gone from that run. Never claim a rule you did not rerun.
- Write in the brief's language (`language:` in `canon.yaml`). Edits touch `.canon/changes/<slug>/brief.md` only. A brief edited after approval is not approved: it needs a new human answer and `canon brief approve` again.
