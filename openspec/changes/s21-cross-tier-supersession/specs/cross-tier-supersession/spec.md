## ADDED Requirements

### Requirement: Every Tier adapter preserves append-only history — no adapter overwrites or discards a prior version in place
`GitTier`, `PgTier`, and `R2Tier` SHALL each retain every version ever written for a given `(kind, natural key)`, indefinitely (subject only to the existing `Tier::age` sweep, which relocates a version to a colder tier but never discards it). No adapter's `write` path SHALL update, delete, or otherwise mutate a previously-written version's stored bytes in place. A write carrying content byte-identical to an already-stored version at the same key SHALL be a no-op (`WriteReceipt.deduped = true`), never an error and never a second copy.

#### Scenario: PgTier retains a superseded version instead of overwriting it
- **WHEN** two `Task` writes land at the same `task_id` with different bodies (a state transition), in either arrival order
- **THEN** both versions are retrievable from `PgTier` afterward — neither write's bytes are gone — distinguishable by their own `at`/`digest`

#### Scenario: A byte-identical resubmission is a no-op on every tier
- **WHEN** the same record (identical serialized body) is written twice to the same tier
- **THEN** the second write reports `deduped: true`, and the tier holds exactly one stored copy — never two, never an error

#### Scenario: An out-of-order arrival never destroys the version it arrives after
- **WHEN** a `PgTier` write carrying an OLDER `at` for a given `(kind, id)` lands strictly after a write carrying a NEWER `at` for the same key
- **THEN** the newer version's stored bytes are still present and unmodified after the older write completes — the older write adds a version, it does not replace one

### Requirement: Tier::read returns every retained version for a kind, uniformly across all three adapters
`Tier::read` SHALL have the identical contract on `GitTier`, `PgTier`, and `R2Tier`: given a kind (and an optional `since` lower bound on `at`), it SHALL return every retained version matching that filter — never a pre-resolved "current only" subset on any one adapter while another returns full history. Resolving "current state" from a `Tier::read` result is exclusively the caller's responsibility, performed by applying the one shared fold (see the total-order-fold requirement below) — no `Tier` adapter SHALL perform its own, adapter-specific pre-folding at read time.

#### Scenario: PgTier and GitTier agree on how many versions a doubly-written key returns
- **WHEN** a `Task` is written twice (two distinct bodies) at the same `task_id`, once against a `GitTier`-routed kind and once against the identical scenario for a `PgTier`-routed kind
- **THEN** `Tier::read` for that kind returns exactly 2 records for BOTH adapters — never 1 for one and 2 for the other

#### Scenario: A single-write key returns exactly one record on every adapter
- **WHEN** a key is written exactly once, on any of the three adapters
- **THEN** `Tier::read` returns exactly 1 record for that key, on every adapter

### Requirement: fold_latest_by_key resolves "current" via a total, machine-independent order — (at, content_digest) — never construction or iteration order
`canon-store::fold::fold_latest_by_key` SHALL determine each key's winning item by comparing `(at, content_digest)` as a total order: strictly greater `at` always wins; on equal `at`, the item whose content digest sorts greater (as data, e.g. lexicographic byte/string comparison) wins. The result for a fixed input SET SHALL be identical regardless of the order the caller constructs, iterates, or supplies that set in.

#### Scenario: Two same-`at` items fold to the same winner regardless of iteration order
- **WHEN** two items sharing an identical `at` but different content digests are folded once with item A iterated before item B, and again with item B iterated before item A
- **THEN** both folds produce the identical winner — the one with the greater digest — in both orderings

#### Scenario: A strictly greater `at` always wins, regardless of digest
- **WHEN** two items for the same key have different `at` values
- **THEN** the item with the strictly greater `at` wins the fold, irrespective of either item's digest

#### Scenario: The fold's output is independent of which machine or filesystem produced the input order
- **WHEN** the same logical set of ledger records is scanned on two different machines whose directory-traversal order differs (e.g. two filesystems returning `readdir` entries in different orders)
- **THEN** `fold_latest_by_key` over each machine's scan produces byte-identical "current" output for every key — the fold's own comparison, not the scan's traversal order, determines the winner

### Requirement: GitTier's kind-directory scan is sorted, independent of the fold's own correctness
`GitTier::scan_kind_where` SHALL traverse a `kind=<kind>/` directory in a deterministic, sorted order (lexicographic by relative path) rather than raw filesystem directory-entry order, so that `TierReadResult.records`' own iteration order is reproducible across runs and hosts independent of whatever fold (if any) a caller later applies to it.

#### Scenario: Two scans of an unchanged kind directory produce identically-ordered results
- **WHEN** `GitTier::read` is called twice in succession over an unchanged `kind=<kind>/` directory
- **THEN** `TierReadResult.records` is returned in the identical order both times

#### Scenario: Scan order does not depend on file creation order
- **WHEN** files under a `kind=<kind>/` directory are created in one order but a directory listing would (on some filesystem) return them in a different, unsorted order
- **THEN** `GitTier::read`'s returned order is the sorted (lexicographic-by-path) order, not the creation or raw-listing order

### Requirement: Every reader of a PgTier-routed kind resolves current state through the shared fold, with no reader exempted
Every in-workspace reader of a kind routed to `PgTier` (per `canon.yaml`'s `routing` section — `task`, `handoff`, `session`, `run`, `event` at time of writing) SHALL apply `fold_latest_by_key` to its `Tier::read` result before treating any row as "the current value" for a key. No reader SHALL assume `PgTier::read` already returns one row per key.

#### Scenario: A migrated reader sees exactly one logical record per key for an unsuperseded corpus
- **WHEN** a pg-routed kind's corpus has exactly one write per key (no supersession)
- **THEN** a migrated reader's fold-resolved output has exactly the same count and content as the corpus's raw rows — the fold is a no-op for an unsuperseded corpus

#### Scenario: A migrated reader sees the newer version for a superseded key, regardless of write arrival order
- **WHEN** a pg-routed key has two writes, the chronologically OLDER one arriving at the store SECOND
- **THEN** a migrated reader's fold-resolved output for that key is the chronologically NEWER version — never whichever write physically landed last
