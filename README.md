# canon

Harness knowledge substrate: systematic spec planning, machine-enforced task
completion, unified agent-session logging, and accumulation-driven harness
improvement — one Rust core, distributed as `bunx @journeykit/canon` (installed bin:
`canon`), usable from any repo.

Named for the musical **canon**: one subject, taken up by many voices, each
developing it in its own register — the same shared backbone on which every
specialist agent role (planning / design / dev / test / review) evolves its own
strategy memory.

## Quick start

```bash
bunx @journeykit/canon --help    # or: cargo install --path crates/canon-cli
canon init                       # write a starter canon.yaml in your repo
canon demo init --repo /tmp/canon-demo   # or try the evidence loop in a sandbox
canon format spec                # validate a spec corpus
canon gate check                 # run the evidence gate
```

Run `canon <command> --help` for any explicit CLI command. Install the
provider-neutral user-facing companion bundle with:

```bash
canon skills install                         # detect .claude/.agents|.codex/.omp/.pi
canon skills install --providers=claude,codex,omp,pi
canon skills check                           # read-only drift check
canon skills doctor                          # diagnostics, never deletes
```

The canonical projection is one directory-shaped `canon` skill per selected
provider: Claude uses `.claude/skills/canon/SKILL.md`, Codex uses
`.agents/skills/canon/SKILL.md` (the directory Codex scans for skills), and
OMP/Pi use `.omp/skills/canon/SKILL.md` and `.pi/skills/canon/SKILL.md`. Every
selected provider receives lazy `reference/**` and `scripts/**` sidecars.
Without `--providers`, an existing `.agents` or `.codex` directory selects
Codex. OMP/Pi are project-local passive bundles; the retrieve pre-dispatch
script is a sidecar, not a native hook.

Canon 0.13.0 and earlier wrote Codex to `.codex/skills/canon.md` and
`.codex/skills/canon/**`, which Codex never reads. `canon skills install`
removes the legacy files whose bytes still match the hash in
`.canon/skills/.install-lock.json`, keeps everything else (including your own
files under `.codex/skills`), and drops `.codex/skills` only if it ends up
empty. `skills check` and `skills doctor` report any leftover legacy Codex
projection as a remnant with the command that migrates it.

`canon/skills-dev/` is contributor-only legacy tooling. It is not part of the
user-facing install; maintainers may materialize it explicitly when developing
Canon with `canon skills install --source canon/skills-dev`.

## Safety boundaries

Canon records and verifies intent, evidence, policy, approvals, and run
lineage; it does not execute agent actions or enforce provider capabilities.
Adapter v1 validation is strict and read-only. Filesystem, network, and secret
capabilities require enforcement by the external provider sandbox. ContextPack
and prompt-registry inputs are captured as immutable selected bytes, while Run
lineage records the pack, provider, model, skill, and policy used.

Risk tiers require current effect/path binding and policy-pinned SSH approval.
Learning is quarantined until paired evaluation and signed approval, and can be
rolled back. Capability authorization defaults to deny (`execution=false`).
Quality and cost outcomes are currently unknown/null; Canon does not claim that
retrieved guidance improves outcomes.

Knowledge-map and check scripts govern canonical-source/projection ownership;
they are not substitutes for implementation or runtime enforcement.

## Status

Public pre-alpha. Core workflows are implemented and dogfooded; interfaces
and storage formats may change. See [`ARCHITECTURE.md`](ARCHITECTURE.md) for
the current architecture contract and [`canon/knowledge-index.json`](canon/knowledge-index.json)
for the machine-readable source/projection/memory map.

## 0.17.0 release

### Canon's own changes and approvals (P2)

**D2: `.canon/changes/` is canon's own plan home.** `canon change new
<slug> --subject <id> --title "…"` writes `.canon/changes/<slug>/brief.md`
(`# <title>`, `## Why` holding the title, `## What Changes`, and a comment
naming where the approval line goes) and `tasks.md` (the same checkbox
row grammar), imports the change and adopts it into the subject. It needs
no plans source and no longer scaffolds `openspec/changes/<slug>/`, so the
old refusal for a repo with no `openspec` source is gone; it still
refuses an unknown subject, or a slug that already has a
`.canon/changes/<slug>` dir (active or archived) or a change record. It
prints `canon change new: wrote .canon/changes/<slug>/brief.md`, the same
for `tasks.md`, and ``canon change new: recorded change `<slug>` adopted
into subject `<id>` — …``. `.canon/changes/` is a native plan source
(dialect id `canon`) that is never listed in `canon.yaml`; a
`plans.sources` entry naming `canon` is refused. Every plan reader
(`canon ingest plans`, `canon gate task`, `canon evidence add --task`,
`canon dispatch --task`, `canon status` and the brief check) reads it
first, with no `plans:` entry, whenever the directory exists, so it owns
a change id an external source also carries. Its records carry the actor
`canon-plan-import-canon`; an external openspec change's carry
`canon-plan-import-openspec`. `canon init` no longer creates
`openspec/changes/`: its `plans:` section is `sources: []`, preceded by
comments explaining `.canon/changes/` and commented `openspec`
(`root: .`) and `superpowers` (`root: docs/superpowers/plans`) examples.
`plans.sources` lists only external plans, which canon reads. With no
`.canon/changes/` and no sources, `canon gate task` and
`canon dispatch --task` say the repo configures no plan sources and
point at `canon change new` or `plans.sources`.

**D3: approvals live only in canon's file.** The `Approved:` line is read
only from `.canon/changes/<change>/brief.md` (outside HTML comments and
fenced code, as before). A change from an external plan gets its brief
there too: a `brief.md` holding a `Source: <path>` line that points at
the external document and, once the human answers, the `Approved:` line.
Such a pointer `brief.md` is not a change of its own (the native reader
skips it), and an `Approved:` line inside an external `proposal.md` is
not read. The brief is enforced when the subject's newest adopted change
has a canon `brief.md` without `Approved:`, or has no canon `brief.md`
but an external openspec `proposal.md` under the plan sources; then the
human step, the `canon evidence add` refusal and the ship guard name the
canon file to write, with its `Source:` line for an external plan
(``create .canon/changes/<c>/brief.md holding a `Source: <proposal>` line
and record their words there as `Approved: <who> <when> — "<their
words>"` ``). `canon status` prints `brief: approved in <brief> — <words>`
or `brief: not approved (…)`, and `--json` `brief` is `{approved,
enforced, change?, brief?, briefExists, source?, approval?}`; the old
`proposal` field is gone, so `statusVersion` is now `3`. `--override-reason` is unchanged.

**D4: `canon gate task` still flips external task rows.** Canon only reads
external plans, with one exception: `canon gate task` flips the task's
checkbox in an external plan's `tasks.md`, the one place it writes into
one. Open for the maintainer: record external flips only in the ledger
instead.

**Upgrading.**

- An approval recorded as an `Approved:` line in an openspec
  `proposal.md` is no longer read. Move it into
  `.canon/changes/<change>/brief.md`, with a `Source: <path to the
  proposal.md>` line; until then `canon status` lists the human step for
  that subject.
- Repos that configured `{ dialect: openspec, root: . }` keep it as an
  external source; new changes go to `.canon/changes/`.
- Scripts that parse `canon status --json` read `brief.brief`,
  `brief.briefExists` and `brief.source` instead of `brief.proposal`.
- Scripts that match `canon change new` output see the
  `.canon/changes/<slug>/brief.md` and `tasks.md` paths.

## 0.16.0 release

Version 0.16.0 lets the agent drive after one approval: the human answers
once per scope, canon does its own bookkeeping, `canon init` offers policy
levels, a reviewer can attach what it looked at, and reviews written before
0.15 are checked against the commit they pinned. (0.17.0 moved the brief
and its `Approved:` line from the change's `proposal.md` to
`.canon/changes/<change>/brief.md`; see the 0.17.0 release above.)

**Why.** The maintainer's rule (2026-10-11): an agent reviewing another
agent is cheap; a human blocking the work is expensive. After one
approval the agent should run on its own, with independent review on every
change kept. Dogfood eval 2 measured two human stops (brief and policy
approval at the start, then `shipped` left for the human after 68 minutes
of autonomous work, because the skill did not say who ships), no canon
refusals in 116 calls, and about a fifth of calls spent on bookkeeping (16
`gate promote`, 5 `format`, 4 `inventory sync`). The skill applied the
human-approved brief to every feature change, including a one-line fix
inside an already approved subject.

**D1: one approval per scope (#21).** Only a new subject, or a change to
an approved subject's scope, needs the human, and one answer approves both
the brief and the policy. The agent records it at once: an
`Approved: <who> <when> — "<their words>"` line in the change's
`proposal.md`, and `canon policy approve --by <the human> --note "<their
words>"` run on the human's behalf; it never approves without a human
answer. The approval is the one thing never assumed (M7): the agent
writes the brief, then stops and reports it, and builds and attests
nothing for that scope until the human answers. Inside an approved scope
the agent never stops for a human: fixes, follow-ups, review rounds and
re-reviews go to scenario → implement →
evidence → independent review, and a fix stays in the approved change as
tasks in its `tasks.md` (a new `canon change new` is a new scope). The
agent moves the subject to `shipped` itself once the ship guard passes,
unless the brief says the human ships. The skill, the `AGENTS.md` block
`canon init` writes, `canon status` and `canon evidence add` carry the
rule:

- The skill's step 1 and the `AGENTS.md` block say: write the brief,
  then stop and report it; build and attest nothing for that scope until
  the human answers. The approval is never assumed, even when the human
  said they cannot answer until you report: the brief is that report.
  "When no human can answer" covers one of the brief's questions and work
  inside an approved scope, never the approval. Step 8 calls the ship
  refusal and the evidence refusal backstops; the stop is at step 1.
- `canon status` shows, for each building, verifying and shipped subject,
  and each proposed subject whose newest adopted change has a
  `proposal.md`, whether its brief is approved, read from the `proposal.md` of the
  subject's newest adopted change under the openspec plan sources. An
  older change's approval does not cover a newer scope, and an
  `Approved:` line counts only outside HTML comments and fenced code.
- An unapproved brief that exists is the subject's only step, whatever
  its status: when the newest adopted change has a `proposal.md` with no
  `Approved:` line, `next:` lists `human: stop and report the brief in
  <proposal.md>; build and attest nothing for <id> until the human answers
  (one answer approves brief and policy)` and no evidence, review or ship
  step for that subject. Its reason adds that `canon evidence add` refuses
  the subject's scenarios and tasks until the line is recorded. With no
  plan source, no adopted change or no `proposal.md` there is no brief to
  answer: the `brief:` line still says not approved, as an advisory, and
  nothing is withheld. With the brief approved (or none to answer) and the
  guard passing, the ship step says to ship it yourself unless the brief
  says the human ships.
- `canon evidence add` refuses evidence under an unapproved brief (M7).
  A `--scenario-id` resolves to the scenario's `@subject` owner; a
  `--task <change>#<n>` resolves to every subject whose adopted changes
  include that change. For each such subject whose brief is enforced and
  not approved, it prints `canon evidence add: unreviewed-promotion
  <subject> — evidence for <scenario or task>: brief not approved — …`
  naming the `proposal.md`, then a `refused` line telling the agent to
  stop, report the brief and wait for the human's answer. It exits 1 and
  stages nothing. Subjects with no brief to answer are not refused, and
  neither are `canon review add` and `canon finding add`.
- `canon evidence add --override-reason <one line>` attests anyway. It
  prints the `waived unreviewed-promotion …` lines and
  ``override recorded by `<actor>` for the N brief refusal(s) above``, and
  the record carries `brief_override: {reason, waived: [{class, subject}],
  actor}` (an optional field on the evidence record, no new record kind).
  A blank or multi-line reason exits 2. An override when nothing was
  refused records nothing and says so. `canon status` lists each
  overridden record as a warning naming the actor, the time, the subject
  and the reason, and `canon gate check` lists them under a
  `brief overrides:` advisory that does not fail the gate.
- The ship guard checks the brief too, through the same check status
  uses, so status never says `ship` for a subject the guard refuses.
  `canon subject status <id> shipped` refuses only where a brief exists:
  a newest adopted change whose `proposal.md` has no `Approved:` line,
  as `unreviewed-promotion <subject>`, naming that `proposal.md` and
  saying a human must answer; the agent writes the line only from that
  answer. `--override-reason` waives it like a review gap and records it
  in the waiver. With no brief to answer the move prints an advisory and
  is not refused. A brief that says the human ships changes who runs the
  move, not the check. This refusal and the evidence refusal are
  backstops: the agent stops at the brief before either fires.
- `status --json` is `statusVersion: 2`: `next[].actor` (`agent` or
  `human`) and `subjects[].brief` (`approved`, `enforced`, `change`,
  `proposal`, `approval`) are new; `enforced` is true when the missing
  approval blocks.

**D2: fewer bookkeeping calls (#22).**

- *Promote on write.* `evidence add`, `finding add`, `finding close` and
  `divergence stage` take `--promote` and `--no-promote`. With
  `--promote`, or under `canon.yaml`'s `write: { promote: auto }`, they
  promote the one record they just staged in the same run (the same
  validation and `run_seq` assignment as `gate promote`) and print one
  success line naming the committed record (`wrote kind=…/….json`, not
  the staged file promotion renamed), ending
  `— staged and promoted in this run; nothing to promote`. A record
  another run staged stays staged, and a promotion that refuses the record
  leaves it staged and exits 1. `canon init` writes `promote: auto` for
  new repos; without a `write:` key a repo keeps the 0.15 behavior
  (`manual`). A malformed `write:` section refuses every staged write
  (exit 2) before anything is staged, and `init --check-config` reports
  it as `[FAIL] write`.
- *Fresh index without a sync step.* `gate check`, `status`,
  `subject status`, `review add` and `evidence add` rebuild a stale
  scenario index from the `.feature` files before reading it: the same
  idempotent sync, materializing only the changed files, with one stderr
  line naming them. A fresh index prints nothing. `inventory sync` stays.
  A lock (`.canon/scenario-index.lock`) serializes concurrent syncs and
  refreshes so each record is written once.
- *Removed scenarios.* When a scenario's id is gone from every
  `.feature` file of its root, sync (or a reader's refresh) appends a
  `removed: true` generation. Every reader, the report marts and artifact
  ingest treat it as absent, so a deleted scenario stops being graded,
  counted or reviewable; re-adding it supersedes the tombstone. A root
  that cannot sync marks nothing removed.

**D3: starter policy levels (#23).** `canon init --policy
light|standard|strict`: `light` requires evidence, `standard` adds a
failure case per feature, `strict` adds independent review. The default
stays `strict`, byte-identical to the 0.15 starter under a first line
`# canon policy level: <level>`; the lower levels carry the left-out
requirements as commented lines. An unknown level, or `--policy` with
`--no-policy`, is a usage error (exit 2) and nothing is written.
`canon status` names the level (the level line, else the level the
`spec_coverage` section matches exactly, else `custom`; `policy.level` in
`--json`, `null` without a policy) and, for `light` and `standard`, how to
raise it.

**D4: small next items.**

- *#8:* `canon init`'s `.gitignore` ignores `.canon/audit/` (the access
  audit `query`, `export` and `purge` append to), the scenario index
  `.canon/ledger/kind=scenario/`, `.canon/scenario-index.lock` and
  `.canon/artifact-store.lock`. canon's own `.gitignore` ignores the audit
  directory and both locks.
- *#9:* the skills install lock records `canon_version` and
  `source_commit` (provenance only; a new commit that leaves the bundle
  unchanged does not rewrite the lock). When the lock's `canon_version`
  is not this canon's, or the lock predates 0.16 and records none,
  `skills check` exits 1 and both `check` and `doctor` (as an `outdated:`
  line) name the install command.
- *#10:* `skills doctor` lists co-installed skills in `.claude/skills`,
  `.agents/skills`, `.omp/skills` and `.pi/skills` whose frontmatter
  description claims build work (one sentence holds a build verb and a
  build object), as an advisory `competing-skill:` line warning that
  canon's loop will not run if an agent picks one for build work. It never
  edits them and still exits 0.

**D5: review artifacts (#11).** `canon review add --artifact <path>`
(repeatable, with `--max-artifact-mib`) stores what the reviewer looked at
like evidence: in-repo, size-limited, written to
`.canon/artifacts/sha256/` before the review, all or nothing, and recorded
as `artifacts: [{path, sha256}]` on the review. `gate check` re-hashes the
stored blobs of every in-effect review; a missing or altered blob is
`stale-evidence` naming the review. A file outside the repository is
refused (exit 2) and no review is written. The skill tells reviewers of
user-visible behavior to attach screenshots or recordings, and authors to
bind them to visual scenarios.

Writes to the artifact store hold a repo lock (`.canon/artifact-store.lock`)
from the "already stored?" check to the last rename, so a rollback never
removes a blob another writer published or counted as stored. A blob
already present with the same bytes counts as stored and is never removed
by a rollback.

**D6: legacy reviews checked against their pin (#12).** A review written
before 0.15 carries no pins, only `pin`, the commit it reviewed.
`gate check`, `status` and `subject status` read the scenario at that
commit through one `git cat-file --batch` process (never fetching) and
decide the same way:

- *current* when the scenario's per-scenario digest at the pin equals
  today's and no declared evidence is newer than the review: it counts;
- *stale* when its spec changed since the pin or evidence is newer:
  `unreviewed-promotion`, like a stale pinned review;
- *undecidable* when the commit or the scenario at it is not in this
  clone (shallow clone, rewritten history): it still counts, as in 0.15.
  A missing commit is never a violation.

A scenario that lived in another feature file at the pin is found there.
The advisory now counts current, stale and undecidable, counts a stale
one as superseded when its scenario already has a qualifying review, names
only the scenarios still owed a review, and suggests `fetch-depth: 0`.
canon's own CI gate job checks out full history. On canon's checked-in
ledger, `canon gate check` prints 338 legacy reviews: 316 current, 14
stale and 8 undecidable, with 9 of the stale ones superseded by a current
review of the same scenario (M6's spec edits staled two more, both already
re-reviewed). The plan's 348 counted every legacy review
record; canon counts only the latest review per scenario and reviewer on a
scenario that still exists, which leaves out 8 replaced by the same
reviewer's later review and 2 of scenarios no feature file declares any
more (`skills.install.05` and `skills.install.06`).

**M6: no bookkeeping steps in the guidance (dogfood eval 3).** The eval 3
acceptance run measured bookkeeping at 11 of 130 canon call sites (8.5%,
over the 5% bar): 5 `format` and 5 `inventory sync` calls, and one
`gate promote` that found nothing staged. Promote on write and the
stale-index refresh both worked; the skill and `canon status` still told
the agent to run the commands, so it always synced first and the refresh
never fired. The guidance now names a bookkeeping command only where it
is needed:

- The skill's loop has no format or sync step: the readers index edited
  `.feature` files themselves, and `canon inventory sync` is named only
  for a command that printed `NOT refreshed` (a format violation, such as
  a hand-written header with no `# canon:` line, or a duplicate id).
- *Fail closed on a refused refresh.* When a root's `.feature` files
  changed but its refresh is refused, `canon gate check` exits 1 with
  `uncovered-cell <root>` (the class the ship gate already uses when a
  subject's scenarios cannot be counted) saying `canon inventory sync`
  lists the problems; `canon subject status <id> shipped` refuses with
  the same violation; and `canon status` lists the sync as its only step.
  All three read one decision (`unrefreshable_roots`). Before, the gate
  graded the old index and could pass. `canon evidence add --task` and
  `canon gate task` read `tasks.md` as it is, so a new task row needs no
  `canon ingest plans`. `canon gate promote` is named only for
  `write.promote: manual`, in the skill, the `AGENTS.md` block and the
  loop in `canon evidence --help`, `canon finding --help` and
  `canon finding add --help`.
- `canon status`: a subject with no scenario says "specify its behavior,
  one scenario per behavior" (was "…, then `canon inventory sync`"); the
  unowned-scenario step is `canon status` after tagging (was
  `canon inventory sync`); the remaining sync step, for `.feature`
  scenarios with an empty index, says the automatic refresh could not
  index them; and the evidence step adds "then `canon gate promote`" only
  under `write.promote: manual`.
- `canon status` shows the brief for a `proposed` subject once its newest
  adopted change has a `proposal.md`. While that brief is unapproved the
  human step is the subject's only step, with no `feature new` or
  `scenario new`. A proposed subject with no adopted change or no
  `proposal.md` is unchanged. M7 makes the human step the only step at
  every status and adds the `canon evidence add` refusal (see D1).
- A promote on write prints one success line naming the committed file
  (`canon evidence add: wrote kind=evidence_record/… — staged and promoted
  in this run; …`), where it printed the promote line and the staged
  file's different name.

**M7: stop at the brief (dogfood eval 3b).** Eval 3b regressed on
criterion (a), one human stop before `shipped`. The agent wrote the
brief, and `canon status` showed the human step, but the agent kept
building for 2 hours with no approval. It read the skill's "when no human
can answer, write the question into the brief, proceed on the assumption"
as covering the approval, and step 8 presented brief approval as a ship
gate only, so it read that only `verifying → shipped` is refused. The
maintainer decided that the approval is the one thing never assumed, and
that canon enforces it: `canon evidence add` refuses a scenario or task
whose subject's brief is enforced and not approved, unless an explicit
`--override-reason` is given, which is recorded. D1 above describes the
skill wording, the refusal, the override and the new status step.

*Migration:*

- Re-run `canon skills install` (with your `--providers`). Until you do,
  `canon skills check` exits 1: the lock was written by 0.15 or earlier,
  which records no `canon_version`.
- Re-run `canon init` in an existing repo to refresh the `AGENTS.md`
  block with the one-approval loop; it leaves `canon.yaml` and the policy
  untouched.
- `canon init --policy light|standard|strict` picks the starter level;
  the default `strict` policy is unchanged apart from its level line, so
  its `policy_digest` differs from a 0.15 starter's. An existing policy
  has no level line and `status` shows the level it matches, or `custom`.
- Add `write: { promote: auto }` to `canon.yaml` to promote on write;
  without it, staged writes still wait for `canon gate promote`.
- Add `.canon/audit/`, `.canon/scenario-index.lock` and
  `.canon/artifact-store.lock` to `.gitignore`
  (`init` does this only for new repos). New repos also ignore the
  scenario index `.canon/ledger/kind=scenario/`; an existing repo may keep
  it tracked, since every reader rebuilds a stale index.
- Set `fetch-depth: 0` on the CI checkout that runs `canon gate check`,
  or every legacy review stays undecidable and unchecked. With full
  history, a legacy review whose scenario drifted since its pin turns the
  gate red until a new `canon review add` pins it.
- Record each subject's brief approval as an `Approved:` line in its
  newest adopted change's `proposal.md`; while that file lacks one,
  `canon status` lists the human step (stop and report the brief) as the
  subject's only step, `canon evidence add` refuses its scenarios and
  tasks, and `canon subject status <id> shipped` refuses. Subjects with no
  adopted change or no `proposal.md` ship as before, with an advisory.
- An agent or script that attests evidence before the brief's `Approved:`
  line exists now gets exit 1 from `canon evidence add`. Record the
  human's answer first, or pass `--override-reason <text>`, which is
  recorded on the evidence record. Repos whose subjects have no
  `proposal.md` (canon's own and Kino's existing subjects) see no change.
- Scripts that match the `canon status` human step must match the new
  text: `human: stop and report the brief in <proposal.md>; build and
  attest nothing for <id> until the human answers (one answer approves
  brief and policy)`.
- Scripts that parse `canon status --json` see `statusVersion: 2`, with
  `policy.level`, `subjects[].brief` and `next[].actor`.

**Changed conformance expectations.** `version` and every case that
prints the version re-blessed for `canon 0.16.0`: the `status-*` cases,
`evidence-add-drop-attachment`, `gate-check-audit-false-success`,
`gate-check-audit-sampling-fixed-seed`, `policy-approve-and-advisory`,
`policy-approve-changed-advisory` and `review-pin-stale-spec`, and the new
`init-policy-light-status`, `init-policy-refused`,
`skills-check-outdated-lock`, `status-brief-unapproved`,
`status-refreshes-stale-index`, `evidence-add-refused-unapproved-brief`
and `evidence-add-brief-override`. Existing cases re-blessed:

- `context-json-policy-summary` — the schema lists the review's
  `artifacts`, the scenario's `removed`, and the evidence record's
  `brief_override` (M7).
- `evidence-add-drop-attachment`, `gate-check-audit-false-success`,
  `policy-approve-and-advisory`, `policy-approve-changed-advisory`,
  `status-gaps`, `status-no-scenarios`, `status-ship-ready`,
  `review-pin-stale-spec` — `status` prints the policy `level:` line.
- `policy-approve-and-advisory` — the starter policy's digest changes
  with its level line.
- `gate-check-audit-sampling-fixed-seed`, `status-gaps`,
  `status-no-policy`, `status-ship-ready` — `status --json` is
  `statusVersion: 2` with `policy.level`, `next[].actor` and, for
  building, verifying and shipped subjects, `subjects[].brief`.
- `status-gaps`, `status-ship-ready` — the fixtures gain an approved
  `proposal.md` and `status` prints the `brief:` line; the ship step reads
  "and the brief is approved; ship it yourself unless the brief says the
  human ships".
- `finding-add-self-found-blocker`, `review-gate-distinct-review-passes`,
  `review-gate-malformed-finding`, `review-gate-open-blocker`,
  `review-gate-self-review-not-counted`, `status-gaps` — the legacy-review
  advisory counts current, stale and undecidable reviews and suggests
  `fetch-depth: 0`.
- `review-pin-stale-spec` — `review add` first refreshes the stale index
  and pins the scenario's current text by its scenario digest, so the
  later `inventory sync` writes nothing, `gate check` is clean (exit 1 → 0
  in `case.yaml`), and `status` counts the review and asks for the brief
  approval. A spec edit after the review is pinned by
  `tests/review_pins.rs`.
- `subject-ship-refused-missing-verdict`,
  `subject-ship-refused-no-tagged-scenarios`,
  `subject-ship-refused-require-cases-gap`, `review-pin-stale-evidence` —
  the subject adopts no change, or its change has no `proposal.md`, so
  the guard adds one advisory line that the brief is not approved; the
  refusals themselves are unchanged.
- `review-pin-stale-spec` — its change `c-cart` has no `proposal.md`, so
  `status` shows the brief as an advisory and suggests the agent's ship
  step instead of the human step.
- `status-brief-unapproved`, `status-gaps`, `status-ship-ready` —
  `subjects[].brief` carries `enforced`.
- `subject-ship-allowed` — the fixture gains an adopted change whose
  `proposal.md` carries an `Approved:` line; its output is unchanged.

- `evidence-add-drop-attachment`, `gate-check-audit-false-success`,
  `gate-check-audit-sampling-fixed-seed` (M6) — the unowned-scenario step
  is `canon status`, "then check again: status re-indexes the edited
  files itself", not `canon inventory sync`.
- `status-no-scenarios` (M6) — the empty subject's reason is "specify its
  behavior, one scenario per behavior", without the sync.
- `evidence-add-promote-on-write`, `status-refreshes-stale-index` (M6) —
  a promote on write prints one line, `wrote <committed file>`, instead of
  the promote line and `staged <staged file>`.
- `status-brief-unapproved`, `status-brief-proposed`,
  `subject-ship-refused-unapproved-brief` (M7) — the human step reads
  `human: stop and report the brief in <proposal.md>; build and attest
  nothing for <id> until the human answers (one answer approves brief and
  policy)`, and its reason adds that `canon evidence add` refuses the
  subject's scenarios and tasks until the line is recorded.

New cases: `evidence-add-promote-on-write`,
`evidence-add-refused-malformed-write-config`,
`gate-check-legacy-review-missing-pin`,
`gate-check-review-artifact-stale-evidence`, `init-policy-light-status`,
`init-policy-refused`, `review-add-artifact-stored`,
`review-add-refused-artifact-outside-repo`, `skills-check-outdated-lock`,
`skills-doctor-competing-skill`, `status-brief-unapproved`,
`status-brief-proposed` (M6), `status-refreshes-stale-index`,
`subject-ship-refused-unapproved-brief`,
`evidence-add-refused-unapproved-brief` (M7),
`evidence-add-brief-override` (M7). Every other case is unchanged.

**Closed issues:** #8, #9, #10, #11, #12, #21, #22, #23. **Deferred:**
#13 (Codex eval), as the maintainer asked.

**Acceptance (plan D7, dogfood eval 3c).** The release ran the eval-1
one-line request ("make a cat-themed Vampire-Survivors-like game") with omp
in a fresh repo initialized with this build. It used three turns: the
request, one human answer, then a follow-up fix with no approval. All four
criteria pass.

- **(a) One human stop before `shipped`.** The agent stopped at the brief
  after 91 seconds, before writing any code. It recorded the human's words
  as `Approved:` and as `canon policy approve`, then shipped in 73 minutes
  with no further stop.
- **(b) The follow-up fix needed no human.** The agent added it as a task in
  the approved change, wrote a new scenario and its evidence, and got an
  independent review.
- **(c) Bookkeeping was gone.** Across the four sessions, 0 of 49 canon call
  sites were bookkeeping (`gate promote`, `inventory sync`, `format`).
  Eval 2 had 21.6%; the first eval-3 run had 8.5%, which M6 fixed.
- **(d) Everything was reviewed and the gate was clean.** All 30 scenarios
  have a review from a different actor and session than their evidence, and
  the gate is clean.

An intermediate rerun (3b) failed (a), which is what M7 fixes. The evidence
is in `~/Workspace/canon-dogfood/PROTOCOL-3.md` and `FINDINGS-3.md`.

## 0.15.0 release

Version 0.15.0 makes the evidence chain hold over time: a review counts only
for what it saw, a re-attest cannot quietly drop proof, an external audit sits
beside the declared result, a reviewer's failed run is visible, and the
policy has an approver.

**Why.** Two inputs. The 2026-10-09 research briefing
(`docs/research/2026-10-09-canon-bard-weekly-briefing.md`) concludes that
success scores, approval records and independent reviews can all silently
turn false unless context, execution and evaluator are bound together; 0.15
takes its TestJack-style audited outcome
([arXiv:2610.10619](https://arxiv.org/abs/2610.10619)) and the reviewer half
of the cross-provider review contract
([arXiv:2610.10961](https://arxiv.org/abs/2610.10961)). Dogfood eval 2, the
0.14 acceptance rerun, showed records going stale or vanishing unnoticed: a
review kept counting after its scenario and evidence changed, a re-attest
dropped bound files from checking, nobody was recorded as approving the
policy, and a reviewer's failure looked like a review never run.

**D1: reviews pin what they saw.** `canon review add` now computes and
pins the reviewed scenario's digest and the ids and digests of its latest
evidence records; the reviewer types neither. The scenario digest covers the
scenario's own block and tags plus what it inherits (Feature tags, Feature
Background, Rule and its Background), normalized, so editing a sibling
scenario in the same `.feature` file leaves the review current. Under
`require_review` a pinned review qualifies only while both pins match;
otherwise the scenario is `unreviewed-promotion` with a "stale review"
detail naming what changed (its spec or its evidence), and `canon status`
counts stale reviews. Reviews written before 0.15 carry no pins: they keep
qualifying, and `gate check`, `status` and the ship guard print one advisory
naming their count (`N review(s) predate review pinning (0.15) …`). Reviews
fold to the latest one per reviewer, so a reviewer's new review replaces
their own earlier one, including a legacy one.

**D4: reviewer outcome and runtime.** `review add` takes
`--outcome signed|indeterminate|unavailable` (default `signed`) and
optional `--reviewer-provider`, `--reviewer-model` and `--reviewer-harness`.
Only a `signed` review qualifies; when a reviewer's latest review is
`indeterminate` or `unavailable`, the gap says "no signed review" and names
each reviewer's outcome, and `canon status` counts unsigned reviews.

**D2: drop guard.** `canon evidence add` refuses (exit 2) a new latest
record for a scenario that leaves out files the previous latest record
bound, naming the missing paths. Re-bind them, or pass
`--drop-attachment <path>` (repeatable) with `--drop-reason <text>`; the
drop is recorded on the record and listed by `canon status`. Naming a path
that was never bound is refused.

**D3: audit evidence.** `canon evidence add --kind audit` records an
external evaluator's re-check (`--evaluator-id`, `--evaluator-version`,
`--evaluator-digest`, `--audit-result confirmed|refuted|indeterminate`, and
counterexample `--report`s, bound and stored like any report) beside the
declared record, never replacing it. `spec_coverage.require_audit: { rate,
seed, scope }` samples deterministically: a scenario is sampled when the
first 8 bytes of `sha256(seed \0 project_id \0 scenario_id)`, read
big-endian, fall below `ceil(rate × 2^64)`; `scope` defaults to
`[verifying, shipped]`. A sampled scenario whose latest audit is missing or
indeterminate is `uncovered-cell`, and `canon status` lists audits sampled,
confirmed, owed and refuted with the command to run. A latest `refuted`
audit is the new tenth failure class, `false-success`: the ship guard
refuses it even with `--override-reason`, and a later `faithful` declared
record does not clear it. The re-audit command the gate and `status`
suggest includes `--drop-attachment` for the refuted audit's
counterexamples. canon never generates or runs audits; an external
evaluator does, and canon records the result.

**D5: policy approval.** `canon policy approve --by <who> [--note]` writes
the tracked `.canon/policy.approval.json` with the policy digest, approver,
time and note. `gate check` and `canon status` print an advisory while the
policy is unapproved or has changed since approval; the exit code is
unchanged. `canon init`'s starter policy is unapproved until a human runs
the command, and `init` and the skill's stop point say so.

**D6: small integrity fixes.**

- `finding add` and `finding close` resolve short SHAs through git and store
  the full SHA; a short SHA that names no commit is refused.
- The access audit (`.canon/audit/access.jsonl`, written by `query`,
  `export` and `purge`) is written only beside the `canon.yaml` the command
  read, never into another repo or into one without `canon.yaml`. `status`,
  `context` and `gate check` read without auditing, as before. The stray
  tracked `crates/canon-cli/.canon/audit/access.jsonl` was removed.
- The skill tells an author to record defects they find in their own work
  when they find them, before review, and that task evidence must bind the
  artifact or report proving the task.
- `canon gate task` warns when a task's evidence has no attachment and its
  ref is a canon command.

**Acceptance.** The eval-2 repo (`nyan-omp-014`) was rechecked on a copy
with the 0.15 build: it stays clean with only the two advisories (33
unpinned reviews, unapproved policy), and every D1–D5 rule refused, turned
the gate red or printed its advisory as planned. The recheck found two
defects, fixed before release: review pins were whole-file digests (now
per-scenario) and the re-audit suggested after a refuted audit tripped the
drop guard (now suggests `--drop-attachment`). Its remaining risk is that
legacy reviews keep qualifying, so D1 and D4 bite in an upgraded repo only
after re-review.

*Migration:*

- Reviews recorded before 0.15 keep counting, with an advisory naming how
  many.
- Re-review (`canon review add` by the same reviewer) to pin a scenario;
  until then its spec and evidence can change without the review going
  stale.
- Have a human run `canon policy approve --by <who>` and commit
  `.canon/policy.approval.json`.
- Scripts that parsed the failure-class list now see ten classes
  (`false-success` added).
- A re-attest that binds fewer files than the previous record now needs
  `--drop-attachment <path> --drop-reason <text>`.
- Re-run `canon skills install` to project the updated skill.

**Changed conformance expectations.** `version` and the `status-*` cases
re-blessed for `canon 0.15.0` (including the new cases that print the
version: `evidence-add-drop-attachment`, `gate-check-audit-false-success`,
`gate-check-audit-sampling-fixed-seed`, `policy-approve-and-advisory`,
`policy-approve-changed-advisory`, `review-pin-stale-spec`). Existing cases
re-blessed:

- `context-json-policy-summary` — the schema lists the new record fields
  (`pins`, `scenario_digest`, `outcome`, `reviewer_provider|model|harness`,
  `audit`, `attachment_drop`) and the `ReviewOutcome` enum.
- `evidence-vault-reports-unstorable`, `evidence-vault-stores-unstored`,
  `gate-check-artifact-stale-evidence`,
  `gate-check-artifact-stored-survives-rewrite`,
  `gate-check-artifact-tampered-blob`,
  `gate-check-artifact-unstored-advisory`, `gate-check-clean`,
  `gate-check-evidence-binding-malformed`, `gate-check-evidence-binding-off`,
  `gate-check-evidence-binding-require`,
  `gate-check-evidence-binding-require-report`,
  `gate-check-evidence-binding-warn`, `gate-check-red-uncovered-cell`,
  `gate-check-require-cases-golden-path`,
  `gate-check-spec-coverage-unevidenced`, `gate-check-unknown-policy-key`,
  `review-gate-unreviewed-refused`, `review-gate-override-recorded` — the
  `policy not approved` advisory.
- `review-gate-distinct-review-passes`, `review-gate-malformed-finding`,
  `review-gate-open-blocker`, `review-gate-override-recorded`,
  `review-gate-same-session-not-counted`,
  `review-gate-self-review-not-counted`, `review-gate-unreviewed-refused`,
  `finding-add-self-found-blocker` — the guard line now reads "a signed
  review, current with the spec and evidence it pinned, by an actor, and
  from a session, other than its evidence's"; cases with an unpinned review
  add the legacy-review advisory (and `gate check` the policy advisory).
- `review-gate-same-session-not-counted` — `review add` prints what it
  pinned.
- `init-scaffold-and-rerun`, `status-no-scenarios` — `init`'s policy line
  names `canon policy approve --by <who>`.
- `status-gaps`, `status-ship-ready`, `status-no-scenarios` — the policy
  (and, in `status-gaps`, legacy-review) warnings; `--json` adds
  `staleReviews` and `unsignedReviews`.

New cases (the tamper suite, see `conformance/README.md`):
`review-pin-stale-spec`, `review-pin-stale-evidence`,
`evidence-add-drop-attachment`, `gate-check-audit-false-success`,
`gate-check-audit-confirmed-clean`, `gate-check-audit-sampled-missing`,
`gate-check-audit-sampling-fixed-seed`, `review-outcome-indeterminate-gap`,
`review-outcome-unavailable-gap`, `policy-approve-and-advisory`,
`policy-approve-changed-advisory`, `policy-approve-refused-no-policy`,
`finding-add-refused-unresolvable-short-sha`,
`gate-task-warns-canon-command-proof`. Every other case is unchanged.

**Deferred to 0.16** (from the briefing):

- Skill-resolution and supply-chain provenance
  ([arXiv:2610.11647](https://arxiv.org/abs/2610.11647),
  [arXiv:2610.11169](https://arxiv.org/abs/2610.11169)): needs a run-lineage
  design dispatch consumers agree on.
- Tool-operation policy results ([GitHub changelog](https://github.blog/changelog/2026-10-07-local-sandboxing-for-github-copilot-now-generally-available/)):
  needs an adapter contract with a runtime that produces them.
- Spec-graph/code divergence ([arXiv:2610.11725](https://arxiv.org/abs/2610.11725)):
  large, and needs an external analyzer.
- Review-quality scoreboard (ReviewBench, GitHub Blog): follows the
  provenance work so its numbers are grounded.

## 0.14.0 release

Version 0.14.0 teaches agents how to work with canon: the skill is a working
loop, `canon init` sets that loop up, bound evidence stays provable, units
and reviews carry a session, and `canon status` says where a repo stands and
what to run next.

**The working loop.** The canon skill now opens with an 8-step loop: brief,
subject, features and scenarios with failure paths, units (one actor id and
session each), implement, evidence, independent review in a different
session, status transition. It names the human stop points (approving the
brief and the policy), and the command reference follows the loop. Its
description now claims build, implement, add-a-feature, change-a-feature and
fix requests in any repo with `canon.yaml`, ahead of generic site, app or
game builder skills; it used to describe itself as a CLI companion.

**`canon init` sets up the loop.** Next to `canon.yaml`, `init` now writes a
starter `.canon/policy.yaml` (`spec_coverage` with `require_evidence: true`,
`require_cases: [failure]` and `require_review: {}`), a canon block in
`AGENTS.md` between `<!-- canon:begin -->` / `<!-- canon:end -->` (created,
or merged without touching text outside the markers), and an `openspec`
plans source rooted at `.` with `openspec/changes/` created. `--no-agents-md`
and `--no-policy` opt out. Rerunning `init` on an existing repo leaves
`canon.yaml` alone, refreshes only the AGENTS.md block and exits 0 (it used
to exit 2); a rerun with nothing to do (`--no-agents-md`) still refuses.
(Since 0.17.0 `init` writes `plans: { sources: [] }` and no
`openspec/changes/`.)

**`canon change new`.** `canon change new <slug> --subject <id> --title <t>`
scaffolds `openspec/changes/<slug>/{proposal,tasks}.md`, imports the change
and adopts it into the subject, with no hand edits to `canon.yaml`. It is
all or nothing up to the adoption, and refuses an existing slug or an
unknown subject. The adopt write it shares with `canon subject adopt` writes
the subject record first (the gate reads adopted changes from it, so a
half-done adoption still has its open blockers counted), writes only the
side lacking the link, and writes nothing for a linked pair. That makes
`canon subject adopt <change> --subject <id>` an idempotent repair: a
failure between the two writes exits 2 and prints that exact command.
(Since 0.17.0 it writes `.canon/changes/<slug>/{brief,tasks}.md` and needs
no plans source.)

**`canon gate check` says when it is not enforcing.** When the corpus has
scenarios and `.canon/policy.yaml` has no `spec_coverage`, the gate prints
`advisory: spec_coverage is off — N scenario(s) are not checked for
evidence; see .canon/policy.yaml` after its result. The exit code is
unchanged.

**Bound evidence stays provable.** `canon evidence add --artifact/--report`
copies each bound file into `.canon/artifacts/sha256/<hex>`, which is
tracked in git. `canon gate check` re-checks every file bound to a latest
evidence record: an intact stored blob is clean even after the working-tree
file is rewritten; a missing blob with a working-tree file that still
matches is clean with an advisory naming `canon evidence vault`; anything
else is `stale-evidence`, naming the path and both digests. `canon evidence
vault` stores the bytes for pre-0.14 records whose files still match. Files
over 25 MiB are refused; `--max-artifact-mib <N>` raises the limit on
`evidence add` and `evidence vault`.

**Evidence is not shaped by tasks.** `--summary` is allowed on
scenario-only evidence. `--report-case` is repeatable and adds cases; every
case whose own leading scenario id is this one is bound, and a `faithful` verdict
is refused when any bound case failed. A flipped task row's note aggregates
every record bound to the task (`— ✅ 3 evidence records (2 faithful, 1
not-applicable); latest: <summary>`), and `gate task` re-ingests the plan
source so `canon query --kind task` agrees with `tasks.md`. A record keyed
by both `--task` and `--scenario-id` now counts for both cells in
`gate check`, the shipped gate and `canon status`.

**Sessions.** `--session-id` on `evidence add`, `review add`, `finding add`
and `finding close` fills `actor.session_id`.
`require_review.distinct_actor` now also rejects a review whose session
equals an evidence session on that scenario, when both are set.
`finding add --introduced-by self` is refused (`introduced_by` is a commit
SHA); an author records a defect in its own work with `--reviewer <me>`,
and its open blocker still blocks.

**Write mode in every write command.** Each write's success line now says
how it landed: `evidence add`, `finding add` and `finding close` end with
``— run `canon gate promote` to commit it``; `divergence stage` names
`canon divergence promote`; every other write (`review add`,
`divergence resolve|defer`, `subject new|adopt|status`, `change new`,
`scenario new`, `feature new`) ends with `— written directly; nothing to
promote`.

**`canon status`.** The new `canon status [--json]` lists subjects by
status with their scenario, evidence and review counts, gaps and policy
warnings, and ends with a `next:` list of concrete commands. `canon
context` now opens with a header pointing to `canon status`, plus a
`warning:` line when no `spec_coverage` is in force; `--json` is
unchanged.

*Migration:*

- Repos initialized before 0.14 keep `plans.sources: []`; `canon change
  new` refuses and names the entry to add, `{ dialect: openspec, root: . }`
  under `plans.sources`.
- Commit `.canon/artifacts/` with the ledger.
- Run `canon evidence vault` once to store bytes for bound records written
  before 0.14, before their working-tree files change.
- Scripts that parse write-command output see the new write-mode suffixes.
- Re-run `canon skills install` to project the new skill.
- A change adopted before this release may already have lost its subject
  link to a re-import; `canon subject adopt <change> --subject <id>`
  restores it.

**Acceptance.** The release was checked by rerunning eval 1's one-line
request ("make a cat-themed Vampire-Survivors-like") in a fresh repo
initialized with this build, on omp. The agent read the skill on its first
call and stopped at the brief for approval. After approval it followed the
loop to `verifying`: 33 scenarios (9 `failure`), all evidenced and
independently reviewed, 17 review findings recorded and closed with
resolution SHAs, and the starter policy left untouched. It left `shipped` to
the human. Of eval 1's sixteen applicable friction items, twelve are
resolved and four partly resolved: unit, session and finder identities are
recorded but are labels the lead chooses, not proven. The run also exposed
the defects fixed below.

**Acceptance rerun fixes.**

- G1: `canon gate task` took ~10 s per flip in a JS repo, because the plan
  import that refreshes the task store digested every file under the
  `root: .` source `init` writes, `node_modules` included. The import now
  digests only the files the dialect reads (for `openspec`, the change
  dirs under `openspec/changes/`), and `gate task` re-imports only the
  flipped change. On a copy of the dogfood repo with its 204 MB
  `node_modules`, a flip went from 9.8 s to 0.06 s (debug build) and from
  1.4 s to 0.02 s (release).
- G7: a report case binds to a scenario only when the scenario id is its
  own: the first scenario-id-shaped token (`<area>.<surface>.<nn>`, dotted
  or underscored) of its full name, classname first, then name; or a
  Cucumber tag equal to it. A case named `game.session.04: restarting as
  in game.session.01` binds to `game.session.04` only. `--report-case`
  still binds any case by name.
- G9: a plan import keeps the `subject_id` the latest change record carries
  when the plan document names none, so a `gate task` flip or a ticked
  checkbox no longer unlinks a change from the subject `change new` or
  `subject adopt` linked it to.
- G10: with subjects but no `.feature` scenario anywhere, `canon status`
  suggests `canon feature new` and each subject's `canon scenario new …
  --subject <id>` instead of `canon inventory sync`, which would index
  nothing.
- G11: a `stale-evidence` line for a stored blob whose bytes were changed
  says "the stored blob does not match its name, so its bytes were
  altered"; "the bound bytes are gone" is kept for a missing blob.

**Changed conformance expectations.** `version` re-blessed for
`canon 0.14.0`. Existing cases re-blessed:

- `evidence-add-refused-report-contradiction` — the refusal now says a
  faithful verdict needs every bound case to pass.
- `gate-check-evidence-binding-off`, `gate-check-evidence-binding-require`,
  `gate-check-evidence-binding-require-report`,
  `gate-check-evidence-binding-warn`, `gate-check-malformed-evidence`,
  `gate-check-red-uncovered-cell` — the `spec_coverage is off` advisory.
- `review-gate-distinct-review-passes`, `review-gate-malformed-finding`,
  `review-gate-open-blocker`, `review-gate-override-recorded`,
  `review-gate-self-review-not-counted`, `review-gate-unreviewed-refused` —
  the guard line now reads "a review by an actor, and from a session, other
  than its evidence's".
- `review-gate-distinct-review-passes`, `review-gate-open-blocker`,
  `review-gate-override-recorded`, `subject-ship-allowed` — `subject status`
  ends with `— written directly; nothing to promote`.
- `status-gaps`, `status-no-policy`, `status-ship-ready` — every write
  suggestion names the unit and its session: `finding close` carries
  `--actor-id <unit> --session-id <session>`, and `subject new` and
  `subject status` carry `--actor-id <unit>`; `review add` quotes
  `"<feature file>"` so it pastes into a shell.
- `evidence-add-refused-no-matching-case` — the no-match refusal now says
  "start the test name with the scenario id or tag it" (G7).

New cases: `change-new`, `init-scaffold-and-rerun`,
`evidence-vault-stores-unstored`, `evidence-vault-reports-unstorable`,
`gate-check-artifact-stale-evidence`,
`gate-check-artifact-stored-survives-rewrite`,
`gate-check-artifact-unstored-advisory`, `finding-add-self-found-blocker`,
`review-gate-same-session-not-counted`, `status-gaps`, `status-no-policy`,
`status-ship-ready` (the `status-*` cases print the canon version and were
re-blessed with it). The `gate-check-evidence-binding-*` fixtures also gained
their stored artifact blobs. The acceptance rerun added
`gate-check-artifact-tampered-blob` (G11) and `status-no-scenarios` (G10).
Every other case is unchanged.

## 0.13.1 fix

0.13.0 and earlier installed the Codex skill as a flattened
`.codex/skills/canon.md`, a directory Codex never reads, so Codex agents never
discovered canon. `canon skills install` now projects Codex the same way as
every other provider, to `.agents/skills/canon/SKILL.md` with its `reference/**`
and `scripts/**` sidecars, and an existing `.agents` or `.codex` directory
selects Codex when `--providers` is omitted.

*Migration:* after upgrading, re-run `canon skills install`; Codex does not
see canon until you do. That run migrates the legacy `.codex/skills/canon.md`
and `.codex/skills/canon/**` projection automatically, but only for files the
lock proves canon wrote: it removes a legacy file only if it is recorded in
`.canon/skills/.install-lock.json` and its bytes still match the recorded
hash, keeps edited, replaced, symlinked, and user-authored files, and removes
`.codex/skills` only if it ends up empty. On unix the cleanup is race-safe:
every directory below the repo is opened without following symlinks and each
file is hashed and unlinked through its parent's handle, so swapping a legacy
directory for a symlink, before or during the run, cannot make it read or
delete a file outside the repo (other platforms check for symlinks first,
which narrows that window without closing it). `skills check` (exit 1) and
`skills doctor` report whatever legacy projection remains, with the fix
command. The developer-only `canon/skills-dev` materializer now writes
`.agents/skills/<name>/SKILL.md` instead of `.codex/skills/<name>.md`; its lock
records no output hashes, so delete old `.codex/skills/<name>.md` files by hand.

**Changed conformance expectations.** `version` re-blessed for
`canon 0.13.1`. Every other case is unchanged.

## 0.13.0 release

Version 0.13.0 makes `canon context --json` parseable, makes an unknown
`policy.yaml` key fail the gate, and has canon pass its own review
requirement.

**Typed policy summary in `canon context --json`.** `spec_coverage`,
`evidence_binding`, and `review.requireReview` are now JSON objects
instead of one-line summary strings. `spec_coverage` and
`evidence_binding` are keyed like `policy.yaml`, with booleans as booleans
and lists as arrays (`{"require_evidence": true, "scope": ["verifying",
"shipped"], "exclude_lanes": [], "require_cases": ["failure"],
"require_review": null}`); `review.requireReview` is
`{"scope": [...], "distinctActor": true, "blockOnFindings": true}` or
`null`. A present but unusable section is `{"invalid": "<detail>"}`
instead of an `INVALID — …` string. `capabilityVersion` is 5. The text
outline is unchanged. *Migration:* a consumer that parsed the old strings
(`require_evidence=true scope=verifying, shipped …`) reads the fields
directly, and checks for an `invalid` key instead of an `INVALID` prefix.

**Unknown `policy.yaml` keys fail the gate.** A top-level key canon does
not know, such as the typo `spec_coverag`, used to be ignored, so a
misspelled section read as not opted in. The new `policy-keys` check makes
`canon gate check` report it as `uncovered-cell <key>`, naming the known
keys: `adapter_capabilities`, `allowed_signers`, `approval`,
`experimental`, `query`, `risk_approvals`, `risk_routing`, `risk_tiers`,
`schema`, `spec_coverage`, `staleness`, `trust_required`, `trust_sample`.
Parsing of the known keys is unchanged. *Migration:* fix or remove any
unknown top-level key before upgrading.

**Canon reviews its own corpus.** Six independent reviewers, none of them
the evidence actor, read each of canon's 257 scenarios, its proving test,
and the code under test. They withheld 17 sign-offs and raised 16 findings
(3 blocker, 13 should-fix). All 16 are fixed, mostly by strengthening the
tests, plus three corrected spec claims (`learn.promotion.01`,
`query.read.04`/`05`, `report.generation.07`); the withheld scenarios were
re-reviewed and every finding is closed as fixed.
`spec_coverage.require_review` is now on in canon's own policy, and review
records (`.canon/ledger/kind=review/`) are tracked in git, so a fresh
checkout, CI included, sees the same reviews.

**Changed conformance expectations.** `version` re-blessed for
`canon 0.13.0`. `context-json-policy-summary` was re-blessed for the typed
`spec_coverage` object, `evidence_binding` as `{"invalid": …}`, and
`capabilityVersion` 5. New case `gate-check-unknown-policy-key` pins the
`uncovered-cell spec_coverag` violation and exit 1. Every other case is
unchanged.

**Website.** The docs site is redesigned as the "Checks Panel" (a
PR-checks hero and a real HTML comparison table, no grid hairlines).

## 0.12.0 release

Version 0.12.0 pins canon's command-line behavior in a conformance corpus,
adds an opt-in independent-review gate, and makes the ledger commands fail
loud on an invalid `canon.yaml`.

**Conformance corpus.** [`conformance/`](conformance/README.md) replays
real `canon` invocations against checked-in fixture repos and compares
normalized stdout, stderr, and exit code byte for byte;
`conformance/regenerate.sh` is the only bless path, and any changed
expectation is a contract change named in the release notes. The case
groups and what they pin:

- `version` — `canon --version` output.
- `gate-check-*` — exit 0 on a clean corpus, 1 on a red cell, 2 on an
  invalid `canon.yaml`; `malformed-evidence` naming the file; the
  `spec_coverage` evidence and `require_cases` golden-path checks; and
  experimental evidence binding off / warn / require / require-report /
  malformed section.
- `usage-error-unreadable-canon-yaml` — exit 2 with the parse error when a
  command cannot read `canon.yaml`.
- `evidence-add-refused-*` — refusals for a file outside the repo, newline
  injection, no matching report case, no subject key, and a report that
  contradicts the verdict.
- `subject-ship-*` and `subject-status-off-chain-transition` — ship refused
  on no tagged scenarios, a missing verdict, or a `require_cases` gap;
  ship allowed; an off-chain status transition.
- `context-json-policy-summary`, `inventory-sync-tag-diagnostics`,
  `query-subject-json` — the read surfaces: `canon context --json` policy
  summary, inventory tag diagnostics, `canon query --kind subject --json`.
- `review-gate-*` — the review gate below: unreviewed promotion refused,
  self-review not counted, distinct review passes, open blocker, recorded
  override, unreadable finding blocks.

**Fail loud on an invalid `canon.yaml`.** The commands that read or write
the ledger (`gate`, `inventory sync`, `evidence`, `review`, `finding`,
`divergence`, `subject`, `report`, `dashboard`, and the rest) now exit 2
naming `<repo>/canon.yaml` and the parse error, where they used to fall
back silently to the default ledger. One deliberate exception:
`canon dispatch begin`/`end` still exit 0, writing the run manifest and
printing the unpersisted ledger write, with the parse error, as a stderr
warning, because failing a live dispatch would lose the run's provenance.
`canon retrieve`, which reads only the learn store, still falls back to its
defaults. *Migration:* a `canon.yaml` that only
worked because of that fallback now refuses — notably a legacy
`tiers: git:` layout; rewrite it to named tiers
(`tiers: { local: { backend: git, root: .canon/ledger } }`) before
upgrading.

**Smaller fixes.** A `malformed-evidence` violation now names the ledger
file for schema-invalid records too (for example `verdict: maybe`), not only
for unparseable JSON. Under `experimental.evidence_binding` with
`strength: report`, the remediation hint names `--report`, never
`--artifact`, since an artifact cannot reach that strength. `canon context`
reports `capabilityVersion` 4.

**Independent review before "done"** ([journeyWorker/canon#2](https://github.com/journeyWorker/canon/issues/2)).
Until now, a subject could reach `verifying` or `shipped` on evidence the
same agent wrote, and open review findings did not affect the gate: nothing
asked whether anyone other than the author had looked at the work. The new
opt-in `spec_coverage.require_review` policy (`scope`, default
`[verifying, shipped]`; `distinct_actor`, default `true`;
`block_on_findings`, default `true`) makes `canon gate check` report each
in-scope scenario with no `canon review add` record by someone other than
its evidence actor as `unreviewed-promotion`, and adds a ninth failure
class, `open-blocker`, for an open `blocker` finding on a change the
subject adopted. `canon subject status` runs the same checks when moving
into a scoped status, prints which ones it ran or skipped, and refuses on
gaps; `--override-reason "<one line>"` (with `--actor-id`) waives only
the violations of those two classes it reports, records exactly those on
the subject, and leaves them listed as advisories by `canon gate check`
until the next status change; a gap that appears later is not covered.
With `block_on_findings`, a finding record canon cannot read (unparseable,
schema-invalid, or misfiled) is a `malformed-evidence` violation naming
its ledger file, in `canon gate check` and in `canon subject status`, and
no override waives it: it might be an open blocker. An unreadable review
record already fails closed, because it counts as no review.
`canon context` now lists finding severities, dispositions, Review fields,
and the active `require_review` setting. Nothing changes for a repo that
does not set `require_review`; canon's own policy leaves it off for now.

**Changed conformance expectations.** `version` re-blessed for
`canon 0.12.0`. `usage-error-unreadable-canon-yaml` `1.err` now reads
``parsing `<repo>/canon.yaml`: canon.yaml TierPolicy: …`` from the shared
fail-loud loader instead of the old inventory-only message.
`context-json-policy-summary` was re-blessed because `canon context --json`
now carries the `review` object, `capabilityVersion` 4, and the Subject
kind's optional `status_override` field. Every other case is unchanged.

**Website.** The docs site is redesigned as the "Grid Manual" (EN and KO
landing, re-themed Starlight docs, a build-time corpus figure).

## 0.11.0 release (experimental evidence binding)

Version 0.11.0 adds an **experimental, off-by-default** way to bind
evidence to files the team's own runner or agent already produced. Canon
still never runs a test. `canon evidence add --artifact <path>` binds any
repository file by sha256 (a trace, screenshots, an agent QA log);
`--report junit:<path>` or `--report cucumber:<path>` also parses the report,
records the case matched to the scenario and its outcome, and refuses a
`faithful` verdict over a failed case. Enforcement is a policy switch,
`experimental.evidence_binding` with `mode: off | warn | require`, a minimum
`strength` (`artifact` or `report`), and optional `case`/`lane`/`scope`
filters; `warn` only lists advisories after `canon gate check`. Nothing
changes for a repo that does not set it. The API and policy shape may change
while experimental.

## 0.10.0 release

Version 0.10.0 makes golden-path-only specs visible. A scenario can carry a
`@case:<value>` tag (`happy`, `failure`, `edge` in the base vocabulary;
`canon scenario new --case`), indexed onto `Scenario.case` by
`canon inventory sync`. The opt-in `spec_coverage.require_cases: [failure]`
then reports every feature surface (`<area>.<surface>`) whose in-scope
scenarios specify no failure path, and `canon subject status <id> shipped`
refuses on the same rule. Nothing changes until a repo sets
`require_cases`; when it does, untagged scenarios satisfy no case, so tag
existing failure-path scenarios first. `canon context` now also prints
`exclude_lanes` and `require_cases`.

## 0.9.1 fix

The `@journeykit/canon` launcher in 0.8.0–0.9.0 exited 0 regardless of the
native binary's result, so refusals such as a blocked `verifying → shipped`
or a red `canon gate check` looked successful to scripts, CI, and hooks.
0.9.1 passes the binary's exit code through again; a spawn failure or signal
death exits 1. Upgrade if anything relies on `canon`'s exit status.

## 0.9.0 release and migration

Version 0.9.0 makes the Gherkin `@subject:<id>` tag the only subject ↔
scenario link. `canon report`'s Subjects panel and the `verifying → shipped`
gate now count the scenarios whose latest `canon inventory sync` generation
carries the tag; the never-populated `Subject.scenario_ids` list is gone
(older records still read). Shipping is now refused when a subject owns no
tagged scenario, so tag each subject's scenarios and re-run
`canon inventory sync` before moving it to `shipped`.

## 0.8.0 release and migration

Version 0.8.0 consolidates provider projections into a single `canon` skill,
with stricter signed-approval and quarantine boundaries. Ingest is now
metadata-only: source bytes remain external while Canon records metadata and
provenance. Update integrations to the current parser generations before
migrating, and review existing approval/quarantine workflows for the stricter
requirements.

## Layout

```
crates/            Rust workspace (canon-model, canon-store, canon-ingest,
                   canon-gate, canon-learn, canon-report, canon-cli)
packages/          Bun workspace (cli launcher + prebuilt native binaries)
docs/              Design docs and specs
openspec/          Imported plan dialect (openspec change dirs) — scaffolded at plan time
```

## Live tiers: env contract

`crates/canon-store`'s hot (Postgres) and cold (S3-compatible) tiers
resolve their credentials from these env vars — never hardcoded, never
committed to `canon.yaml` (which only names WHICH var to read, e.g.
`tiers.hot.dsn_env: CANON_PG_DSN`, keeping the config file itself
commit-safe).

| Var | Purpose |
|---|---|
| `CANON_PG_DSN` | Full Postgres DSN URL (`postgres://user:pass@host:port/db`) for the hot tier. A URL, not split user/pass/host fields: one atomic secret to rotate, and the env-name indirection (`canon.yaml`'s `dsn_env` names this var, never the DSN itself) is what keeps `canon.yaml` safe to commit. |
| `CANON_R2_BUCKET` | Cold-tier bucket name (`canon.yaml`'s `tiers.cold.bucket_env` default). |
| `CANON_S3_ENDPOINT` | S3-compatible endpoint URL (MinIO, Cloudflare R2, real S3, …). |
| `CANON_S3_ACCESS_KEY` | S3-compatible access key. |
| `CANON_S3_SECRET_KEY` | S3-compatible secret key. |
| `CANON_S3_REGION` | S3 region; defaults to `us-east-1` in every build (a wrong region is not the silent-misdirection risk a defaulted endpoint/credential pair is). |

**Debug builds** default every `CANON_S3_*` var to the local
`docker-compose.yml` MinIO stack (`http://127.0.0.1:59000`,
`canon`/`canoncanon`) when unset — zero exported env vars needed for
local dev/CI. **Release builds** (`cargo build --release`) REQUIRE
`CANON_S3_ENDPOINT`, `CANON_S3_ACCESS_KEY`, and `CANON_S3_SECRET_KEY`
to be set explicitly (s29 `store-hardening` D1); a missing one fails
attachment loud, naming every unset var, rather than silently
attaching to the loopback dev stack.

`docker compose up -d --wait postgres minio` starts the local stack:
Postgres on `127.0.0.1:55432` (`canon`/`canon`, database `canon_v1`),
MinIO on `127.0.0.1:59000` (`canon`/`canoncanon`). See
`docker-compose.yml`'s own header comment for the full quick-start
(including `docker compose up minio-init` to create the bucket).
