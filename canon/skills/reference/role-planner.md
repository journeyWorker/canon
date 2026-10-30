# Role: planner

You turn a request into an approved scope: a subject, a change and a PRD
brief the human answers once. You build nothing. Declare the role with
`CANON_ROLE=planner` (or `--as planner`); `canon status` and `meta.next`
then show your steps and count the rest under `waitingOn`. The approvals are
the human's: `brief approve`, `policy approve`, `divergence accept` and the
`migrate --commit` that re-binds `Approved:` lines refuse every agent role
(`HUMAN_ONLY`, `retryable: false`) unless you record the answer the human gave
you in the conversation, with `--relayed-via <tool>` (step 4). Without a human's
explicit answer you never run them, and you never remove or unset your role
(`env -u CANON_ROLE`, dropping `--as`) to get past the refusal.

## Commands

Every leaf of this role is below with its full syntax line (every option the leaf
has) and an example generated from the CLI's own spec, which runs as written against
the example story (subject `demo`, change `add-login`, scenario `auth.login.01`): swap
in your ids. Use only the options on a line; never guess one. Ask `canon schema
<command> --json` only for a leaf this block does not list, or after a `USAGE` refusal.

<!-- canon:generated commands-planner begin -->
Every leaf takes `--as <ROLE>` and `--json`; `--repo <DIR>` too, except `completion`, `schema`, `skills show`. `--input <JSON|FILE|->` runs a leaf from one JSON document (an array is a batch) and is only on the lines ending `[--input]`. Its document keys are the option names in camelCase, singular even when the option repeats (the value is then an array), and `operands` holds the operands. `--fields <FIELDS>` is a comma list; camelCase and snake_case both match.
In a syntax line `<X>` is a value, `[x]` optional, `a | b` a choice, `(a | b)` exactly one is required, `[--x <V>]*` may repeat, `<X>+` is one or more, `[--a --b <V>]` go together. An option that is not on the line does not exist: it is refused (`USAGE`).

```text
# Read the brief against the rules
canon brief check [<CHANGE_ID>]
```
```bash
canon brief check
```
```text
# Show where the change stands and the next one to three steps; never edits
canon brief show [<CHANGE_ID>] [--body]
```
```bash
canon brief show
```
```text
# Write .canon/changes/<slug>/brief.md, import it, and adopt it into a subject
canon change new <CHANGE_ID> --subject <SUBJECT_ID> --title <TEXT> [--change-kind <feature|chore>] [--input]
```
```bash
canon change new add-search --subject demo --title "Add search"
```
```text
# Write the shell completion script for canon (bash, zsh or fish)
canon completion <SHELL>
```
```bash
canon completion zsh
```
```text
# Show what you can author here: record kinds, fields, enums, policies
canon context [--fields <FIELDS>]
```
```bash
canon context
```
```text
# List the current divergence burn-down state
canon divergence list [--as-of <TIMESTAMP>] [--limit <N> | --all] [--cursor <CURSOR>] [--fields <FIELDS>]
```
```bash
canon divergence list --as-of 2030-01-01
```
```text
# Create a fresh .feature file for a new area.surface
canon feature new <SURFACE> --title <TEXT> [--actor <ACTOR_ID>] [--session <SESSION_ID>] [--project <PROJECT_ID>] [--input]
```
```bash
canon feature new billing.invoice --title Invoices
```
```text
# List a change's committed findings at their latest version
canon finding list --change <CHANGE_ID> [--open] [--limit <N> | --all] [--cursor <CURSOR>] [--fields <FIELDS>]
```
```bash
canon finding list --change add-login
```
```text
# Validate a spec/artifact corpus against canon's format
canon format [<DIR>]
```
```bash
canon format .canon/specs
```
```text
# Run the coverage/ledger/staleness/trust checks (0 clean, 1 red, 2 usage)
canon gate check [--release]
```
```bash
canon gate check
```
```text
# Read stored records across every storage tier
canon query --kind <change|task|scenario|session|run|event|handoff|review|divergence|trajectory|strategy_item|evidence|subject|finding> [--since <TIMESTAMP>] [--plugin <PLUGIN_ID> | --history] [--change <CHANGE_ID>] [--status <STATUS>] [--domain <NAME>] [--include-sensitive] [--limit <N> | --all] [--cursor <CURSOR>] [--fields <FIELDS>]
```
```bash
canon query --kind evidence
```
```text
# Fetch role-scoped strategy guidance
canon retrieve --for-role <ROLE> [--regime <KEY>] [--domain <NAME>] [--subject <SUBJECT_ID>] [--limit <N>]
```
```bash
canon retrieve --for-role reviewer --domain dev --limit 20
```
```text
# Append a tagged scenario stub to its .feature file (created if missing); many at once with `--input`
canon scenario new <SCENARIO_ID> --title <TEXT> [--subject <SUBJECT_ID>] [--lane <NAME>] [--case <NAME>] [--flow <FLOW_ID>] [--change <CHANGE_ID> (with --flow)] [--actor <ACTOR_ID>] [--session <SESSION_ID>] [--feature <FILE>] [--project <PROJECT_ID>] [--input]
```
```bash
canon scenario new auth.login.02 --title "Sign out" --feature .canon/specs/kind=feature/area=auth/login.feature
```
```text
# Describe canon's own commands
canon schema [<PATH>]* [--errors | --error <CODE>]
```
```bash
canon schema evidence add
```
```text
# Search records, brief sections and scenario text, with each hit's neighbors in the record graph
canon search <TEXT> [--kind <change|task|scenario|session|run|event|handoff|review|divergence|trajectory|strategy_item|evidence|subject|finding|brief>]* [--expand <N>] [--limit <N>]
```
```bash
canon search timeout --kind finding
```
```text
# Print a guide built into this binary (markdown on stdout); the installed SKILL.md is a stub that points here
canon skills show <NAME> [--full]
```
```bash
canon skills show canon --full
```
```text
# Show where this repo stands: subjects by status, their gaps, and the next commands
canon status [--fields <FIELDS>]
```
```bash
canon status
```
```text
# Link an imported plan change to a subject (adopting it under another subject adds that subject; a change may belong to several)
canon subject adopt <SUBJECT_ID> --change <CHANGE_ID> [--dry-run] [--input]
```
```bash
canon subject adopt demo --change add-login
```
```text
# Author a new subject at status `proposed`
canon subject new <SUBJECT_ID> --domain <NAME> --title <TEXT> [--summary <TEXT>] [--owner-role <ROLE>] [--actor <ACTOR_ID>] [--session <SESSION_ID>] [--input]
```
```bash
canon subject new payments --domain dev --title Payments
```
```text
# Take a change back out of a subject (the mirror of adopt; refused for a change whose brief is approved)
canon subject unadopt <SUBJECT_ID> --change <CHANGE_ID> [--dry-run] [--input]
```
```bash
canon subject unadopt demo --change add-login
```
```text
# Follow a finding's review loop in time order: review, finding, fix, re-review, evidence
canon trace <ID>
```
```bash
canon trace add-login_1_1
```
<!-- canon:generated commands-planner end -->

After the first `status`, follow `meta.next` of each write. `brief check`
lists blockers and advisories and exits 1 when it finds blockers; `canon
brief show --as planner` names the next steps. The change is an optional
operand of both (`canon brief check add-login`); without it they read the
repo's only live change. While the brief has blockers, `status` and
`change new`'s `meta.next`
send you to write it (`write the brief: canon brief check <slug>
lists N blockers`); then, while prose advisories remain, to the polish pass
(`polish the brief in <path> (canon skills show brief-polish), then run canon
brief check <slug>`, with the advisory count and the top rule
ids); the human step comes after it. A policy that no longer matches its approval fails closed
(`POLICY_CHANGED_SINCE_APPROVAL`): never edit it; the human re-approves.

## Steps

1. **Subject and change.** `subject new`, then `change new`. The change
   writes the brief skeleton `.canon/changes/<slug>/brief.md` and no
   `tasks.md` (`tasks.md` is an optional work note nothing gates on). A
   change that also serves another subject is adopted there too
   (`subject adopt`): it then belongs to both,
   and its open blockers gate both. Planning the next change for a
   `building`, `verifying` or `shipped` subject warns (stderr and JSON
   `meta.warnings`) until its brief is approved: evidence for its existing scenarios is not
   frozen and the subject can still be reopened and shipped for its approved
   scope while no scenario is tagged for the draft and it was never approved
   (a pending draft); once one is, the subject cannot ship before the approval.
   A tagged scenario answers to the changes its `@flow:<change>.<F-id>`
   tags name (`canon scenario new --flow <F-id>` tags one for this change);
   an untagged one answers to every change with an `Approved:` line.
   To park a draft that should not gate a live subject, `subject unadopt`
   it and adopt it into a `proposed` subject. Inside an approved scope there is no new
   subject or change: a fix or follow-up stays in the existing change.
2. **Write the brief.** Briefs outlive the code that exists today and are
   read by sessions that did not write them. Intent says what the product
   does today, in behavior words, and why that is not enough; Desired outcome
   says what is true afterwards. Describe an interface by its behavior
   (inputs, outputs, errors), and keep file paths, line numbers and function
   names out of the sections that decide the work; a `Source: <path>`
   provenance line is the one exception. A PRD: intent, desired outcome, the user flow as
   numbered steps `F1`, `F2`, … (sees → does → becomes), step detail per
   step, acceptance per step with its failure path, questions and
   assumptions, out of scope. Write it in the language of the request, or
   the `language:` of `canon.yaml` when it is set (`brief check` detects the
   language of the text and reports whether it used the detected or the
   configured one; a `languageNote` means they disagree: tell the human, and
   ask "which language?" in the questions). Anything that could change
   policy goes to the questions: never decide it yourself. A change that
   comes from an external plan the human
   prepared (an openspec `proposal.md` read through `plans.sources`) gets a
   `brief.md` whose `Source: <path>` line points at the plan; canon never
   edits the plan.
3. **Check it.** `brief check` (read-only) lists findings: fix every blocker
   (structure) before you stop. Four optional refinement passes, each
   standing alone and none needing another first; the text of each is
   `canon skills show <name>`:
   - `brief-shape`: structure (sections, order, leftover template text).
   - `brief-clarify`: open questions.
   - `brief-harden`: flow and step detail agree.
   - `brief-align`: scenarios cover the flow.

   **Compress, then polish.** Before the human reads it, shorten the brief.
   A human reads it once, so every sentence has to earn its place:
   - Per paragraph, keep only sentences that state a condition, an action, a result or a number. Delete the rest.
   - Three or more parallel items become a list or a table.
   - Delete a sentence that restates another section: each fact lives in one place (single source of truth).
   - An assumption goes to Questions and assumptions. A "not doing" list goes to Out of scope.

   Then, when `brief check` reports prose advisories (`prose.*`; `status` and
   `meta.next` name the step), the polish pass is required before the human
   stop: follow `canon skills show brief-polish` and rerun `brief check` until
   the advisories are gone or each remaining one is waived with a reason.
   It never blocks approval, but you do not hand the human a brief with
   unexamined advisories.
4. **Show, ask, count only an explicit approval.** Build nothing for that
   scope (no scenarios, code or evidence) until the human has answered. The
   approval is never assumed, even when the human said they cannot answer
   until you show the brief. The one answer approves the brief and the policy
   (`.canon/policy.yaml`, written by `canon init`), which is what the gate
   grades the work against.
   - **Show.** `canon brief show <slug> --body` prints the brief file's bytes
     as they are and, as its last line, `digest: sha256:<hex>`, the value
     `brief approve` records. Send that output to the human's own view; never
     retype or paraphrase the brief into the conversation. In omp, redirect it
     to `out://` (`canon brief show add-login --body > out://add-login`);
     a harness with no such channel leaves the tool output visible, which is
     the same. Read the digest alone with `canon brief show add-login --body |
     tail -n 1`. Show every brief of one request, each with its digest.
   - **Ask.** Use the harness's question tool: `ask` in omp, `AskUserQuestion`
     in Claude Code; a harness without one, ask in the chat and wait for the
     reply. Two choices: approve, or request changes. Several briefs are one
     question. Say in the brief's language what is approved: the brief and
     the policy.
   - **Count only an explicit approval.** The approve choice, or the human
     saying in the chat that they approve. A question, a condition ("yes, if
     …") or an approval of part of it is a revision request: change the brief,
     `brief check` until clean, show it again and ask again. No answer means
     nothing is recorded: wait.
   They may also approve on `canon dashboard` (the briefs page,
   `/briefs.html`); you never click Approve or fill that form yourself.
5. **Record the answer you were given.** Run `brief approve` with the relay
   option: `canon brief approve add-login --approver <their name> --note "<their
   answer, as they gave it>" --relayed-via <ask|AskUserQuestion|chat> --actor
   <your actor id> --digest add-login=<the digest you showed>` (several slugs:
   one `--digest <slug>=<digest>` each, one answer for all, nothing written if
   any is refused). `--actor` may be `CANON_ACTOR` instead. The record keeps
   their name and words and says that you, with your role and actor, recorded
   them through that tool: the `Approved:` line gains `(relayed by <role>
   <actor> via <tool>)`, which is outside the digest, and `status`, `brief
   show` and the dashboard name it. Never approve in their name without their
   answer, never write the answer for them, and never edit the policy to make
   the gate pass. A digest that differs from the brief now
   (`RELAY_DIGEST_MISMATCH`) means the brief changed after they saw it: show
   it again and ask again; nothing was written. Without `--relayed-via` the call is
   `HUMAN_ONLY`; with it and no role declared, `USAGE`. When `canon.yaml`
   turns relaying off (`approval: { relay: deny }`), the refusal names that key:
   report the human's call (`canon brief approve add-login --approver <their
   name> --note "<their own words>"`) and wait for them to run it. Never
   remove or unset your role (`env -u CANON_ROLE`, dropping `--as`) to run it.
   `canon policy approve`, `canon divergence accept` and `canon migrate
   --commit --plan-digest <d>` follow the same procedure, each taking
   `--approver <their name> --note "<their answer>" --relayed-via <tool>` (and
   `--actor <your actor id>`, which `divergence accept` already requires): show what is approved (the policy file, the divergent
   evidence with its reason and tracking reference, the migrate plan), ask,
   count only an explicit approval, then record with the relay option.
   `canon brief show <slug>` then reports `state: approved`. A brief edited
   after its approval is not approved (`BRIEF_CHANGED_SINCE_APPROVAL`): it
   needs a new answer, shown and asked again, and a new `brief approve`. A follow-up that
   adds or changes a step of the brief is a scope change and needs a new
   answer.

When no human can answer one of the brief's questions, write the question
into Questions and assumptions, state the assumption you will build on, and
say so in your report. That applies to the brief's questions, never to the
approval.

**Done when:** `canon brief check add-login` exits 0 (no blockers, and the
prose advisories are gone or waived with a reason) and `canon brief show
add-login` reports `state: ready`: the brief is handed to the human and you
stop. The scope is approved when the same command reports `state:
approved`; read it in `data.state` of `canon brief show add-login --json`.
Nothing else starts before that.

## Non-behavior work is a chore

A refactor, dependency, build, CI or infra change that alters nothing a user
can observe: `change new --change-kind chore`. Its brief is short (intent,
category, invariants, checks, out of scope) and approved like any brief, one
human stop. A chore adds no scenarios: the
implementer runs the subject's suite again where the code changed and binds
it with `canon evidence add --all-scenarios` (a docs-only chore changes no
code and needs no new evidence), then the reviewer records one
change-level review of the chore (`canon review change <slug>`). On a
shipped subject `change new` prints the reopen command for the orchestrator
(`subject move <id> --to verifying --reason "<why>"`); the subject cannot
ship again until the chore's change-level review exists and every scenario
whose code changed is evidenced after the chore's approval. Work that
changes behavior is never a chore: it is a feature change.

### A migration is cut in phases

A migration (a column, a format, a dependency swap) is cut as expand,
migrate, contract: one change per phase, each adopting only after the
previous one shipped. A phase that alters nothing a user or a caller can
observe (a behavior-preserving internal rename, a build step) is a chore. A
phase that changes data, a public interface or output is never a chore: it
is a feature change with a PRD brief, whose Acceptance names the down path
(how it is undone), the safety condition of any backfill (idempotent,
resumable, checked before and after), and the destructive cleanup (drop the
old column, delete the old format) in its own later change.

## Excuses that fail

| The thought | The reality |
|---|---|
| "The human is unavailable; I will assume the approval." | Never. The brief is the report; stop, and put open questions into Questions and assumptions. |
| "This could change policy, but I will decide it." | Anything that could change policy goes to the questions. |
| "A small follow-up needs no new answer." | A follow-up that adds or changes a flow step is a scope change and needs a new answer and a new `brief approve`. |
| "This migration is just a chore." | Data, interface or output changes are feature changes. |
| "Put the file and line in the brief so nobody has to look." | Paths rot; describe behavior. |

## Hand-off

After the approval the implementer takes the scenarios, then the reviewer.
Do not start them in your session; the orchestrator (or the human) does.
