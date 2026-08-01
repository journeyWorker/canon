# s43 — findings-are-records

## Why

v0.4.0 shipped after **eleven code-review rounds**. Every finding, every
round, and every "this defect was introduced by the previous round's fix"
relationship existed only in agent transcripts. Canon recorded none of it.

Then I hand-wrote the release summary: *"49 real issues, four of them
defects in the previous round's fix."* Both numbers are wrong. The true
fix-of-fix count is **2** — rounds 10 and 11, whose findings were defects
in the prior round's fix. Rounds 8 and 9 found defects in the ORIGINAL
feature, which is what review is normally for; round 9's own commit title
says `s42 itself introduced` and the summary still filed it under
fix-of-fix. The `49` was a tally carried across prompts that does not
reconcile: it was quoted as "rounds 1–7 = 33" and later as
"rounds 1–8 = 38", but 33 + round 8's 10 findings is 43.

That claim is now in a published git tag. Nothing caught it. Canon gates
`EvidenceRecord`s and task checkboxes; a narrative claim in a commit, tag,
or release note is ungated prose, and prose drifts.

The gap is not a missing gate. It is a missing **record**. Canon already
classifies these findings: `crates/canon-ingest/src/artifact_adapter.rs`
carries `ArtifactEventKind::CodeReviewFinding` as a transient
normalization enum on the way to a verdict. Canon recognises a code-review
finding, uses it, and then has nowhere to put it — so thirty of them from
this release evaporated, and the only surviving account of them was a
number I typed from memory.

This is the same shape s42 closed for evidence, one layer up: canon builds
the mechanism, then stops one step short of the seam that would make it
usable on itself. s42's `675 checked boxes / 1 EvidenceRecord` is this
release's `11 rounds / 0 findings`.

## What Changes

**A finding becomes a record.** `RecordKind::Finding`, canon's fourteenth
kind, carries what a review round actually produces: the reviewed commit,
the round, severity, disposition, the reviewer, the finding text, the
commit that resolved it, and — when it can be SOURCED — the commit that
introduced it.

**Fix-of-fix is derived, never typed.** A finding is a fix-of-fix when its
`introduced_by` equals some earlier finding's `resolution_sha`. Nobody
labels it; the join computes it. That is precisely the number this change
exists because I got wrong, so it must stop being a number anyone types.

**The counts reach the report.** A `mart_review_rounds` and its panel put
rounds, findings by severity and disposition, and the derived fix-of-fix
count into `.canon/REPORT.md` — which is generated-never-edited and
already drift-gated by `canon report --check`. A release note written by
reading a generated table cannot drift from it the way one written from
memory did.

**`canon finding add`** authors one, with the same refusal discipline
`canon evidence add` gained in s42: no line break reaches a document, and
a claim is refused before anything is staged.

**v0.4.0's own rounds are backfilled** from the review artifacts that
recorded them, so the corrected numbers in this repo are derived from
records rather than asserted in prose a second time.

## What This Change Deliberately Does NOT Do

- **No inferred `introduced_by`.** A finding whose introducing commit
  cannot be sourced carries `None`. The derived fix-of-fix count is
  therefore a documented FLOOR, and the panel says so. Guessing from
  timing or commit adjacency would manufacture exactly the kind of number
  this change exists to stop manufacturing.
- **No retroactive findings for review rounds whose findings were not
  captured.** Backfill covers v0.4.0's rounds because their findings
  survive in review artifacts with severity, file refs, and bodies —
  transcription, not fabrication. Earlier rounds stay unrecorded, the same
  posture s42 took toward the 675 unevidenced flips.
- **No `finding_id` join-spine key.** The natural key is the composite
  `{change_id}__{round}__{seq}`. A finding belongs to the change under
  review, which canon already models, so it joins on the existing spine;
  `introduced_by` points at a SHA, not at another finding, so a tenth
  spine key would cost `ids.rs`, `join_spine_doc.rs`, and a regen for a
  join nothing performs.
- **A review round does not require a commit.** `reviewed_sha` is
  optional, because most agent review rounds read a working tree that is
  never committed — round 8 of this release did exactly that, and a
  required sha would have silently excluded it from the backfill this
  change exists to produce. Absent means the reviewed state was
  uncommitted, not unknown.
- **Canon still does not route or schedule.** Unchanged from s42.

## Impact

- `crates/canon-model`: `envelope.rs`, `records.rs`, `schema_export.rs`,
  `fixtures.rs`, a new `finding.json` fixture, and the fourteen-kind
  regeneration.
- `crates/canon-store`: `partition.rs`, `sql/views.sql`.
- `crates/canon-report`: a new mart, panel, snapshot table, and the
  dashboard twin.
- `crates/canon-cli`: a new finding module, `main.rs`.
- `canon.yaml`: routing for the new kind.
- **Behavior change, intentional:** `.canon/REPORT.md` gains a panel, so
  `canon report --check` fails against a stale committed copy until it is
  regenerated.
