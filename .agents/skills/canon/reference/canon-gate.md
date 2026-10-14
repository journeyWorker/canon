# canon-gate

`canon gate` is canon's trust spine: a two-layer evidence gate over a
repo's artifact corpus. It answers two DIFFERENT questions per artifact —
"does required evidence exist" (coverage) and "did the evidence pass, by
whom, how stale" (verdict-ledger) — never collapsing them into one
"done" boolean.

## The trust ladder

Each artifact climbs a trust ladder as evidence accrues: an unreviewed
record is weaker than a reviewed one, which is weaker than a ratified
one. `policy.yaml` sets a `trust_required` level per artifact class; the
release-scoped check (below) enforces it. A human-only `flagged` overlay
overrides the ladder — a flagged artifact is never green regardless of
evidence. (Author `policy.yaml`'s trust/staleness fields via `canon-policy`.)

## The ten failure classes

Every violation carries one of these stable, grep-able strings:

| Class | Meaning |
|---|---|
| `uncovered-cell` | Either a policy-required evidence cell (role × artifact) with no matching record, or — with `spec_coverage` enabled — a spec scenario that is unimplemented or mismatched, or a feature surface missing a `require_cases` case. The detail string distinguishes them. Coverage means "a test exists", not "a test passed": even a `Divergent` verdict satisfies the role-cell form. |
| `unreviewed-promotion` | An artifact tagged `reviewed` has no matching ledger review record — or, with `spec_coverage.require_review` set, an in-scope scenario has no qualifying `Review`: none at all, only self-reviews, only stale reviews (the spec or evidence it pinned changed since), or only `indeterminate`/`unavailable` ones. See `canon-review`. |
| `trust-below-required` | Achieved trust level is below `policy.yaml`'s `trust_required` for its class — RELEASE-scoped only (`canon gate check --release`); never fires on an ordinary run. |
| `stale-evidence` | A passing record degraded to stale: its declared surface changed since its `evidence_sha`, or HEAD moved past `staleness.max_commits_behind`. Only degrades an already-green record. Also: a file bound to a latest evidence record whose bytes are gone — no intact blob in `.canon/artifacts/sha256/` and no working-tree copy with the recorded digest — or a file bound to an in-effect review with no intact blob (see Bound evidence stays provable below). |
| `malformed-evidence` | A candidate record doesn't parse, is misfiled, or carries an unparseable interim tag. Malformed evidence is no evidence. |
| `flagged` | The human-only `flagged` overlay is set — never green regardless of passing evidence. |
| `unevidenced-flip` | `canon gate task <task_id>` was asked to flip a checkbox with no matching, non-`Divergent` evidence record. |
| `fabricated-evidence` | An evidence note contains a blocklisted marker (`"would pass"`, `"TBD"`, `"n/a"`) standing as a whole word — never inside a path, a file name, a URL or a longer word, so `pack.json/atlas` is not `n/a` — or a bare `verified` claim with no attached command result. `canon evidence add` refuses such a note before writing (`canon evidence add: refused — fabricated-evidence …`, exit 1). |
| `open-blocker` | With `spec_coverage.require_review` set (and `block_on_findings`, the default), a Finding whose latest version is severity `blocker`, disposition `open`, on a change an in-scope subject adopted. Subject string `<change_id>#<round>.<seq>`. See `canon-review`. |
| `false-success` | The latest audit record (`canon evidence add --kind audit`) of a scenario says `refuted`: an external evaluator found a counterexample to the claimed success. Always on, regardless of policy or sampling, and takes precedence over any declared faithful verdict; only a newer audit (`confirmed` or `indeterminate`) clears it. The detail names the evaluator, its version, its sha256 and the counterexample paths. |

## `canon gate check [--repo <dir>] [--release]`

Assembles coverage + ledger + staleness + the always-on trust-ladder
check + the bound-artifact check, plus the opt-in spec-coverage check, over the resolved repo's
corpus and runs them, printing every violation grouped by failure class.
`--release` additionally engages the release-scoped
`trust-below-required` check; the trust-ladder check is never dropped
when `--release` is passed.

```bash
canon gate check --repo .            # ordinary evaluation
canon gate check --repo . --release  # + trust-below-required
```

Exit `0` clean, `1` gate-red (any violation), `2` usage/load failure
(unreadable ledger, corrupt `canon.yaml`). `--repo` (or its omission)
resolves through the nearest-ancestor `canon.yaml` walk, so it reads the
repo ROOT's `.canon/policy.yaml` and `.canon/ledger` from any subdirectory.

When `.canon/policy.yaml` exists but has no approval, or its bytes no
longer match the approved digest, the gate prints `advisory: policy not
approved: …` or `advisory: policy changed since approval: …` after its
result, naming `canon policy approve --by <who>`; the exit code is
unchanged (see `canon-policy`).

Before it reads the scenario index, the gate rebuilds any stale part of
it from the changed `.feature` files (the same code `canon inventory
sync` runs) and says so in one stderr line, silent when the index is
fresh:

```
canon gate check: refreshed the scenario index from 1 changed feature file(s) (.canon/specs/kind=feature/area=cart/add.feature): 1 record(s) written
```

A root that fails to sync (format violations, a duplicate id) is named
instead, ending ``NOT refreshed: root `x` (N format violation(s), 0 sync
error(s)) — `canon inventory sync` lists them``, and the gate fails
closed: it adds `uncovered-cell <root> — the scenario index for spec root
`<root>` is stale and could not be refreshed (…); `canon inventory sync`
lists the problems to fix` and exits 1, since the index it would grade
does not hold the edits. The `verifying → shipped` guard refuses with
the same violation, and `canon status` lists the sync as its only step.
Fix what the sync lists and run the gate again. See `canon-storage`
(The scenario index).

A spec root still on the pre-0.17 layout (a `.feature` file under
`<root>/features/`) fails closed the same way, whether or not the index
already holds its scenarios: the gate adds `uncovered-cell <root> — spec
root `<root>` (`<path>`) is on the pre-0.17 layout (…)…; until then its
scenarios are not graded` and exits 1, the ship guard refuses with the
same violation, and `canon status` names `canon migrate` as its only
step. `canon migrate` moves `features/kind=feature/` to
`<root>/kind=feature/`; run it, then the gate again.

### The spec-derived worklist (`spec_coverage`, opt-in)

Coverage groups the EVIDENCE corpus, so it can only ever enumerate
artifacts that already have evidence. A `.feature` scenario nobody has
attested to appears in no group and is invisible to it. `spec_coverage`
adds the other direction: it starts from the Scenario corpus
`canon inventory sync` materializes and left-joins evidence onto it, so
an unimplemented or mismatched spec becomes a reported violation.

```yaml
# .canon/policy.yaml — absent means zero violations; `canon init` writes a starter
spec_coverage:
  require_evidence: true
  scope: [building, verifying]   # optional; omit for the whole corpus
  exclude_lanes: [process]       # optional
  require_cases: [failure]       # optional; see Golden-path only below
  require_review: {}             # optional; see Independent review below
```

`canon init` writes a starter `.canon/policy.yaml` with this section on
(`require_evidence: true`, `require_cases: [failure]`,
`require_review: {}`; see `canon-policy`). When the section is absent and
the indexed corpus has scenarios, the gate says so after its result, on
stdout like its other advisories, and the exit code is unchanged:

```
advisory: spec_coverage is off — 12 scenario(s) are not checked for evidence; see .canon/policy.yaml
```

A present section, even `require_evidence: false` or a malformed one
(already a violation), and an empty corpus print no advisory.

- **Unimplemented** — no `EvidenceRecord` carries the scenario's
  `(project_id, scenario_id)`. Author one with
  `canon evidence add --scenario-id <id> --project-id <root-id>`; both
  flags are required together, because two spec roots may carry the same
  scenario id. `--summary` is allowed here too (scanned for fabrication
  markers like a task's); it is a row suffix only when the record also
  names a `--task`. A record carrying both `--task` and `--scenario-id`
  is one attestation answering both joins: it counts for the task cell
  and for the scenario cell (verdicts, the shipped gate, the review gate).
- **Mismatched** — the folded divergence state is `open`,
  `still-divergent`, or `resolved-invalid` (a resolution whose app sha
  has moved), or the latest ledger verdict is `divergent`.
- **`scope`** narrows blocking to scenarios whose `@subject:<id>` tag
  links them to a Subject in one of the named statuses. With a `scope`
  set, an untagged scenario is out of scope; with `scope` omitted, every
  scenario is in scope. A tag naming no Subject record is reported as a
  dangling link, never silently skipped.
- **Golden-path only** (`require_cases`) — every feature surface
  (`<area>.<surface>`) among the in-scope scenarios must carry at least
  one scenario tagged `@case:<v>` for each listed value, or the SURFACE
  is reported (subject `<area>.<surface>`, detail naming the missing
  case). This is the gap evidence presence cannot see: every scenario
  attested, none of them specifying a refusal. Untagged scenarios satisfy
  no case, so an unclassified corpus is reported, not passed. Works with
  `require_evidence: false` too. `canon subject status <id> shipped`
  applies the same rule to the scenarios the subject owns.
- **Independent review** (`require_review`) — scenarios whose subject is
  in `require_review.scope` (default `[verifying, shipped]`; `[]` = every
  scenario; independent of `scope` above) need a `Review` record
  (`canon review add`) whose latest version per reviewer is `signed`,
  still matches the scenario digest and latest evidence it pinned
  (a review written before 0.15 has no pins and skips this check), and,
  when `distinct_actor` (default `true`), is by a reviewer and actor
  other than the scenario's evidence actor and from a session other than
  any evidence session when both carry one (`--session-id`);
  gaps are `unreviewed-promotion`, and the detail says whether the
  review is missing, a self-review, stale (naming whether the spec, the
  evidence or both changed), or not signed. With `block_on_findings`
  (default `true`), an open `blocker` Finding on a change an in-scope
  subject adopted is `open-blocker`. `exclude_lanes` also exempts
  scenarios from review. Full contract: `canon-review`.

  ```yaml
  spec_coverage:
    require_evidence: true
    require_review:                 # absent = nothing changes
      scope: [verifying, shipped]   # default
      distinct_actor: true          # default
      block_on_findings: true       # default
  ```

- **Sampled audits** (`require_audit`) — a deterministic sample of
  in-scope scenarios (`require_audit.scope`, default `[verifying,
  shipped]`; `exclude_lanes` applies) must carry an external audit
  (`canon evidence add --kind audit`, see Audit records below). A sampled
  scenario whose latest audit is missing or `indeterminate` is
  `uncovered-cell`:

  ```
  uncovered-cell cart.add.01 — sampled for audit by spec_coverage.require_audit (rate 0.1, seed `s1`) and has no audit record — …
  uncovered-cell cart.add.01 — sampled for audit by spec_coverage.require_audit (rate 0.1, seed `s1`), but its latest audit is indeterminate (evaluator `judge` 1.2) — …
  ```

  Keys, defaults and the exact sampling formula: `canon-policy`.

Two refusals rather than a silent pass, both surfacing as
`uncovered-cell` on the subject `spec_coverage`:

- A **malformed section** (unknown key, unknown `scope` status, or a
  malformed `require_cases`/`require_review`/`require_audit` — unknown
  key, bad status, `null`, non-mapping, a missing or out-of-range
  `rate`) refuses at check time (`uncovered-cell spec_coverage — ...
  unusable`). It is never treated as
  absent — a typo must not be silently equivalent to not opting in.
- A **corpus kind routed off the gate's rung** (`scenario`,
  `divergence`, or `subject` sent anywhere but `local`) refuses, because
  the gate reads one tier and would otherwise pass by seeing nothing.

Malformed rows in those three kinds do NOT surface as
`malformed-evidence`; they are kept off the evidence violation set on
purpose, so enabling nothing changes no existing verdict.

### Audit records (`--kind audit`)

canon never generates or runs an audit. An external evaluator does, and
canon records its result:

```bash
canon evidence add --kind audit --project-id root --scenario-id cart.add.01 \
  --ref "judge run 42" --role reviewer \
  --evaluator-id judge --evaluator-version 1.2 \
  --evaluator-digest <64 lowercase hex sha256> \
  --audit-result refuted --report junit:audit/counterexample.xml
```

- `--kind audit` requires `--evaluator-id`, `--evaluator-version`,
  `--evaluator-digest` (sha256, 64 lowercase hex), `--audit-result
  confirmed|refuted|indeterminate`, and `--scenario-id` with
  `--project-id`. It refuses `--task`, `--verdict`, `--surface-ref` and
  `--approval-*`; the four evaluator flags are refused on any other kind.
  All refusals exit `2`.
- The record carries `audit: {evaluator_id, evaluator_version,
  evaluator_digest, result}`; its verdict is derived (`confirmed` →
  faithful, `refuted` → divergent, `indeterminate` → not-applicable).
- Optional `--report`/`--artifact` counterexamples are bound and stored
  like any report; the latest audit's bindings are re-hashed by the
  artifact check.
- Audits fold apart: per `(project_id, scenario_id)` there is a latest
  declared record and a latest audit. An audit never becomes the
  scenario's latest declared verdict.
- `canon subject status <id> shipped` refuses `false-success`, and an
  owed sampled audit when `require_audit.scope` covers `shipped`.
- `canon status` shows an `audits` section (sampled / confirmed / owed /
  refuted) and suggests `canon evidence add --kind audit …` in `next:`.

### Dropping a binding (`--drop-attachment`)

`canon evidence add` for a scenario refuses (exit `2`) when the
scenario's current latest record of the same fold (declared records, or
audit records for an audit) — committed ledger plus `_staging` — binds
files the new record does not. The refusal names the paths and two ways
out: re-bind them (`--artifact`/`--report`; new bytes at the same path
are fine), or drop them on purpose with `--drop-attachment <path>`
(repeatable) and one single-line `--drop-reason <text>`. The drop is
recorded as `attachment_drop: {paths, reason}`. Refused (`2`): a drop
without a reason, a reason without a drop, a drop path that is not
actually dropped, a multi-line reason, or a drop without
`--scenario-id` (task-only evidence has no drop guard). `canon status`
lists recorded drops (`dropped bindings (N):`; `drops[]` in `--json`).

### Nothing is attested before the brief is approved (`--override-reason`)

The approval is never assumed: the agent writes the brief, stops and
reports it, and builds nothing for that scope until the human answers.
`canon evidence add` is the backstop for that stop (0.16 M7). It
refuses (exit `1`, nothing staged or stored) while a subject the record
attests for has an enforced, unapproved brief: the newest adopted
change's canon `.canon/changes/<change>/brief.md` has no `Approved:`
line, was edited after its approval (`BRIEF_CHANGED_SINCE_APPROVAL`), or
there is no canon `brief.md` yet but the change comes from an
external openspec plan with a `proposal.md` under the plan sources (the
check `canon status` and the ship guard use; see
`canon-subject`). The subjects are the scenario's `@subject` owner for
`--scenario-id`, and every subject adopting the task's change for
`--task`:

```
canon evidence add: unreviewed-promotion demo-subject — evidence for world.demo.01: brief not approved — .canon/changes/c-demo/brief.md has no `Approved:` line, so the human has not answered it; once they do, record their words with `canon brief approve --change c-demo --by <the human> --note "<their words>"`
canon evidence add: refused — stop, report the brief to the human and wait for their answer: nothing is built or attested for a subject before its brief is approved. Record their answer with `canon brief approve --by <the human> --note "<their words>"` only from that answer; to attest anyway, pass --override-reason <text>, which is recorded on the evidence record
```

For a change from an external plan with no canon brief yet, the first
line says instead ``.canon/changes/c-ext/brief.md does not exist yet:
change `c-ext` comes from the external plan
openspec/changes/c-ext/proposal.md, which canon reads and never edits, so
its approval lives in that canon file, so the human has not answered it;
once they do, create .canon/changes/c-ext/brief.md holding a `Source:
openspec/changes/c-ext/proposal.md` line, then record their words with
`canon brief approve --change c-ext --by <the human> --note "<their words>"` ``. An `Approved:` line in the
external `proposal.md` itself is never read.

A subject with no brief to answer (no adopted change, or a newest change
with neither a canon `brief.md` nor an external `proposal.md`) and a
scenario no subject owns are unaffected.
`canon review add` and `canon finding add` are never refused.
`--override-reason "<one line>"` attests anyway, prints the `waived …`
lines and `override recorded by `<actor>` for the N brief refusal(s)
above: <reason>`, and records on the evidence record:

```yaml
brief_override:
  reason: the human approved in chat; the line follows
  waived:
    - { class: unreviewed-promotion, subject: demo-subject }
  actor: { agent_id: lead }   # from --actor-id
```

A blank or multi-line reason exits 2; an override when nothing refused is
not recorded. `canon status` lists each such record as a `warning:`, and
`canon gate check` prints them after the result without failing:

```
brief overrides: 1 evidence record(s) attested before their brief was approved — not failing the gate:
  evidence for world.demo.01 was attested by `lead` at 2026-10-12T07:00:00Z while the brief of demo-subject was not approved (override: the human approved in chat; the line follows)
```

### Review-waiver advisories

A subject moved into a review-scoped status with `canon subject status
--override-reason` (see `canon-subject`) keeps the `unreviewed-promotion`
and `open-blocker` gaps its waiver recorded visible without failing the
gate. Only those exact `(class, subject)` pairs are waived; a gap that
appears later is a violation. The waived ones print after the
violations:

```
review waivers: 1 advisory(ies) — not failing the gate:
  waived unreviewed-promotion world.demo.01 — no review record ... [waiver: subject `demo-subject` moved to verifying by `lead`: reviewer out until Monday]
```

The waiver lasts only until the subject's next status write.

`canon subject status <id> shipped` also refuses where a brief exists and
is not approved (the subject's newest adopted change has a canon
`brief.md` with no `Approved:` line or one the brief's text no longer
matches, or an external `proposal.md` and no canon `brief.md` yet;
see `canon-subject`), as an `unreviewed-promotion` naming
the subject, and while `canon brief check` has structure blockers or a
flow step has no scenario or no failure scenario (the coverage rules gate
shipping, never approval). With no brief to answer it prints an advisory
and does not refuse. Like the evidence refusal above, it is a backstop:
the stop is at the brief. `canon gate check` does not grade briefs, so a
waived ship brief prints no advisory here; `canon status` keeps showing
the brief as not approved.

### Reviews that predate pinning (checked against their pin)

With `require_review` on, the reviews in effect (latest per reviewer and
scenario) that carry no pins were written before 0.15. Each still names
`pin`, the commit it reviewed, and the gate checks it there (0.16):

- **current** — the scenario's digest at that commit (read with `git`,
  normalized exactly as `canon inventory sync` computes
  `scenario_digest`) equals its current digest, and no declared evidence
  record of the scenario is newer than the review. It counts. When the
  scenario lived in another `.feature` file at that commit, the spec
  root's files at that commit are searched for its id, then the files
  under each of the root's `former_roots` (where `canon migrate` moved
  it from, e.g. `specs` for a root now at `.canon/specs`). Only the
  root's own directories are searched, never another root's.
- **stale** — the scenario changed since the commit, or it has newer
  evidence. It no longer counts, and an in-scope scenario left with no
  other qualifying review is `unreviewed-promotion`:

  ```
  unreviewed-promotion gate.check.15 — stale review by `canon-reviewer-g1` (written before review pinning; checked against commit b556ab7529a332aa0b28d0a0f98bdd62fcbb9a63): it has declared evidence newer than the review; a review counts only while what it saw is current — review it again (`canon review add`)
  ```

  A spec change reads `its spec changed since <pin> (the scenario's
  digest at that commit is not its current one)`.
- **undecidable** — the pin is not a commit in this clone (a shallow
  clone, rewritten history), or no feature file of the scenario's root
  holds its id at that commit. It keeps counting, as in 0.15: a missing
  commit never becomes a violation.

One advisory line after the violations counts the three. A stale one
whose scenario already has another qualifying review (current, signed,
distinct) is counted as superseded, since nothing is owed for it; only
the scenarios a stale one leaves without a qualifying review are named
for re-review. The exit code is unchanged by the line itself:

```
advisory: 338 review(s) predate review pinning (0.15) and were checked against the commit each one pinned: 318 current, 12 stale, 8 undecidable; 7 stale one(s) superseded by a current review of the same scenario, so nothing is owed for them; a stale one with no current review in its place no longer counts — review its scenario again (`canon review add`): platformer.movement.01, platformer.movement.02, …; an undecidable one still counts unchecked, because its pinned commit or the scenario at it is not in this clone — check out full history (`fetch-depth: 0` in CI) to check it
```

A review of a scenario no feature file declares any more is not counted.
In CI, check out full history (`actions/checkout` with `fetch-depth: 0`)
for the job that runs `canon gate check`, or every legacy review is
undecidable. Each pinned commit is read once per run through one `git
cat-file --batch` process, and never fetched over the network.

## Experimental: evidence binding (`experimental.evidence_binding`, off by default)

An evidence record is an attestation: canon never runs `--ref`, and it
never will — it cannot run every language and every agent-driven QA
tool a team uses. Binding lets a record point at files the team's own
runner or agent ALREADY produced, so a reviewer (and the gate) can check
what backed the claim:

```bash
# any file: a Playwright trace, screenshots, an agent QA log — by sha256
canon evidence add --scenario-id cart.add.04 --project-id root \
  --kind agent-qa --ref "stagehand run 7" --role implementer \
  --artifact test-results/cart-trace.zip
# a JUnit XML or Cucumber JSON report: also records the matched case + outcome
canon evidence add ... --report junit:target/nextest/default/junit.xml
canon evidence add ... --report cucumber:reports/cucumber.json
# a Swift Testing event stream (swift test --experimental-event-stream-output <path>)
canon evidence add ... --report swift-testing:.build/events.jsonl
```

A case is bound when the scenario id is its OWN: the first
scenario-id-shaped token (`<area>.<surface>.<nn>`) in its full name —
classname (a describe path) first, then name — dotted (`cart.add.04`) or
underscored (`cart_add_04`, the form a test function carries), equals
the scenario id; a Cucumber tag equal to it binds too. Start each test
name with its scenario id: `game.session.04: restarting as in
game.session.01` binds to `game.session.04` only, never to the id it
mentions. Each bound case is its own attachment with its name and
outcome. `--report-case <name>` (repeatable; a Rust test by its
function name) ADDS cases; it never hides the scenario's own. No match,
or a `--report-case` naming no case, refuses (exit 2); a `faithful`
verdict when ANY bound case failed or was skipped refuses (exit 1). Files
must be inside the repository, are recorded by relative path, and are
stored (see Bound evidence stays provable).

A Swift Testing case is named by its display name (`@Test("cart.add.04
refuses an out-of-stock item")`), else its function name
(`cart_add_04_refuses()`). A test with a failing issue failed, a skipped
test was skipped, a test that started and ended without one passed, and
a test that started and never ended failed.

Strength, weakest first: `attested` (no attachment) < `artifact` (any
bound file) < `report` (a parsed report whose case passed). The policy
decides whether the gate cares:

```yaml
experimental:
  evidence_binding:
    mode: warn          # off (default) | warn | require
    strength: artifact  # artifact (default) | report
    case: [failure]     # optional filters — only these scenarios are held
    lane: [behavior]
    scope: [verifying]  # subject statuses
```

`off` checks nothing. `warn` prints the distribution and lists each
scenario below `strength` as `warn <id>` after the gate result, never
failing it. `require` reports each as `uncovered-cell`. The latest
evidence record per scenario decides; scenarios with no evidence are
`spec_coverage`'s finding, not this one's. A malformed section is a
violation in every mode. Canon still cannot know whether the bound test
actually exercises the scenario — binding narrows the trust gap, it does
not close it.

### One report, many scenarios (0.18)

One test run proves many scenarios. Bind it once:

```bash
canon evidence add --project-id root --all-scenarios \
  --report junit:target/nextest/default/junit.xml \
  --kind test-run --ref "cargo nextest run" --role implementer
```

Each indexed scenario of `--project-id` that owns a case (the rule above)
gets its own record, with only its own cases. Its verdict comes from
them: `faithful` when every case passed, `divergent` when one failed,
`not-applicable` when one was skipped and none failed. `--verdict <v>`
claims the same verdict for every record instead. A case whose id is not
indexed is named and skipped (`skipped <id> — not an indexed scenario`);
a report no indexed scenario owns is refused (exit 2). The records share
the stored report and every `--artifact`, and the output lists each
scenario with its verdict on its own line.

It is all or nothing: a claimed `faithful` verdict over a failed case,
a fabrication marker in `--summary`, an unapproved brief for any
scenario's subject, or a scenario whose latest record binds a file the
batch does not refuses the whole batch, and nothing is written.
`--scenario-id`, `--task`, `--report-case`, `--kind audit`,
`--drop-attachment` and the approval flags name one subject and are
refused with `--all-scenarios`.

A task's evidence with `--report` and no `--scenario-id` binds EVERY case
in the report, so a `faithful` verdict needs all of them to pass.

### A write that makes a review stale says so (0.18)

When new evidence (one record or a batch) supersedes the evidence a
current pinned review saw, `canon evidence add` prints one line per
review it makes stale, with the command that re-pins it:

```
canon evidence add: review by `reviewer-2` of cart.add.01 is now stale (it pinned the evidence this record supersedes); re-pin it: canon review add --project-id root --scenario-id cart.add.01 --reviewer reviewer-2 --actor-id reviewer-2 --role reviewer --pin <HEAD sha> --original-spec-ref specs/kind=feature/area=cart/add.feature
```

The decision is the gate's own (the same stale-review rule `canon gate
check` reports). The command repeats the review's reviewer, actor, role,
session, provenance ref, runtime labels and artifacts, pinned at `HEAD` (`<sha>`
when the repository has no commit). A record that is only staged says
the review "goes stale once this evidence is promoted": promote first,
then re-pin. The reviewer re-reviews before re-pinning; several
scenarios re-pin in one `canon review add` (see `canon-review`).

## Bound evidence stays provable (the artifact store)

`canon evidence add --artifact/--report` copies each bound file into
`.canon/artifacts/sha256/<hex>`, named by its sha256. Writes go through a
temp file and a rename, and a digest already stored intact is not written
again. The directory is tracked in git (`canon init` never ignores it):
commit it with the ledger. A file larger than 25 MiB is refused (exit 2);
raise the limit with `--max-artifact-mib <N>`.

`canon gate check` re-checks every file bound to a LATEST evidence record
(the latest per task and per `(project_id, scenario_id)`):

- the stored blob exists and hashes to the recorded digest → clean, even
  after the working-tree file is rewritten (a smoke script that rewrites
  `reports/smoke.json` on every run no longer orphans the record);
- no blob, but the working-tree path still hashes to the recorded digest
  → clean, with an advisory naming `canon evidence vault` (records from
  before the store existed):

  ```
  evidence artifacts: 1 bound file(s) not in the artifact store — not failing the gate; run `canon evidence vault` to store them before the working tree changes:
    unstored cart.add.01 reports/junit.xml (sha256 b7cb…) is verified against the working tree only
  ```
- anything else → `stale-evidence` naming the path, the recorded digest,
  and what the store and the working tree hold now (`missing` or a
  digest), ending with why: a stored blob that no longer hashes to its
  name "does not match its name, so its bytes were altered"; a missing
  one means "the bound bytes are gone". Re-run the test and re-attest
  with `canon evidence add`.

The check applies to every latest record, not only subjects in
`spec_coverage.scope`: a binding is a claim about bytes whatever the
subject's status, and a repo with no `spec_coverage` would otherwise never
have its bindings checked. A superseded record is not checked —
re-attesting is the fix.

Files bound to a review (`canon review add --artifact`, see
`canon-review`) are checked by the same rule for every CURRENT in-effect
review: the latest per `(project_id, scenario_id, reviewer)`, less any
that went stale against its scenario (its spec or evidence pin moved, or
a review from before pinning is stale at its commit). A stale review is
superseded — the gate reads the newer review, or reports the missing
one as `unreviewed-promotion` — so its files are not re-checked, as a
superseded evidence record's are not. A missing blob whose working-tree
file still matches is an advisory, listed with the reviewer:

```
  unstored cart.add.01 reports/cart-badge.png (sha256 9848…, review by `reviewer-2`) is verified against the working tree only
```

Anything else is `stale-evidence` on the review's scenario, and the
detail names the review: ``review by `<reviewer>` (actor `<id>`,
<time>)``.

`canon evidence vault [--max-artifact-mib <N>]` stores the bytes for
existing evidence records and current reviews (a superseded review's files are not vaulted): every attachment whose
working-tree file still matches its digest is copied in, and everything
else is listed as not stored, with both digests. It exits `0` whenever it
ran (the gate decides whether an unstored file matters), `2` when the
ledger cannot be read or a write fails.

## Effect-aware risk approvals (`risk_tiers`)

`risk_tiers` is absent by default and remains a no-op when absent. When it
is present, `canon gate check` selects the highest-ranked configured tier
whose current evidence binding matches a path or exact `effect:<kebab-slug>`.
The current generation is selected deterministically; approvals from an
older `evidence_sha`, a non-`faithful` verdict, or an unverified attestation
never count. A required tier also fails closed when the current record has
no usable `surface_ref`/effect binding, so a caller cannot suppress a
configured rule by declaring a safe path.

`canon evidence add --surface-ref src/auth/login.rs` (repeatable) or
`--surface-ref effect:secret-access` persists an explicitly validated
binding. This is still attestation input, not proof of the changed paths;
run/diff metadata is authoritative when available, and an unavailable
binding is a gate violation rather than a clean result.

Approval identity is separate from attestation text. An approval counts
only when it carries a detached SSH signature (`ssh-keygen -Y sign -n
canon-approval-v1` over `canon evidence approval-payload`'s bytes) that
verifies against the signers pinned by policy `approval.allowed_signers`,
with `--approval-role human`. `CANON_ACTOR`, caller-supplied strings, and
`agent` roles never satisfy a risk tier, and canon never signs on a
user's behalf.

Malformed `risk_tiers` sections produce a stable `uncovered-cell` policy
violation (`risk_tiers policy-invalid: ...`) instead of degrading to an
empty clean map.

## `canon gate task <task_id> [--repo <dir>]`

For imported external plans, and for a `tasks.md` that already exists in
a canon change. `canon change new` writes no `tasks.md` (0.19), and
nothing gates on one: a canon-owned change is covered by its flow steps'
scenarios (`canon status`).

The evidence-gated task checkbox flip. Resolves `<task_id>`
(`<change_id>#<n>`) through `.canon/changes/` (canon's own changes, read
first whenever the directory exists, with no `plans:` entry) and then
`canon.yaml`'s `plans.sources` — the first dialect whose `PlanWriteBack`
locates the task's document wins, so the flip is plan-dialect agnostic
(canon's own `tasks.md`, external `openspec` change dirs, `superpowers`
plan docs). A repo with no `plans:` section falls back to the compat
default `[{ dialect: openspec, root: <repo> }]`; no source locating the
task at all is a loud failure naming every source consulted. A repo with
no `.canon/changes/` and `sources: []` is refused: "this repo configures
no plan sources (no `.canon/changes` and no `plans.sources` entries in
canon.yaml) … write the change with `canon change new`, or list your
external plans under `plans.sources`" (`canon dispatch --task` says the
same).
It requires
a matching non-`Divergent` evidence record, and flips `- [ ]` → `- [x]`
with an appended evidence note ONLY on a clean check. The note
aggregates every record bound to the task, on one line:
`— ✅ 3 evidence records (2 faithful, 1 not-applicable); latest: <summary>`
(the latest `--summary`, or a default naming the latest passing record).
Every other path — missing evidence, a `Divergent` verdict, a fabricated
note on any of the task's records — leaves the row byte-unchanged and
exits `1` with the blocking violation on stderr. After a flip, the plan
source is re-ingested so the record store's task status agrees with the
checkbox (`canon query --kind task` reads `done`); a failed re-ingest is
a stderr warning naming `canon ingest plans`, never a failed flip.
The success line is `canon gate task: <task_id> flipped — written
directly; nothing to promote`. An already-`[x]` row is an idempotent no-op (exit `0`). An unknown
`task_id` is reported (exit `1`).

A flip on evidence that cannot prove the task still succeeds, but warns
on stderr: when none of the task's evidence records binds a file and one
of them cites a canon command as its `--ref` (`canon gate check`,
`canon status`), the warning says to bind the artifact or report that
proves the task (`canon evidence add --task <id> --artifact <path>` or
`--report <format>:<path>`). A canon command reports on canon's own
records, so it is never the proof of a task.

```bash
canon gate task my-change#5.2 --repo .
```

This is canon's own authority for the checkbox grammar — always flip
through `canon gate task`, never by hand.

Canon only reads external plans, with this one exception: for a task
from an external plan source, `canon gate task` flips the checkbox in
that plan's own `tasks.md` (or plan document). It is the one place canon
writes into an external plan; their briefs and approvals still live in
`.canon/changes/<change>/brief.md`. (Open for the maintainer: record
external flips only in the ledger instead.)

## `canon gate promote [--repo <dir>] [--dry-run]`

Staging → committed: every well-formed record under
`.canon/ledger/_staging/` is re-validated with the SAME checks the gate
applies, assigned a monotonic per-(role, surface) `run_seq` (gap-free
within one invocation, continuing from the committed max), and moved into
the committed ledger. A malformed or unpartitionable candidate is refused
— exit `1`, no `run_seq` consumed, the file left in place. `--dry-run`
prints the plan (target path + assigned `run_seq` per candidate) without
writing or deleting.

```bash
canon gate promote --repo .            # land every staged candidate
canon gate promote --repo . --dry-run  # preview only
```

Which writes need this step: `evidence add`, `finding add` and `finding
close` stage, and under `write: { promote: manual }` (or an absent
`write:` key) their success line ends ``— run `canon gate promote` to
commit it``. Under `write: { promote: auto }` (what `canon init` writes)
or with `--promote`, they promote the one record they just staged —
same validation and `run_seq` assignment as this command — and print
one success line naming the committed record (`wrote kind=…/….json`), not
the staged file, ending `— staged and promoted in this run; nothing to
promote`, so no separate
`canon gate promote` is needed for it; `--no-promote` keeps one write
staged. A record another run staged is not touched: only `gate promote`
commits everything staged. It stays safe to run anyway: with nothing
staged it prints
`nothing staged` and exits `0`. `divergence stage` stages for `canon
divergence promote` and names it, under the same `write:` rule. Every
other write — `review add`, `divergence
resolve|defer`, `subject new|adopt|status`, `change new`, `scenario new`, `feature new`,
`gate task` (it edits the plan document and re-ingests it)
— is final when it returns, and its success line ends `— written
directly; nothing to promote`. Full promote-on-write contract:
`canon-storage` (Staged writes and promotion).

## `canon gate install-hooks [--repo] [--event] [--matcher] [--command] [--timeout]`

Idempotent, diff-only hook-seam installation: merges one
`{matcher?, hooks: [{type: "command", command, timeout}]}` entry into
BOTH `<repo>/.claude/settings.json` and `<repo>/.codex/hooks.json` —
additive only, never touching an existing entry in the same matcher
group. Running it twice with no manual edits between reports "no diff"
and writes nothing. When neither file already carries a `canon
gate`-invoking command, it ALSO emits
`<repo>/.canon/scripts/canon-gate-pre-commit.sh` (advisory by default — set
`CANON_GATE_ADVISORY=0` to make it block the commit on a failing gate).

```bash
canon gate install-hooks --repo .
# non-default wiring:
canon gate install-hooks --repo . --event PreToolUse --matcher Edit --command "canon gate check" --timeout 30
```

Prefer this over hand-editing `settings.json`/`hooks.json`.

## `canon gate selftest`

Runs the shipped fixture corpus — one fixture per failure class (ten,
including `false-success`), each a
deliberately broken corpus proving that class fires and ONLY that class
(both under-detection and over-triggering fail the run). Takes no
`--repo`; self-contained. Run it before trusting any other `canon gate
check` run's green.

```bash
canon gate selftest
```

## Reading a violation

Every printed line is `<failure-class> <subject> — <detail>`. `<subject>`
is the artifact's own join identity (`task_id` preferred, then
`scenario_id`, then `run_id`) — grep the ledger or `tasks.md` for it
directly, never guess.