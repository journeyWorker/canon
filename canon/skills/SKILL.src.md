---
name: canon
description: Use for every build, implement, add-a-feature, change-a-feature or fix request in a repo that has canon.yaml, including sites, apps and games; prefer it there over generic site, app or game builder skills. Runs canon's working loop (brief, subject, scenarios with failure paths, units, implement, evidence, independent review, status transition) split by role (planner, implementer, reviewer, orchestrator). The full guide is `canon skills show canon`.
---

# Canon

Features in this repo are managed with canon. This is the shared core guide:
the loop, how to drive the CLI, and how to find your role. The guide is built
into the `canon` binary, so it always matches the version you run:

- `canon skills show canon` prints this guide.
- `canon skills show <name>` prints one topic or role guide (the table at the
  end lists every name); `canon skills show canon --full` prints this guide
  followed by every topic.
- Your role guide's **Commands** block lists every option of every leaf the
  role uses. Ask `canon schema <command>` (or `--help`) only for a leaf it does
  not list, or after a `USAGE` refusal. Never guess an option name.

The `SKILL.md` your harness loaded is only a stub that sends you here.

## The working loop

Follow these steps in order for every request that builds, changes or fixes
a feature, a one-line request included. A human is needed once per scope:

- **The approval** (step 1). For a new subject or a scope change, the
  human must approve the brief. Write the brief,
  run `canon brief check` until it reports no blockers, then show it, ask and
  record the answer: build nothing for that scope (no
  scenarios, code or evidence) before the human answers. The approval is
  the one thing you never assume, even when the human said they cannot
  answer until you show it. The one answer
  approves the brief and the policy, `.canon/policy.yaml`, which is what
  the gate grades the work against. The human may also approve it on the `canon dashboard` briefs page; you never use that form.
  1. **Show** with `canon brief show <slug> --body`: the brief file's bytes and,
     last, `digest: sha256:<hex>`. Send it to the human's own view without
     retyping it (omp: redirect it to `out://`, which is off for subagents and
     non-TUI sessions and caps at 64 KiB; there, and in a harness with no such
     channel, leave the tool output visible). A subagent has no question tool:
     the orchestrator shows, asks and records.
  2. **Ask** with the harness question tool (`ask` in omp, `AskUserQuestion` in
     Claude Code, otherwise in the chat): approve or request changes, several
     briefs in one question.
  3. **Count only an explicit approval.** A question or a condition is a
     revision request (change the brief, check, show and ask again); no answer
     records nothing.
  4. **Record it** with the relay option:
     `canon brief approve <slug> <slug> --approver <their name> --note "<their words>" --relayed-via <ask|AskUserQuestion|chat> --actor <your actor id> --digest <slug>=<digest shown>`
     (one or more slugs, each with its own `--digest`, one answer for all, nothing written if any is refused).
     The `Approved:` line then carries `(relayed by <role> <actor> via <tool>)`.
     A digest that no longer matches the brief is `RELAY_DIGEST_STALE`: show
     it again. Without `--relayed-via` an agent role gets `ACTOR_DENIED`, and so
     does a repo whose `canon.yaml` says `approval: { relay: deny }`: then the
     human runs the call themselves. Never remove or unset your role
     (`env -u CANON_ROLE`, dropping `--as`) to run it yourself.
  Never edit the policy to make the gate pass: a policy that differs from
  the approved one fails closed (`POLICY_APPROVAL_STALE`, not
  waivable); stop and ask the human. A brief edited after its
  approval is not approved (`BRIEF_APPROVAL_STALE`): it needs a new
  answer. The ship guard and `canon evidence add` refuse an unapproved or
  edited brief, but those are backstops: the stop itself is at step 1.
- **Approval is scoped.** Evidence for a scenario tagged `@flow:<change>.<F-id>`
  needs the approval of the changes its tags name only, so a new unapproved
  change on a live subject waits on its own scenarios; an untagged scenario
  answers to every change with an `Approved:` line (an edited one refuses); `change new` and
  `subject adopt` warn when the subject is live. `canon subject unadopt <subject-id> --change <change>`
  takes a draft change back off (refused once approved). The subject
  answers for every adopted change that is tagged (a scenario of the subject
  is tagged `@flow:<change>.*`), has an `Approved:` line, or is a chore:
  reopen and ship pass only when all of them are approved now, whatever the
  order they were adopted or approved in, and `canon status` lists one entry
  per such change in `briefs`. A pending draft (never approved, not a chore,
  tagged by no scenario) blocks nothing and the transition prints one line
  naming every draft; a draft a scenario is tagged for blocks until it is
  approved. `canon skills show canon-subject`
- **Inside an approved scope, never stop for a human.** Fixes, follow-ups,
  review rounds and re-reviews stay in the approved change (never
  `canon change new`: a new change is a new scope) and go straight on: scenario,
  implement, evidence, independent review. A follow-up that adds or changes
  a step of the brief edits the brief: that is a scope change and needs a
  new answer. A follow-up on a shipped subject reopens it
  (`canon subject move <id> --to verifying --reason "<why>"`); a fix
  closed `fixed`, and an approved chore, then need evidence again for every
  scenario whose code changed (see "Evidence follows the code" below) before
  it ships, and a chore also needs one change-level review
  (`canon review change <slug>`).
- When no human can answer one of the brief's questions, write your
  assumption into the brief's Questions and assumptions, build on it, and say
  so in your report. That covers the brief's questions and work inside an
  approved scope, never the approval itself.

1. **Brief.** The planner writes the PRD brief (`.canon/changes/<slug>/brief.md`), checks it with `canon brief check`, shows it to the human, asks, and records their explicit approval. `canon skills show role-planner`
2. **Subject.** `canon subject new`, then `canon change new` (`--change-kind chore` for work that alters no behavior); the change writes the brief skeleton. `canon skills show role-planner`
3. **Scenarios.** `canon scenario new --flow <F-id> --case <case>` per behavior of a flow step, failure paths (`--case failure`) included; many at once with `--input` (below). `canon skills show role-implementer`
4. **Units.** Split the work; one unit is one flow step's slice with a named verify command, and each unit has its own actor id and session id, passed on everything it records. A unit never attests another unit's work.
5. **Implement.** Code with tests named by scenario id (`game.run.01: …`). `canon skills show role-implementer`
6. **Evidence.** `canon evidence add` binds the test run to its scenarios; one `--input` array records many at once. `canon skills show role-implementer`
7. **Independent review.** A different session, a separate process with its own session id, that can run the checks it signs off, records `canon finding add` and `canon review scenario`. Every change. `canon skills show role-reviewer`
8. **Transition.** `canon subject move <id> --to <state>` along `proposed → specced → building → verifying → shipped`, with `canon gate check` clean; fix a refused guard rather than overriding it. `canon skills show role-orchestrator`

**Done when:** `canon gate check` exits 0 with no violations, `canon subject show <id>` reports status `shipped`, and `canon status` names no next step for the subject (a human step left in `meta.next` is the one thing you report instead of doing).

Every write command says whether the record is final (`written directly`,
or `staged and promoted in this run`) or staged; only a staged record
needs `canon gate promote`, and only `write.promote: manual` leaves one.

### Commits and uncommitted code

Records that are about code take `--rev <REV>`: any git revision (`HEAD~1`,
a branch, a tag, a short sha), default `HEAD`, so you never copy a sha out of
git. `finding add` also takes `--resolution-rev` (default HEAD when the
finding is `fixed`) and `--introduced-rev` (no default: say where the defect
came from). Evidence, a review or a finding of uncommitted work is allowed,
never refused: the record carries `codeTree` (the git tree id of the code the
command saw: `.canon/`, the spec roots and the `code.exclude` globs are left
out, so editing a `.feature` file or a brief never changes it) and
`worktreeDirty`. The commit made from a working tree has the same `codeTree`
the record carries, so a review of a working tree survives the commit made
from it; a review is judged by what it pins (the scenario and what its
evidence says), never by the commit or the code tree. Outside a git work
tree, or in a repo with no commits, a command that needs a commit refuses
`COMMIT_REQUIRED`.

### Evidence follows the code

After a chore approval or a reopen, a scenario whose latest evidence was
recorded on the current `codeTree` needs nothing new: a docs-only chore then
needs its change-level review alone. What counts as code is `code.exclude` in
`canon.yaml` (globs, default `docs/**` and `*.md`). A chore approved over the
code as it still is (the `(code tree <id>)` of its `Approved:` line is the
current tree) asks for no evidence at all, whatever tree the evidence is on, and
`status` says so; an approval with no recorded tree follows the rule above.
When code did change,
`canon status`, `meta.next` and the refusal hint lead with one batch over
every report, for example
`canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer`
(with the real report paths, and `--project <id>` when the repo has several
spec roots), never a loop per scenario. With a `--report`, `--class` defaults
to `test-run` and `--ref` to the report paths, and the bound cases decide the
verdict: any failed case `divergent`, a skipped one and none failed
`not-applicable`, else `faithful`.

### State

The ledger is latest-only. It holds the current record of each key; writing a
newer version moves what it supersedes to `.canon/archive`, and `status`,
`gate check`, the guards and `query` read the ledger only. Earlier versions
are one flag away: `canon query --kind evidence --history`. A ledger written
before 0.23, or one whose move was interrupted, is brought to one file per
key by `canon ledger compact` (`canon migrate` runs it). A record binds only
what its own write names: evidence carries nothing forward from an earlier
record, and a review stays current while the verdict and checks of the
evidence it saw are unchanged and the code tree is the one it saw.

Where artifact bytes live is the repo's choice: `canon.yaml`'s `artifact_store:`
block defaults to the local, git-tracked `.canon/artifacts/sha256/`, or names
an object-store tier, in which case `evidence add`, `review scenario`, `review change` and
`artifacts store` upload by digest and keep nothing locally. The gate and
`canon artifacts verify` (`--full` re-hashes every blob) ask the store and
refuse, never pass, when it cannot be reached. Moving an existing store with
`canon artifacts move --tier <tier>` is the repo owner's decision, not a step
of the loop.

### Search and trace

`canon search "<text>"` finds a record, a brief section or flow step, or a
scenario's text (title, steps, tables, `Examples`, the Background and Rule it
inherits) and lists each hit with its neighbors in the record graph: the
records it names or that name it, the review loop around a finding, the brief
step a scenario covers. A reference no committed source holds is a neighbor
marked `placeholder`, never an error. `--kind` (repeatable) keeps hits of those
kinds, `--expand <N>` sets the hops (default 1), `--limit <N>` the hits (default
20). Korean is found from three characters; a shorter text is found as a part of
a node's text, in id order. The text is read as typed: quotes and `AND` are not
syntax. `canon trace <finding key|review id|sha>` follows the review, finding,
fix, re-review and evidence chain of a loop in time order, the archive's
earlier versions included. Both read, and first refresh, a derived index,
`.canon/index/search.db`: gitignored, never a source of records, safe to delete
(the next run rebuilds it, and only `data.index.rebuilt` differs). Records still
staged, and those in postgres or an object store, are not in it; `data.index`
names the stores not read.

## Know your role

canon cannot detect a role; you declare it. Pass `--as <role>` or set
`CANON_ROLE=planner|implementer|reviewer|orchestrator`; a flag wins over the
variable, and an unknown value is refused (`ROLE_INVALID`).

- **A role is set, or you were spawned as one:** load only
  `canon skills show role-<role>` and do that role's steps. `canon status`
  and `meta.next` then show only your role's steps, and count the others in
  `data.waitingOn`; `canon schema --as <role>` lists only your role's commands.
  Do not load another role's guide.
- **No role (one agent doing everything):** follow `canon status` and load
  each role guide as its phase starts: planner for steps 1–2,
  implementer for 3–6, reviewer for 7 (in a separate session), orchestrator
  for 8.

A role is guidance, not enforcement: `--role` on `evidence add` and
`review scenario` defaults to it. `CANON_ROLE` never filters or fails
`canon schema`, `canon context` or `canon skills show`; only `--as` filters.

## Driving the CLI

- Start a session with `canon status`: where each subject stands and the next
  commands. After that, do not call it only to learn the next step: every
  write returns `meta.next` (`[{command, argv, reason, x-canon: {actor, role}}]`), the same
  steps `status` would give, narrowed to what the write touched. Each entry is
  runnable: `argv` is the command without `canon` (with `--json` when you
  called in JSON mode). Guidance that needs a value you must fill in is not a
  next entry; it is in the error `hint` or the report text. `x-canon.actor: human`
  marks a step only the human takes.
- Your role guide's **Commands** block (`canon skills show role-<role>`) is
  your complete command set, every option of every leaf included. Run `canon schema <command> --json` only for a
  command it does not list, or after a `USAGE` refusal (read `error.field`
  and `error.expected` first). `canon schema --json` is the tree of every group and command
  with its `effect` and `x-canon.roles` (`--as <role>` lists one role's, and `x-canon.as` names it);
  its `options` define once the options that two or more commands share (`--repo`, `--as`, `--actor`, …)
  and a command names them. `canon schema <group> --json` lists a group's
  commands; `canon schema <command> --json` is the call surface of one command and nothing
  else: `options` (each name of a tree option, or an object with `valueType`, a typed
  `default`, `possibleValues`, `env`, `flagOnly`), `operands`, `constraints`, `validators`, `errors`,
  `outputs`, `input` (what one `--input` document is and a complete `example`) and `examples`.
  A member at its default is absent. `--json` is always accepted, `--input` when the command
  has `input`, `--fields` when an output has `fields`, and the codes `USAGE` (with canon's
  finer `*_INVALID`/`*_REQUIRED` argument codes), `INTERNAL`, `INPUT_*`, `BATCH_INCOMPLETE`,
  `PLAN_STALE` (with `--commit`) and `CURSOR_INVALID` (with `--cursor`) can come without
  being listed. `--commit` and `--plan-digest` are listed on exactly the commands that offer
  them: a command that lists `commit` previews on a bare run. `canon schema <command> --schemas --json`
  gives the JSON Schema of each report in `schemas`, `Input` (the schema of one `--input`
  document) and `rejects` (what each validator refuses, in order); `#/schemas/<Name>` in
  the command document resolves there. The first line of every `--help` points at it.
- Pass `--json` and read stdout as one document, nothing on stderr. Success is
  `{"ok":true,"data":…,"meta":…}`; failure is
  `{"ok":false,"error":{…},"meta":…}` (a check that fails keeps its report in
  `data`). `meta` carries `tool`, `toolVersion`, `contract` (`atip/1`),
  `contractVersion` (`1.4`), `schemaVersion`,
  `command`, and when present `next`, `warnings` (`[{code, message}]`) and
  `page`; a repeated write that changed nothing reports `data.unchanged: true`.
  Absent members are omitted, never `null` or `[]` (a value you sent empty comes back as `received: ""`).
  Read the members you know and ignore the ones you do not: a later canon
  adds members, and a value of a closed set you have not seen (a `severity`, an
  `outcome`, a `reason`) is one item to leave out, never a reason to reject the document. A paged list puts
  `meta.page.nextCursor` in the first entry of the call's `meta.next` as `--cursor=<nextCursor>`
  (run that entry as it is); no `nextCursor` means no more.
  On failure branch on `error.code`, and on `error.reason` where the code has
  reasons (`INPUT_INVALID` says `NOT_JSON`, `UNKNOWN_KEY`, `STDIN_TTY`, … so you
  know whether to fix the JSON or the shell); fix the option `error.field` names
  using the `error.expected` values (`field` is an option name, or a JSON
  pointer such as `/2/scenario` into an `--input` document, `/<index>/<key>` in a batch), and
  follow `error.hint`: it is one runnable command line that starts with `canon`, the
  same with `<PLACEHOLDER>`s you fill in, or one sentence. The codes
  are stable; `canon schema --errors --json` lists them with their reasons and
  `canon schema --error <CODE>` explains one. A warning code (`meta.warnings`) is in
  `canon schema --warnings --json`; `canon schema --warning <CODE>` explains one.
- **Exit codes:** only 0, 1 and 2 exist: 0 ok; 1 the command ran and its answer
  is negative or incomplete (a failed check, a violation, `unverifiable` when
  nothing was examined, a partly failed batch); 2 refused to run (usage,
  `CONFIG_REQUIRED`, an id or file that does not exist, an invalid argument).
  A refusal leaves the tree unchanged.
- Repeating a write is safe: a record that already exists with identical
  content is reported `unchanged` (`data.unchanged`), not duplicated.
- An option that takes an id is the bare noun (`--change add-login`,
  `--scenario auth.login.01`, `--actor reviewer-1`, `--project root`,
  `--session s-1`), and an `--input` document key is that name without `Id`
  (`scenario`, `actor`; the stored record and report JSON keep `scenario_id`,
  `scenarioId`). The primary object of a command is its operand
  (`canon subject move <id> --to verifying`, `canon change new <slug>`), and
  `--kind` values carry no `_record` suffix (`--kind evidence`).
- `canon status` sends the planner to write the brief while `brief check`
  has blockers; the approval step appears only when the brief is clean and
  unapproved.
- Page long lists with `--limit <n>` and `--cursor <nextCursor>` (`--all`
  for everything); a cursor holds only for the call that issued it, so change nothing but `--limit`, `--all`, `--fields` and `--json`, and keep `CANON_ROLE` as it was; the next call is `meta.next[0].argv` as it stands (the arguments you gave and the cursor, never a default). An argument you add is a change even when it states the default (`--repo .`): canon binds an explicit default to the cursor (`CURSOR_INVALID` with `error.reason` `ARGUMENTS_CHANGED`, `error.field` `--cursor`; `FOREIGN` for a cursor of another command or another canon version: start again from the first page). `--limit`, `--cursor`, `--all` and `--fields` are flags: an `--input` document key of that name is `INPUT_INVALID` with `reason` `FLAG_ONLY_KEY`. Project fields with `--fields a,b`
  (`status`, `export`, `query`, `finding list`, `divergence list`, `context`): the paths `canon schema <command> --json` lists under `outputs[].fields`, spelled as the answer spells them; `meta.fields` lists what was kept.
- `--override-reason <text>` is the only way to proceed past a waivable
  refusal; it is recorded, echoed in `data.overrides[]` and listed by `status`
  for the human. There is no `--force`. `brief approve`, `policy approve`,
  `divergence accept` and the `migrate --commit` that re-binds `Approved:`
  lines record a human's decision and refuse every agent role (`ACTOR_DENIED`)
  unless they record the explicit answer the human gave you, with
  `--relayed-via <tool>` (show, ask, record, as for the brief; `canon skills
  show role-planner`); never unset your role to run one.

### Many records at once: `--input`

Every write leaf takes `--input` instead of flags: inline JSON, a file, or `-`
for stdin. The keys are the option names in camelCase, and `operands` holds
the positional arguments. An object is one invocation; a JSON array (or NDJSON
from a file or stdin) is a batch, run in order and answered with
`data.items[]` (`index`, `status` of `written`, `unchanged`, `failed` or
`skipped`, and that element's `data` or `error`). A batch is validated whole
before anything is written, and that check reads no state: an invalid element writes nothing and exits 2 with
its own code, `error.field` pointing at `/<i>/<key>` and no `data`. An element that fails when it runs (the object
it names does not exist) stops the batch: exit 1, `BATCH_INCOMPLETE`, `error.field` exactly `/<i>`, the element
`failed` with its own `error` (no `data`), the later ones `skipped`, and the earlier ones done (not undone). Re-running the same batch reports the finished elements
`unchanged`. A typo in a key is refused (`INPUT_INVALID`, `field` naming the key), never dropped; a document with several problems reports them all, the first as the error (`field` is the first `error.details` entry's) and every one in `error.details` in the order the document wrote its keys, and a value outside a closed set is `INPUT_INVALID` too (`reason` `INVALID_VALUE`, `field` the key), not `USAGE`. Every non-empty line of an NDJSON file is a document (a `#` line is not a comment). Integers compare by number: `1`, `"1"` and `--round +1` are one value, `"+1"`, `"0x1"` and `1.0` in a document are `INVALID_VALUE`, and an integer beyond ±9007199254740991 is refused as a number and as a string, in a document (`INVALID_VALUE`, `field` its pointer) and on the command line (`USAGE`). `--input` may give a required option and the command line only the operand; an option given in both with different values is `INPUT_CONFLICT` at the key's pointer. A repeatable option's none is the key left out: `[]` is refused (`INPUT_INVALID`, `reason` `INVALID_VALUE`, `field` the key's pointer), as is `[]` for any other option and for `operands`.
`canon schema <command> --json` shows the leaf's `input` (what one document is, and a complete `example`); `canon schema <command> --schemas --json` gives `Input`, the JSON Schema of one document.

```text
canon scenario new --input '[{"operands":["auth.login.01"],"title":"Logs in","case":"happy"},{"operands":["auth.login.02"],"title":"Wrong password","case":"failure"}]'
canon evidence add --input '[{"scenario":"auth.login.01","class":"test-run","ref":"npm test","role":"implementer"},{"scenario":"auth.login.02","class":"test-run","ref":"npm test","role":"implementer"}]'
canon finding add --input '[{"change":"add-login","round":1,"seq":1,"severity":"note","actor":"reviewer-1","summary":"Token is logged"},{"change":"add-login","round":1,"seq":2,"severity":"blocker","introducedUnknown":true,"actor":"reviewer-1","summary":"No rate limit"}]'
```

`--input` never carries `--commit` or `--plan-digest`, and the destructive
leaves below take one object only.

### Preview, then `--commit`

The five commands that delete or rewrite existing data (`canon purge`,
`canon tier age`, `canon ledger compact`, `canon artifacts move`,
`canon migrate`) act in two steps. The bare run previews: it writes nothing,
exits 0 and prints the effects and a `planDigest`. Read the effects, then act
with `--commit --plan-digest <planDigest>`. It refuses (`PLAN_STALE`) when
the plan differs from the one you previewed, so run the preview again. Appends
and state transitions (`gate promote`, `subject move`) run directly. Where a
preview helps elsewhere (`init`, `skills install`, `export`, `inventory sync`,
`subject adopt`, …) `--dry-run` validates fully and writes nothing.

```bash
canon purge --kind session --before 2026-01-01
```

## Upgrading a repo from 0.27

0.28 changed the CLI grammar with no aliases: an old spelling fails with exit 2
and an `error.hint` naming the replacement. `canon status`, `canon gate check`
and `canon context` warn when the repo holds 0.27-era state and put
`canon migrate --json` first in `meta.next`. Then:

1. `canon migrate` previews and prints the `planDigest`.
2. `canon migrate --commit --plan-digest <digest>` with that digest applies it.
   It moves the layout, rewrites canon's own hooks and scripts, compacts the
   ledger, replaces an old full skill with the stub, and backs up every file it
   rewrites under `.canon/migrate-backup/` (local and git-ignored; `--revert` needs it). It refuses uncommitted changes
   under a path it moves.
3. `canon skills install` refreshes the stub and the scripts; `canon skills check`
   confirms them.

**Done when:** `canon migrate` reports nothing left to move, `canon skills check` exits 0, and `canon status` and `canon gate check` no longer warn about 0.27 state.

## Setup and provider projection

```bash
# canon.yaml, starter policy (--policy light|standard|strict, default strict; see canon-policy), AGENTS.md block, commented plans examples
canon init
# harnesses for the canon skill; without it init takes the detected .claude/.codex/.omp/.pi (it never asks)
canon init --providers=claude,codex
# follows canon.yaml skills.providers (else the install lock's, else the detected ones; none detected installs nothing); prunes projected files the bundle no longer ships when their bytes match the lock, keeps a modified one and reports `kept:`
canon skills install
# any subset or none; rewrites skills.providers
canon skills install --providers=claude,codex,omp,pi
# read-only drift check; exit 1 (SKILLS_CHECK_FAILED) names each orphan, a projected file the bundle no longer ships
canon skills check
# diagnostics, incl. a stray canon projection for an unselected provider and a projection a formatter rewrote, and `orphan:` lines for projected files the bundle no longer ships; never deletes user files
canon skills doctor
```

`install` writes, per selected harness, a short `SKILL.md` stub (this file's
frontmatter plus a pointer to `canon skills show canon`) and the `scripts/**`
sidecars; the guides themselves are not copied, they come from the binary.
Claude gets `.claude/skills/canon/`, Codex `.agents/skills/canon/`, OMP
`.omp/skills/canon/`, Pi `.pi/skills/canon/` (the retrieve script is a sidecar,
not an installed hook). `check` reports a stub written by another canon
version; `install` rewrites it. The harness list lives in `canon.yaml` as
`skills: { providers: [...] }`; change it with `canon skills install --providers=<list>`,
never by assuming a default.

`install` and `doctor` end with `owned-paths:`, the globs of every path
canon writes. Add them to the repo's formatter and linter ignore lists; a
formatter that rewrites a projection or a ledger record makes canon read
drift (`doctor` reports it as `formatter-rewritten` and names the entry to
add). Specs, `policy.yaml` and the changes are written by hand and are not
on the list.

**Done when:** `canon skills check` exits 0 and `canon skills doctor` lists no `orphan:`, `stray:` or `outdated:` line.

## Route by task

- **Fix a bug:** the approved change if there is one, else the planner (`canon change new`); first a failing run of the scenario (`canon scenario new --case failure`, then `canon evidence add`), then the fix. `canon skills show role-implementer`.
- **Add or change a feature:** `canon subject new`, `canon change new`, then the brief. `canon skills show role-planner`.
- **Pure refactor, dependency, build, CI or infra:** `canon change new <slug> --subject <id> --change-kind chore`; data or interface changes are not chores. `canon skills show role-planner`.
- **Review someone's change:** `CANON_ROLE=reviewer`, `canon brief check <slug>`. `canon skills show role-reviewer`.
- **Resume:** `canon status --json`, then follow `meta.next`. `canon skills show role-orchestrator`.
- **Plan more work on a live subject:** `canon change new <slug> --subject <id>` (it warns until the brief is approved; `canon subject unadopt` parks a draft). `canon skills show canon-subject`, `canon skills show role-planner`.
- **Upgrade canon in a repo:** `canon migrate` (previews), then `canon migrate --commit --plan-digest <digest>`, then `canon skills install`. See "Upgrading a repo from 0.27" above and `canon skills show role-orchestrator`.

**Done when:** you have named your role and loaded its guide, and `canon status --json` lists your next step in `meta.next`.

## Safety contract

`canon status`, `canon format`, `canon skills check`, `canon skills show` and
`canon gate check` do not change the repo's records. Do not hand-edit generated
reports, provenance lines, locks, or task evidence. Inspect `canon context`
before authoring typed artifacts, and treat every failure class as actionable
evidence rather than bypassing the gate.

## Guides: `canon skills show <name>`

Every name below is a guide built into the binary. Load only the one the
current step needs.

| Name | What it is for |
| --- | --- |
| `canon` | This core guide: the loop, driving the CLI, routing. |
| `role-planner` | Planner steps 1–2: subject, change, PRD brief, the approval. |
| `role-implementer` | Implementer steps 3–6: scenarios, units, code, evidence. |
| `role-reviewer` | Reviewer step 7: findings and signed reviews in a separate session. |
| `role-orchestrator` | Orchestrator step 8: dispatch, transitions, resuming, upgrading. |
| `brief-shape` | Brief pass: the structure `canon brief check` expects. |
| `brief-clarify` | Brief pass: turn open points into questions a human answers in one reply. |
| `brief-harden` | Brief pass: make flow and step detail agree, with failure paths. |
| `brief-align` | Brief pass: scenarios cover the brief's flow and the brief still says what they prove. |
| `brief-polish` | Brief pass: make the brief readable once (clears `prose.*` advisories). |
| `canon-subject` | Status, subjects and changes: `canon subject new|adopt|show|move`, `canon change new`. |
| `canon-authoring` | Write or reorganize `.feature` specs; the scaffold. |
| `canon-context` | `canon context`: fields, enums, policy bindings, context packs and prompts. |
| `canon-vocab` | Typed authoring vocabulary and the handoff template. |
| `canon-fmt` | `canon format`: validate a spec or artifact corpus. |
| `canon-inventory` | `canon inventory sync`: index specs into scenario records. |
| `canon-gate` | Evidence, gates, task checkboxes, hooks. |
| `canon-review` | Findings and reviews. |
| `canon-artifact-ingest` | `canon ingest records`: artifacts into verdict trajectories. |
| `canon-session-ingest` | `canon ingest sessions`: agent transcripts into session records. |
| `canon-plan-import` | `canon ingest plans`: plan state into change and task records. |
| `canon-storage` | Storage rungs, tiers, retention, export, purge. |
| `canon-policy` | The policy and the CEL expressions behind it. |
| `canon-plugins` | Plugin overlays on core records. |
| `canon-learn` | Strategy memory: promote, approve, roll back. |
| `canon-retrieve` | Dispatch-time guidance (`canon retrieve`); `scripts/canon-retrieve-pre-dispatch.sh` is the hook sidecar. |
| `canon-report-dashboard` | `canon report build|check|snapshot` and `canon dashboard`. |

**Done when:** `canon skills show <name>` prints the guide you needed; a name that is not in the table is a usage error (exit 2) whose `error.expected` lists the valid ones.
