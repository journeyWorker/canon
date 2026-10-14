# Brief pass: polish

Tighten the prose of the normative sections so a reader can act on it. Run it
last or whenever `canon brief` reports prose advisories; it never blocks
approval.

## Scope

Sections with `role=normative`: Desired outcome, User flow, Step detail,
Acceptance (feature) or Invariants and Checks (chore). Context and question
sections are not judged. Prose rules ship for `language: en` and `ko`; any
other language gets the structure rules only.

## Inspect

- `canon brief check --json`, the `prose.*` findings: `evaluative`, `connective`, `contrast`, `colloquial` (ko), `degree`, `sentence-length`, `paragraph-length`, `key-sentence`. Each carries the matched phrase or the threshold and the measured value.
- Replace an evaluative or degree word with the observable fact it stood for. Split a long sentence or paragraph. Cut connectives that only decorate.
- Each normative section carries exactly one key sentence marked `==**…**==`: the one line a reader must not miss.
- A finding is a prompt to reread, not a verdict: if the phrase is the precise term (a state name, a quoted label), keep it and waive that rule in that section with `<!-- canon:waive <rule-id> <section-key> "<reason>" -->`.

## NEVER

- Never change meaning: no flow step, rule, limit or failure path added, removed or softened while rewording.
- Never rewrite to dodge a rule (swap a listed phrase for a near synonym); say the concrete thing.
- Never polish sections that are not normative, or touch flow ids and subheadings.
- Never treat a clean prose run as a claim about who wrote the brief or whether it is good.

## Verify

`canon brief check` no longer lists the `prose.*` findings you rewrote (or lists them as waived with a reason); no blockers appeared.

## Next

Report to the human and `canon brief approve` from their answer.

## Invariants (every brief pass)

- Never invent policy. An edit that could change what the gate demands, who approves, or what ships goes to the brief's Questions and assumptions, not into the text as a decision.
- Explaining writes nothing. Asked what a finding means or what a pass would do, answer in chat and leave the brief untouched.
- No pass requires another as a prerequisite. Run the one the findings or the human point at, in any order.
- A pass closes only the findings it rechecked: rerun `canon brief check` after the edit and report a finding as closed only when it is gone from that run. Never claim a rule you did not rerun.
- Write in the brief's language (`language:` in `canon.yaml`). Edits touch `.canon/changes/<slug>/brief.md` only. A brief edited after approval is not approved: it needs a new human answer and `canon brief approve` again.
