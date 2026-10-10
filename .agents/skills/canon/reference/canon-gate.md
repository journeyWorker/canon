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

## The nine failure classes

Every violation carries one of these stable, grep-able strings:

| Class | Meaning |
|---|---|
| `uncovered-cell` | Either a policy-required evidence cell (role × artifact) with no matching record, or — with `spec_coverage` enabled — a spec scenario that is unimplemented or mismatched, or a feature surface missing a `require_cases` case. The detail string distinguishes them. Coverage means "a test exists", not "a test passed": even a `Divergent` verdict satisfies the role-cell form. |
| `unreviewed-promotion` | An artifact tagged `reviewed` has no matching ledger review record — or, with `spec_coverage.require_review` set, an in-scope scenario has no qualifying `Review` (none at all, or only reviews by its own evidence actor). See `canon-review`. |
| `trust-below-required` | Achieved trust level is below `policy.yaml`'s `trust_required` for its class — RELEASE-scoped only (`canon gate check --release`); never fires on an ordinary run. |
| `stale-evidence` | A passing record degraded to stale: its declared surface changed since its `evidence_sha`, or HEAD moved past `staleness.max_commits_behind`. Only degrades an already-green record. Also: a file bound to a latest evidence record whose bytes are gone — no intact blob in `.canon/artifacts/sha256/` and no working-tree copy with the recorded digest (see Bound evidence stays provable below). |
| `malformed-evidence` | A candidate record doesn't parse, is misfiled, or carries an unparseable interim tag. Malformed evidence is no evidence. |
| `flagged` | The human-only `flagged` overlay is set — never green regardless of passing evidence. |
| `unevidenced-flip` | `canon gate task <task_id>` was asked to flip a checkbox with no matching, non-`Divergent` evidence record. |
| `fabricated-evidence` | An evidence note contains a blocklisted marker (`"would pass"`, `"TBD"`, `"n/a"`) or a bare `verified` claim with no attached command result. |
| `open-blocker` | With `spec_coverage.require_review` set (and `block_on_findings`, the default), a Finding whose latest version is severity `blocker`, disposition `open`, on a change an in-scope subject adopted. Subject string `<change_id>#<round>.<seq>`. See `canon-review`. |

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
  (`canon review add`), by a reviewer and actor other than the
  scenario's evidence actor, and from a session other than any evidence
  session when both carry one (`--session-id`), when `distinct_actor`
  (default `true`);
  gaps are `unreviewed-promotion`. With `block_on_findings` (default
  `true`), an open `blocker` Finding on a change an in-scope subject
  adopted is `open-blocker`. `exclude_lanes` also exempts scenarios
  from review. Full contract: `canon-review`.

  ```yaml
  spec_coverage:
    require_evidence: true
    require_review:                 # absent = nothing changes
      scope: [verifying, shipped]   # default
      distinct_actor: true          # default
      block_on_findings: true       # default
  ```

Two refusals rather than a silent pass, both surfacing as
`uncovered-cell` on the subject `spec_coverage`:

- A **malformed section** (unknown key, unknown `scope` status, or a
  malformed `require_cases`/`require_review` — unknown key, bad status,
  `null`, non-mapping) refuses at check time. It is never treated as
  absent — a typo must not be silently equivalent to not opting in.
- A **corpus kind routed off the gate's rung** (`scenario`,
  `divergence`, or `subject` sent anywhere but `local`) refuses, because
  the gate reads one tier and would otherwise pass by seeing nothing.

Malformed rows in those three kinds do NOT surface as
`malformed-evidence`; they are kept off the evidence violation set on
purpose, so enabling nothing changes no existing verdict.

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

Every case carrying the scenario id in its name, classname, or Cucumber
tag — dotted (`cart.add.04`) or underscored (`cart_add_04`, the form a
test function carries) — is bound, each as its own attachment with its
name and outcome. `--report-case <name>` (repeatable; a Rust test by its
function name) ADDS cases; it never hides an id-carrying one. No match,
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
  digest). Re-run the test and re-attest with `canon evidence add`.

The check applies to every latest record, not only subjects in
`spec_coverage.scope`: a binding is a claim about bytes whatever the
subject's status, and a repo with no `spec_coverage` would otherwise never
have its bindings checked. A superseded record is not checked —
re-attesting is the fix.

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
close` stage, and their success line ends ``— run `canon gate promote`
to commit it``. `divergence stage` stages for `canon divergence promote`
and names it. Every other write — `review add`, `divergence
resolve|defer`, `subject new|adopt|status`, `change new`, `scenario new`, `feature new`,
`gate task` (it edits the plan document and re-ingests it)
— is final when it returns, and its success line ends `— written
directly; nothing to promote`.

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

Runs the shipped fixture corpus — one fixture per failure class (nine,
including `open-blocker`), each a
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