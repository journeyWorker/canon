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
   --scenario-id <id> --project-id <root-id> ... --session-id <session>`;
   final when it returns under `write: { promote: auto }`, which
   `canon init` writes, and followed by `canon gate promote` only under
   `write.promote: manual`). See `canon-gate`.
2. **Independent review** of the change(s) the subject adopted. Record
   each issue the reviewer raises as a Finding, fix it, then close it:

   ```bash
   canon finding add --change-id c-demo --round 1 --seq 1 \
     --severity blocker --disposition open --reviewer reviewer-2 \
     --summary "unbounded subprocess wait" --session-id review-session-1
   # ... fix, commit ...
   canon finding close --change-id c-demo --round 1 --seq 1 \
     --disposition fixed --resolution-sha "$(git rev-parse HEAD)"
   ```

   Under `write.promote: manual` (or no `write:` key), run
   `canon gate promote` after each of the two.

3. **One Review per scenario**, by someone other than the evidence actor,
   from a session other than the one that attested the evidence, after
   the evidence is in (the review pins what it saw):

   ```bash
   canon review add --project-id root --scenario-id world.demo.01 \
     --reviewer reviewer-2 --actor-id reviewer-2 --role reviewer \
     --session-id review-session-1 \
     --pin "$(git rev-parse HEAD)" --upstream-ref "pr#41"
   ```

   `review add` writes directly; there is nothing to promote. If the
   reviewer could not reach a verdict, record that instead of signing:
   `--outcome indeterminate` (or `unavailable` when it could not run).
   Changing the scenario or re-attesting it afterwards makes the review
   stale; review it again. A reviewer of user-visible behavior attaches
   the screenshots or recordings it looked at with `--artifact <path>`
   (below).

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
  [--reviewed-sha <sha>] [--file-ref path/to/file.rs:120-134] [--actor-id <id>] [--session-id <id>]
canon finding close --change-id <c> --round <n> --seq <n> \
  --disposition fixed --resolution-sha <sha> [--session-id <id>]   # or rejected / deferred / open
```

Both stage a candidate. Under `write: { promote: auto }` (what
`canon init` writes) the same run promotes it and the success line names
the committed record (``wrote kind=finding/… — staged and promoted in this
run; nothing to promote``); under `write.promote: manual`
`canon gate promote` commits it, and the line says so (``staged … — run
`canon gate promote` to commit it``). `finding close`
changes only the disposition (and `resolution_sha` with it); severity,
reviewer, summary, and refs are copied from the committed record. It
appends a second version at the same natural key — the first stays. Its
`--actor-id` and `--session-id` name who authored the transition; the
raiser's stay on the first version.

Every sha flag (`--reviewed-sha`, `--resolution-sha`, `--introduced-by`)
takes a full 40-character sha or a short one (4 or more lowercase hex
characters). A short sha is resolved with `git rev-parse --verify
<sha>^{commit}` in the repo, and the record stores the full sha. It is
refused (exit `2`) when it names no commit, when more than one commit
starts with it (pass more characters), when it only resolves by peeling a
tag, or outside a git work tree.

### A defect the author found in its own work

Record it like any other finding, at the moment you find it and before
review, with `--reviewer` and `--actor-id` set to your own id; never ask
the reviewer to record it later under theirs. Nothing refuses a finding
whose reviewer is also the implementer, and an open `blocker` the author
raised blocks under `block_on_findings` exactly like a reviewer's.
`--introduced-by` names the commit that introduced the defect, not who
found it: `--introduced-by self` is a usage error (exit 2) that points at
`--reviewer`.

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
  (--upstream-ref <ref> | --original-spec-ref <ref>) [--actor-id <id>] [--session-id <id>] \
  [--outcome signed|indeterminate|unavailable] \
  [--reviewer-provider <p>] [--reviewer-model <m>] [--reviewer-harness <h>] \
  [--artifact <path>]... [--max-artifact-mib <MiB>]
```

One attributed Review record for one scenario, joined on
`(project_id, scenario_id)` — both are required because two spec roots
may carry the same scenario id. Exactly one provenance ref is required.
`--actor-id` defaults to `canon`; `--session-id` fills `actor.session_id`.
A Review carries no verdict about the behavior; the `verifying → shipped`
ship gate still refuses missing or `Divergent` evidence verdicts on its
own. It is written directly to the ledger, with no staging step, and says
so: `canon review add: wrote <path> (new) — written directly; nothing to
promote`.

### What the reviewer looked at (artifacts)

`--artifact <path>` (repeatable) binds a file the reviewer looked at — a
screenshot, a recording, a trace — exactly as `canon evidence add
--artifact` binds one: the path must resolve to a file inside the
repository, a file over 25 MiB is refused unless `--max-artifact-mib <N>`
raises the limit, and the bytes are copied into
`.canon/artifacts/sha256/<hex>` (commit it with the ledger) before the
review is written. Any refusal exits 2 and writes and stores nothing. The
review records each distinct file once, in the order given:

```json
"artifacts": [{"path": "reports/title.png", "sha256": "<64 hex>"}]
```

The field is absent when no `--artifact` is given. `review add` prints
`canon review add: artifact store holds <n> bound file(s) under
.canon/artifacts/sha256/ (<k> newly stored) — commit them with the
ledger`.

`canon gate check` re-checks every file bound to a current in-effect
review (the latest per `(project_id, scenario_id, reviewer)`, unless it
went stale against its scenario) by the evidence rule (see `canon-gate`,
"Bound evidence stays provable"). A missing blob whose working-tree file
still matches is an advisory naming `canon evidence vault`, which stores
it back. A missing blob with a changed or missing working-tree file, or
an altered blob, is `stale-evidence` on the review's scenario, naming
the review:

```
stale-evidence world.demo.01 — review by `reviewer-2` (actor `reviewer-2`, 2026-10-11T09:00:00Z) bound file `reports/title.png` recorded sha256 <hex>; stored blob .canon/artifacts/sha256/<hex>: missing; working tree: <other hex> — the bound bytes are gone, look again and re-record the review with `canon review add --artifact <path>`
```

The reviewer's later review replaces the earlier one, so its files are
the ones checked. A review that went stale (its scenario's spec or
evidence moved since) is not checked either: it no longer counts, and the
scenario's newer review is the one whose files are checked. Canon binds
and re-checks bytes only; it never compares images or judges what a
screenshot shows.

### What the review saw (pins)

`review add` reads the corpus and records `pins` on the review; the
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
- `evidence` — the scenario's latest evidence record per role, each as
  `{key, digest}`: the ledger file `kind=evidence_record/<key>__<digest>.json`.
  A scenario with no evidence yet is pinned with an empty list.

It prints `canon review add: pinned <id> at scenario digest <12 hex> and
<n> latest evidence record(s); …` (`at source digest` for a file pin). A
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

`--outcome` records how the reviewer's run ended: `signed` (the default:
an approval), `indeterminate` (it ran and reached no verdict) or
`unavailable` (it could not run). Only `signed` counts; the other two
are recorded so a failed review is visible, never mistaken for a
sign-off or a rejection, and `review add` says so. `--reviewer-provider`,
`--reviewer-model` and `--reviewer-harness` record the runtime that
produced the review, as given. An empty or padded label is a usage error.

`--session-id` (on `evidence add`, `review add`, `finding add` and
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
  and its actor (`--actor-id`) differ from every actor on that
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
   `source_digest`, and its current latest evidence record per role —
   when it has pins; or, written before 0.15 without pins, still
   describes its scenario at the commit it pinned (below);
3. passes `distinct_actor`, when that is on.

### Reviews written before pinning

A review written before 0.15 has no pins, only `pin`: the commit it
reviewed. The gate reads the scenario's `.feature` file at that commit
(`git`, with the same per-scenario normalization `canon inventory sync`
uses for `scenario_digest`; if the scenario was in another file then, the
spec root's files at that commit are searched for its id) and decides:

- **current** when that digest equals the scenario's current one and no
  declared evidence record of the scenario is newer than the review: it
  counts under rules 1 and 3;
- **stale** when the scenario changed since the commit (`its spec changed
  since <pin>`) or has evidence newer than the review (`it has declared
  evidence newer than the review`): it no longer counts, exactly like a
  stale pinned review;
- **undecidable** when the pin is not a commit in this clone (shallow
  clone, rewritten history) or the scenario id is absent at it: it keeps
  counting, as in 0.15. A missing commit never becomes a violation.

`canon gate check` prints one advisory line with the split (exit code
unchanged by the line itself). A stale one whose scenario already has
another qualifying review is counted as superseded and not named:

```
advisory: 338 review(s) predate review pinning (0.15) and were checked against the commit each one pinned: 318 current, 12 stale, 8 undecidable; 7 stale one(s) superseded by a current review of the same scenario, so nothing is owed for them; a stale one with no current review in its place no longer counts — review its scenario again (`canon review add`): platformer.movement.01, platformer.movement.02, …; an undecidable one still counts unchecked, because its pinned commit or the scenario at it is not in this clone — check out full history (`fetch-depth: 0` in CI) to check it
```

`canon status` prints the same line as a warning, and `canon subject
status` prints it on stderr for the subject's own scenarios; all three
make the same decision. Re-review exactly the scenarios the line names:
the new review pins what it saw and replaces the reviewer's legacy one.
In CI, check out full history (`fetch-depth: 0`) for the job that runs
the gate, or every legacy review is undecidable.

### `unreviewed-promotion`

An in-scope scenario with no qualifying review. When several latest
reviews fail for different reasons, the detail names the one closest to
counting: a current signed review failing only `distinct_actor`, then a
stale review, then non-signed outcomes.

```
unreviewed-promotion world.demo.01 — no review record for this scenario; spec_coverage.require_review wants an independent review (`canon review add`) once its subject is claimed done
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
unreviewed-promotion world.demo.01 — stale review by `reviewer-2`: its evidence changed (the latest evidence records are not the pinned ones) since the review; a pinned review counts only while what it saw is current — review it again (`canon review add`)
```

or, when every latest review recorded no sign-off:

```
unreviewed-promotion world.demo.01 — no signed review: the latest review recorded no sign-off (`reviewer-2` indeterminate); an indeterminate or unavailable review is a gap, never an approval — run the review again and record `--outcome signed`
```

A non-signed review is a gap even without `distinct_actor`.

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

A subject moved with `--override-reason` (below) has the exact gaps its
waiver recorded printed after the violations as advisories; the gate
stays green for those. A gap the waiver did not record (a blocker raised
later, a scenario tagged to the subject later) is a violation as usual:

```
review waivers: 1 advisory(ies) — not failing the gate:
  waived unreviewed-promotion world.demo.01 — no review record ... [waiver: subject `demo-subject` moved to verifying by `lead`: reviewer out until Monday]
```

## The status guard

With `require_review` present, `canon subject status <id> <state>`
prints on stderr which review checks it ran or skipped:

```
canon subject status: review guard for building → verifying (spec_coverage.require_review, scope: verifying, shipped)
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
blocker; actor from `--actor-id`, default `canon`). The command prints
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
