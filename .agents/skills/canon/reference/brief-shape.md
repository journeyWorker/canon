# Brief pass: shape

Make the brief's structure what `canon brief check` expects. Run it when the
check reports `brief.section-missing`, `brief.section-duplicate`,
`brief.section-order`, `brief.placeholder`, `brief.flow-no-steps`,
`brief.waiver-invalid`, or (chore) `brief.chore-section`,
`brief.chore-category`, `brief.chore-empty`.

## Scope

`.canon/changes/<slug>/brief.md`: section markers, order, and leftover
template text. A feature brief runs intent → outcome → flow → detail →
acceptance → questions → out-of-scope; a chore brief runs intent → category →
invariants → checks → out-of-scope (`<!-- canon:kind chore -->` under the
title). Sections are found by their `<!-- canon:section <key> role=… -->`
marker, never by heading text.

## Inspect

- `canon brief check --json`: each finding names the rule, section key, line and hint.
- Every section present once, in order, with its marker; no `<!-- canon:hint … -->` comment left; no `TODO`/`TBD`; no empty section (write `None.` where nothing applies).
- The flow is a numbered list with ids `F1`, `F2`, … each reading sees → does → becomes.
- Intent describes what the product does today and Outcome what is true after; leftover implementation paths belong in a `Source:` line or nowhere.
- A chore's category is exactly one of `refactor`, `dependency`, `build`, `ci`, `infra`; its invariants and checks say something.
- A `canon:waive <rule-id> <section-key> "<reason>"` comment is well formed with a real reason.

## NEVER

- Never fill a section with text you made up to silence a placeholder: write what the request and the repo support, and put the rest in Questions.
- Never delete a section, retag a marker's role, or add a global waiver to pass. A waiver is one rule in one section with a reason the human can read.
- Never turn a chore's missing invariants into "none" to pass: behavior-changing work is a feature change.

## Verify

`canon brief check` reports none of the rules above. Report only the ones it no longer lists.

## Next

`clarify` if the Questions section still has open lines; `harden` if flow and detail disagree; otherwise report to the human and `canon brief approve` from their answer.

## Invariants (every brief pass)

- Never invent policy. An edit that could change what the gate demands, who approves, or what ships goes to the brief's Questions and assumptions, not into the text as a decision.
- Explaining writes nothing. Asked what a finding means or what a pass would do, answer in chat and leave the brief untouched.
- No pass requires another as a prerequisite. Run the one the findings or the human point at, in any order.
- A pass closes only the findings it rechecked: rerun `canon brief check` after the edit and report a finding as closed only when it is gone from that run. Never claim a rule you did not rerun.
- Write in the brief's language (`language:` in `canon.yaml`). Edits touch `.canon/changes/<slug>/brief.md` only. A brief edited after approval is not approved: it needs a new human answer and `canon brief approve` again.
