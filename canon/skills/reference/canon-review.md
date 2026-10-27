# canon-review

Evidence says a test exists and what verdict it recorded. It does not say
that anyone other than the author looked at the work. Canon records that
second look as two ledger kinds — a **Finding** per issue a reviewer
raises, and a **Review** per scenario a reviewer signs off — and the
opt-in `spec_coverage.require_review` policy makes `canon gate check` and
`canon subject move` hold a subject until both are in order.

Both records are attestations: canon never checks a finding's summary
against the code, or that a reviewer actually read the scenario. What the
gate can check is that the records exist, who authored them, and what
state they are in.

## Finish a subject

The order an agent follows to claim a subject done:

1. **Test-run evidence** per owned scenario (`canon evidence add
   --scenario <id> --project <root-id> --session <session>` and the
   rest of its options;
   final when it returns under `write: { promote: auto }`, which
   `canon init` writes, and followed by `canon gate promote` only under
   `write.promote: manual`). See `canon-gate`.
2. **Independent review** of the change(s) the subject adopted. Record
   each issue the reviewer raises as a Finding, fix it, then close it:

   ```bash
   canon finding add --change c-demo --round 1 --seq 1 \
     --severity blocker --introduced-unknown --disposition open --actor reviewer-2 \
     --summary "unbounded subprocess wait" --session review-session-1
   # ... fix, commit ...
   canon finding close c-demo --round 1 --seq 1 \
     --disposition fixed --resolution-rev HEAD
   ```

   Under `write.promote: manual` (or no `write:` key), run
   `canon gate promote` after each of the two.

3. **One Review per scenario**, by someone other than the evidence actor,
   from a session other than the one that attested the evidence, after
   the evidence is in (the review pins what it saw):

   ```bash
   canon review scenario world.demo.01 --project root \
     --actor reviewer-2 --role reviewer \
     --session review-session-1 \
     --rev HEAD --upstream-ref "pr#41"
   ```

   `review scenario` writes directly; there is nothing to promote. If the
   reviewer could not reach a verdict, record that instead of signing:
   `--review-outcome indeterminate` (or `unavailable` when it could not run).
   Changing the scenario or re-attesting it afterwards makes the review
   stale; review it again. A reviewer of user-visible behavior attaches
   the screenshots or recordings it looked at with `--artifact <path>`
   (below).

4. **Move the subject**: `canon subject move <id> --to verifying`, later
   `shipped`. See `canon-subject`.

**Done when:** `canon gate check` exits 0 (no `unreviewed-promotion`, no
`open-blocker`) and `canon status` shows the subject with every scenario
reviewed and `openBlockers: 0` (`data.subjects[]`).

## `canon finding add` / `canon finding close`

A Finding's natural key is `(change_id, round, seq)`: the change under
review, the 1-based review round, and the 1-based index within that
round. `seq` is never auto-assigned; `finding add` refuses an occupied
key, because two different findings under one key are two reviewers'
work collapsed into one identity.

```text
canon finding add --change <c> --round <n> --seq <n> \
  --severity blocker|should-fix|note --disposition open \
  --actor <who> --summary "<one line>" \
  [--rev <rev>] [--resolution-rev <rev>] [--introduced-rev <rev> | --introduced-unknown] [--file-ref path/to/file.rs:120-134] [--scenario <id>] [--session <id>]
canon finding close <c> --round <n> --seq <n> \
  --disposition fixed --resolution-rev <rev> [--actor <id>] [--session <id>]   # or rejected / deferred / open
canon finding close <c> --round <n> --seq <n> \
  --disposition rejected|deferred --reason "<why, one line>"
```

`--reason` is required for `rejected` and `deferred` (refused with
`FINDING_REASON_REQUIRED` otherwise) and optional for `fixed`. It is stored
on the closing version, shown by `finding list` as `[reason: …]` and carried
in the JSON. Give the reason when you reject or defer: it is the only record
of why the finding was not fixed.

`--scenario` (repeatable, each a scenario the index knows; refused with
`FINDING_UNKNOWN_SCENARIO` otherwise) names the scenarios the finding is
about. After a `fixed` close, fresh evidence for those scenarios alone
re-verifies the fix; a finding naming none asks every evidenced scenario of
the subject, as before. `finding close` copies the list.

Both stage a candidate. Under `write: { promote: auto }` (what
`canon init` writes) the same run promotes it and the success line names
the committed record (``wrote kind=finding/… — staged and promoted in this
run; nothing to promote``); under `write.promote: manual`
`canon gate promote` commits it, and the line says so (``staged … — run
`canon gate promote` to commit it``). `finding close`
changes only the disposition (and `resolution_sha` with it); severity,
reviewer, summary, and refs are copied from the committed record. It
appends a second version at the same natural key — the first stays. Its
`--actor` and `--session` name who authored the transition; the
raiser's stay on the first version.

Every commit option (`--rev`, `--resolution-rev`, `--introduced-rev`)
takes any git revision (`HEAD~1`, a branch, a tag, a full or short sha),
resolved with `git rev-parse --verify <rev>^{commit}`; the record stores
the full sha. `--rev` (the commit the round reviewed) defaults to `HEAD`,
and so does `--resolution-rev` on a `fixed` close; `--introduced-rev` has
no default and is only sourced, never guessed. A new `blocker` or `should-fix`
needs it or `--introduced-unknown` (never both). A revision that names no
commit is refused (exit `2`). A finding recorded against uncommitted code
is allowed: the record carries the `codeTree` it saw and `worktreeDirty`.
Outside git, or in a repository with no commit, the write is refused with
`NO_COMMIT`.

Many findings at once: `--input` takes inline JSON, a file or `-`; an array
(or NDJSON) is a batch validated before any write. Keys are the option
names in camelCase (`change`, `actor`, `resolutionRev`); `canon schema
finding add --json` shows the input schema and an example. An identical
repeat is `unchanged` (`meta.unchanged`) and writes nothing; a different
finding under an occupied key is `FINDING_SEQ_OCCUPIED`.

### A defect the author found in its own work

Record it like any other finding, at the moment you find it and before
review, with `--actor` set to your own id; never ask
the reviewer to record it later under theirs. Nothing refuses a finding
whose reviewer is also the implementer, and an open `blocker` the author
raised blocks under `block_on_findings` exactly like a reviewer's.
`--introduced-rev` names the commit that introduced the defect, not who
found it: `--introduced-rev self` is a usage error (exit 2) that points at
`--actor`.

### Severities

| Severity | Meaning |
|---|---|
| `blocker` | Must be resolved before the work is claimed done. With `require_review`, an open blocker on an adopted change fails the gate (`open-blocker`). |
| `should-fix` | Worth fixing; may be scheduled for later. Never blocks the gate. |
| `note` | An observation; no obligation. |

Severity is the reviewer's own judgement, never a score canon derives.

Write a review's or finding's `--summary` as prose that also names the blast
radius: what else the change touches that the checks do not exercise. It is
text in the summary, not a field.

### Dispositions

| Disposition | Meaning |
|---|---|
| `open` | Raised and not yet dealt with. The default on `finding add`; `finding close --disposition open` reopens. |
| `fixed` | Fixed by a commit. Carries `--resolution-rev <rev>` (default `HEAD`) naming a commit this repo holds; only `fixed` may carry one. |
| `rejected` | The reviewer's point was considered and declined. Needs `--reason`. |
| `deferred` | Accepted, but postponed to later work. Needs `--reason`. |

### How findings fold

Every reader takes the **latest version per natural key**
`(change_id, round, seq)` — the same fold the report's finding panel
uses. A blocker raised `open` and later closed `fixed` counts as fixed; a
fixed finding reopened with `--disposition open` counts as open again.

## `canon review scenario`

```text
canon review scenario <scenario-id> [<scenario-id>] --project <root-id> \
  --actor <who> [--rev <rev>] --role <role> \
  (--upstream-ref <ref> | --original-spec-ref <ref>) [--session <id>] \
  [--review-outcome signed|indeterminate|unavailable] \
  [--reviewer-provider <p>] [--reviewer-model <m>] [--reviewer-harness <h>] \
  [--artifact <path>] [--artifact <path>] [--max-artifact-mib <MiB>]
canon review change <slug> --project <root> --actor <who> [--rev <rev>] \
  --role reviewer (--upstream-ref <ref> | --original-spec-ref <ref>)   # a chore, below
```

One attributed Review record per scenario, joined on
`(project_id, scenario_id)` — both are required because two spec roots
may carry the same scenario id. Exactly one provenance ref is required.
`--actor` is the reviewer and is required; `--session` fills `actor.session_id`.
A Review carries no verdict about the behavior; the `verifying → shipped`
ship gate still refuses missing or `Divergent` evidence verdicts on its
own (a `Divergent` one passes only while a human's `canon divergence
accept` holds). It is written directly to the ledger, with no staging step, and says
so: `canon review scenario: wrote <path> (new) — written directly; nothing to
promote`.

`--rev` is the commit the review is about: any git revision, default
`HEAD`. The review records the full sha and the `codeTree` of the code it
saw (the tree with `.canon/`, the spec roots and `code.exclude` removed), and
`worktreeDirty` when that code is uncommitted. Reviewing uncommitted code is
allowed, and the commit made from it has the same code tree, so committing it
afterwards leaves the review as it was: a review is judged by its pins (the
scenario and what its evidence says), not by the commit. Outside git, or
with no commit, the write is refused with `NO_COMMIT`. An identical review
written again is `unchanged` (`meta.unchanged` under `--json`) and adds no
record.

**Done when:** `canon gate check` reports no `unreviewed-promotion` for the
scenarios you reviewed; a review that does not count shows up there with
its reason.

### Several scenarios in one call (0.18)

The scenario operand is repeatable, and an `--input` array (see
`canon schema review scenario`) batches several calls. One reviewer, pin, provenance ref and set of artifacts cover
every review; each review pins its own scenario and evidence, and each
gets its own `pinned` and `wrote` line. The artifacts are stored once.
It is all or nothing: a scenario with no record, or an operand
that is not a scenario id, refuses the call (exit 2) and writes no review.
This is also the batch re-pin after `canon evidence add` reports reviews
it made stale (see `canon-gate`).

### A chore is reviewed as a change (`review change`, 0.23)

A chore adds no scenarios, and recording the suite again after it leaves
every scenario review current (a review pins the evidence's verdict and
checks, so the same verdicts from the same cases change nothing). What the
chore needs instead is one change-level review: the reviewer signs its
invariants and checks against the diff with `canon review change
<slug>`, in place of `review scenario`. The review pins the chore's brief
(its digest, less the `Approved:` line), so an edit to the brief stales it,
and `--to shipped` refuses a chore with no signed, current change-level
review (`spec_coverage.require_review.distinct_actor` asks it of a reviewer
and session that did not attest the subject's evidence). `review change` takes a
chore's slug and refuses a feature change.

### What the reviewer looked at (artifacts)

`--artifact <path>` (repeatable) binds a file the reviewer looked at — a
screenshot, a recording, a trace — exactly as `canon evidence add
--artifact` binds one: the path must resolve to a file inside the
repository, a file over 25 MiB is refused unless `--max-artifact-mib <N>`
raises the limit, and the bytes are stored by digest (in
`.canon/artifacts/sha256/<hex>` unless `canon.yaml`'s `artifact_store:` names an
object store, where they are uploaded and nothing is kept locally; commit
the local store with the ledger) before the
review is written. Any refusal exits 2 and writes and stores nothing. The
review records each distinct file once, in the order given:

```json
"artifacts": [{"path": "reports/title.png", "sha256": "<64 hex>"}]
```

The field is absent when no `--artifact` is given. `review scenario` prints
`canon review scenario: artifact store holds <n> bound file(s) under
.canon/artifacts/sha256/ (<k> newly stored) — commit them with the
ledger`.

`canon gate check` re-checks every file bound to a current in-effect
review (the latest per `(project_id, scenario_id, reviewer)`, unless it
went stale against its scenario) by the evidence rule (see `canon-gate`,
"Bound evidence stays provable"). A missing blob whose working-tree file
still matches is an advisory naming `canon artifacts store`, which stores
it back. A missing blob with a changed or missing working-tree file, or
an altered blob, is `stale-evidence` on the review's scenario, naming
the review:

```
stale-evidence world.demo.01 — review by `reviewer-2` (actor `reviewer-2`, 2026-10-11T09:00:00Z) bound file `reports/title.png` recorded sha256 <hex>; stored blob .canon/artifacts/sha256/<hex>: missing; working tree: <other hex> — the bound bytes are gone, look again and re-record the review with `canon review scenario <scenario-id> --artifact <path>`
```

The reviewer's later review replaces the earlier one, so its files are
the ones checked. A review that went stale (its scenario's spec or
evidence moved since) is not checked either: it no longer counts, and the
scenario's newer review is the one whose files are checked. Canon binds
and re-checks bytes only; it never compares images or judges what a
screenshot shows.

### What the review saw (pins)

`review scenario` reads the corpus and records `pins` on the review; the
reviewer never types them:

- `scenario_digest` — the scenario record's `scenario_digest`: a sha256
  over that scenario alone, as `canon inventory sync` stores it. It
  covers the scenario's tags and block (header, steps, doc strings,
  tables, `Examples:`) and the context it inherits: the Feature's tags,
  the Feature `Background:`, and an enclosing `Rule:` with its tags and
  `Background:`. Layout does not count: comments, blank lines, line
  indentation, tag order, and table alignment (cells are compared
  trimmed and unescaped). Values do: any step, tag or cell change, and
  whitespace inside a doc string after the delimiter's own indentation
  is removed. Other scenarios never count. A record synced before 0.15 has no
  `scenario_digest`; then the review pins its `source_digest` (the
  sha256 of the whole `.feature` file) and records
  `scenario_digest_scope: file`;
- `checks` — the scenario's latest evidence per role, each as `{role,
  verdict, checks}`: its verdict and the identity of what it checked (the
  report format and the matched case names, as `junit:<case>`, or the
  artifact digests). It names no evidence record, so recording the same
  verdict from the same checks again leaves the review current. A scenario
  with no evidence yet is pinned with an empty list.

It prints `canon review scenario: pinned <id> at scenario digest <12 hex> and the
verdict and checks of its <n> role(s) of evidence; …` (`at source digest` for
a file pin). A
scenario with no record (never synced) is refused with exit 2 and nothing
is written: there is nothing to pin.

The gate compares each pin like for like: a scenario pin with the
scenario's current `scenario_digest`, a file pin with its current
`source_digest`. Adding or editing another scenario in the file leaves a
scenario-pinned review current; editing this scenario, or a Background
or Rule it inherits, makes it stale. A file-pinned review stays a file
pin until it is reviewed again. Review after the spec and evidence are
final.

### The reviewer's outcome and runtime

`--review-outcome` records how the reviewer's run ended: `signed` (the default:
an approval), `indeterminate` (it ran and reached no verdict) or
`unavailable` (it could not run). Only `signed` counts; the other two
are recorded so a failed review is visible, never mistaken for a
sign-off or a rejection, and `review scenario` says so. `--reviewer-provider`,
`--reviewer-model` and `--reviewer-harness` record the runtime that
produced the review, as given. An empty or padded label is a usage error.

`--session` (on `evidence add`, `review scenario`, `finding add` and
`finding close`) takes one opaque token: non-empty, no surrounding
whitespace, no control characters. Anything else is a usage error
(exit 2) and nothing is written.

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
  and its actor (`--actor`) differ from every actor on that
  scenario's evidence records, and, when the review carries a session,
  that session differs from every session on those evidence records. An
  agent cannot review its own attested work, under another id or from
  the same session. A record with no session never matches one, so
  records written before sessions were recorded count by actor alone.
- **`block_on_findings`** — fail on open blocker findings (below).

A malformed `require_review` (an unknown key, an unknown status, `null`,
a non-mapping) poisons the whole `spec_coverage` section: `canon gate
check` reports it invalid rather than treating it as off, the same as a
malformed `require_cases`.

Canon's own `.canon/policy.yaml` enables `require_review: {}`, and so does
the starter policy `canon init` writes.

## How it meets the gate

### Which review counts

The reviews of a scenario are folded to the **latest per `reviewer`**
first (by `at`, then schema, then content digest — the fold every reader
uses). A reviewer's later review replaces their earlier one: a reviewer
who signed and later recorded `indeterminate` no longer counts as
signing, and a later `signed` review counts again. Reviews by different
reviewers never replace each other.

The scenario is reviewed when at least one of those latest reviews:

1. is `signed`;
2. still matches its pins — the scenario record's current
   `scenario_digest` (or `source_digest` for a file pin), and the verdict and
   checks of its latest evidence per role — when it has pins; reviews
   written before 0.23 count only after `canon migrate` (below);
3. passes `distinct_actor`, when that is on.

### Reviews written before 0.23 pinning

A review written before 0.23 pins something the gate no longer compares: a
0.15–0.22 review pins evidence record files, and a review written before
0.15 has no pins, only `pin`, the commit it reviewed. Such a review does not
count until `canon migrate` rewrites it to what the evidence says (see
`canon-gate`). A review that was current under the rule it was written under
becomes a current review; one that was stale stays stale, and its scenario
needs a new `canon review scenario`, which pins what it saw and replaces the
reviewer's earlier one. The gate, `canon status` and `canon subject move`
print one advisory naming how many scenarios are waiting for `canon
migrate`. In CI, check out full history (`fetch-depth: 0`) for the job that
runs `canon migrate`, or a pre-0.15 review whose commit is missing is pinned
as the scenario stands instead of being checked.

### `unreviewed-promotion`

An in-scope scenario with no qualifying review. When several latest
reviews fail for different reasons, the detail names the one closest to
counting: a current signed review failing only `distinct_actor`, then a
stale review, then non-signed outcomes.

```
unreviewed-promotion world.demo.01 — no review record for this scenario; spec_coverage.require_review wants an independent review (`canon review scenario`) once its subject is claimed done
```

or, when the only reviews are self-reviews:

```
unreviewed-promotion world.demo.01 — every review of this scenario is by an actor that also attested its evidence (`impl-agent`); spec_coverage.require_review.distinct_actor wants a reviewer other than the evidence actor
```

or, when another actor id reviewed from a session that attested the
evidence:

```
unreviewed-promotion world.demo.01 — every review of this scenario is by an actor that also attested its evidence (`impl-agent`) or from a session that did (`sess-impl`); spec_coverage.require_review.distinct_actor wants a reviewer other than the evidence actor, in a session other than any evidence session
```

or, when the scenario's spec, its evidence, or both changed after the
review (`its spec changed`, `its evidence changed`, `its spec and its
evidence changed`):

```
unreviewed-promotion world.demo.01 — stale review by `reviewer-2`: its evidence changed (the latest evidence records are not the pinned ones) since the review; a pinned review counts only while what it saw is current — review it again (`canon review scenario`)
```

or, when every latest review recorded no sign-off:

```
unreviewed-promotion world.demo.01 — no signed review: the latest review recorded no sign-off (`reviewer-2` indeterminate); an indeterminate or unavailable review is a gap, never an approval — run the review again and record `--review-outcome signed`
```

A non-signed review is a gap even without `distinct_actor`.

### `open-blocker`

With `block_on_findings`, a Finding whose latest version is severity
`blocker` and disposition `open`, on a change listed in an in-scope
subject's `change_ids` (`canon subject adopt`). The subject string is
`<change_id>#<round>.<seq>`:

```
open-blocker c-demo#1.1 — open blocker finding by `reviewer-2` on change `c-demo` (subject `demo-subject`): unbounded subprocess wait; fix it and `canon finding close <change> --round <n> --seq <n> --disposition fixed`, or close it as rejected or deferred
```

Findings on changes no in-scope subject has adopted do not block.

### Review-waiver advisories

A subject moved with `--override-reason` (below) has the exact gaps its
waiver recorded printed after the violations as advisories; the gate
stays green for those. A gap the waiver did not record (a blocker raised
later, a scenario tagged to the subject later) is a violation as usual:

```
review waivers: 1 advisory(ies) — not failing the gate:
  waived unreviewed-promotion world.demo.01 — no review record ... [waiver: subject `demo-subject` moved to verifying by `lead`: reviewer out until Monday]
```

## The status guard

With `require_review` present, `canon subject move <id> --to <state>`
prints on stderr which review checks it ran or skipped:

```stderr
canon subject move: review guard for building → verifying (spec_coverage.require_review, scope: verifying, shipped)
  ran unreviewed-promotion — every owned scenario needs a signed review, current with the spec and evidence it pinned, by an actor, and from a session, other than its evidence's
  ran open-blocker — 1 adopted change(s) checked for open blocker findings
```

A target status outside the scope prints `skipped unreviewed-promotion —
`specced` is not in require_review.scope`. A transition into an in-scope
status with gaps prints the violations, exits 1, leaves the record
unchanged, and names `--override-reason`.

`--override-reason "<one line>"` waives ONLY `unreviewed-promotion` and
`open-blocker` — never ship-gate verdict or case gaps, never unreadable
routing. The new Subject record lists exactly the violations it let
through:
`status_override: {to, reason, waived: [{class, subject}], actor: {agent_id}}`
(`subject` is the scenario id, or `<change_id>#<round>.<seq>` for a
blocker; actor from `--actor`, default `canon`). The command prints
the `waived ...` lines and `override recorded by `lead` for the N
violation(s) above: <reason>`. A blank or multi-line reason exits 2. The
waiver covers only what it recorded, and clears on the subject's next
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
