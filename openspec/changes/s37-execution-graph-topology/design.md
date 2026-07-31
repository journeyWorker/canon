# s37 execution-graph-topology — design

## D1. The flywheel is repaired in `canon-cli`, not `canon-learn`

Both severed links are integration-layer omissions, not design flaws in
`canon-learn`. That crate's whole discipline is "pure functions over
already-resolved samples": `PromotionGate::evaluate` is documented as
taking `(regime_key, samples, as_of)` with "neither gate reads a store OR
a wall clock directly", and `canon_learn::webhook`'s doc explicitly
assigns the complementary job elsewhere — "wiring a real HTTP endpoint
that … gathers candidates from a live store is `canon-cli`'s job".

So the fix resolves samples in `canon-cli` and calls the existing pure
seams. No signature in `canon-learn` changes. Rejected alternative:
widening `promote_strategy` to take a `&dyn TrajectoryStore` — it would
push I/O and config-reading into the crate that exists precisely to have
none, and would break the `plan_promotion`/`promote_strategy` symmetry.

## D2. Promotion fails closed, with no override flag

`canon.yaml`'s parser already rejects `promotion.<role>.n_min: 0` with
the stated reason that it "would let `OccurrencePromotionGate` promote
with zero corroborating successes … defeating the n-occurrence gate
entirely". A `--force` flag would reintroduce exactly the defeat that
validation exists to prevent, one layer up. An anchor with a bypass
switch is not an anchor.

A refusal is a statement that the evidence is not there yet. The
remedy is to resolve the regime's trajectories (`canon ingest
artifacts`, per D1) and retry — which is why both halves of this change
ship together. Fixing only the gate would make promotion permanently
impossible; fixing only the marking would leave the gate dead.

`--dry-run` still renders its preview when blocked (an operator asking
"what would be written" deserves the answer alongside the reason) but
its exit code reports the refusal.

## D3. `Pending` is left pending, never forced

`mark_trajectory_verdict` rejects `VerdictOutcome::Pending` outright,
because `Pending` is the unset default and allowing it would let a caller
re-open a resolved trajectory. The `dev` reward formula reaches `Pending`
legitimately: `compute_dev_reward`'s additive triad
(`pr-merged` 0.4 + `ci-pass` 0.3 + `no-rollback` 0.3) stays `Pending`
below a full `1.0`, waiting on a no-rollback timer the S7 webhook
receiver owns.

So ingest counts that case (`trajectories_left_pending`) and moves on. It
does NOT invent a resolution. A trajectory awaiting a real covering
signal is correct state, not an error — and surfacing the count is what
makes the distinction visible instead of silent.

## D4. Marking happens BEFORE `rebuild_namespace`

`mark_verdict` is the only path permitted to rewrite a stored
trajectory; `rebuild_namespace` must leave those bytes untouched, which
`parquet_trajectory`'s own round-trip test asserts. Mark-then-rebuild is
therefore the tested-safe order. Distillation reads `verdicts`, not
`verdict_record`, so the order is not semantically load-bearing for
strategy content — only for the byte-identity invariant.

## D5. Every lineage field is additive; no `schema` bump

Per `skill://state-model`: "Non-breaking additions (a new `Option<T>`
field with `#[serde(default)]`) do not require a bump." All five new
record fields are `Option<T>` or `Vec<T>` with `#[serde(default)]`, so
every existing fixture and stored record keeps parsing unchanged, and no
coordinated migration is needed. `schemas/*.schema.json` and
`JOIN_SPINE.md` are regenerated with `cargo xtask write`, never
hand-edited.

## D6. Session grouping is preserved; identity is added alongside it

The Claude adapter's sidechain → parent-session collapse is two separate
behaviors fused together:

1. **Attributing a subagent's billable rows to the parent session.**
   Deliberate and correct — cost and token accounting belongs to the
   session a user actually started. Other code depends on it. KEPT.
2. **Discarding which subagent produced them.** A capitulation to
   `UnifiedRow` having no agent field, stated as such in the module doc.
   REMOVED, now that the field exists.

This split is why the change is additive rather than a rewrite: nothing
about grouping, dedup, or the watermark cursor moves. The regression that
matters most is that a plain single-agent session still normalizes to
exactly one `Run` with `parent_run_id: None`.

## D7. `depends_on` is DECLARED intent, never a scheduler input

`Task.depends_on` records what the plan corpus says, so that the
declared graph can later be diffed against the graph that actually ran
(reconstructed from `Run.parent_run_id`). canon never reads it to
order, block, or dispatch work. Extraction is conservative and
fail-soft, matching `canon inventory sync`'s handling of a malformed
`@subject:` tag: an unresolvable reference is dropped and counted as an
import diagnostic, never an import failure. A dialect that has no
dependency expression populates nothing — an empty-but-honest field
beats an invented syntax.

## D8. The plan-vs-actual graph diff is the next increment, not this one

Once `depends_on` carries the declared graph and `parent_run_id` carries
the executed one, the interesting artifact is their difference: work that
ran out of declared order, fan-out that was declared but never happened,
dependencies satisfied by accident. canon already has the right record
kind for that (`Divergence`) and the right vocabulary for reporting it.

That diff is deliberately NOT in this change. It requires both halves to
exist and be populated against real corpora first; specifying its
semantics before there is data to look at would be guesswork. This change
ships the two halves it needs.
