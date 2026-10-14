# Brief pass: clarify

Turn what is unresolved into questions a human can answer in one reply, and
make every assumption explicit. Run it when `canon brief` says lines under
the questions section are still open, or the request left a decision the
brief quietly made.

## Scope

The Questions and assumptions section, plus any sentence elsewhere in the
brief that decides something the request did not say.

## Inspect

- Each open line: is it a question or an assumption you will build on? Label it so.
- Decisions hidden in Intent, Outcome, Step detail or Acceptance that the request never stated: move each to Questions with your assumption.
- Anything that could change policy (what the gate demands, who approves, what ships, retention, access): it is a question, never a decision.
- Whether the language is settled: with `language:` unset, "which language?" belongs in the questions.
- Questions answerable from the repo or a tool: answer them from there instead of asking.

## NEVER

- Never answer a question yourself by editing the brief as if the human had; an assumption is stated as an assumption.
- Never drop a question you could not answer to make the section read as done. `None.` is for a section that is truly empty.
- Never ask what a command, the code or the request already answers.
- Never ask more than the human can answer in one reply: order by what changes the scope most, keep the rest as assumptions.

## Verify

Every open line is a question with a proposed default or a stated assumption. `canon brief check` still has no blockers (the pass changes no structure). `canon brief` still counts the lines left open, and those go in the report to the human, not away.

## Next

`harden` if the flow changed; otherwise report to the human, who answers the questions with the approval.

## Invariants (every brief pass)

- Never invent policy. An edit that could change what the gate demands, who approves, or what ships goes to the brief's Questions and assumptions, not into the text as a decision.
- Explaining writes nothing. Asked what a finding means or what a pass would do, answer in chat and leave the brief untouched.
- No pass requires another as a prerequisite. Run the one the findings or the human point at, in any order.
- A pass closes only the findings it rechecked: rerun `canon brief check` after the edit and report a finding as closed only when it is gone from that run. Never claim a rule you did not rerun.
- Write in the brief's language (`language:` in `canon.yaml`). Edits touch `.canon/changes/<slug>/brief.md` only. A brief edited after approval is not approved: it needs a new human answer and `canon brief approve` again.
