# Brief pass: shape

Make the brief's structure what `canon brief check` expects. Run it when the
check reports `brief.section-missing`, `brief.section-duplicate`,
`brief.section-order`, `brief.placeholder`, `brief.flow-no-steps`,
`brief.waiver-invalid`, or (chore) `brief.chore-section`,
`brief.chore-category`, `brief.chore-empty`.

## Scope

`.canon/changes/<slug>/brief.md`: section headings, order, and leftover
template text. A feature brief runs intent → outcome → flow → detail →
acceptance → questions → out-of-scope; a chore brief runs intent → category →
invariants → checks → out-of-scope (`kind: chore` in the frontmatter). Sections
are found by their English `##` heading, whatever the brief's language; a
`##` line that is not one of the headings below is body text of the section
before it. `language:` only picks the language of the prose rules and the
guidance.

| Section | Heading | Role | What goes in it |
| --- | --- | --- | --- |
| intent | `## Intent` | context | What problem this solves, for whom, and why now. One to three sentences. |
| outcome | `## Desired outcome` | normative | What is observably true once this ships, from the user's side. |
| flow | `## User flow` | normative | A numbered list; each item starts with its id and reads sees → does → becomes: `1. F1 The user sees … → does … → becomes …`. |
| detail | `## Step detail` | normative | One `### F1 …` subheading per flow step, in the flow's order: rules, states and edge cases of that step. |
| acceptance | `## Acceptance` | normative | Per step: what must be observable, and which failure path is covered. Do not write scenario ids here. |
| questions | `## Questions and assumptions` | question | What is unresolved and what you assumed; anything that could change policy goes here. Write `None.` if nothing. |
| out-of-scope | `## Out of scope` | context | What this change deliberately does not do. Write `None.` if nothing. |
| category (chore) | `## Category` | context | Exactly one of `refactor`, `dependency`, `build`, `ci`, `infra`. |
| invariants (chore) | `## Invariants` | normative | What must not change: behavior, public interfaces, data, outputs. A change that alters any of them is a feature, not a chore. |
| checks (chore) | `## Checks` | normative | How you show the invariants held: the suite that is run again at the chore commit, and anything else you compare. |

`canon change new` writes the brief with these headings and nothing under
them: an empty section is a section not yet written, and `canon brief show`
prints the guidance of each empty one (in `language:`). A wrong heading
(`## Intnet`, `## intent`) is reported with the nearest heading.

## Inspect

- `canon brief check --json`: each finding names the rule, section key, line and hint.
- Every section present once, in order, with its exact English heading; no `TODO`/`TBD`; no empty section (write `None.` where nothing applies).
- The flow is a numbered list with ids `F1`, `F2`, … each reading sees → does → becomes.
- Intent describes what the product does today and Outcome what is true after; leftover implementation paths belong in a `Source:` line or nowhere.
- A chore's category is exactly one of `refactor`, `dependency`, `build`, `ci`, `infra`; its invariants and checks say something.
- Waivers live in the frontmatter, visible on the page, and the approval covers them:
  ```
  ---
  waive:
    - rule: prose.length
      section: brief
      reason: "why the rule does not apply"
      match: "optional text the finding contains"
  ---
  ```
  Each entry has a known `rule`, a `section` key (or `brief`) and a non-empty `reason`; an unknown key is a blocker. Adding one after the approval is a change after the approval.
- A brief that still has `<!-- canon:… -->` comments or Korean headings is in the old format: `canon migrate` (preview first) rewrites it, and the human runs `--commit` when it re-binds an approval.

## NEVER

- Never fill a section with text you made up to silence a placeholder: write what the request and the repo support, and put the rest in Questions.
- Never delete a section, rename a heading to another role, or add a global waiver to pass. A waiver is one rule in one section with a reason the human can read.
- Never turn a chore's missing invariants into "none" to pass: behavior-changing work is a feature change.

## Verify

`canon brief check` reports none of the rules above. Report only the ones it no longer lists.

**Done when:** `canon brief check <slug> --json` lists none of the rules above (read `data.findings[]`); report only the ones it no longer lists.

## Next

`clarify` if the Questions section still has open lines; `harden` if flow and detail disagree; otherwise report to the human, who approves with `canon brief approve` (theirs; agent roles are refused).

## Invariants (every brief pass)

- Never invent policy. An edit that could change what the gate demands, who approves, or what ships goes to the brief's Questions and assumptions, not into the text as a decision.
- Explaining writes nothing. Asked what a finding means or what a pass would do, answer in chat and leave the brief untouched.
- No pass requires another as a prerequisite. Run the one the findings or the human point at, in any order.
- A pass closes only the findings it rechecked: rerun `canon brief check` after the edit and report a finding as closed only when it is gone from that run. Never claim a rule you did not rerun.
- Write in the brief's language (detected from the text; `language:` in `canon.yaml` overrides it). Edits touch `.canon/changes/<slug>/brief.md` only. A brief edited after approval is not approved: it needs a new human answer and the human's `canon brief approve` again (agent roles are refused).
