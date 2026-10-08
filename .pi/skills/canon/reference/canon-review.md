# canon-review

Evidence says a test exists and what verdict it recorded. It does not say
that anyone other than the author looked at the work. Canon records that
second look as two ledger kinds — a **Finding** per issue a reviewer
raises, and a **Review** per scenario a reviewer signs off — and the
opt-in `spec_coverage.require_review` policy makes `canon gate check` and
`canon subject status` hold a subject until both are in order.

Both records are attestations: canon never checks a finding's summary
against the code, or that a reviewer actually read the scenario. What the
gate can check is that the records exist, who authored them, and what
state they are in.

## Finish a subject

The order an agent follows to claim a subject done:

1. **Test-run evidence** per owned scenario (`canon evidence add
   --scenario-id <id> --project-id <root-id> ...`, then `canon gate
   promote`). See `canon-gate`.
2. **Independent review** of the change(s) the subject adopted. Record
   each issue the reviewer raises as a Finding, fix it, then close it:

   ```bash
   canon finding add --change-id c-demo --round 1 --seq 1 \
     --severity blocker --disposition open --reviewer reviewer-2 \
     --summary "unbounded subprocess wait"
   canon gate promote
   # ... fix, commit ...
   canon finding close --change-id c-demo --round 1 --seq 1 \
     --disposition fixed --resolution-sha "$(git rev-parse HEAD)"
   canon gate promote
   ```

3. **One Review per scenario**, by someone other than the evidence actor:

   ```bash
   canon review add --project-id root --scenario-id world.demo.01 \
     --reviewer reviewer-2 --actor-id reviewer-2 --role reviewer \
     --pin "$(git rev-parse HEAD)" --upstream-ref "pr#41"
   canon gate promote
   ```

4. **Move the subject**: `canon subject status <id> verifying`, later
   `shipped`. See `canon-subject`.

## `canon finding add` / `canon finding close`

A Finding's natural key is `(change_id, round, seq)`: the change under
review, the 1-based review round, and the 1-based index within that
round. `seq` is never auto-assigned; `finding add` refuses an occupied
key, because two different findings under one key are two reviewers'
work collapsed into one identity.

```bash
canon finding add --change-id <c> --round <n> --seq <n> \
  --severity blocker|should-fix|note --disposition open \
  --reviewer <who> --summary "<one line>" \
  [--reviewed-sha <sha>] [--file-ref path/to/file.rs:120-134] [--actor-id <id>]
canon finding close --change-id <c> --round <n> --seq <n> \
  --disposition fixed --resolution-sha <sha>     # or rejected / deferred / open
```

Both stage a candidate; `canon gate promote` commits it. `finding close`
changes only the disposition (and `resolution_sha` with it); severity,
reviewer, summary, and refs are copied from the committed record. It
appends a second version at the same natural key — the first stays.

### Severities

| Severity | Meaning |
|---|---|
| `blocker` | Must be resolved before the work is claimed done. With `require_review`, an open blocker on an adopted change fails the gate (`open-blocker`). |
| `should-fix` | Worth fixing; may be scheduled for later. Never blocks the gate. |
| `note` | An observation; no obligation. |

Severity is the reviewer's own judgement, never a score canon derives.

### Dispositions

| Disposition | Meaning |
|---|---|
| `open` | Raised and not yet dealt with. The default on `finding add`; `finding close --disposition open` reopens. |
| `fixed` | Fixed by a commit. Requires `--resolution-sha <sha>` naming a commit this repo holds; only `fixed` may carry one. |
| `rejected` | The reviewer's point was considered and declined. |
| `deferred` | Accepted, but postponed to later work. |

### How findings fold

Every reader takes the **latest version per natural key**
`(change_id, round, seq)` — the same fold the report's finding panel
uses. A blocker raised `open` and later closed `fixed` counts as fixed; a
fixed finding reopened with `--disposition open` counts as open again.

## `canon review add`

```bash
canon review add --project-id <root-id> --scenario-id <id> \
  --reviewer <who> --pin <sha-or-ref> --role <role> \
  (--upstream-ref <ref> | --original-spec-ref <ref>) [--actor-id <id>]
```

One attributed Review record for one scenario, joined on
`(project_id, scenario_id)` — both are required because two spec roots
may carry the same scenario id. Exactly one provenance ref is required.
`--actor-id` defaults to `canon`. A Review carries no verdict; the
`verifying → shipped` ship gate still refuses missing or `Divergent`
evidence verdicts on its own. Commit with `canon gate promote`.

## The `require_review` policy (opt-in)

```yaml
# .canon/policy.yaml
spec_coverage:
  require_evidence: true
  require_review:                 # absent = nothing changes, byte for byte
    scope: [verifying, shipped]   # default; [] = every scenario
    distinct_actor: true          # default
    block_on_findings: true       # default
```

`require_review: {}` takes every default.

- **`scope`** — the subject statuses whose scenarios need a review. A
  scenario is in scope when its `@subject:<id>` subject is in one of
  them; `[]` means every scenario. It is independent of
  `spec_coverage.scope`. `spec_coverage.exclude_lanes` also exempts
  scenarios from review.
- **`distinct_actor`** — a review only counts when both its `reviewer`
  and its actor (`--actor-id`) differ from every actor on that
  scenario's evidence records. An agent cannot review its own attested
  work.
- **`block_on_findings`** — fail on open blocker findings (below).

A malformed `require_review` (an unknown key, an unknown status, `null`,
a non-mapping) poisons the whole `spec_coverage` section: `canon gate
check` reports it invalid rather than treating it as off, the same as a
malformed `require_cases`.

Canon's own `.canon/policy.yaml` does not enable `require_review`.

## How it meets the gate

### `unreviewed-promotion`

An in-scope scenario with no qualifying review:

```
unreviewed-promotion world.demo.01 — no review record for this scenario; spec_coverage.require_review wants an independent review (`canon review add`) once its subject is claimed done
```

or, when the only reviews are self-reviews:

```
unreviewed-promotion world.demo.01 — every review of this scenario is by an actor that also attested its evidence (`impl-agent`); spec_coverage.require_review.distinct_actor wants a reviewer other than the evidence actor
```

### `open-blocker`

With `block_on_findings`, a Finding whose latest version is severity
`blocker` and disposition `open`, on a change listed in an in-scope
subject's `change_ids` (`canon subject adopt`). The subject string is
`<change_id>#<round>.<seq>`:

```
open-blocker c-demo#1.1 — open blocker finding by `reviewer-2` on change `c-demo` (subject `demo-subject`): unbounded subprocess wait; fix it and `canon finding close --disposition fixed --resolution-sha <sha>`, or close it as rejected or deferred
```

Findings on changes no in-scope subject has adopted do not block.

### Review-waiver advisories

A subject moved with `--override-reason` (below) has its review gaps
printed after the violations as advisories; the gate stays green:

```
review waivers: 1 advisory(ies) — not failing the gate:
  waived unreviewed-promotion world.demo.01 — no review record ... [waiver: subject `demo-subject` moved to verifying by `lead`: reviewer out until Monday]
```

## The status guard

With `require_review` present, `canon subject status <id> <state>`
prints on stderr which review checks it ran or skipped:

```
canon subject status: review guard for building → verifying (spec_coverage.require_review, scope: verifying, shipped)
  ran unreviewed-promotion — every owned scenario needs a review by an actor other than its evidence actor
  ran open-blocker — 1 adopted change(s) checked for open blocker findings
```

A target status outside the scope prints `skipped unreviewed-promotion —
`specced` is not in require_review.scope`. A transition into an in-scope
status with gaps prints the violations, exits 1, leaves the record
unchanged, and names `--override-reason`.

`--override-reason "<one line>"` waives ONLY `unreviewed-promotion` and
`open-blocker` — never ship-gate verdict or case gaps, never unreadable
routing. The new Subject record carries
`status_override: {to, reason, checks: [classes], actor: {agent_id}}`
(actor from `--actor-id`, default `canon`), and the command prints the
`waived ...` lines and `override recorded by `lead`: <reason>`. A blank
or multi-line reason exits 2. The waiver clears on the subject's next
status write, so it never outlives the status it was granted for.

Without `require_review`, the guard prints nothing and changes nothing.

## Vocabulary in `canon context`

`canon context` (text and `--json`) carries a top-level `review` section:
`findingSeverities` (`blocker`, `should-fix`, `note`),
`findingDispositions` (`open`, `fixed`, `rejected`, `deferred`),
`reviewFields` (the Review kind's fields), and `requireReview` (the
active policy summary, or absent). The `spec_coverage` summary appends
` require_review=(scope=... distinct_actor=... block_on_findings=...)`
when set. Read it before authoring findings or reviews rather than
guessing the enums.
