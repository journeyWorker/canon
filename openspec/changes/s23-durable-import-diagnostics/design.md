# Design — s23 durable import diagnostics (watermark-on-failure)

## Current state (accurate baseline, verified)

- **The watermark gate and the cursor-advance decision are two SEPARATE
  checks, computed at two different points in `plans.rs`'s per-source
  loop.** The gate (`plans.rs:311-312`) reads: has this source's present
  file set already been fully digested and recorded? If yes
  (`source_unchanged`), skip the whole parse/persist pass and report
  `skipped_unchanged: true` (`plans.rs:314-330`) — nothing runs, nothing is
  re-warned. The advance decision (`plans.rs:377-388`) runs only when the
  gate did NOT skip: after a real parse + persist attempt, `fully_durable =
  changes_unwritten == 0 && tasks_unwritten == 0` (`plans.rs:380`) decides
  whether THIS pass's file set gets written as the new cursor for the NEXT
  run's gate to compare against.
- **`fully_durable` answers "did every routable candidate this pass tried
  to persist actually land," not "did this pass produce anything worth
  remembering."** For the s18 B1 near-miss (`root:` one level above
  `openspec/changes`), the openspec adapter's `parse` (`plan_adapters/
  openspec.rs`) finds zero real change dirs and one `malformed` entry
  before any `Change`/`Task` is ever constructed — `changes`/`tasks` coming
  back from `parse` are both empty, so the persist loop (`plans.rs:356-375`)
  never runs, `changes_unwritten`/`tasks_unwritten` stay `0` by construction
  (there was nothing to attempt), and `fully_durable` is vacuously `true`.
  Nothing about `fully_durable`'s definition distinguishes "there was
  nothing to persist because the source is a legitimately fresh, empty
  plan tree" from "there was nothing to persist because the source is
  malformed and found nothing usable."
- **s18 already computes the exact fact that's missing from the cursor
  decision, one check later, for a different purpose.**
  `plans.rs:400`: `if !malformed.is_empty() && changes_persisted == 0 &&
  tasks_persisted == 0` — this is precisely "a wholly-unproductive
  malformed pass," and it already drives `outcome.non_clean_sources`
  (the unconditional stderr WARN + non-zero exit, s18's
  `loud-plan-import-diagnostics` capability). It is computed AFTER the
  cursor-advance decision (`plans.rs:377-388` runs before `plans.rs:400`)
  and is never consulted by it — the two checks are independently
  maintained, currently happen to describe the same underlying event, and
  have no mechanism keeping them in sync if either is edited later.
- **A written cursor is unconditionally honored by the NEXT run's gate,
  with no distinction for "this cursor came from a malformed pass."**
  `SourceCursor` (`cursor.rs:104-129`) carries only `{source_id,
  last_seen_at, last_seen_digest, files: BTreeMap<PathBuf, FileSeen>}` — no
  status field, no notion of "this snapshot was malformed." `source_unchanged`
  (`cursor.rs`'s per-source predicate) is a pure content-equality check: the
  present file set's digests match the stored index, full stop. It has no
  way to know, and no reason to ask, whether the PASS that produced the
  stored index was itself a success.
- **The reproduced failure sequence, run-by-run (Developer's `reviews/
  developer.json`, `canon ingest plans --repo . --json` against the
  unfixed `root: openspec` config, s18 binary):** run #1 — `malformed: 1`,
  `changes_persisted: 0`, `tasks_persisted: 0`, s18's WARN fires, `exit 1`,
  and (the bug) a cursor IS written keyed off `openspec/`'s own file set
  (which includes the single `changes/` malformed entry, whatever else
  lives under `openspec/` unrelated to the plan tree, and does not change
  between runs). Run #2/#3 — `present_digests` for `openspec/` is
  byte-identical to what run #1 stored, `source_unchanged` returns `true`,
  the ENTIRE parse is skipped, `skipped_unchanged: true`,
  `outcome.non_clean_sources` is never populated (that computation sits
  inside the branch the gate just skipped), `exit 0`.

## The decision

**Two candidate shapes for "stay loud on every run against an unchanged,
wholly-malformed source," both preserving idempotence for the legitimate
unchanged-CLEAN case:**

- **(a) Non-advance: a malformed-nonzero, zero-persisted pass does not write
  a cursor for its source at all.** The gate has nothing to compare against
  next run (or compares against a stale prior-GOOD cursor whose file set the
  now-broken config no longer matches either way), so `source_unchanged` is
  `false`, the full parse + persist + s18 diagnostic path re-runs from
  scratch on every subsequent invocation, unconditionally, until the
  underlying files change (the config gets fixed) — at which point a clean
  pass finally earns a cursor and the source goes quiet again.
- **(b) Advance-with-marker: write the cursor as today (so `source_unchanged`
  still gates the SCAN), but persist an additional `last_result: Malformed`
  (or similar) field on `SourceCursor`, and have the gate check it: even
  when the file set is unchanged, re-emit the WARN + non-zero exit (skipping
  only the actual re-parse/re-scan work) whenever the stored marker says
  the last pass was malformed.

**Decision: (a) is accepted.**

### Why (a), not (b)

- **(a) needs zero new persisted state; (b) needs a new field whose own
  staleness becomes a second thing to get right.** `SourceCursor`
  (`cursor.rs:104-129`) is deliberately minimal — S3 3.1's `{source_id,
  last_seen_at, last_seen_digest}` plus the per-file soundness index, no
  status enum anywhere in the type today. (b) adds a field whose value must
  itself be correctly invalidated the moment the source becomes clean again
  (a config fix that makes the NEXT pass fully durable must flip
  `last_result` back, or a since-fixed source would keep re-warning
  forever, the mirror-image bug of the one this change closes) — a second
  piece of mutable state to keep synchronized with reality, for a fact (a),
  the ABSENCE of a cursor, already encodes for free.
- **"Malformed evidence is no evidence."** A `malformed`-only,
  zero-persisted pass produced NO durable record — no `Change`, no `Task`
  reached any tier. There is nothing about this source's plan corpus that
  is actually "known" yet. Treating it as un-ingested (no cursor) is a more
  literal match for what the watermark cursor's own module doc says it
  means — "an already-ingested-through-canon-store" high-water mark — than
  advancing a cursor over a pass that stored zero canon-store rows.
- **(a) reuses the EXACT boolean s18 already computes for the WARN,
  collapsing two independently-maintained checks into one.** Both this
  change's cursor-eligibility exclusion and s18's `non_clean_sources` flag
  need "did this pass have `malformed > 0` and persist nothing" — (a) lets
  both read from one shared local computed once, per source, in the loop
  (proposal.md's Impact); (b) would need the SAME fact read at write-cursor
  time (to set the marker) AND at read-gate time (to decide whether to
  re-check it even when unchanged) AND at clear-marker time (when a
  now-clean pass supersedes it) — three call sites needing to agree instead
  of one.
- **(a)'s failure mode on a bug is fail-LOUD (more re-scanning than
  necessary); (b)'s failure mode on a bug is fail-SILENT (right back to the
  original hazard).** If a future edit to the cursor-eligibility condition
  in (a) is wrong in the "too conservative" direction, the worst outcome is
  an unnecessary re-parse of an already-fine source — wasted work, never a
  swallowed diagnostic. If (b)'s marker-clearing logic has a bug (the
  marker fails to clear on a genuinely fixed source, or fails to SET on a
  malformed one), the failure mode is silently going back to "loud once,
  quiet forever" — the exact defect this change exists to close, now
  hidden one layer deeper inside a marker instead of the cursor's mere
  presence. (a)'s correctness rests on ONE already-existing, already-tested
  predicate (`source_unchanged`); (b)'s rests on a NEW predicate this change
  would have to introduce and separately prove correct.
- **(a) costs strictly more re-work only on the exact input this change
  targets: an unchanged, still-broken source.** For every OTHER case — a
  clean source, a since-fixed source, a partially-successful source — (a)
  and (b) behave identically (cursor advances, gate skips next run). The
  re-scan cost (a) pays on a still-broken source is bounded by that
  source's own file count (typically small — a misconfigured `root:`
  pointing one level too high finds exactly the same handful of dirs every
  run) and is the CORRECT cost for "this operator has not fixed their
  config yet, and canon should keep telling them so on every CI run until
  they do" — re-scanning is the point, not overhead to be optimized away.

### Preserving idempotence for the legitimate unchanged-clean case

- **A clean pass (`malformed.is_empty()`, whether because everything
  persisted or because the source is genuinely, freshly empty) is
  UNAFFECTED — it still satisfies `fully_durable`'s existing
  `changes_unwritten == 0 && tasks_unwritten == 0` AND the new exclusion is
  false (`malformed.is_empty()` is true, so `!malformed.is_empty() ==
  false`), so the whole `changes_unwritten == 0 && tasks_unwritten == 0 &&
  !(has_malformed_zero_persisted)` conjunction is `true` exactly as
  `fully_durable` alone was before this change.** Its cursor is written
  precisely as today; the next run against unchanged files hits
  `source_unchanged == true`, skips the whole parse, and stays a silent,
  zero-cost `exit 0` no-op — S3 §3's `--watch` win (the entire reason the
  watermark cursor exists) is preserved byte-for-byte for the case it was
  built for.
- **A partially-malformed-but-some-persisted pass is likewise unaffected**
  (`changes_persisted > 0 || tasks_persisted > 0` makes the new exclusion's
  condition false regardless of `malformed`'s count) — it advances its
  cursor and goes quiet on repeat runs exactly as today, matching s18's own
  "a source with some malformed dirs but at least one persisted record
  stays clean" scenario, which this change does not touch.
- **Once an operator fixes the broken `root:`, the very next run is a
  normal fully-durable pass** (real change dirs now parse, `malformed`
  drops to its true count for whatever remains genuinely broken — likely
  zero) — that pass's file set differs from whatever the LAST written
  cursor held (there was none, in the all-malformed case, or a stale one
  from before the misconfiguration in the config-regressed case), so
  `source_unchanged` is naturally `false`, the fixed pass runs, persists
  real records, and (being fully durable with `malformed.is_empty()` or
  `persisted > 0`) finally writes a cursor — no manual cursor-reset step,
  no operator-visible ceremony, the SAME "the files changed, so the gate
  naturally re-opens" mechanism the watermark already uses for every other
  transition.

## Risks

- **R1 — a persistently-broken malformed source now re-parses on EVERY
  `canon ingest plans` invocation, forever, until fixed — a real, ongoing
  cost, not a one-time re-check.** Accepted, deliberately: this is the
  entire point (see "Why (a)" above) — a `canon ingest plans` CI job
  running against a misconfigured `root:` is SUPPOSED to keep failing every
  run until someone fixes the config; a canon repo with dozens of
  chronically-misconfigured `plans:` sources sitting unfixed for a long
  time would pay a real, repeated re-parse cost, but that repo already has
  a WORSE, silent correctness problem (plan evidence it believes is being
  ingested, isn't) that this change is explicitly trading re-parse CPU
  cycles to surface loudly instead of hiding.
- **R2 — the shared boolean this change factors out
  (`malformed_nonzero_zero_persisted`, computed once per source and read by
  both the cursor-eligibility check and `non_clean_sources`) must be
  computed from the SAME `malformed`/`changes_persisted`/`tasks_persisted`
  values s18's existing check already uses, not a re-derived copy — a
  divergent second computation would reopen exactly the "two checks that
  happen to agree today" risk this change closes.** Mitigated: proposal.md's
  Impact and the task list require ONE local computed once per source-loop
  iteration, read by both sites, never two independently-maintained
  expressions.
- **R3 — a source whose PRIOR run was clean (cursor written) and whose
  NEXT run regresses to malformed-zero-persisted (e.g. someone deletes the
  real `proposal.md` files but leaves the directory names) does not get a
  FRESH cursor written (by this change's own design), but the STALE clean
  cursor from before the regression remains on disk.** Accepted, not a
  regression: `source_unchanged` compares the CURRENT present file set
  against that stale cursor's stored digests — the regression necessarily
  changed at least one file's bytes (the `proposal.md` that got deleted is
  now simply absent from `present_digests`, a set-membership change
  `source_unchanged` already detects), so the gate correctly reports
  `unchanged == false`, the pass re-runs, finds the regression, and (being
  malformed-zero-persisted) again withholds the cursor — the stale cursor
  is inert dead weight on disk, never consulted successfully again until a
  future clean pass overwrites it.

## Sequencing

Single-crate (`canon-cli`), single-function change (`plans.rs`'s per-source
loop) — no phased rollout. The cursor-eligibility exclusion and its shared
boolean land together in one commit; there is no meaningful subset that
ships independently.
