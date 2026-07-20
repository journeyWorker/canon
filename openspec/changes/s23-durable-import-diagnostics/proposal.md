## Why

The round-2 usability re-review (`target/usage-review/eno-drift/SYNTHESIS-ROUND2.md`,
finding #2, "important") verified live that s18's B1 fix — making a
wholly-unproductive `canon ingest plans` pass fail loud (`loud-plan-import-diagnostics`,
"A malformed-nonzero, zero-persisted source makes canon ingest plans non-clean
at the process level") — is **incomplete**: it is loud on run #1 only.
Developer's round-2 evidence (`reviews/developer.json`) reproduced the exact
s18 near-miss (`plans.sources[].root: openspec`, one level above
`openspec/changes`) and ran `canon ingest plans` three times in a row against
the SAME unfixed config:

- Run #1: named `missing-proposal-md` malformed entry + root-near-miss hint +
  unconditional stderr WARN + `exit 1` — s18's fix working exactly as spec'd.
- Run #2 and run #3: `<dialect> (<root>): skipped unchanged (watermark)` on
  stdout, **no WARN, `exit 0`** — identical to the pre-s18 silent-failure
  the fix was written to close.

**Root cause, grounded in the shipped code (`crates/canon-cli/src/plans.rs`):**

- The per-source watermark gate (module doc "Watermark cursor", S3 §3
  generalized) skips a source's ENTIRE scan/parse/persist pass whenever its
  present file set is byte-identical to what `CursorStore` has on record
  (`plans.rs:311-312`, `cursor.source_unchanged(&present_digests)`) — this is
  correct and load-bearing for the legitimate case (an unchanged, ALREADY
  successfully-imported source should stay a silent no-op).
- What decides whether a source's cursor gets WRITTEN in the first place is
  `fully_durable` (`plans.rs:380`): `changes_unwritten == 0 && tasks_unwritten
  == 0` — this correctly blocks the cursor when the routed tier is
  unreachable (the `unwritten` seam), but says nothing about whether the pass
  produced any USABLE evidence at all. A source whose `root:` points one
  level too high parses zero real change dirs, finds one `malformed` entry,
  and persists nothing — `changes_unwritten` and `tasks_unwritten` are both
  `0` (there was nothing to attempt persisting in the first place), so
  `fully_durable` is `true`, and `plans.rs:381-388` writes a fresh cursor
  keyed off the malformed config's OWN file set (the misconfigured `root:`
  directory's one `changes/` entry) regardless.
- The SAME condition that already makes this pass non-clean at the process
  level — `!malformed.is_empty() && changes_persisted == 0 && tasks_persisted
  == 0` (`plans.rs:400`, feeding `outcome.non_clean_sources` and s18's
  unconditional stderr WARN + non-zero exit) — is never consulted when
  deciding whether to advance the cursor. The two checks compute the
  identical fact (a wholly-unproductive malformed pass) and disagree on what
  to do about it: one flags it loud, the other silently commits to never
  looking at it again.
- Once that cursor is written, `source_unchanged` (`cursor.rs` §"Cursor shape
  + a sound gate") is, correctly, unconditionally sound for its OWN stated
  contract — "a whole adapter source's parse is skipped IFF its entire
  present file set is byte-identical to the cursor." The misconfigured
  `root:`'s file set genuinely IS unchanged run #2 onward (nobody edited
  the broken config between runs), so the gate is doing exactly what it was
  built to do — it was simply handed a watermark it should never have been
  given, from a pass that proved nothing.

**Why this matters, same framing s18 used for the original B1:** a
`canon ingest plans && deploy.sh` CI job that misconfigures `root:` fails its
FIRST run (a human notices) but goes green from the SECOND run onward with
zero code change — a config typo becomes permanently invisible to CI the
moment it survives one look. This is the identical "loud once, silent
forever after" hazard s18 was funded to close; the fix landed one layer
above the actual persistence boundary.

## What Changes

- **A source whose pass is malformed-nonzero and zero-persisted does NOT
  advance its watermark cursor.** `plans.rs`'s cursor-advance decision grows
  the identical condition its own non-clean-source flag already computes:
  cursor-eligible becomes `changes_unwritten == 0 && tasks_unwritten == 0 &&
  !(malformed.is_empty() == false && changes_persisted == 0 && tasks_persisted
  == 0)` — i.e. `fully_durable` stays the unwritten-free gate it already is,
  AND the pass must have persisted something OR found zero malformed
  constructs. No new field, no new cursor shape: `SourceCursor` and
  `CursorStore` are untouched; this is purely which passes are allowed to
  reach `pending_cursors.push(..)`.
- **Effect: every re-run against an unchanged, wholly-malformed source
  re-scans, re-parses, re-warns, and re-exits non-zero — forever, until the
  config changes.** Because no cursor is ever written for such a source,
  `cursor.as_ref().is_some_and(|c| c.source_unchanged(..))` (`plans.rs:312`)
  has nothing to compare against (or compares against a STALE prior-good
  cursor whose file set the now-broken config no longer matches either way),
  so `unchanged` is `false` on every subsequent run and the full parse +
  s18 loud-diagnostic path (named malformed entry, root-near-miss hint,
  unconditional stderr WARN, non-zero exit) reproduces byte-for-byte on
  run #2, #3, and every run after — not just run #1.
- **The legitimate unchanged-clean case is untouched.** A source with
  `malformed.is_empty()` (zero malformed constructs, whether because it
  persisted real records or because it is genuinely, cleanly empty) keeps
  advancing its cursor exactly as today; a clean re-run against such a
  source still hits `unchanged == true` and stays a silent, zero-cost,
  `exit 0` no-op — the watermark's whole reason to exist (S3 §3's `--watch`
  win) is preserved for every pass that isn't the malformed-and-fruitless
  one.
- **A partially-malformed-but-some-persisted pass is untouched.** A source
  with `malformed > 0` AND `changes_persisted > 0` or `tasks_persisted > 0`
  (s18's own "partial success is not the targeted near-miss" scenario)
  still advances its cursor on a fully-durable pass, exactly as today — this
  change narrows the near-miss it targets to the SAME zero-persisted
  condition s18's WARN already targets, never widens it to "any malformed
  entry blocks the cursor."
- **design.md decides between two shapes for "stay loud" and picks
  cursor-non-advance (never advance the watermark for a wholly-unproductive
  malformed pass) over the alternative of advancing the cursor with an
  embedded `last_result: malformed` marker that forces a re-warn on an
  otherwise-skipped pass** — see design.md's Decisions for the full
  trade-off; the chosen shape needs zero new persisted state and zero new
  code path through the `unchanged`-skip branch.

### Added Capabilities

- `durable-import-diagnostics`: the watermark cursor's advance decision for
  `canon ingest plans` is extended so a wholly-unproductive
  (`malformed > 0 && changes_persisted == 0 && tasks_persisted == 0`) pass
  never gets marked "seen" — every subsequent run against an unchanged,
  still-broken source re-attempts the scan and re-emits s18's loud
  diagnostics (named malformed entries, root-near-miss hint, unconditional
  stderr WARN, non-zero exit), on every run, not only the first. A
  legitimately clean or partially-successful pass's watermark behavior is
  byte-identical to today.

### Explicit non-goals

- **No change to the `unchanged`/`skipped_unchanged` skip PATH itself.**
  `plans.rs:311-330`'s "read every present file, digest it, compare against
  the stored cursor, skip the whole parse if identical" logic, and
  `cursor.rs`'s `SourceCursor`/`CursorStore`/`source_unchanged` shape, are
  untouched. This change is exclusively about WHICH passes are eligible to
  WRITE a fresh cursor, never about how a written cursor is later read.
- **No new `SourceCursor` field, no `last_result`/`last_status` marker
  persisted anywhere.** design.md's Decisions record why: the chosen fix
  needs no new state to persist a malformed-source's "I saw this and it was
  broken" fact — the ABSENCE of an advanced cursor already, correctly,
  forces a re-scan next time, with no risk of that marker itself going
  stale or being read by a code path that forgets to check it.
- **No change to s18's `loud-plan-import-diagnostics` capability's own named
  requirements** (per-construct path+reason naming, the root-near-miss hint,
  the non-clean WARN + non-zero-exit condition itself). Those already fire
  correctly on run #1 and continue to; this change's ONLY job is making
  them fire again on every subsequent run against the same unfixed source,
  by withholding the cursor that was previously suppressing them.
- **No change to the `unwritten` seam or `fully_durable`'s existing
  `changes_unwritten == 0 && tasks_unwritten == 0` unreachable-tier gate.**
  This change ADDS a second, independent condition a pass must ALSO satisfy
  to advance its cursor; it does not alter or relax the existing one. A
  pass with any unwritten candidate still never advances its cursor, exactly
  as today, regardless of this change.
- **No change to a NON-malformed empty source's behavior.** A source whose
  root exists, is well-formed, and genuinely contains zero change
  directories yet (`malformed.is_empty()`, `changes_parsed == 0`) still
  advances its cursor and stays a clean, silent, `exit 0` no-op on every
  re-run — s18's own "A legitimately empty source stays a clean silent
  no-op" scenario is unaffected; this change's condition is
  `malformed > 0`, never `changes_parsed == 0` alone.
- **No change to `canon-store`, `canon-gate`, `canon-model`, `canon-vocab`,
  `canon-plugin`, or `canon-learn`.** Connector-never-authority holds
  unchanged: `canon-gate` reads nothing importer-specific, `canon gate
  check` verdicts stay byte-identical, the closed 12-`RecordKind` set
  (`RecordKind::ALL.len() == 12`) is untouched, and `canon inventory sync`
  keeps its single-`Scenario`-producer status. The entire diff lives in
  `canon-cli/src/plans.rs`'s cursor-advance predicate.
- **No new `canon` subcommand or flag.** `canon ingest plans`'s existing
  flags, output shape (`format_human`/`format_json`), and exit-code
  convention are unchanged beyond firing on more runs than before.

## Impact

- **`canon-cli`**: `plans.rs`'s per-source loop (`plans.rs:377-388`) — the
  `fully_durable`/cursor-eligibility computation gains the
  malformed-nonzero-zero-persisted exclusion, reusing the exact boolean
  expression `plans.rs:400` already computes for `non_clean_sources` (the
  two are now derived from ONE shared local, not two independently
  maintained conditions that happen to agree today and could silently
  drift apart later).
- **`canon-store`**: UNCHANGED. `cursor.rs`'s `SourceCursor`, `CursorStore`,
  `source_unchanged`, `file_digest` — no new field, no schema change, no new
  method.
- **s18's specs** (`loud-plan-import-diagnostics`): its existing scenarios
  remain valid and unmodified; this change's `durable-import-diagnostics`
  spec sits alongside as an ADDED capability closing the gap between "loud
  once" (what s18 shipped) and "loud on every run against the same broken
  source" (what this change adds).
- **No new crate.** `canon-model`/`canon-store`/`canon-gate`/`canon-ingest`/
  `canon-vocab`/`canon-plugin`/`canon-learn` are unchanged; `canon-ingest`'s
  `PlanAdapter`/`PlanParseOutcome` shape (s17/s18) is read, not modified.
