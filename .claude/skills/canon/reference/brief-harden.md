# Brief pass: harden

Make the flow and its step detail agree, and give each step an observable
result and a failure path. Run it when the check reports
`brief.flow-duplicate-id` or `brief.detail-mismatch`, or a step reads vague.

## Scope

User flow, Step detail and Acceptance sections of a feature brief. A chore
brief has none of them: harden its Invariants and Checks instead.

## Inspect

- Flow ids `F1`, `F2`, … are unique and in order; Step detail has one `### F<n> …` subheading per step, same ids, same order.
- Each step reads sees → does → becomes, and names who and what state it starts from.
- Acceptance states, per step, what must be observable and which failure path is covered (invalid input, empty state, interrupted, repeated, denied).
- Step detail holds rules, states and edge cases of that step only; a rule that spans steps is stated once, where it first applies.
- Intent states current behavior and Outcome the desired behavior; interfaces are described by behavior (inputs, outputs, errors); no file path, line number or function name stands in a section that decides the work (a `Source:` line may).
- A flow step is one unit an implementer can verify with one named command; split a step that needs two.
- Acceptance writes no scenario ids: scenarios do not exist yet and point at steps.

## NEVER

- Never renumber ids after scenarios carry `@flow:<change>.<F-id>` tags without moving the tags (`canon status` lists coverage per id).
- Never add a step the request did not imply; an extra step is scope the human has not approved: put it in Questions.
- Never write a failure path as "handles errors": name the condition and what the user sees.
- Never invent a rule, limit or copy the request and repo do not give: ask in Questions.

## Verify

`canon brief check` no longer lists `brief.flow-duplicate-id` or `brief.detail-mismatch`; every flow id has a detail subheading and an acceptance line with a failure path.

## Next

`align` once scenarios exist; `polish` for prose; otherwise report to the human.

## Invariants (every brief pass)

- Never invent policy. An edit that could change what the gate demands, who approves, or what ships goes to the brief's Questions and assumptions, not into the text as a decision.
- Explaining writes nothing. Asked what a finding means or what a pass would do, answer in chat and leave the brief untouched.
- No pass requires another as a prerequisite. Run the one the findings or the human point at, in any order.
- A pass closes only the findings it rechecked: rerun `canon brief check` after the edit and report a finding as closed only when it is gone from that run. Never claim a rule you did not rerun.
- Write in the brief's language (detected from the text; `language:` in `canon.yaml` overrides it). Edits touch `.canon/changes/<slug>/brief.md` only. A brief edited after approval is not approved: it needs a new human answer and `canon brief approve` again.
