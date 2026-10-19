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

Run `canon <command> --help` for any explicit CLI command. `canon init` asks
which harnesses get the provider-neutral companion skill (on a terminal; the
detected ones are checked), records the answer in `canon.yaml`, and installs
it. Afterwards:

```bash
canon skills install                         # follows canon.yaml skills.providers
canon skills install --providers=claude,codex,omp,pi   # any subset; rewrites skills.providers
canon skills check                           # read-only drift check
canon skills doctor                          # diagnostics, never deletes
```

The canonical projection is one directory-shaped `canon` skill per selected
provider: Claude uses `.claude/skills/canon/SKILL.md`, Codex uses
`.agents/skills/canon/SKILL.md` (the directory Codex scans for skills), and
OMP/Pi use `.omp/skills/canon/SKILL.md` and `.pi/skills/canon/SKILL.md`. Every
selected provider receives lazy `reference/**` and `scripts/**` sidecars.
Without `--providers`, the commands serve `canon.yaml`'s
`skills: { providers: [...] }`; a repo without that key (set up before 0.17)
uses the providers its install lock records; with neither, the harnesses
whose `.claude`, `.codex`, `.omp` or `.pi` directory exists. Nothing
undetected is ever added: with nothing detected, nothing is installed and
the command says how to choose. OMP/Pi are project-local passive bundles; the
retrieve pre-dispatch script is a sidecar, not a native hook.

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

## 0.24.0 release

Version 0.24.0 gives each concept one name across every command, and makes a
batch of evidence bind each file to the scenario it belongs to. Plan:
`docs/superpowers/plans/2026-10-18-canon-0.24.0-one-name.md`.

**Why.** Dogfood evals 7–11 (0.19–0.23): every wrong-shape canon call an agent
made, 0–3 per run and each fixed by its next call, came from names that differ
between commands (`finding list --change`, `scenario new --project-id`,
`query --kind review_record`), not from commands it could not find (#85). In
eval 11 both implementers looped every screenshot into one `--all-scenarios`
batch, so each of 103 records bound about 30 unrelated files (#83).

### D1–D3: one name per concept (#85)

- A flag whose value is the id of a record or configured entity is
  `--<noun>-id`: `--change` → `--change-id` (brief approve, brief check, review
  add, scenario new), `--project` → `--project-id` (feature new, scenario new),
  `--subject` → `--subject-id` (change new, retrieve, scenario new, subject
  adopt), `--actor` → `--actor-id` (feature new, learn rollback, scenario new),
  `--run` → `--run-id` (dispatch end, ingest artifacts).
- `--kind` values of `query`, `export` and `purge` drop the `_record` suffix:
  `evidence_record` → `evidence`. The record's own `kind` field, ledger paths
  and `canon.yaml` routing keys are storage and keep `evidence_record`.
- Old spellings still run until 0.25.0. Each use prints
  `canon <command>: --<old> is deprecated — use --<new> (removed in 0.25.0)` on
  stderr and adds `meta.deprecated: [{used, use, removedIn}]` to JSON;
  `canon schema <command>` lists them as `deprecatedAliases`; help hides them.
  `<kind>_record` is accepted for every `--kind`.
- Everything canon prints for an agent to type back (hints, `meta.next`,
  examples, help, the skill, the website, the AGENTS block) uses the new names.
- A contract test walks the command tree and fails when two flags differ only
  by an `-id` suffix, a listed concept is spelled another way, or a `--kind`
  value ends in `_record`.
- `evals/dogfood/trace_check.py` counts naming failures and deprecated spellings.

### D4: attachments by test (#83)

- A report says which file belongs to which test: JUnit `[[ATTACHMENT|<path>]]`
  lines in a case's `<system-out>` (Playwright's JUnit reporter; relative paths
  resolve against the report's directory) and Cucumber JSON `embeddings`. A
  record binds the attachments of the cases it owns, in batch and
  single-scenario mode alike.
- `evidence add --all-scenarios` refuses a bare `--artifact <path>` (it would
  bind to every scenario); `--artifact <scenario-id>=<path>` binds one file to
  one scenario of the batch. A report attachment that is missing or over the
  size limit is refused, naming the case and path; nothing is written.
- Review pins are unchanged: report evidence pins the format and matched case
  names, so a re-run's new screenshot bytes do not stale a review.

### Upgrading

- Nothing to migrate. Scripts using the old flags or `*_record` kinds keep
  working with a warning until 0.25.0; switch to the names above.
- A batch that passed screenshots as bare `--artifact` is now refused: let the
  report carry them per test, or use `--artifact <scenario-id>=<path>`.

### Conformance changes

- New: `usage-deprecated-spellings` (stderr line, `meta.deprecated`, still
  runs) and `evidence-add-all-scenarios-attachments`.
- Re-blessed on the new spellings and the `evidence=N` tally: the change,
  evidence, finding, gate, review, status and subject cases that type a
  renamed flag; version strings (`0.23.0` → `0.24.0`).

ACCEPTANCE-PLACEHOLDER: dogfood eval 12

## 0.23.0 release

Version 0.23.0 keeps only the current decision in canon's working set. A
record that a newer one supersedes is history: it moves to an archive, is never
copied into new records, and is read only when history is asked for. Rules that
existed only to patch an earlier rule are removed, and one freshness rule
replaces several. Plan:
`docs/superpowers/plans/2026-10-18-canon-0.23.0-latest-only.md`.

**Why.** Issue #80 and the 2026-10-18 efficiency audit of a 100-scenario repo
(dogfood eval 10) and of canon's own ledger.

- *Carry-forward (#80).* 0.21 re-listed every earlier attachment in each new
  evidence record: records grew from 0.7 KB to 5.4 KB, with 20.6 attachments on
  average (33 at most), stale reports from other changes attached, and agents
  could not find how to drop one.
- *History in the working set.* 541 of 641 `evidence_record` files were
  superseded (84%), as were 96% of `subject` and 75% of `finding` files, and
  every read scanned them before folding to the latest.
- *Artifact bytes in git.* Content-addressed, so never duplicated, but every
  new screenshot stayed in history for good.
- *Work done twice.* A write loaded the corpus for its guard and again for
  `meta.next`; each command reread the same records, `.feature` files and blobs;
  `gate check` took 3.4 s on eval 10's repo.
- *Reviews pinned to record ids.* Recording the suite again staled every
  review: 82 unchanged scenarios were re-reviewed after a feature and all 100
  after a behavior-preserving chore.
- *Overlapping freshness rules.* Review pins, the 0.22 re-verification
  triggers and accepted-divergence pins each implemented "pinned to what, stale
  when" on their own; pre-0.15 reviews used a second algorithm (348 such
  reviews measured in 0.16).
- *Three staging shapes*, with divergence bypassing staging altogether.

### D1: no carry-forward (#80)

An evidence record binds exactly the files its own write names; nothing earlier
is added. `--drop-attachment`/`--drop-reason`, the record's `attachment_drop`
field and the `EVIDENCE_CARRY_BLOB_MISSING` code are removed (the flags are now
a usage error, exit 2). Old records are not hidden: they are history (D2), and
a new record is judged on its own files (D5).

### D2: latest-only ledger, `.canon/archive`

- **Archive on append.** Appending a version of a natural-keyed record moves the
  versions it supersedes from `.canon/ledger/kind=<k>/…` to
  `.canon/archive/kind=<k>/…` (same layout and file names, tracked in git). The
  ledger then holds one file per key. A move is a rename; a crash between the
  write and the move leaves a readable ledger and the next append or compaction
  finishes it.
- **Default reads see the ledger only.** `status`, `gate check`, the guards and
  `canon query` read the working set. `canon query --kind <kind> --history`
  returns every version, ledger and archive, oldest first. Rules that genuinely
  need history read the archive explicitly: the gate's evidence history
  (accepted-divergence timing, coverage counts) and `gate promote`'s recovery
  and duplicate checks for evidence, finding and divergence records.
- **`canon ledger compact [--dry-run] [--json]`** moves the superseded versions
  of an existing ledger once; idempotent; `canon migrate` runs it. Exit 0 done,
  1 a move failed (`LEDGER_COMPACT_FAILED`; run it again), 2 refused
  (`LEDGER_ARCHIVE_UNAVAILABLE`: the tier root is not a directory named
  `ledger`, so there is no archive and every version stays).
- **Archived kinds:** `change`, `subject`, `scenario`, `finding`,
  `evidence_record`, `review`, `divergence`.
- **What stays unarchived, and why.** Execution kinds (`task`, `handoff`,
  `session`, `run`, `event`, `trajectory`) route to the hot and cold rungs, not
  git; `strategy_item` has no folding reader. Within an archived kind, a record
  stays while it still wins any fold a reader applies (the evidence ledger is
  folded per cell, per scenario, per task and per run_seq partition) or carries
  a claim a rule reads without folding: a `flagged` overlay or `reviewed`
  lifecycle tag, a brief override (listed by `status`), and an approval bound to
  the current evidence generation (risk tiers count distinct approvers). A
  record that does not parse or has no join key stays too: compaction never
  guesses.

### D3: artifact bytes off git (`artifact_store:`)

`canon.yaml` gains `artifact_store: { backend, prefix }`; the existing
`artifacts:` block stays the ingest source config.

- `backend: local` (default, also when the block is absent) is
  `.canon/artifacts/sha256/`, git-tracked and credential-free, as before.
- `backend: <tier>` names an `s3` tier of `tiers:` (R2 or S3, same credentials)
  and stores blobs at `<prefix>sha256/<digest>` (`prefix` default `artifacts/`).
  `evidence add`, `review add` and `evidence vault` upload there and keep nothing
  locally; records keep only digests.
- `canon artifacts verify [--full]` checks every file the latest evidence and
  the current reviews bind: presence and size/digest metadata (one HEAD per
  digest), or with `--full` download and re-hash. Exit 0 all stored, 1 a file
  missing or altered (`ARTIFACTS_VERIFY_FAILED`), 2 the store cannot be reached
  (`ARTIFACTS_STORE_UNREACHABLE`: a refusal, never a pass). The gate asks the same
  store.
- `canon artifacts migrate --to <tier> [--dry-run]` hashes every local blob,
  uploads it by digest, reads it back, writes the `artifact_store:` block, only
  then removes the local copies and adds `.canon/artifacts/` to `.gitignore`.
  A blob that does not match its name, a target that is not an `s3` tier or a bad
  stored copy is `ARTIFACTS_MIGRATE_REFUSED` (exit 2). A run stopped anywhere
  leaves a repo the gate still passes; a second run finds nothing to do. Moving
  bytes back to local is not supported.

### D4: compute once

One process reads each record file, each `.feature` file and each artifact blob
once: record scans are memoized by path while size and modification time are
unchanged (a file rewritten within one timestamp tick is always read again), a
large kind is read by several threads and returned in path order, `.feature`
files are scanned once for the staleness check, the index refresh, status, the
gate and the brief's flow coverage, and a blob hashed for one record is not
hashed again for the next that binds it. Results equal a fresh scan. On
eval 10's repo `gate check` went 2.9 s → 0.4 s in this slice alone.

### D5: reviews pin what the evidence says

- **Pin.** A review pins its scenario's digest and, per role, the latest
  evidence's verdict and the identity of its checks: the report format and
  matched case names (`junit:<case>`), or the artifact digests for evidence bound
  to artifacts only. It names no evidence record. Recording the suite again with
  the same verdicts from the same cases leaves the review current; a different
  verdict, a case that appears or disappears, or a changed scenario stales it,
  and the message says which.
- **Review JSON.** `pins.evidence: [{key, digest}]` becomes
  `pins.checks: [{role, verdict, checks: [...]}]`; `scenario_digest` and
  `scenario_digest_scope` are unchanged.
- **Chore.** Re-recording the suite after a chore no longer stales unchanged
  scenarios. A chore is satisfied by either one change-level review, `canon
  review add --change <slug> …` (pins the chore's brief less its `Approved:` line,
  so editing the brief stales it; refused for a feature change; `distinct_actor`
  applies), or, as 0.22 reviewed chores, every evidenced scenario having a current
  signed review recorded after the chore's approval and after its latest
  evidence. Upgraded repos stay green. `canon status` lists each subject's
  `choreReviews` (JSON).
- **Fixed blocker.** Still needs evidence newer than the close; scenario reviews
  stay current when verdicts and checks are unchanged.

### D6: one freshness rule, one staging shape, no legacy path

- **One module.** `canon_gate::freshness` answers "pinned to what, stale when"
  for review pins, accepted-divergence pins and the re-verification triggers.
- **One staging shape.** Evidence, finding and divergence all stage under
  `.canon/ledger/_staging/` and are committed by `canon gate promote` (its
  summary line now counts `divergence=N`; `canon divergence promote` commits only
  staged divergences). `divergence resolve|defer|accept` stage and promote their
  own record in the same run and print "staged and promoted in this run; nothing
  to promote". `--no-promote` still leaves a record staged.
- **Legacy freshness removed.** The pre-0.15 review algorithm (read the scenario
  at the pinned commit, with its advisory) is deleted from the gate. A review
  that predates 0.23 pins does not count until `canon migrate` pins it: the gate,
  `status` and `subject status` print one advisory naming how many scenarios wait,
  and a transition that needs the review is refused as `unreviewed-promotion`.
- **`canon migrate` re-pins.** After the file moves it runs `ledger compact` and
  then rewrites each 0.15–0.22 or pre-0.15 review and accepted divergence that was
  *current under the rule it was written under* to the 0.23 pin, keeping its
  envelope, reviewer and artifacts. A stale one stays stale and its scenario needs
  a new review. A pre-0.15 commit this clone cannot read (shallow clone, rewritten
  history) pins the review as the scenario stands, and the run says how many. A
  review of a scenario no `.feature` file declares is left alone. Idempotent;
  `MIGRATE_PINS_FAILED` means files moved but pinning did not finish (run it again).

**Measured.** Dogfood eval 10's repo (100 scenarios), 0.22.0 against the 0.23
release build after `canon migrate`:

| | 0.22.0 | 0.23 |
|---|---|---|
| ledger | 1441 files / 7.2 MB | 735 files / 3.2 MB (706 superseded versions in `.canon/archive`) |
| `status --json` | 382 ms | 84 ms |
| `gate check` | 2229 ms | 135 ms |
| `gate check` result | clean | clean (identical before and after migrate) |

Recording the suite again with the same JUnit reports left 100 gate lines about
stale or unreviewed scenarios under 0.22 and 18 under 0.23; the 18 are
hit-feedback scenarios whose reviewed evidence also bound determinism files the
re-record did not name, so they are genuinely different checks. On canon's own
repo `canon migrate` archived 284 versions and re-pinned 494 reviews (101 stale
ones left stale); the git pack went 679 MB → 74 MB after `git gc`, which is
unreachable objects from deleted branches, not ledger data. W4 timings (median
of 5, same repo, before the archive): `gate check` 2860 → 390 ms, `evidence add`
1162 → 557, `review add` 589 → 382, `subject status --to verifying` 662 → 317.

**Migration** from 0.22.

- Run `canon migrate` once (commit the result). It compacts the ledger into
  `.canon/archive` and re-pins reviews and accepted divergences that were
  current. Reviews that were already stale stay stale and are re-reviewed per
  scenario, as before. Until you run it, an old review does not count and the
  gate says how many scenarios are waiting. In CI, check out full history
  (`fetch-depth: 0`) for the job that runs it, or a pre-0.15 review whose commit
  is missing is pinned as the scenario stands instead of being checked.
- `evidence add --drop-attachment/--drop-reason` are removed, and so is
  `EVIDENCE_CARRY_BLOB_MISSING`. A record binds only what its write names; name
  every file the evidence should bind.
- Review JSON: `pins.evidence` is now `pins.checks` (`{role, verdict, checks}`);
  `Review.scenario_id` is optional and `change_id` is new (change-level
  reviews); `attachment_drop` is gone from the evidence schema; the status
  subject entry gains `choreReviews`.
- `canon query` returns the ledger only. Add `--history` to read the archive
  too; a script that read superseded versions needs it.
- Divergence commands (`resolve`, `defer`, `accept`) print "staged and promoted
  in this run; nothing to promote"; `gate promote` reports `divergence=N`.
- A chore ships with one `canon review add --change <slug>` or with per-scenario
  re-reviews recorded after its evidence; recording the suite again no longer
  stales unchanged scenarios.
- `artifact_store:` is optional; without it nothing changes. Use `canon
  artifacts migrate --to <tier>` to move bytes off git.

**Changed conformance expectations.**

- Version strings (`0.22.0` → `0.23.0`) in every case that prints one.
- `canon gate promote` summary and the nothing-staged line name divergence
  (`promoted evidence_record=N, finding=N, divergence=0`; `… (evidence_record,
  finding, divergence)`): `evidence-add-all-scenarios`,
  `evidence-add-all-scenarios-refused-contradiction`,
  `evidence-add-promote-on-write`, `evidence-add-refused-report-contradiction`,
  `evidence-add-refused-unapproved-brief`, `finding-add-self-found-blocker`,
  `finding-list-open-json`, `gate-check-audit-confirmed-clean`,
  `gate-check-audit-false-success`, `gate-check-audit-sampled-missing`,
  `gate-task-warns-canon-command-proof`, `review-gate-open-blocker`,
  `review-gate-same-session-not-counted`, `review-pin-stale-evidence`.
- `review add` says "pinned … at scenario digest … and the verdict and checks of
  its N role(s) of evidence" (was "N latest evidence record(s)"), and review JSON
  carries `pins.checks`: `review-add-artifact-stored`, `review-add-batch`,
  `review-outcome-indeterminate-gap`, `review-outcome-unavailable-gap`,
  `review-gate-same-session-not-counted`, `review-pin-stale-evidence`,
  `review-pin-stale-spec`, `evidence-add-refused-unapproved-brief`.
- The pre-0.15 review advisory is gone, because fixtures hold current pins:
  `finding-add-self-found-blocker`, `review-gate-distinct-review-passes`,
  `review-gate-malformed-finding`, `review-gate-open-blocker`,
  `review-gate-self-review-not-counted`, `status-gaps`.
- `review-outcome-indeterminate-gap` step 8: `query --kind review` returns the
  latest review only (1 record, was 2).
- `review-pin-stale-evidence`: the re-attestation now records a different verdict
  (`--verdict not-applicable`), since the same verdict from the same checks leaves
  a review current; the stale message reads "the latest evidence says a different
  verdict or names different checks than the review saw".
- `review-gate-open-blocker`: the reverify hint (step 3) record the suite again
  "(a review stays current while the verdicts and checks are the same)".
- `gate-check-audit-false-success` step 6: the false-success hint no longer
  suggests `--drop-attachment`.
- `status` JSON gains `choreReviews: []` on each subject: `status-brief-proposed`,
  `status-brief-unapproved`, `status-gaps`, `status-ship-ready`.
- `context-json-policy-summary`: `evidence_record` loses `attachment_drop`;
  `review` gains `change_id`, and `scenario_id` is no longer required.
- `evidence-add-drop-attachment` becomes `evidence-add-binds-only-named`
  (binding only the named files; `--drop-attachment` is a usage error).
- `gate-check-legacy-review-missing-pin` becomes
  `gate-check-unmigrated-review-needs-migrate` (see below). Fixture reviews in
  five cases were rewritten to 0.23 pins (file names change with the digest).

**New conformance cases.** `evidence-add-binds-only-named` (re-attesting binds
exactly the named files, nothing carried forward, `--drop-attachment` exit 2) and
`gate-check-unmigrated-review-needs-migrate` (a pre-0.23 review is refused and
advised until `canon migrate` pins it; then the transition and the gate pass).
The archive, `ledger compact`, `artifacts verify|migrate`, review pins and
change-level reviews, the chore rule, migration pinning and the compute-once
caches are covered by contract tests (`ledger_archive.rs`,
`artifacts_object_store.rs`, `review_pins.rs`, `migrate_pins.rs`,
`reverify.rs`), not the corpus.

Closes #80

**Acceptance (dogfood eval 11, omp, `language: ko`, orchestrated roles).**
The same baseline as eval 10 (`canon migrate` applied with the 0.23 build:
gate clean before and after). An orchestrator spawned planner, implementer
and reviewer subagents for a combo system, then for a chore moving the combo
math into one module.

- Feature: the brief stopped for the human in under 3 minutes (0 blockers),
  then 20 new scenarios (103 in all) shipped with gate clean. The reviewer
  found a flaky combo e2e test; it was fixed inside the approved change and
  re-reviewed. 66 canon calls for 20 new scenarios (3.3 per scenario; eval 10:
  2.2), most of the difference from two review rounds' findings (16 calls)
  and `status` (10).
- Chore: one change-level review instead of re-reviewing every scenario (eval
  10's chore re-reviewed 100). The reviewer caught the implementer binding two
  determinism scenarios to the chore's comparison instead of their own and
  sent them back; shipped, gate clean.
- After the run the ledger holds one file per key (103 evidence records; 614
  superseded versions in `.canon/archive`), and no record carries a file
  forward.
- Found by the run: both implementers looped every screenshot into an
  `--all-scenarios` batch, so each record binds ~30 unrelated files (#83);
  role-implementer.md now says a batch carries reports and a screenshot goes
  with its own scenario. `evals/dogfood/trace_check.py` counts only code that
  landed in git as implementation (a reviewer's uncommitted diagnostic spec is
  scratch).

## 0.22.0 release

Version 0.22.0 makes canon's gates honest in both directions. An honest
`divergent` verdict on a known, tracked gap no longer forces a false
`faithful` or a waiver; a fixed blocker, a chore and a weakened policy no
longer pass without the checks they imply; and every refusal says what to do
next. The 0.21 deprecated names are removed. Plan:
`docs/superpowers/plans/2026-10-17-canon-0.22.0-honest-gates.md`.

**Why.** The second dogfood round on 0.21.1 and the 2026-10-16 briefing.
Honest verdicts were penalized: a `divergent` scenario blocked shipping with
`hint: null`, and the only ways through were a waiver or a false `faithful`
(#78). Some changes were not re-verified: a chore reopened a shipped subject
and the old evidence shipped it again (#77), and a blocker closed `fixed`
lifted the block with no new evidence (#72). Policy drift was advisory only:
weakening the approved `policy.yaml` passed the ship guard and `gate check`
(#71). `status` asked the human to answer a brief that was still an empty
skeleton (#70). The reviewer reference never asked whether a test checks the
scenario or only mirrors the implementation (#73). And three issues in a row
(#62, #63, #78) were refusals with a null hint, which 0.20's contract was
meant to end.

### D1: accepted divergence (#78)

`canon divergence accept --project-id <root> --scenario-id <id> --reason
<text> --tracking <url-or-ref> [--expiry YYYY-MM-DD] --reviewer <id> [--actor-id
<id>] [--session-id <id>] --role reviewer` records a reviewer's acceptance of a
scenario whose latest evidence is `divergent` as a known, tracked gap.
`--scenario-id` repeats; the call is all or nothing. It is a reviewer step
(`--role reviewer`, or `--as reviewer`/`CANON_ROLE`), and it is refused with
`DIVERGENCE_ACCEPT_REFUSED` (exit 2, nothing written) for another role, for
evidence that is not divergent, and for the evidence's own actor or session.
An empty `--reason`/`--tracking`, an unindexed scenario or an unparseable
`--expiry` is `DIVERGENCE_INVALID_ARGUMENT`.

- **Pinned.** The record is a `Divergence` with the new status `accepted`
  (`reason`, `tracking`, optional `expiry`, `evidence`: the pins of the
  evidence it saw, as a review pins). It is not a new record kind.
- **Stale.** New evidence for the scenario, divergent or not, ends the
  acceptance; so does reaching `--expiry` (a date is the start of that day,
  UTC; an RFC 3339 time is taken as given). The latest acceptance wins.
- **Counted.** While it holds, the ship guard treats the cell as covered. A
  stale or expired acceptance blocks again, and the refusal says why.
- **Never silent.** `canon status` lists `accepted-divergence <id> —
  <tracking> (expires <date>|no expiry)` (JSON: `accepted_divergences`, omitted
  when empty) and `canon gate check` prints the same lines as an
  `accepted_divergence` advisory; neither fails the gate.
- **The way out is named.** The `uncovered-cell` refusal for a divergent
  verdict, and the `status` step, name both: fix and re-attest, or
  `canon divergence accept`.

### D2: re-verification after a fix and after a chore (#72, #77)

One freshness rule, two triggers: a subject cannot ship while one of its
scenarios with evidence has no evidence recorded after the trigger.

- **Fixed blocker (#72).** A blocker finding whose latest disposition is
  `fixed` keeps blocking (`open-blocker`) until every evidenced scenario of
  its change's subjects has evidence newer than the time it was closed
  `fixed` (not the time it was raised). The review pins go stale on that new
  evidence, so the re-review follows. This is a review check, so it is waived
  by `--override-reason` like the others, and it applies where
  `require_review` blocks on findings.
- **Chore (#77).** An approved chore change on the subject needs evidence
  newer than its approval, for every evidenced scenario, before `--to shipped`
  (an `uncovered-cell` violation; `gate check` reports it for a subject past
  `building`; not waivable). The `status` next step after the reopen names
  `canon evidence add --all-scenarios …`, then the re-review.
- **Approval time.** `canon brief approve` now writes the approval time as
  RFC 3339 UTC to the second (`Approved: <who> 2026-10-17T05:12:33Z — "…"`).
  An `Approved:` line from before, carrying only a date, is read as the start
  of that day. A chore whose approval line has no readable time triggers
  nothing.
- **Reopen.** `canon change new` on a shipped subject prints the reopen
  command (`canon subject status <id> --to verifying --reason …`) and adds it to
  `meta.next`, with the reason it is not yet worked under the subject.
- Every refusal names the scenarios and the batch command in its `hint`.

### D3: approved policy fails closed (#71)

A `.canon/policy.yaml` whose bytes differ from the approved digest
(`Changed`), or whose approval file cannot be read (`Unreadable`), is a
violation: `canon gate check` exits 1 (`unreviewed-promotion policy — policy
changed since approval` or `policy approval unreadable`), and shipping or
reopening a subject is refused with `POLICY_CHANGED_SINCE_APPROVAL` or
`POLICY_APPROVAL_UNREADABLE` (exit 1). `--override-reason` does not waive it;
the human re-approves with `canon policy approve --by <human> --note "<words>"`
or the approved files are restored from git. A policy never approved
(`Unapproved`) stays advisory. `canon status` still warns and exits 0.
`canon policy show [--json]` prints the policy path, its current digest, the
approval state (`none`, `approved`, `unapproved`, `changed`, `unreadable`),
approver, time, approved digest and note, and says when the gate fails closed.

### D4: status follows the brief (#70)

While the current change's brief has `canon brief check` blockers, a subject's
step in `status` is the planner's `canon brief check --change <slug>` (`N
blocker(s): write the brief …`, `actor: agent`, `role: planner`) instead of
the human stop. The human step appears only when the brief is clean and
unapproved. `change new`, `subject adopt` and every other `meta.next` follow
the same rule, since they use the same engine.

### D5: every refusal carries a hint

`crates/canon-cli/tests/hints.rs` reads the sources and fails when a
`CliError::refused`/`failed` construction has no `.hint(…)` in its chain; its
allowlist of errors whose message already is the next step is empty, and an
entry that matches nothing fails too. Every refusal that lacked one now has
one (about thirty cases in the conformance corpus gain a `hint:` line).

### D6: the reviewer checks the checks (#73)

`role-reviewer.md` gains "Check the checks": read each bound test against its
scenario's Then steps, and record a finding when a check pins behavior the
scenario does not require or skips a Then step it states. Skill text only; it
also teaches `divergence accept` and the re-verification after a fixed
blocker. The planner, implementer, orchestrator and `canon-gate`/`-policy`/
`-subject` references and the website docs (English and Korean) follow D1–D4.

### D7: 0.21 deprecations removed

`context-pack create`, `prompt register`, `divergence stage`, `demo attest`
and the two-positional `subject status <id> <state>` are gone, with
`deprecation.rs`, `meta.deprecated` in JSON and `deprecated` in `canon
schema`. They are now plain usage errors.

**Migration** from 0.21.

- Use the 0.21 names: `context-pack new`, `prompt add`, `divergence add`,
  `demo add`, `subject status <id> --to <state>`. Callers that read
  `meta.deprecated` can drop the branch.
- `canon gate check` exits 1 when `policy.yaml` changed since its approval or
  its approval is unreadable; a CI job that tolerated the advisory now needs
  the human's `canon policy approve` (or the file restored).
- `subject status --to shipped` (and the reopen) is refused with
  `POLICY_CHANGED_SINCE_APPROVAL`/`POLICY_APPROVAL_UNREADABLE` in that case,
  and, after a blocker closed `fixed` or an approved chore, until the suite is
  recorded again with `evidence add --all-scenarios`.
- Human stderr gains a `hint: …` line on every refusal that had none; JSON
  `error.hint` is now a string on all of them.
- `canon status` may show the planner's brief step where it showed the human
  stop; an agent that stopped on any `human` step now stops only when the
  brief is clean.
- New `Approved:` lines carry a timestamp; older date-only lines still read.
- JSON shapes: the status subject entry gains the optional
  `accepted_divergences` (omitted when empty); `meta.deprecated` and the
  schema `deprecated` field are gone; `gate check` advisories can include
  `accepted_divergence`; `divergence` records gain the status `accepted`
  (`divergence.schema.json` is regenerated). The subject JSON `status_reason`
  from 0.21.1 is unchanged.

**Changed conformance expectations.**

- Version strings (`0.21.1` → `0.22.0`) in every case that prints one.
- A `hint:` line on refusals that had none: `change-new` (steps 4, 5),
  `evidence-add-all-scenarios-refused-contradiction`,
  `evidence-add-brief-override`, `evidence-add-fabrication-marker-whole-token`,
  `evidence-add-refused-malformed-write-config`,
  `evidence-add-refused-newline-injection`,
  `evidence-add-refused-no-subject-key`,
  `evidence-add-refused-report-contradiction`,
  `finding-add-refused-unresolvable-short-sha`, `gate-check-invalid-canon-yaml`,
  `gate-check-refused-pre-0-17-layout`, `init-providers-refused-unknown`,
  `init-scaffold-and-rerun`, `migrate-refused-uncommitted`,
  `policy-approve-refused-no-policy`, `review-add-batch`,
  `review-gate-malformed-finding`, `review-outcome-indeterminate-gap`,
  `subject-reopen-refused-unapproved-brief`,
  `subject-ship-refused-missing-verdict`,
  `subject-ship-refused-no-tagged-scenarios`,
  `subject-ship-refused-require-cases-gap`,
  `subject-status-off-chain-transition`, `usage-error-unreadable-canon-yaml`.
- The planner brief step replaces the human stop while the brief has
  blockers: `change-new`, `status-brief-proposed` (steps 3–5),
  `subject-adopt-second-subject` (steps 4–6), and the status cases that print
  a next step for a brief.
- `review-gate-open-blocker`: after `finding close --disposition fixed` the
  next step is the suite again (`evidence add --all-scenarios`), the move to
  `verifying` is refused as a fixed blocker not re-verified, and passes after
  fresh evidence and re-review.
- `policy-approve-changed-advisory` becomes `policy-approve-changed-fails-closed`:
  `gate check` reports the changed policy as a violation and exits 1;
  `status` still warns and exits 0; re-approval makes the gate clean.

**New conformance cases.** `policy-approve-changed-fails-closed` (replaces
`policy-approve-changed-advisory`). `review-gate-open-blocker` gains steps 5
and 6 (the reverified move and the clean gate check). Accepted divergence,
chore re-verification, `policy show` and the hint rule are covered by contract
tests (`divergence_accept.rs`, `reverify.rs`, `policy_fail_closed.rs`,
`hints.rs`), not the corpus.

Closes #70

Closes #71

Closes #72

Closes #73

Closes #77

Closes #78

**Acceptance (dogfood eval 10, omp, `language: ko`, orchestrated roles).**
An orchestrator agent spawned planner, implementer and reviewer subagents for
a hit-feedback feature on the shipped game (screen flash, cat blink, and
phone vibration that headless desktop Chrome cannot produce), then for a
chore splitting weapon tables into per-weapon files.

- Right after `change new`, `meta.next` was the planner's `canon brief check
  --change hit-feedback` with 7 blockers, not a human stop (#70).
- The vibration limit was recorded honestly: the brief scoped automated
  checks to a stubbed `navigator.vibrate` and named real-device vibration as
  unverified, and the report repeated it. No verdict claimed more.
  `divergence accept` was not needed in this run; it is covered by
  `tests/divergence_accept.rs`.
- The chore: one human stop; `brief approve` named the reopen in
  `meta.next`; the subject was red until the suite was recorded again after
  the chore's approval and re-reviewed by another session (#77); shipped,
  100/100 evidenced and reviewed, gate clean.
- Feature turn: 37 canon calls for 17 new scenarios (2.2 per scenario). No
  canon refusal in any session carried a null hint.
- Found by the run: carried attachments pile up across changes and agents
  did not find how to drop a stale one (#80);
  `evals/dogfood/trace_check.py` now treats scratch paths outside the repo,
  including unexpanded `$VAR` paths, as neither code nor a pre-stop write.

## 0.21.1 release

Patch release for three dogfood findings from a game repo on 0.21.0.

- **Outside-the-repository evidence now says what to do (#62).** `canon
  evidence add --artifact|--report` and `canon review add --artifact` still
  refuse a file outside the repository (bound files are recorded by
  repository-relative path; canon does not accept outside paths), but the
  refusal now carries a hint (copy the file anywhere under the repository and
  bind that path; the bytes are stored under `.canon/artifacts/sha256` either
  way) and, in JSON, `error.field` naming the flag. Every other
  `EVIDENCE_BIND_REFUSED` cause (unreadable or missing file, directory, over
  the size limit, not UTF-8, unparseable report, unmatched `--report-case`,
  no case for an indexed scenario) has a hint too.
- **`--reason` on every transition (#63).** `canon subject status <id> --to
  <state> --reason "<text>"` is accepted on any transition and recorded on the
  subject as `status_reason: { reason, actor }` (a new optional field on the
  Subject record, cleared by the next status write like `reopen` and
  `status_override`; `subject status <id>` prints it). The reopen
  (`shipped → verifying`) still requires it and still records `reopen`;
  `SUBJECT_REASON_REQUIRED` now has a hint with the full command.
- **Dropped provenance lines are explained (#64).** `canon scenario new`
  (single and `--from`) says to keep the `# canon: {...}` provenance lines when
  filling in the scenario (stdout line; `meta.note` in JSON). The
  `missing-provenance` violation now lists the lines of the headers lacking
  it and the line to restore (directly after `Feature:`, and directly above a
  scenario's tags, in the shape `canon scenario new` writes), and the
  `FORMAT_VIOLATIONS` and `INVENTORY_ABORTED` hints repeat the fix when one is
  among the violations. `role-implementer.md` and `canon-authoring.md` say
  the same in one line. There is no `--fix-provenance` command.

**Migration.** `SUBJECT_REASON_UNEXPECTED` no longer exists: `--reason` is
accepted on every transition. Callers that branched on that code can drop the
branch. Subject records gain the optional `status_reason` field (the schema
`subject.schema.json` is regenerated); existing records read unchanged.

**Changed conformance expectations.** Version strings (`0.21.0` → `0.21.1`)
in every case that prints one; `evidence-add-refused-file-outside-repo`,
`evidence-add-refused-no-matching-case` and
`review-add-refused-artifact-outside-repo` gain a `hint:` line;
`subject-reopen-allowed` gains the `SUBJECT_REASON_REQUIRED` hint line; `status-refreshes-stale-index` and
`status-shipped-red-reopen` (both run `scenario new`) gain the provenance note line; the subject field lists in
`context-json-policy-summary` gain `status_reason`.

Closes #62

Closes #63

Closes #64

## 0.21.0 release

Version 0.21.0 gives canon one verb grammar and makes it answer "what now?"
itself: every successful write names the next steps, an agent can declare
its role and see only its own steps and commands, the skill is a short core
plus one reference per role, and a re-attested scenario keeps the files its
earlier evidence bound. Plan:
`docs/superpowers/plans/2026-10-15-canon-0.21.0-roles-and-next-steps.md`.

**Why.** Issue #60, from the 0.20 audit and dogfood evals 8 and 9. After a
successful write an agent had nothing to read, so it called `canon status`
again to find the next step (eval 8: 4 `status` and 5 `gate` calls for one
feature). One skill and one command list served every agent, so a reviewer
loaded PRD-writing passes it never uses, and a spawned subagent had no way
to say what it was. Four verbs were synonyms of the shared ones
(`context-pack create`, `prompt register`, `divergence stage`,
`demo attest`) and `subject status <id> <state>` took two positionals of
different meaning. Eval 8 and 9 also hit #58: a suite-wide
`evidence add --all-scenarios` was refused when earlier evidence had bound
attachments the batch did not repeat, so the agent counted them as dropped
work and re-attached by hand.

### D1: one grammar, with deprecation

`new` creates a resource, `add` appends a ledger record, `list` lists, `show`
shows one; domain verbs with their own meaning (`approve`, `promote`,
`resolve`, `defer`, `close`, `check`, `sync`, `install`, `begin`, `end`, …)
stay. Four renames and one reshape:

| 0.20 | 0.21 |
|---|---|
| `canon context-pack create` | `canon context-pack new` |
| `canon prompt register` | `canon prompt add` |
| `canon divergence stage` | `canon divergence add` |
| `canon demo attest` | `canon demo add` |
| `canon subject status <id> <state>` | `canon subject status <id> --to <state>` (`subject status <id>` alone shows the subject) |

The old forms are deprecated, not aliased: for this release they still run,
print `canon <old>: deprecated — use canon <new>` on stderr, and in JSON
mode add `meta.deprecated: { "use": "<new>", "removedIn": "0.22.0" }`.
`canon schema` marks them with `deprecated` and lists them only under
`--all`. `schema` also stops mislabelling two commands: `subject status` is
`write` (with a note that it only reads without `--to`) and `format` is
`write`.

### D2: `meta.next` after every write

Every successful write returns `meta.next: [{ command, why, actor, role }]`
(plus `nextOmitted` when more were cut), the same shape as `status`'s
`next[]`, computed by the same engine from the state after the write, for
the subject, change or scenarios the write touched. Human mode ends with one
`next: <command> — <why>` line (`(+N more)` when there are others). A staged
write names `canon gate promote`. canon never runs a step; `actor: human`
marks the steps only a human takes (brief approval, whose `role` is `null`).
The envelope now covers the writes that were still bare: `dispatch begin` and
`end`, `learn approve` and `rollback`, `purge`, `export`, and `ingest
artifacts` and `plans`. Reads do not carry `meta.next`; `status` and bare
`brief` keep their own.

### D3: roles

`CANON_ROLE` or the global `--as <role>` (the flag wins) declares one of
`planner`, `implementer`, `reviewer`, `orchestrator`; any other value is
`UNKNOWN_ROLE` (exit 2, with `field` and `expected`). canon cannot detect a
role, so this is guidance, not enforcement: session separation between
implementer and reviewer stays ledger-based (#39).

- Unset: every command behaves as in 0.20.
- Set: `status` and `meta.next` keep that role's steps and summarize the rest
  as counts, `meta.waitingOn` in JSON (`{ "reviewer": 2 }`) and a "waiting
  on reviewer: 2" line in human mode.
- `canon schema` gives every command `roles: [...]`; bare `canon schema`
  lists the declared role's commands, `--all` lists every one. A command
  with no listed role is shown to everyone.
- `--role` on `evidence add` and `review add` defaults to the declared role
  when it is `implementer` or `reviewer`.

### D4: the skill by role

The skill is a shared core (the loop, `status`, `schema`, `meta.next`, how
to learn your role) plus `reference/role-planner.md`, `role-implementer.md`,
`role-reviewer.md` and `role-orchestrator.md`. An agent with a declared role
loads the core and its own reference; the brief passes (`brief-*.md`) are
linked from the planner reference only. Each role reference ends with a
Commands block listing exactly the commands that role calls, so it never
needs `schema` for them. The reviewer reads a PRD with `canon brief check`
and checks scenarios against its flow; the orchestrator reference says how
to spawn each role with `CANON_ROLE` set and a separate session for review.
An agent with no role loads the core and follows `status`. The AGENTS block
gains a roles line.

### D5: smaller contract items

- `--fields a,b` on `status`, `export`, `divergence status` and `context`,
  with 0.20's projection rules and `UNKNOWN_FIELD`.
- Errors gain optional `field` and `expected` (`subject status --to bad`:
  `field: "--to"`, `expected: ["specced", …]`), set by usage errors and input
  validation; in JSON they sit beside `code`, `message`, `hint`.
- `canon schema <command>` returns `example`, one minimal invocation, and
  `requiredOneOf` for a clap group where one of several flags is required
  (`review add`'s provenance); a test runs every example.
- Retry safety: repeating `evidence add` or `review add` with identical
  input writes nothing, names the record already there and answers
  `meta.unchanged: true` with exit 0; a batch reports each unchanged scenario.
- Exit meanings: the per-command exceptions are justified in `schema`'s
  `exits` (`context`, `dispatch diff` and `divergence status` always 0 on a
  successful read; `evidence vault` 0 on a partial pass, with
  `notStored` listed; `subject status` 0 also when it only shows).
- `learn promote`: when a promotion is blocked and the demotion it triggered
  after a contradicting trajectory also fails, the error is
  `LEARN_DEMOTION_FAILED` (the strategy is still active), not
  `LEARN_PROMOTION_BLOCKED`.

### #58: attachments carry forward

Re-attesting a scenario that names fewer files than its latest record no
longer counts the rest as dropped. Every file the latest record of the same
fold binds and the new write does not name again is carried into the new
record unchanged (same path, digest and stored blob), and a file named again
replaces its binding; JSON lists them as `carried` and human mode prints
`carried forward N earlier attachment(s) unchanged: …`. `--drop-attachment
<path> --drop-reason <text>` stays the only way to lose a binding and is
still recorded. A carried file whose blob the artifact store does not hold
intact is refused as `EVIDENCE_CARRY_BLOB_MISSING` (exit 2, nothing written;
re-bind it, `canon evidence vault`, or drop it). The audit fold is the
exception: an audit record never inherits bindings, so one that leaves out
an earlier binding is still refused unless dropped. A suite-wide
`--all-scenarios` batch carries per scenario.

`evals/dogfood/trace_check.py` also changed (eval 9): a brief written by a
subagent stops the root session, and `canon` calls spelled out inside eval
cells (`Bun.$`, subprocess) are counted.

**Migration.** For a 0.20 consumer:

- Run `canon skills install` for the 0.21 skill (core plus role references).
- The deprecated names above still run in 0.21 and are removed in 0.22.0:
  switch to `context-pack new`, `prompt add`, `divergence add`, `demo add`
  and `subject status <id> --to <state>`. A script that read stderr will see
  the `deprecated —` line; JSON consumers can read `meta.deprecated`.
- Every write answers `{ data, meta }` with `meta.next`. Shapes that moved:
  `dispatch begin` puts `run_id` at `.data.run_id`; `export`'s page state
  (`returned`, `total`, `hasMore`, `nextCursor`) is in `meta`; `brief
  approve`, `subject *`, `context-pack new` and `prompt add` are enveloped;
  `dispatch end`, `learn approve|rollback`, `purge` and `ingest
  artifacts|plans` JSON is enveloped too. Read `.data`, not the top level.
- `status` and `meta.next` steps gain a `role` member (`null` for a human
  step).
- `review add` with neither provenance flag is now a clap `USAGE` error
  (was `REVIEW_INVALID_ARGUMENT`); enum flags (`--kind`, `--verdict`, …)
  fail as clap closed-set `USAGE` errors, which name the flag in `field`.
- Human output gains a trailing `next:` line after writes; a script that
  matched the last line of a write's output should match the leading lines.
- `evidence add` repeated with identical input now returns unchanged
  (exit 0) instead of staging a duplicate; `evidence add --all-scenarios`
  and single re-attestations carry earlier attachments forward instead of
  refusing with `EVIDENCE_REFUSED`.
- `CANON_ROLE` set to anything but the four roles is `UNKNOWN_ROLE`.

**Changed conformance expectations.** `git diff` of `conformance/` against
0.20.0 touches 63 cases and no exit code except the first one below.

- `evidence-add-drop-attachment`: the re-attest that names fewer files is
  now exit 0 and carries the file forward (was exit 2 `EVIDENCE_REFUSED`);
  the drop with `--drop-attachment` is unchanged. The table row in
  `conformance/README.md` follows.
- `evidence-add-promote-on-write`: the third step uses a different `--ref`
  so it is a new record, not an unchanged repeat; its usage-error stderr
  lists `--role` after `--project-id`.
- Version: every case that prints the version prints 0.21.0 (`version`,
  `gate-check-refused-pre-0-17-layout`, `init-policy-refused`,
  `skills-check-outdated-lock`, and the status and gate cases listed below).
- Human writes end with a `next:` line (29 cases): `change-new`,
  `evidence-add-all-scenarios`,
  `evidence-add-all-scenarios-refused-contradiction`,
  `evidence-add-brief-override`,
  `evidence-add-fabrication-marker-whole-token`,
  `evidence-add-refused-report-contradiction`,
  `evidence-add-refused-unapproved-brief`, `evidence-add-swift-testing`,
  `finding-add-refused-unresolvable-short-sha`, `finding-list-open-json`,
  `gate-check-audit-confirmed-clean`, `gate-check-audit-false-success`,
  `gate-check-audit-sampled-missing`,
  `gate-check-audit-sampling-fixed-seed`,
  `gate-task-warns-canon-command-proof`, `init-policy-light-status`,
  `init-providers-detected`, `init-scaffold-and-rerun`,
  `inventory-sync-tag-diagnostics`, `migrate-pointer-brief-and-config`,
  `policy-approve-and-advisory`, `policy-approve-changed-advisory`,
  `policy-approve-refused-no-policy`, `review-add-artifact-stored`,
  `review-add-batch`, `review-add-refused-artifact-outside-repo`,
  `status-brief-proposed`, `status-no-scenarios`,
  `subject-adopt-second-subject`; so do `evidence-add-drop-attachment`,
  `evidence-add-promote-on-write` and the cases in the next group.
- `subject status <id> <state>` becomes `--to <state>` in the case's args
  and in the `canon subject status …` text that `status` prints (24 cases):
  `finding-add-self-found-blocker`, `gate-check-legacy-review-missing-pin`,
  `review-gate-distinct-review-passes`, `review-gate-malformed-finding`,
  `review-gate-open-blocker`, `review-gate-override-recorded`,
  `review-gate-same-session-not-counted`,
  `review-gate-self-review-not-counted`, `review-gate-unreviewed-refused`,
  `review-outcome-indeterminate-gap`, `review-outcome-unavailable-gap`,
  `review-pin-stale-evidence`, `review-pin-stale-spec`,
  `status-refreshes-stale-index`, `status-ship-ready`,
  `status-shipped-red-reopen`, `subject-reopen-allowed`,
  `subject-reopen-refused-unapproved-brief`, `subject-ship-allowed`,
  `subject-ship-refused-missing-verdict`,
  `subject-ship-refused-no-tagged-scenarios`,
  `subject-ship-refused-require-cases-gap`,
  `subject-ship-refused-unapproved-brief`,
  `subject-status-off-chain-transition`.
- `status --json`'s `next[]` entries gain `role`:
  `gate-check-audit-sampling-fixed-seed`, `status-brief-proposed`,
  `status-brief-unapproved`, `status-gaps`, `status-no-policy`.
- `evidence-add-drop-attachment` prints the carry-forward line on the
  re-attest; `evidence-add-repeat-unchanged` (below) shows the unchanged
  line.

**New conformance cases:** `evidence-add-repeat-unchanged` (an identical
`evidence add` writes nothing and exits 0; a different `--ref` is a new
record). The rest of the coverage lives in `tests/grammar.rs`,
`tests/schema_examples.rs`, `tests/evidence_batch.rs` and
`tests/learn_promote.rs`.

**Closed issues:** #58, #60.

**Acceptance (dogfood evals 9 and 9b, omp, `language: ko`).** An
orchestrator agent was asked for a pause menu on the shipped game and
spawned planner, implementer and reviewer subagents.

- Eval 9 (before the skill fixes below) shipped: 11 new scenarios, 94/94
  evidenced and reviewed, gate clean. Each subagent read only the core
  skill and its own role reference; the reviewer was a separate session,
  and its first round failed one e2e test that the implementer then fixed.
  It cost 53 canon calls (4.8 per new scenario, against 2.1 for one agent
  in eval 8): 16 were `canon schema` probes, and the orchestrator called
  `status` right after writes. It also dropped 82 still-valid screenshot
  bindings to get a batch past the attachment guard (#58).
- Fixed from eval 9 in this release: role references carry complete
  Commands blocks and send agents to `schema` only for unlisted commands;
  the orchestrator, implementer and reviewer follow `meta.next` /
  `meta.waitingOn` instead of re-reading `status`; attachments carry
  forward (#58); `review add`'s one-of provenance is reported as
  `requiredOneOf`; scenario steps belong to the implementer; `scenario
  new --from` batches scenario creation; `evals/dogfood/trace_check.py`
  reads a brief written by a subagent and canon calls made from eval cells.
- Eval 9b reran the same request on the fixed build. Up to where it
  stopped it made 57 canon calls for 22 new scenarios (2.6 per scenario;
  22 of them one `scenario new` each, which `--from` now makes one call),
  1 `schema` probe, no dropped bindings, 105/105 scenarios evidenced and
  100 reviewed; its reviewer filed a real blocker (Title did nothing
  after a restart) and a second implementer was fixing it. The run did not
  reach `shipped`: the disk holding the repo was disconnected mid-run.

## 0.20.0 release

Version 0.20.0 makes canon's CLI something an agent can drive without
reading `--help`: `canon schema` describes every command, every command
takes `--json` and keeps stdout to one JSON document, every refusal has a
stable code, lists are bounded and pageable, `purge` deletes only a
reviewed plan, and `canon … | head` ends quietly. Plan:
`docs/superpowers/plans/2026-10-15-canon-0.20.0-cli-contract.md`.

**Why.** Issue #53, from `docs/research/deep-research-report.md`: an agent
spends its calls on discovery and recovery (wrong command, `--help`, parse
prose, retry), not on the work. The plan inventoried 0.19 against the
report's checklist:

| Checklist item | 0.19 |
|---|---|
| Discovery without reading help | `canon context --json` described record kinds, not commands |
| `--json` on every command | missing on `init`, `migrate`, `format`, `feature new`, `scenario new`, `change new`, `inventory sync`, `plugin sync`, `skills *`, `tier age`, `report`, `dashboard`, `gate check` |
| Stable string error codes | only `canon brief` / `brief approve`; everything else was free text on stderr |
| Exit codes documented | 0/1/2 by convention, with per-module exceptions |
| Bounded lists | no `--limit`/`--cursor`; `query`, `finding list`, `divergence status`, `export` returned everything |
| Field projection | none |
| One verb per meaning | `learn request` duplicated `learn approve` |
| Risky writes need a plan digest | `purge --dry-run` existed, with no confirm-by-digest |

### D1: `canon schema` (discovery)

`canon schema [<command path>…] [--json] [--errors]` reads the clap command
tree, so it cannot drift from what the parser accepts, plus one metadata
table that adds what clap cannot know. It touches no repo and no network.
Bare `canon schema` (or a group such as `canon schema evidence`) lists
leaf commands; `canon schema evidence add --json` returns one command.
With `--json`, the output is `{ "data", "meta": { "command": "schema" } }`;
a command is:

```json
{ "command": "evidence add", "path": ["evidence", "add"], "aliases": [],
  "about": "…", "effect": "write", "supportsJson": true,
  "arguments": [], "options": [{ "name": "…", "kind": "value", "short": null,
    "aliases": [], "valueName": "…", "required": false, "repeatable": false,
    "possibleValues": [], "default": [], "help": "…" }],
  "exits": [{ "code": 0, "meaning": "staged" }],
  "errorCodes": ["BRIEF_UNAPPROVED", "…"] }
```

`effect` is `read`, `write` or `destructive` (`purge` is the destructive
one); `kind` is `positional`, `flag`, `count` or `value`. `canon schema
--errors` prints the registry of every error code. A command path that
does not resolve is `UNKNOWN_COMMAND`. Two tests keep it honest: one fails
when a leaf command has no metadata row (or a row has no leaf), the other
derives each command's `errorCodes` from the sources and fails on any
difference.

### D3: one error contract, and the exit policy

Every refusal and operational failure is one `CliError { code, message,
hint, retryable, exit }`.

- Human mode, on stderr: `canon <command path>: <CODE> — <message>`, then
  an optional `hint: <next step>` line. The old `canon <cmd>: refused — …`
  and `canon <cmd>: <rule> <subject> — …` lines are gone.
- JSON mode, on stdout: `{"error": {"code", "message", "hint", "retryable"}}`
  (`hint` is `null` when there is none); the human lines still go to
  stderr.
- A refusal that used to print one line per violation (the subject, ship
  and reopen guards) is one error: its message lists every violation and
  the next step is the `hint`; the code names the kind
  (`SUBJECT_GATE_VIOLATION`, `BRIEF_UNAPPROVED`,
  `BRIEF_CHANGED_SINCE_APPROVAL`, …). A failure that also has a report
  (`gate check`, `format`) keeps its human report on stdout and, in JSON
  mode, answers one document carrying `error`, `data` and `meta`.
- Exit codes keep their 0.19 meaning: 0 ok, 1 a gate failure or an
  operational error, 2 a refusal or a usage error (clap usage errors are
  code `USAGE`). The per-command exceptions (`context` always 0,
  `dispatch diff` 0, `evidence vault` 0 on a partial pass) stay and are now
  listed in `canon schema` under `exits`. CI that reads exit codes keeps
  working; the code is the precise contract.

### D2: `--json` everywhere, JSON-only stdout

Every leaf command accepts `--json` (`canon schema` reports
`supportsJson`; a sweep test runs each leaf with `--json` and parses its
stdout). In JSON mode stdout is exactly one JSON document, success or
error; narration, refresh notices and advisories go to stderr. Existing
shapes keep their fields (`status` keeps `statusVersion: 3`). A command
that gained `--json` answers `{ "data": …, "meta": { "command": "<path>" } }`.

### D4: bounded lists and `--fields`

`query`, `finding list`, `divergence status` and `export` take
`--limit <n>` (default 100, at least 1), `--cursor <opaque>` and `--all`
(which conflicts with `--limit`). The JSON result gains a `meta` object
with `returned`, `total` (every record the command matched, across all
pages), `hasMore` and `nextCursor` (`null` on the last page). Human mode
ends with `N of M shown; next cursor: … (rerun with --cursor …, or --all)`
when more remain. A cursor is opaque: it names the listing it came from and
the key of the last record returned, so it stays valid while the result is
unchanged and is refused as `INVALID_CURSOR` for another listing or when
that record is gone. `--fields a,b,c` keeps only those top-level fields of
each record in `query` and `finding list`; an unknown name is
`UNKNOWN_FIELD` and the hint lists the known ones.

### D5: `purge` plans, then confirms

`canon purge --kind <kind> --before <time> --plan` prints the records it
would remove and a `planDigest`; nothing is deleted. `--confirm <digest>`
recomputes the plan and deletes only if the digest still matches, else
`PLAN_CHANGED` and nothing is deleted. The digest covers the kind, the
cutoff, each rung's count and the identity of every record the plan would
remove (the sorted record keys, including the learn store), so a record
added, removed or swapped between `--plan` and `--confirm` changes it even
when the count does not. A bare `purge` is refused with
`PURGE_PLAN_REQUIRED`. `--dry-run` is gone (a clean cutover: `--plan` is
the same step with a digest).

### D6: one name per meaning

`canon learn request` is removed; `canon learn approve` was already the
same implementation. `fmt` stays a clap alias of `format`, but `schema`
and errors name `format`. No other command was renamed.

### D7: skill and AGENTS block, and a closed pipe

- The skill tells an agent unsure of a command to start from `canon schema
  <command> --json`, to pass `--json` and read `error.code`, and to page
  with `--cursor`; the AGENTS block gains one line. The website CLI pages
  follow.
- A closed stdout ends canon quietly. Rust starts a process with SIGPIPE
  ignored, so `canon status | head` panicked and exited 101; canon now
  restores the default action and dies of SIGPIPE (exit 141 in a shell),
  as other command-line tools do. `canon dashboard` keeps SIGPIPE ignored
  so a client that disconnects never kills the server.

**Migration.** For a 0.19 consumer:

- Run `canon skills install` for the 0.20 skill.
- `query`, `finding list` and `export` return at most 100 records by
  default and carry a `meta` object (`returned`, `total`, `hasMore`,
  `nextCursor`); a script that read everything must pass `--all` or follow
  `nextCursor`. `divergence status` is bounded the same way. Existing
  fields are unchanged.
- `canon purge --dry-run` is removed: use `--plan`, then `--confirm
  <planDigest>`. A bare `purge` is refused.
- `canon learn request` is removed: use `canon learn approve`.
- stderr lines are now `canon <cmd>: <CODE> — <message>` followed by
  `hint: …`; a script that matched `refused —` or a rule name should match
  the code. A refusal that listed several violations is one error with one
  hint. Exit codes did not change.
- Commands that gained `--json` answer `{ "data", "meta" }`; in JSON mode
  a failure is `{ "error": { code, message, hint, retryable } }` on stdout.
- `canon … | head` now ends by SIGPIPE (exit 141) instead of panicking
  with 101.

**Changed conformance expectations.** Every case that prints the version
prints 0.20.0. Forty-seven expected-stderr files change to the new error contract:
`canon <cmd>: refused — …` and `canon <cmd>: <rule> <subject> — …` become
`canon <cmd>: <CODE> — …`, and the trailing guidance moves to a `hint:`
line (`change-new`, the `evidence-add-*` refusals and overrides,
`finding-add-*`, `gate-check-*`, `init-*`, `migrate-refused-uncommitted`,
`policy-approve-refused-no-policy`, `review-add-*`, `review-gate-*`,
`review-outcome-*`, `review-pin-stale-evidence`, `subject-reopen-*`,
`subject-ship-*`, `subject-status-off-chain-transition`,
`usage-error-unreadable-canon-yaml`). `finding-list-open-json`,
`query-subject-json`, `review-add-batch`, `review-gate-override-recorded`,
`review-outcome-indeterminate-gap`, `subject-adopt-second-subject`,
`subject-reopen-refused-unapproved-brief` and
`subject-ship-refused-unapproved-brief` gain the `meta` object in their
JSON output. No exit code changed.

**New conformance cases:** none (coverage lives in `tests/schema.rs`,
`tests/schema_codes.rs`, `tests/cli_error.rs`, `tests/json_contract.rs`,
`tests/paging.rs` and `tests/pipe.rs`).

**Closed issues:** #53.

**Acceptance (dogfood eval 8, omp, `language: ko`,
`docs/research/2026-10-15-dogfood-eval-8.md`).**

- A results-screen feature on the shipped game: a Korean PRD brief and a
  stop after 94 s with no write outside `.canon/`; after the answer,
  `canon brief approve`, 11 new scenarios (4 failure cases), 83/83
  evidenced and reviewed by another session, gate clean.
- canon calls per new scenario fell from 3.1 (eval 7's feature turns, 37
  calls for 12 scenarios) to 2.1 (23 calls for 11). Every attributable
  canon failure was recovered by the next call. The agent used
  `canon schema` three times to check a command's flags.
- Fixed from the run: a closed stdout panicked canon (`canon … | head`);
  `canon schema "evidence add"` (the path quoted as one word) was refused;
  `evals/dogfood/trace_check.py` counted `canon review add --help` as a
  review and scratch files outside the repo as code.
- Open from the run: #58, a suite-wide `--all-scenarios` batch is refused
  when earlier evidence bound attachments the batch does not repeat.

## 0.19.0 release

Version 0.19.0 makes the brief a PRD that canon can check: a fixed
skeleton with numbered flow steps, `canon brief check` for structure and
prose, an approval bound to the brief's bytes, flow-step coverage in place
of the checklist, a short brief for non-behavior work, and a language for
the brief and the report. Plan:
`docs/superpowers/plans/2026-10-14-canon-0.19.0-brief-as-prd.md`.

**Why.** Dogfood eval 6 (`docs/research/2026-10-14-dogfood-eval-6.md`)
passed its gates, but the brief was two loose sections that mixed intent,
an implementation choice, the scenario plan and the questions; the
change's `tasks.md` shipped 0/7 checked because nothing read it; an
art-pipeline swap with no behavior had no path of its own; a Korean
request got an English report; and an `Approved:` line stayed valid after
the brief under it changed. Research behind the decisions:
`docs/research/2026-10-14-compared-tools-research.md`,
`2026-10-14-non-feature-work-research.md`, `deep-research-report.md`.

### D1, D2: the brief is a PRD, and canon checks it (#44, #45)

- `canon change new` writes Intent → Desired outcome → User flow → Step
  detail → Acceptance → Questions and assumptions → Out of scope. Each
  heading follows a `<!-- canon:section <key> role=<normative|context|question> -->`
  marker, so headings can be in any language. Flow steps start with
  stable ids `F1`, `F2`, … and read "sees → does → becomes"; step-detail
  subheadings follow the same ids in order.
- `canon brief check [--change <slug>] [--json]` is read-only. Blockers
  (`brief.*`): missing, duplicate or out-of-order sections, leftover
  template hints, duplicate flow ids, detail headings that do not match
  the flow, and, once the brief is approved and its subject is building,
  a flow step no scenario covers or one with no failure case. Advisories
  (`prose.*`, normative sections only): evaluative phrases, essay
  connectives, contrast rhetoric, colloquial endings (`ko`), degree
  adverbs, sentence and paragraph length (a flow step is measured clause
  by clause), and a missing `==**key sentence**==`. Exit 0 clean, 2
  blockers, 1 operational error with a string `error.code`.
- A waiver is `<!-- canon:waive <rule-id> <section-key> "<reason>" -->`:
  one rule in one section, reason required, listed in the output.
- Bare `canon brief` prints the brief's state and the next one to three
  steps; it never edits. The ship guard refuses on the brief's blockers;
  `canon status` shows the finding counts.
- A brief without section markers (every brief before 0.19) checks as one
  `brief.legacy` advisory and gates as before.

### D3: the approval is bound to the brief's bytes (#46)

`canon brief approve --change <slug> --by <human> --note "<their words>"`
writes `Approved: <who> <when> — "<words>" (brief sha256:<digest>)` and
records the policy approval in the same call. The digest covers the brief
without its `Approved:` lines. It refuses while the brief has structure
blockers; coverage rules do not apply to approval, since the scope is
approved before its scenarios exist. Once the brief changes, `canon
status`, `canon evidence add`, the ship guard and the reopen guard read it
as not approved (`BRIEF_CHANGED_SINCE_APPROVAL`) and name the re-approval.
An `Approved:` line with no digest stays approved, with a status advisory
to re-approve.

### D4: flow coverage replaces the checklist (#48, closes #43)

`canon scenario new --flow <F-id> [--change <slug>]` tags
`@flow:<change>.<F-id>` and refuses a step the brief does not define.
`canon status` lists each flow step of the subject's current change with
its scenarios and their evidence and review state, and flags a step with
no scenario or no failure case. `canon change new` no longer writes
`tasks.md`; an existing one is still read, and `canon gate task` and
`evidence add --task` are unchanged for imported plans.

### D5: chore changes (#49)

`canon change new --kind chore` writes a brief with Intent, Category
(`refactor|dependency|build|ci|infra`), Invariants, Checks and Out of
scope, marked `<!-- canon:kind chore -->`. A chore adds no scenarios: its
evidence is the subject's suite run again with `evidence add --report …
--all-scenarios`, which stales the pinned reviews and forces a re-review.
The ship guard is the normal one. Work that changes behavior is a feature
change.

### D6: language (#50)

`canon init --language <tag>` writes `language:` to `canon.yaml`.
Templates and prose rules ship for `en` and `ko`; another tag gets `en`
headings and structure rules only. The skill writes the brief and the
report in that language.

### D7–D9: skill, evals, website (#47, #51, #52)

- The skill's loop uses the PRD brief, `brief check`, `brief approve`,
  `--flow` and `--kind chore`; five references (`brief-shape`,
  `brief-clarify`, `brief-harden`, `brief-align`, `brief-polish`) each
  carry scope, what to inspect, NEVER cases, verification and the next
  step. No pass invents policy, and explaining writes nothing.
- `evals/dogfood/trace_check.py` reads omp and Codex session logs and
  fails on a write before the brief stop, a write in a question-only
  turn, or a reviewer session that also wrote code; it reports canon calls
  per verb, first-command validity and recovery depth.
- The compare table's five stale cells are rechecked against their
  sources, the canon row describes 0.19, and long cell notes wrap.

**Migration.** For a 0.18 consumer:

- Run `canon skills install` for the new skill and references.
- Existing briefs keep working as `legacy`; `canon status` advises
  re-approving a digest-less approval with `canon brief approve`.
- `canon change new` writes no `tasks.md`. Scripts that read it should
  treat it as optional.
- `canon policy approve` still exists; `canon brief approve` records the
  policy approval too.
- `canon status --json` (`statusVersion` stays `3`) adds, per subject,
  `brief.findings {blockers, advisories}`, `brief.changed`,
  `brief.legacyApproval` and `flow[]`.

**Changed conformance expectations.** Every case that prints the version
prints 0.19.0. `change-new`, `status-brief-proposed` and
`subject-adopt-second-subject` no longer print a `tasks.md` line.
`status-brief-proposed` gains the `brief check` line and the `findings`
JSON field. `status-gaps`, `status-ship-ready`,
`status-shipped-red-reopen` and `migrate-pointer-brief-and-config` gain
the digest-less approval advisory or `legacyApproval`. Guidance lines in
the evidence, status, subject and policy cases name `canon brief approve`
instead of a hand-written `Approved:` line. No exit code changed; no new
conformance case (coverage lives in `tests/brief_check.rs`,
`tests/brief_approve.rs` and `evals/dogfood/test_trace_check.py`).

**Closed issues:** #43, #44, #45, #46, #47, #48, #49, #50, #51, #52.

**Acceptance (dogfood eval 7, omp, `language: ko`,
`docs/research/2026-10-15-dogfood-eval-7.md`).**

- A character-select feature on the shipped game: a Korean PRD brief
  (F1–F4) and a stop after 99 s with no write outside `.canon/`; after the
  answer, `canon brief approve` wrote a digest-bound approval, every flow
  step got scenarios with a failure case, 72/72 scenarios evidenced and
  reviewed by another session, gate clean.
- Editing one word of the approved brief made `canon status`, `canon
  evidence add` and the reopen guard refuse with
  `BRIEF_CHANGED_SINCE_APPROVAL`.
- A refactor request became a `--kind chore` change: one stop, no new
  scenario, the suite re-bound with `--all-scenarios`, re-reviewed by
  another session, shipped, gate clean.
- Two defects found by the run are fixed in this release: coverage rules
  fired on a new scope before its approval, and flow steps drew sentence
  length advisories for their three clauses.

## 0.18.0 release

Version 0.18.0 cuts the calls an attestation costs and makes the
lifecycle honest: one `evidence add` attests every scenario a test report
covers, one `review add` reviews a batch, a shipped subject can be
reopened inside its approved scope, a change can belong to several
subjects, and the gate stops failing on what is not wrong. Plan:
`docs/superpowers/plans/2026-10-12-canon-0.18.0-fewer-calls.md`.

**Why.** Dogfood runs (FINDINGS-3, FINDINGS-4) spent most of their calls
on bookkeeping: one `evidence add` and one `review add` per scenario, a
refused `faithful` for a task report, a silent pin break the agent found
only at the gate. Follow-up work on a shipped subject had no lawful path
back, a change shared by two subjects could name only one, the gate
refused a summary for `n/a` inside a file name and failed on a lost blob
of a superseded review, a formatter's rewrite read as skill drift, a
fresh repo's report showed no tasks, and the example attested its
scenarios with canon's own tests instead of the game's.

### D1, D2: one report, many scenarios; the write that breaks a pin says so (#27, #33, #26, #24)

- **One report, many scenarios.** `canon evidence add --project-id <root>
  --all-scenarios --report <fmt>:<path> …` writes one record per indexed
  scenario that owns a case in the report (the existing own-leading-id
  rule). Each record binds only its own cases and gets the verdict they
  show (`faithful` all passed, `divergent` one failed, `not-applicable`
  one skipped and none failed), unless `--verdict` claims one for all. An
  unindexed id is named and skipped. All or nothing: a claimed `faithful`
  over a failed case, a fabrication marker, an unapproved brief, or a
  dropped binding writes no record. The records share the one stored
  report blob, and the output lists each scenario with its verdict.
- **Batch review.** `canon review add` takes `--scenario-id` repeated and
  `--scenario-ids-from <file|->` (one id per line, `#` comments): one
  reviewer, pin and set of artifacts, one review per scenario, all or
  nothing. Single-scenario output is unchanged.
- **`--task --report` with no scenario id** binds every case in the
  report; `faithful` needs all of them to pass (it was refused before).
- **`--report swift-testing:<path>`** reads the JSON-lines event stream of
  `swift test --experimental-event-stream-output`: the display name, else
  the function name, carries the scenario id; issues and `testSkipped`
  decide the outcome. `ReportFormat` gains `swift-testing` (schema
  regenerated). A stream captured from a real Swift 6.4 run is checked in
  at `crates/canon-cli/tests/fixtures/swift-testing/`.
- **A write that breaks a pin says so.** When `evidence add` (single or
  batch) supersedes the evidence a current pinned review saw, it prints
  one line per review now stale with the reviewer, the scenario and the
  `canon review add … --pin <sha>` that re-pins it (`HEAD`, or `<sha>`
  with no commit). The decision is the gate's own `review_gap`.

### D3, D4: reopening a shipped subject, and a change in several subjects (#31, #28)

- **Reopen (D3, #31).** `canon subject status <id> --to verifying --reason
  "<why>"` reopens a `shipped` subject for follow-up work. Inside the
  approved scope it needs no human: either the brief of the subject's newest
  adopted change is approved, or there is no brief to answer. Otherwise it
  exits 1 and names the brief to approve, and `--override-reason` does not
  waive that.
  - The new Subject record carries `reopen: { reason, change, actor }`. The
    next status write clears it.
  - `--reason` is required for the reopen and refused on any other
    transition (exit 2).
  - The review guard does not run on a reopen. Shipping again runs every
    guard.
  - `canon status` flags a shipped subject whose ship guard would now fail
    (`gate: red — <n> <class> (first: <subject>)`, `red` in `--json`). It
    uses the same check as the guard, and the subject's only step is the
    reopen command.
- **A change in several subjects (D4, #28).** `Change.subject_id` is now
  `subject_ids`, a list (`Change` schema generation `2`).
  - A record written before 0.18 reads as a one-element list.
  - A second `canon subject adopt <change> --subject <other>` adds the
    subject and prints that the change now belongs to both. It never
    re-points.
  - Every reader joins a subject to its changes from both sides: the gate's
    open-blocker rule, the status guard, `canon status`, the brief and the
    evidence refusal. An open blocker on a shared change therefore gates
    every subject it belongs to.
  - `canon query --kind change` now returns each change's current version
    only. It used to return every version.

### D5: gate correctness (#32, #36)

**Fabrication markers match whole words (#32).** `"would pass"`, `"TBD"`
and `"n/a"` in a `--summary` or `--command-result` are fabrication markers
only where they stand as a whole word. The same letters inside a path, a
file name, a URL or a longer word are not a marker, so dogfood eval 3's
summary `… pack.json/atlas JSON …` is kept (it used to be refused for
`n/a`). A `.` or `-` next to a marker joins it into a longer token only
when a letter, digit, `_`, `/` or `\` follows (`n/a.json`, `x-tbd`);
sentence punctuation does not (`result: n/a.`). Every standalone marker
the earlier tests pinned is still caught.

The refusal now reads like every other gate-red refusal of `evidence add`,
one line per finding, still exit 1 and nothing written:

```
canon evidence add: refused — fabricated-evidence cart.add.01 — summary field contains fabrication marker "n/a": "atlas check: n/a"
```

**Review attachments follow the evidence rule (#36).** `canon gate check`
re-checks the files bound by `canon review add --artifact` the way it
re-checks evidence attachments, and only for current reviews. A current
review is the latest per `(project_id, scenario_id, reviewer)` that has
not gone stale against its scenario. A stale review is superseded (the
gate reads the scenario's newer review, or reports the missing one as
`unreviewed-promotion`), so its files are not checked. That is the review
in FINDINGS-3 T5: `reviewer-2`'s review went stale when the evidence
moved and `reviewer-4` reviewed again, yet a lost blob of `reviewer-2`'s
still failed the gate.

A missing blob whose working-tree file still matches is now an advisory,
not a `stale-evidence` violation:

```
evidence artifacts: 1 bound file(s) not in the artifact store — not failing the gate; run `canon evidence vault` to store them before the working tree changes:
  unstored cart.add.01 reports/cart-badge.png (sha256 9848…, review by `reviewer-2`) is verified against the working tree only
```

`canon evidence vault` stores the files of current reviews, the set the
gate re-checks, as well as evidence files; a superseded review's file is
neither checked nor vaulted. To read the same reviews as the gate, the
vault first refreshes a stale scenario index, as `canon gate check` does.
A missing blob whose working-tree file changed or is gone, and an altered
blob, are still `stale-evidence` naming the review.

### D6: consumer-repo hygiene and reads (#25, #29, #34, #37)

**Paths canon owns (#25).** `canon skills install` and `canon skills
doctor` now end with the globs of every path canon writes, for a
formatter's or linter's ignore list: the projection of each selected
provider first, then every machine-written path under `.canon/`, sorted.
The list comes from canon's own path constants, so a path a future
release adds is listed the day it ships. `.canon/specs/`,
`.canon/policy.yaml` and `.canon/changes/` are left out: people write
them by hand, and linters should still see them. For a repo that
selects Claude:

```
owned-paths: canon writes these; exclude them from formatters and linters:
  .claude/skills/canon/**
  .canon/REPORT.md
  .canon/artifact-store.lock
  .canon/artifacts/**
  .canon/audit/**
  .canon/dashboard-snapshot/**
  .canon/hot.db*
  .canon/ingest/**
  .canon/learn/**
  .canon/ledger/**
  .canon/plugins/**
  .canon/policy.approval.json
  .canon/r2/**
  .canon/scenario-index.lock
  .canon/scripts/canon-gate-pre-commit.sh
  .canon/skills/**
  .canon/strategies/**
  .canon/vocab/**
```

**A formatter's rewrite is not drift (#25).** A projected skill file
whose bytes differ from the bundle but whose content matches after
normalization limited to what formatters do (CRLF, trailing whitespace,
runs of blank lines, the final newline, and in markdown list markers,
`_` emphasis and table-cell padding; a JSON file by value)
is now `formatter-rewritten`, not `stale`. `canon skills check` still
exits 1 on it. `canon skills doctor` names the ignore entry and the
command that restores the file:

```
formatter-rewritten: ./.claude/skills/canon/SKILL.md (claude) — the same content as canon's projection after whitespace and format normalization, so a formatter rewrote it; add `.claude/skills/canon/**` to that formatter's ignore list, then restore it with `canon skills install --providers=claude`
```

Whitespace between tokens, backslashes and fenced code blocks are never
normalized, so `rm -rf a b` against `rm -rfab`, a joined `\`
continuation, or a changed fenced command stays `stale`.

**`canon report` reads the sqlite hot tier (#29).** `canon init` routes
`task`, `handoff`, `session`, `run` and `event` to a sqlite `hot` rung,
and the report read only the git ledger and local parquet, so a fresh
repo's trust matrix and scope status showed `—` for every task column
and the report warned `Not read directly: event, handoff, run, session,
task`. The report now reads the sqlite file: it is local and needs no
credential. Before each DuckDB run it writes the hot tier's rows under a
scratch directory (`CANON_HOT_ROOT`) that a new `stg_hot_records` view
reads, beside `stg_git_records` and `stg_r2_records`. A db file that does
not exist yet is no rows and is never created. A file that is not a
sqlite database fails the report (exit 1) and names the file, instead of
rendering a report without its tasks. `Backend::Sqlite` now counts as
read directly, so no sqlite-routed kind appears under `## Kinds not read
directly`. A hot row whose identical record (same kind and content
digest) is also in the git ledger or `.canon/r2` is dropped from
`stg_records`, so one record in two tiers counts once. Postgres and S3
rungs are unchanged.

**`canon finding list` (#34).** `canon finding list --change-id <id>
[--open] [--json]` prints a change's committed findings at their latest
version, ordered by round and seq. `--open` keeps the findings whose
current disposition is `open`, so a finding closed after it was raised
stays out:

```
round  seq  severity    disposition  reviewer  summary
1      1    blocker     rejected     rev-a     teardown races HMR
1      2    should-fix  open         rev-b     the count was typed by hand
```

`--json` prints `{"change_id", "findings": [{round, seq, severity,
disposition, reviewer, summary, resolution_sha, file_ref}]}`. A staged
finding is not a record yet; the command names how many are staged and
points at `canon gate promote`.

**Skill wording (#37).** A review runs in a session that can run the
checks it signs off. A reviewer that cannot run the tests says so and
does not sign off. A report bound with `--report` is kept in
`.canon/artifacts`, so its working-tree copy may be gitignored.

### D7: the example's real evidence (#30)

`examples/platformer`'s 16 scenarios were attested with
`cargo test -p canon-gate spec_coverage`, which tests canon's gate and not
the game. The example now has its own vitest suite in
`examples/platformer/tests/`, one test per scenario, each named with its
scenario id (`platformer.movement.03: No double jump; …`). Each test drives
the game's own code and checks every Then:

- The engine runs headless: `GameSimulation` with the game's levels, or a
  small grid in the game's level format, stepped one fixed physics step at
  a time.
- The React screens (HUD, pause, game over, level complete, level select)
  mount in jsdom. The keyboard goes through the game's `InputCapture`, and
  frames go through `runFrame`, which is the two lines extracted from
  `PixiStage`'s ticker, so the tests drive frames the same way the game
  does. jsdom has no WebGL, so two things are stubbed: the `PixiStage`
  component wherever a test mounts `<App/>`, and, in `enemy.test.tsx`
  only, pixi.js's `Sprite` class, so that `renderPlayer`'s blink rule
  runs without the renderer.

Writing the tests found one bug. A buffered jump fired one step after
touchdown, but `platformer.movement.03` says it fires on the landing
frame. The game now starts a buffered jump on the step that grounds the
player.

`bun run test:junit` writes `reports/junit.xml`. That path is gitignored,
because the bound copy is kept in `.canon/artifacts`. One call attests
every scenario, the first real use of D1:

```
canon evidence add --project-id platformer --kind test-run \
  --ref "cd examples/platformer && bun run test:junit" --role implementer \
  --report junit:examples/platformer/reports/junit.xml --all-scenarios \
  --session-id 018-n5-impl --actor-id 018-n5-impl --promote
```

It wrote 16 records, each `faithful` from its own passing case. All 16
share one stored report blob. The four old `spec_coverage` records are
superseded by the latest-fold, and their files stay in the ledger because
the ledger is append-only. The new `platformer` job in the Test workflow
runs the suite. The platformer's legacy reviews are now stale, and the
gate's advisory names the platformer scenarios to review again. An
independent re-review clears them.

**Migration.** For a 0.17 consumer:

- Run `canon skills install` to refresh the projected skill (#37's
  wording) and to see the `owned-paths:` list for formatter and linter
  ignores.
- `canon query --kind change` now returns one row per change, its current
  version; it used to return every version.
- `Change` records of schema generation 1 (`subject_id`) are read as a
  one-element `subject_ids`; nothing needs rewriting. New records are
  written at generation `2`.
- `canon status --json` has a `red` field for shipped subjects whose ship
  guard would now fail (`statusVersion` stays `3`).
- `canon report` now reads the sqlite hot tier: a fresh repo's task
  columns fill in, and a `hot` db file that is not sqlite fails the report
  (exit 1).
- `canon skills check` reports a formatter-only rewrite of a projected
  file as `formatter-rewritten` instead of `stale`; it still exits 1.
  Scripts that match `stale` should match both.

**Changed conformance expectations.**

- `version` and every case that prints the version now print 0.18.0.
- `review-pin-stale-evidence` step 3 carries the stale-review line (D2).
- `gate-check-review-artifact-stale-evidence` now changes the
  working-tree file, since a matching one no longer makes the case red
  (D5).
- `evidence-vault-stores-unstored` and `evidence-vault-reports-unstorable`:
  the index-refresh line moves from the later `gate check` step to the
  `evidence vault` step (D5).
- `skills-check-outdated-lock`, `skills-doctor-competing-skill`,
  `skills-doctor-stray-projection` and `init-providers-detected` end with
  the `owned-paths:` list (D6).
- `context-json-policy-summary` shows `Change` schema `2` with
  `subject_ids` and `reopen` (D3, D4).
- `subject-status-off-chain-transition` names `shipped → verifying` as an
  allowed reopen (D3).

No exit code changed.

**New conformance cases:** `evidence-add-all-scenarios`,
`evidence-add-all-scenarios-refused-contradiction`,
`evidence-add-swift-testing`, `review-add-batch`,
`subject-reopen-allowed`, `subject-reopen-refused-unapproved-brief`,
`status-shipped-red-reopen`, `subject-adopt-second-subject`,
`evidence-add-fabrication-marker-whole-token`,
`gate-check-review-artifact-vault-advisory`,
`gate-check-review-artifact-superseded`, `finding-list-open-json`.

**Closed issues:** #24, #25, #26, #27, #28, #29, #30, #31, #32, #33, #34,
#36, #37.

**Acceptance (plan D8).**

- **(a) Eval 5 on omp.** The eval-1 one-line game request ran in a fresh repo initialized with this build, in three turns: the request, one human answer, then a follow-up fix after shipping. All four criteria pass:
  - **One human stop.** The agent stopped once, at the brief (88 s). After one answer it shipped the first version with 28 scenarios; the turn-3 follow-up brought the total to 29.
  - **Follow-up without a human.** The follow-up reopened the shipped subject with `--reason`, went in as a task in the approved change, got a scenario, evidence and an independent review (which found and fixed one weak test), and shipped again with every guard re-run. No human was involved.
  - **Fewer calls than scenarios.** For 29 scenarios there were 4 `evidence add` calls (2 with `--all-scenarios`) and 3 `review add` calls (1 batch). That is 38 canon calls in all, 0 of them bookkeeping; eval 3c (0.16) took 49.
  - **Independent reviews.** Every scenario was reviewed by a separate omp session, and the gate is clean.
- **(b) A Swift Testing event stream binds with `--all-scenarios`.** The fixture is a stream captured from a real `swift test --experimental-event-stream-output` run (Swift 6.4), checked in with its package source.
- **(c) The platformer example attests with its own tests.** One `--all-scenarios` call attested all 16 scenarios, and one batch `review add` re-reviewed them. All 25 stale legacy reviews are now superseded by current ones. The 8 undecidable legacy reviews are pinned to commits that predate their scenarios, so their count cannot change; every one of those scenarios now also has a current review.

Evidence: `~/Workspace/canon-dogfood/PROTOCOL-5.md` and `FINDINGS-5.md`.

## 0.17.0 release

Version 0.17.0 gives canon its own home: the spec corpus, canon's own
changes and every approval live under `.canon/`, external plans are read
and not written (one exception: the evidence-gated `canon gate task`
still flips a task's checkbox in an external `tasks.md`, D4), `canon init`
asks which harnesses get the canon skill, and `canon migrate` moves a 0.16
repo over in one command.

**Why.** The maintainer's rules (2026-10-12): canon keeps what it writes
in `.canon/`; canon reads external plans and never writes into them,
except for that evidence-gated checkbox write-back; canon
asks which harnesses to install for. 0.16 kept the corpus at
`specs/features/kind=feature/…` (where `features` and `kind=feature` say
the same thing), had `canon init` create `openspec/changes/` and
`canon change new` scaffold there, wrote the human's `Approved:` line into
that external `proposal.md`, and picked skill providers from directory
markers, falling back to Claude and Codex without asking. Consumers on
0.16 must move with one command, and ledger joins (scenario ids, change
ids, review pins) must survive the move.

**D1: spec layout.** A spec root now holds `kind=feature/area=<area>/<surface>.feature`
directly. The `features/` directory between the root and `kind=feature/`
is gone; `kind=` stays at the top so other kinds can sit beside it later.

A new repo's default spec root is `.canon/specs` (`id: root`). `canon
init` writes it into `canon.yaml`:

```yaml
specs:
  # The .feature corpus: <root>/kind=feature/area=<area>/<surface>.feature.
  roots:
    - id: root
      root: .canon/specs
```

An absent `specs:` key, or an entry with no `root:`, resolves to
`.canon/specs` too; `specs.roots[]` still takes any path. `.canon/specs`
is authored and git-tracked, and no canon gitignore line covers it. The
walkers never consult `.gitignore` and never skip hidden directories, so
a root inside `.canon/` is read like any other. A fresh repo's first
scenario lands at `.canon/specs/kind=feature/area=cart/add.feature`:

```
canon status: refreshed the scenario index from 1 changed feature file(s) (.canon/specs/kind=feature/area=cart/add.feature): 1 record(s) written
```

canon does not read both layouts. A spec root that holds any `.feature`
file under `<root>/features/` is refused, and every refusal names
`canon migrate`:

- `canon format` reports one `layout-grammar` violation at path
  `features`: the root is on the pre-0.17 layout, canon reads
  `kind=feature/area=<area>/<surface>.feature` directly under the root,
  and `canon migrate` moves `features/kind=feature/` there.
- `canon inventory sync` aborts the root: 0 writes, exit 1.
- A reader's index refresh (`canon status`, `canon gate check`,
  `canon evidence add`, `canon review add`, `canon subject status`)
  names the root on stderr, does not refresh its scenario index, and
  writes no tombstones.
- `canon gate check` exits 1 with `uncovered-cell <root>` (the root's
  scenarios are not graded until it moves), whether or not the index
  already holds them. The ship guard of `canon subject status … shipped`
  refuses with the same violation.
- `canon status` warns, and its only next step is `canon migrate`.
- `canon scenario new` and `canon feature new` exit 2 and write nothing.

An empty leftover `features/` directory is not a refusal. `canon migrate`
is the one idempotent command that moves the layout: `--dry-run` prints
the plan, a root at the old repo-level `specs` default moves to
`.canon/specs`, and any other root only drops its `features/` level.


canon's own corpus moved to `.canon/specs/kind=feature/…` through
`canon migrate` (recording `former_roots: [specs]`, see D6);
`examples/platformer/specs/kind=feature/…` only dropped its `features/`
level.

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

**D5: harness choice.**

- `canon.yaml` gains `skills: { providers: [claude, codex, omp, pi] }` (any
  subset, `[]` for none). `canon init --check-config` validates it; a
  malformed section or an unknown name fails `skills install|check|doctor`.
- `canon init` asks, on a terminal, which harnesses get the canon skill, with
  the detected ones checked (`.claude`, `.codex`, `.omp`, `.pi`
  directories; `.agents` alone no longer selects Codex). `--providers a,b`
  answers without a prompt; `--providers none` records `[]`. With no
  terminal and no flag it takes the detected ones; with none detected it
  installs nothing and names `canon skills install --providers=<list>`. It
  writes the answer to `canon.yaml` and installs the skill from the default
  source, as `canon skills install` does. If that source is missing, the
  choice stays recorded and init exits 2 naming the install to run.
- `canon skills install|check|doctor` follow `skills.providers`. An explicit
  `--providers` on install rewrites the list in an existing `canon.yaml`
  (every other byte kept) and never creates one. Without the key they use
  the install lock's providers, then the detected ones. The Claude+Codex
  fallback is gone.
- `canon skills doctor` adds a `stray:` advisory for a canon projection
  (`.claude/skills/canon`, `.agents/skills/canon`, `.omp/skills/canon`,
  `.pi/skills/canon`) whose provider is not selected, with the `rm -r` that
  removes it and the install command that selects it. `check` does not fail
  on it. `check` and `doctor` name where the providers came from.

**D6: `canon migrate`.** `canon migrate [--dry-run] [--repo <dir>]` moves a repo that canon 0.16 or
earlier set up onto the 0.17 layout in one idempotent command. It prints
the plan and then applies it. `--dry-run` stops after the plan, and a
second run finds nothing to move.

- **Spec roots.** A root's `features/kind=feature/` moves to
  `<root>/kind=feature/`, with `git mv` when the files are tracked. A root
  at the repo-level `specs` moves to `.canon/specs` from either layout.
  That covers 0.16's default, which is also what an absent `specs:` key or
  an entry without `root:` meant then. Every other root stays put and only
  drops its `features/` level. Emptied directories are removed, and any
  other file left in them is named.
- **`former_roots`.** A root that moved records where it was:
  `specs.roots[].former_roots: [specs]`. Reviews written before 0.15 are
  checked against the commit they pinned, and at a commit from before the
  move `.canon/specs` holds nothing. The legacy lookup therefore searches
  the root's directory now, then each former root. A former root belongs
  to its project id, so the lookup never matches another root's scenario
  with the same id. On canon's own repo, without the record, 329 of 337
  legacy reviews would turn undecidable.
- **Changes `canon change new` wrote under openspec** move to
  `.canon/changes/<slug>/`, with `proposal.md` renamed `brief.md` and
  `tasks.md` alongside. Then they are re-imported from canon's own source
  under the same change id, so task ids (`<slug>#<n>`) and the adoption
  keep joining. A change counts as canon's only when both signals hold:
  its files still open with that command's 0.14–0.16 scaffold (through
  the `## What Changes` and row-grammar comments), and its first stored
  version is already adopted, because `change new` adopts before any
  ingest. A change with one signal stays, named with the signal it lacks.
  0.16 recorded no durable provenance, so this is the strongest evidence
  there is. A false positive keeps its content and ids, is printed in the
  plan, and is undone with one `git mv`; `--dry-run` shows exactly which
  changes move. Every other change stays as an external plan. `canon init`'s
  `{ dialect: openspec, root: . }` source, and its
  `openspec/changes/.gitkeep`, go only once nothing is left under it: no
  change that stays, no directory the openspec reader cannot read (each is
  named with the reader's diagnostic), and no other file in
  `openspec/changes/`.
- **Approvals.** An external `proposal.md` that holds the `Approved:` line
  0.16 recorded gets a canon pointer brief,
  `.canon/changes/<change>/brief.md`, holding `Source: <proposal>` and that
  line. The proposal is not edited, and the output names the leftover line.
- **Skills, .gitignore, index.** `skills.providers` is written from the
  install lock when `canon.yaml` has no `skills:` key. The missing canon
  `.gitignore` lines are added, and a scenario index still tracked in git
  is named. The index is rebuilt last with a full `canon inventory sync`.
- **canon.yaml.** Every edit is textual, as `canon skills install` edits
  `skills:`: comments and CRLF are kept. A result that does not parse to
  exactly the intended change is refused, with the edit to make by hand.
- **Refusals** (exit 2, nothing written): uncommitted changes under a path
  it moves (untracked files included), a destination that already exists,
  a change directory that could stop halfway (a missing `proposal.md` or
  `tasks.md`, a `brief.md` in the way, or a directory that is not
  writable), or a `canon.yaml` edit it cannot make safely.
- **Resumable.** `canon.yaml` is written whole or not at all (a temp file,
  then a rename). A run that stops partway is finished by the next one: a
  spec root already at `.canon/specs` while `canon.yaml` still names
  `specs` gets its `root:` and `former_roots` edit, a change left at
  `.canon/changes/<id>/proposal.md` gets its rename to `brief.md`, a moved
  change that was not re-imported is re-imported, and a change whose
  `proposal.md` → `brief.md` rename fails is moved back. The openspec
  source is removed only after every pointer brief is written, so a failed
  brief write leaves the approval readable for the rerun.

**`canon init` on an existing repo (#35).** A rerun also adds the canon
`.gitignore` lines the repo lacks: hot db, audit log, scenario index,
index lock and artifact-store lock. It prints `canon init: added to
.gitignore: …`, and its first line now reads `… left unchanged; the
AGENTS.md block and the canon .gitignore lines are refreshed`. With
`--no-agents-md` it adds only the lines. It still refuses (exit 2) when
nothing is missing.

**Fix: an old scenario index no longer turns legacy reviews
undecidable.** The readers' stale-index check compared only each
scenario's `source_digest`. A generation written before 0.15 carries no
`scenario_digest`, so for a file unchanged since, the refresh kept it as
the latest record. The legacy check then had no digest to compare and
read the review as undecidable. Repos that tracked the index before 0.16
keep such files. On canon's own repo the count was 225 current / 17 stale
/ 95 undecidable instead of 307 / 22 / 8. The check now also compares
`scenario_digest`, as `canon inventory sync` always did, so the first
reader rewrites those generations. `canon review add` on such a scenario
now pins at scenario scope, as it does on a fresh index.

**Migration.** For a repo on canon 0.16:

1. Install canon 0.17.0.
2. Run `canon migrate --dry-run` and read the plan: the spec moves, the
   changes it re-homes, the pointer briefs, the `canon.yaml` and
   `.gitignore` edits. Commit anything uncommitted under a path it moves
   first; it refuses otherwise.
3. Run `canon migrate`. A second run finds nothing to move.
4. Commit the result.
5. Run `canon skills check` (and `canon skills doctor` for `stray:`
   projections of unselected providers).

What `canon migrate` leaves alone, and names in its output: external plan
changes (they stay as external sources, and an external `proposal.md`
keeps its now-unread `Approved:` line), a change carrying only one of the
two canon signals, other files left in an emptied spec directory, and a
scenario index still tracked in git. Other upgrade notes:

- An `Approved:` line in an openspec `proposal.md` is no longer read; it
  counts only in `.canon/changes/<change>/brief.md` (migrate writes that
  pointer brief for each approved proposal it finds). Until then
  `canon status` lists the human step for that subject.
- Repos that configured `{ dialect: openspec, root: . }` keep it as an
  external source; new changes go to `.canon/changes/`.
- `canon status --json` has `statusVersion` 3: scripts read
  `brief.brief`, `brief.briefExists` and `brief.source` instead of
  `brief.proposal`.
- Scripts that match `canon change new` output see the
  `.canon/changes/<slug>/brief.md` and `tasks.md` paths; scripts that
  parse `canon init` output see the skill install result (or the
  no-harness hint) as its last line instead of `next: canon skills install`.
- A repo with an install lock keeps its providers (migrate records them as
  `skills.providers`). A repo with no lock and only an `.agents` directory
  no longer gets Codex by default; pass `--providers`.
- Rerunning `canon init` on an existing repo now adds the canon
  `.gitignore` lines it lacks (#35, above).

**Changed conformance expectations.** `version` and every case that
prints the version now print 0.17.0. **Conformance.** New cases: `migrate-refused-uncommitted` and
`migrate-pointer-brief-and-config`. 39 cases were re-blessed, every diff
for one of three causes:
- Their fixtures carry hand-made pre-0.15 index records, so their first
  reader prints one `refreshed the scenario index from N changed feature
  file(s) …` line.
- `canon review add` prints `pinned … at scenario digest <x>` instead of
  `source digest`, and the record's `pins` gain `scenario_digest_scope:
  scenario`.
- `init-scaffold-and-rerun` shows the new rerun line.

No exit code changed.

**Closed issues:** #35, #13.

**Acceptance (plan D7).**

- **(a) canon migrates itself.** `canon migrate` moved canon's own corpus to `.canon/specs/kind=feature/` (31 files, `git mv`). It recorded `former_roots: [specs]` and left the 51 human-written openspec changes in place as an external plan. `canon gate check` and `canon status` printed byte-identical output before and after: clean, with 307 current, 22 stale and 8 undecidable legacy reviews. A second run is a no-op.
- **(b) Throwaway copies of Kino and Lifeshot migrate the same way.** The originals were not touched. Gate output was byte-identical before and after: Kino moved 42 feature files, Lifeshot 25. Lifeshot's status differs in three lines, which are the 0.17 brief wording, not the move.
- **(c) Eval 4 on Codex (#13).** The eval-1 one-line game request ran in a fresh repo initialized with `--providers codex`. The agent stopped once, at a brief in `.canon/changes/`. After one answer it recorded the approval there, shipped, and handled the follow-up with no human. Specs landed in `.canon/specs/kind=feature/`, no `openspec/` directory was created, and only `.agents/skills/canon` was projected. The gate was clean.
- **What the run also showed.** Codex has no subagents, so its "independent" reviews were written by the implementing session under a new label (#39, tracked with #14), and its evidence was a `curl` check. Canon cannot tell either from a real review or a real test yet.
- **A false start.** The first Codex run was void: Codex's login shell put the installed 0.16 binary ahead of the 0.17 build. The evidence is in `~/Workspace/canon-dogfood/PROTOCOL-4.md` and `FINDINGS-4.md`.

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
  `canon subject status <id> --to shipped` refuses only where a brief exists:
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
  `divergence add` take `--promote` and `--no-promote`. With
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
  lists the problems; `canon subject status <id> --to shipped` refuses with
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
  tasks, and `canon subject status <id> --to shipped` refuses. Subjects with no
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
``— run `canon gate promote` to commit it``; `divergence add` names
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
scenarios specify no failure path, and `canon subject status <id> --to shipped`
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
