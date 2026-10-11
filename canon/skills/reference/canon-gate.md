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
| `fabricated-evidence` | An evidence note contains a blocklisted marker (`"would pass"`, `"TBD"`, `"n/a"`) or a bare `verified` claim with no attached command result. |
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
canon gate check: refreshed the scenario index from 1 changed feature file(s) (specs/features/kind=feature/area=cart/add.feature): 1 record(s) written
```

A root that fails to sync (format violations, a duplicate id) is named
instead, ending ``NOT refreshed: root `x` (N format violation(s), 0 sync
error(s)) — `canon inventory sync` lists them``, and the gate still runs.
See `canon-storage` (The scenario index).

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

### Reviews that predate pinning

With `require_review` on, the reviews in effect (latest per reviewer and
scenario) that carry no pins were written before 0.15. They keep
counting, and one advisory line after the violations counts them; the
exit code is unchanged:

```
advisory: 340 review(s) predate review pinning (0.15) and still count without a check that their scenario's spec and evidence are unchanged; a new `canon review add` pins what it saw
```

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
`canon-review`) are checked the same way for every in-effect review (the
latest per `(project_id, scenario_id, reviewer)`), with one difference:
they were stored when the review was written, so only an intact blob is
clean. A missing or altered blob is `stale-evidence` on the review's
scenario, and the detail names the review: ``review by `<reviewer>`
(actor `<id>`, <time>)``. The working-tree file is reported but never
vouches for it.

`canon evidence vault [--max-artifact-mib <N>]` stores the bytes for
existing records: every attachment whose working-tree file still matches
its digest is copied in, and everything else is listed as not stored,
with both digests. It exits `0` whenever it ran (the gate decides whether
an unstored file matters), `2` when the ledger cannot be read or a write
fails.

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

The evidence-gated task checkbox flip. Resolves `<task_id>`
(`<change_id>#<n>`) through `canon.yaml`'s `plans:` sources — the first
configured dialect whose `PlanWriteBack` locates the task's document
wins, so the flip is plan-dialect agnostic (`openspec` change dirs,
`superpowers` plan docs). A repo with no `plans:` section falls back to
the compat default `[{ dialect: openspec, root: <repo> }]`; no source
locating the task at all is a loud failure naming every source consulted.
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
same validation and `run_seq` assignment as this command — and end
`— staged and promoted in this run; nothing to promote`, so no separate
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