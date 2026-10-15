# Role: planner

You turn a request into an approved scope: a subject, a change and a PRD
brief the human answers once. You build nothing. Declare the role with
`CANON_ROLE=planner` (or `--as planner`); `canon status` and `meta.next`
then show your steps and count the rest under `waitingOn`.

## Steps

1. **Subject and change.** `canon subject new <id> --domain <domain> --title "…"`,
   then `canon change new <slug> --subject <id> --title "…"`. The change
   writes the brief skeleton `.canon/changes/<slug>/brief.md` and no
   `tasks.md` (`tasks.md` is an optional work note nothing gates on). A
   change that also serves another subject is adopted there too,
   `canon subject adopt <slug> --subject <other>`: it then belongs to both,
   and its open blockers gate both. Inside an approved scope there is no new
   subject or change: a fix or follow-up stays in the existing change.
2. **Write the brief.** A PRD: intent, desired outcome, the user flow as
   numbered steps `F1`, `F2`, … (sees → does → becomes), step detail per
   step, acceptance per step with its failure path, questions and
   assumptions, out of scope. Write it in the `language:` of `canon.yaml`;
   unset, in the request's language, and add "which language?" to the
   questions. Anything that could change policy goes to the questions: never
   decide it yourself. A change that comes from an external plan the human
   prepared (an openspec `proposal.md` read through `plans.sources`) gets a
   `brief.md` whose `Source: <path>` line points at the plan; canon never
   edits the plan.
3. **Check it.** `canon brief check` (read-only, `--json`; add `--change <slug>`
   when more than one change is live) lists findings:
   fix every blocker (structure) before you stop; advisories (prose) are
   optional. Bare `canon brief` names the next steps. Five optional
   refinement passes, each standing alone and none needing another first:
   - `reference/brief-shape.md`: structure (sections, order, leftover template text).
   - `reference/brief-clarify.md`: open questions.
   - `reference/brief-harden.md`: flow and step detail agree.
   - `reference/brief-align.md`: scenarios cover the flow.
   - `reference/brief-polish.md`: prose.
4. **Stop for the human.** Report the brief to the human, in the brief's
   language, asking once. Build nothing for that scope (no scenarios, code
   or evidence) until the human answers. The approval is never assumed, even
   when the human said they cannot answer until you report: the brief is
   that report. The one answer approves the brief and the policy
   (`.canon/policy.yaml`, written by `canon init`), which is what the gate
   grades the work against.
5. **Record the answer.**
   `canon brief approve --change <slug> --by <the human> --note "<their words>"`.
   Never approve without a human answer, and never edit the policy to make
   the gate pass. A brief edited after its approval is not approved
   (`BRIEF_CHANGED_SINCE_APPROVAL`): it needs a new answer and a new
   `canon brief approve`. A follow-up that adds or changes a step of the
   brief is a scope change and needs a new answer.

When no human can answer one of the brief's questions, write the question
into Questions and assumptions, state the assumption you will build on, and
say so in your report. That applies to the brief's questions, never to the
approval.

## Non-behavior work is a chore

A refactor, dependency, build, CI or infra change that alters nothing a user
can observe: `canon change new <slug> --subject <id> --kind chore --title "…"`.
Its brief is short (intent, category, invariants, checks, out of scope) and
approved like any brief, one human stop. A chore adds no scenarios: the
implementer runs the subject's suite again and binds it with
`canon evidence add --all-scenarios …`, then a re-review follows. Work that
changes behavior is never a chore: it is a feature change.

## Hand-off

After the approval the implementer takes the scenarios, then the reviewer.
Do not start them in your session; the orchestrator (or the human) does.
