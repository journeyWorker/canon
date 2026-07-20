## Context

canon-ingest is the first of two ingest crates (S3 sessions, S4 review/handoff/
artifact) and the first consumer of the S1 model + S2 store. It has no upstream
canon dependents yet inside this change, but S4's verdict stream and every later
role-namespaced strategy (S6/S7) eventually needs `session_id` to join a
trajectory back to the run that produced it (S1 join spine table) — so canon-ingest
must land the `session_id` join contract correctly the first time.

Three real, already-shipped implementations exist to study and adapt
(design §3 donor table — attributed-port posture: canon ports/adapts the
vendored upstream launcher's logic with ported-from-upstream provenance
comments, per the launcher's MIT license;
operator directive 2026-07-10 supersedes the earlier clean-room posture):

- The vendored upstream launcher declares one static entry per client —
  `{root: PathRoot::Home, relative: ".claude/projects", pattern: "*.jsonl",
  headless, parse_local}` — and its scanner walks the enabled set, honoring
  the launcher's extra-scan-dirs env var / settings.json `extraScanPaths` overrides per client.
- Each source gets its own parser (the Claude Code parser reads
  `~/.claude/projects/**/*.jsonl`; the Codex fixture tests read
  `~/.codex/sessions/**/*.jsonl`) that
  emits a shared `UnifiedMessage { client, model_id, provider_id, session_id,
  workspace_key, timestamp, TokenBreakdown, duration_ms, … }`.
- The launcher's aggregator folds `UnifiedMessage` rows into a
  `SessionContribution` keyed on `session_id` (`acc.entry(msg.session_id.clone())`),
  the exact shape canon-ingest's cost-parity fixture must reproduce.
- The launcher's scanner queues Codex's live `~/.codex/sessions/`
  dir AND a sibling `~/.codex/archived_sessions/` dir (Codex CLI's own
  session-rotation behavior, not a launcher invention) under one logical
  `Codex` source, deduping the union by canonicalized path. A Codex adapter
  that scans only the live directory silently under-counts any session Codex has rotated out.
- Claude Code's streaming API re-writes the same logical message multiple
  times as a response completes; the launcher's Claude Code parser dedups
  by a composite `messageId:requestId` key and merges by taking the
  per-field max across duplicates. Codex's `token_count` events are
  *cumulative session totals*, not deltas; the launcher's Codex parser
  diffs each new total against the previous snapshot and detects
  forked-child sessions replaying their parent's history so tokens aren't
  double-counted under the child's own session id. Both are load-bearing
  for D4's cost-parity acceptance bar, not optional refinements.

## Goals / Non-Goals

**Goals:**
- One adapter registry trait + per-adapter descriptor (source id, root resolution
  — which MAY return multiple unioned roots, e.g. Codex's live + archived
  session directories, D5 — scan glob, parser fn) that the omp/pi, Claude Code,
  and Codex adapters implement.
- Normalize every adapter's raw records into canon-model's `Session`/`Run`/`Event`
  envelope (`{schema, kind, at, actor}`, S1) plus a token/cost row keyed by
  `session_id`.
- Incremental (watermark) + idempotent (content digest) re-ingest: two consecutive
  `canon ingest sessions` runs over an unchanged fixture set produce byte-identical
  normalized output, and a full re-scan never duplicates a record.
- Source-level reconciliation (D6): Claude Code's streamed duplicate
  messages are deduped and merged, and Codex's cumulative token totals are
  diffed with fork-replay detection, BEFORE either contributes to a
  token/cost row — a precondition for the cost-parity goal below, not an
  independent nicety.
- Cost parity: canon-ingest's token/cost rows match the vendored upstream launcher's own numbers on the
  same fixture input, within rounding.

**Non-Goals:**
- Additional adapters beyond omp/pi, Claude Code, Codex (amp/zai/grok/copilot/…
  from the launcher's registry are out of scope for S3; the registry is designed to
  accept them later without a shape change).
- Pricing-table maintenance (canon-ingest re-uses whatever pricing source the
  fixture's expected-cost numbers were generated against; it does not ship or
  update a pricing catalog).
- Live tailing beyond `--watch`'s poll loop (no filesystem-event/inotify backend
  in this change).
- Writing to the pg/r2 tiers directly — canon-ingest calls canon-store's (S2)
  write path; it does not open its own tier connections.

## Decisions

**D1 — Adapter registry is a trait + static table, not a plugin system.**
Each adapter is a Rust value implementing an `Adapter` trait (`source_id()`,
`resolve_roots(cfg) -> Vec<PathBuf>`, `scan(root) -> Vec<RawRecord>`,
`normalize(RawRecord) -> Vec<Event>`); `canon-ingest::registry()` returns the
static `[omp, claude, codex]` array in adapter-declaration order (S10's
plugin.yaml-driven extension mechanism is a later, separate spec — S3 does not
anticipate it). Rationale: mirrors the launcher's static-array client shape
(no dynamic plugin loading needed for three built-in sources), keeps the crate
dependency-free of S10, and keeps `canon ingest sessions` deterministic —
enumerating the registry always yields the same adapter order.
Alternative considered: a `plugin.yaml`-registered adapter set (S10 shape) —
rejected for S3 because S10 lands in wave W4, after S3; pulling it forward would
invert the wave ordering the design (§6) fixed.

**D2 — Watermark is per-adapter-source, stored as an ingest cursor record.**
Each adapter persists a cursor `{source_id, last_seen_at, last_seen_digest}`
through canon-store's git or pg tier (tier choice is a `canon.yaml` policy, not
an ingest-crate decision — S2 concern). A scan only reads files/records whose
mtime or (for append-only jsonl session files) byte offset is past the cursor.
Rationale: matches the "Incremental (watermark)" acceptance line in design §5 S3
verbatim, and avoids re-reading multi-MB transcript files on every invocation —
the same problem the launcher's source-message cache solves for
its own scans.
Alternative considered: full-corpus re-scan every run, relying only on the
content digest (D3) for dedup — rejected: correct but O(corpus) per run, and the
acceptance criterion explicitly calls out watermark as a distinct mechanism from
digest-idempotence, not a substitute for it.

**D3 — Idempotence key is a content digest per normalized record, not per file.**
`canon-ingest` computes a stable digest (sha256 over the normalized record's
canonical JSON, excluding volatile fields the source may re-emit non-
deterministically) and uses it as the record's identity for the store's
upsert-by-digest write. Rationale: a single transcript file can be appended to
between runs (the Claude Code / Codex jsonl sources are append-only logs) — a
file-level digest would invalidate on every append and re-import already-seen
lines; a record-level digest lets watermark (D2) skip already-scanned byte
ranges while digest still guards against a watermark reset or `--watch` restart
re-emitting an already-stored record. Malformed source records never reach the
digest step: they are skipped as violations per design §7 ("malformed evidence
is no evidence — skip + violation, never crash, never count").

**D4 — Cost parity is verified against the vendored upstream launcher's own CLI output, not
re-derived pricing math.**
The S3 fixture corpus ships the launcher's computed cost for the same fixture
transcripts (captured once, checked into the fixture directory) as the
expected value; canon-ingest's own token/cost rows are diffed against it within
a rounding tolerance (acceptance: "costs match the launcher's numbers on the same
input within rounding" — design §5 S3). Rationale: canon is not the pricing
authority and does not want two pricing tables to drift; treating the launcher's
number as ground truth for the fixture keeps the parity check meaningful without
canon-ingest depending on the launcher's pricing crate at build time (no build-time
dependency on the launcher's pricing crate — the expected cost is a recorded fixture
value; parser/scanner logic itself is an attributed port).

**D5 — Codex adapter unions live and archived session roots.**
`resolve_roots()` for the `codex` adapter returns BOTH
`${CODEX_HOME:-~/.codex}/sessions` and
`${CODEX_HOME:-~/.codex}/archived_sessions`, deduped by canonicalized path
before scanning, feeding one logical `codex` adapter identity — mirroring
the launcher's own union.
Rationale: `archived_sessions/` is Codex CLI's own session-rotation
behavior (not a launcher invention) — a Codex adapter that scans only the
live directory silently under-counts any session Codex has rotated out,
which would make the S3 acceptance bar ("ingesting a fixture set … yields
identical normalized output across two runs" and cost parity) pass on an
incomplete input without ever surfacing the gap. Effort is trivial (~20
lines against the launcher's own equivalent), purely additive to D1's adapter
shape, and has no precondition on any other decision in this document.
Alternative considered: scan only the live `sessions/` directory and treat
`archived_sessions/` as a future adapter enhancement — rejected: the
cost-parity fixture (D4) would then silently exclude any archived session
in the launcher's own comparison output, making the acceptance check compare
incomplete data against complete data and pass for the wrong reason.

**D6 — Source-level reconciliation precedes normalization for Claude Code
and Codex, before any record reaches the digest/store path.**
Two distinct problems, both load-bearing for D4's cost-parity acceptance
bar (not addressed by D2's watermark or D3's storage-level digest, which
guard against *duplicate writes*, not *duplicate/cumulative source
records*): (a) Claude Code's streaming API re-writes the same logical
message multiple times as a response completes — the `claude` adapter
dedups by a composite `messageId:requestId` key and merges duplicates by
taking the per-field max across them, mirroring the launcher's Claude Code
dedup logic; (b) Codex's `token_count` events are *cumulative
session totals*, not deltas — the `codex` adapter diffs each new total
against the previous snapshot (never sums raw per-line values) and detects
a forked-child session replaying its parent's history, attributing those
replayed tokens to the fork-source identity
(`session_forked_from_id.or(session_id_from_meta)`, never the adapter's
own filename-derived surface id) so they are not double-counted under the
child's `session_id` — mirroring the launcher's Codex dedup logic. Rationale:
D4's acceptance bar is a hard equality check against the launcher's own
numbers; a naive per-line-sum implementation would double-count Claude
Code's re-streamed messages and grossly over-count Codex's cumulative
totals, failing D4's fixture test by construction — this is not an
optional refinement layered on top of normalization, it is a precondition
normalization must satisfy before a record is eligible for the digest step
(D3) at all.
Alternative considered: implement dedup/reconciliation as a post-hoc
correction pass after a naive per-line-sum first version — rejected per
the source audit's own precondition note: retrofitting this after cost
numbers are already believed correct means re-deriving them, which is
strictly more expensive than building the reconciliation step into each
adapter's `normalize()` from the start.

## Risks / Trade-offs

- **Risk:** adapter-specific transcript formats change upstream (Claude Code /
  Codex session schema revisions) and silently break normalization.
  **Mitigation:** malformed-record skip (D3) turns a format drift into a visible
  violation count rather than a crash or silent data loss; `canon selftest`
  (design §8) fixture diff catches a regression before it reaches a real corpus.
- **Risk:** watermark cursor state (D2) drifts from the actual store contents if
  a write partially fails between the record write and the cursor advance.
  **Trade-off accepted:** the cursor advances only after the corresponding
  record write is durably acknowledged by canon-store; a crash mid-batch re-scans
  the unadvanced tail on the next run, which digest-idempotence (D3) makes safe
  (re-normalizing and re-attempting an already-stored record is a no-op write).
- **Risk:** fixture transcripts are real (sanitized) samples — sanitization must
  not remove the exact fields (`session_id`, token counts, timestamps) the
  parity and idempotence checks depend on, or the fixtures stop testing anything
  real.
  **Mitigation:** fixture sanitization is reviewed as part of this change's
  tasks (redact prose/PII fields only; token/cost/id fields pass through).
- **Risk:** the Codex fork-replay detection (D6) is the most intricate
  single piece of adapter logic in S3 — a state machine over cumulative
  totals, snapshot regressions (e.g. after context compaction), and
  fork-source attribution, not a stateless per-record transform.
  **Mitigation:** the S3 fixture corpus (task group 6) explicitly includes
  a forked-session sample and a compaction-regression sample so
  `canon selftest` exercises this path directly, rather than relying on
  the aggregate cost-parity check alone to catch a reconciliation bug.
