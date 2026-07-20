## Context

s27 (`tier-role-backend-split`) deliberately left `local`/`hot`/`cold`
accepting ANY `Backend` — design D1's own "Any rung MAY be tagged with
any backend" scenario, defended as a robustness property for D2's
report-inclusion derivation (a `cold` rung backed by Postgres must
stay EXPRESSIBLE so the boundary derivation can be proven to key on
backend capability, not rung identity). That robustness argument is
sound for the DERIVATION; it does not justify accepting the config
itself. `canon.yaml`'s `tiers.local: { backend: postgres }` parsing
successfully is user-hostile: nothing downstream can make a live
database behave like a diffable git ledger, so the config is
guaranteed to misbehave the moment anything reads or writes through
that rung, just later and less clearly than a parse-time rejection
would.

A second, independent defect was found in the same file while fixing
the first: `Backend::offline_file_readable()` returns `true` for `S3`.
`canon-report`'s DuckDB views (`crates/canon-store/sql/views.sql`)
read `stg_git_records` (`read_text` over the git ledger) and
`stg_r2_records` (`read_parquet` over `CANON_R2_ROOT`, a LOCAL
directory defaulted to `<repo>/canon/r2`) — never a live S3 client.
`canon tier age` (`crates/canon-store/src/r2_tier.rs`) writes
cold/S3 records to the LIVE bucket, not to that local directory. So
for the common case — an S3-backed `cold` rung, canon's own default —
the report's local `canon/r2` mirror is empty unless an operator
manually, separately populates it; there is no automatic sync.
`offline_file_readable()` returning `true` for S3 tells every reader
of `Backend::offline_file_readable()` (today: exactly
`tier_boundary::non_offline_readable_kinds`) that this data IS safely
in the report, when in the default, unmirrored case it is not. This
is the round-3 F2 bug class (s25 `report-pg-tier-boundary`'s own
motivating finding) reintroduced onto S3.

## Goals / Non-Goals

**Goals:**
- Every `tiers.<rung>` entry `TierPolicy::from_yaml` accepts is
  class-compatible with its rung — `local`/`hot`/`cold` become
  coherent capability roles again, not "any backend" containers.
- `canon report`'s report-inclusion signal (`Backend::
  read_directly_by_report`, renamed from `offline_file_readable`)
  never overclaims: it is `true` ONLY for the backend whose own store
  is one of `canon-report`'s actual local read roots.
- The rendered boundary note and stderr `WARN` never assert an
  absolute "not reflected" — a listed kind's data MAY exist locally if
  separately materialized; the wording says exactly that, no more, no
  less.
- The two axes (D1 compatibility, D2 report-inclusion) stay two
  distinct methods on `Backend`, never collapsed, even though they
  single out the same backends today.

**Non-Goals:** (mirrors proposal.md's Explicit non-goals verbatim — no
live read added to `canon-report`, no automatic `canon/r2`
materialization/sync feature, no closed-kind-set change,
connector-never-authority preserved, report stays
offline/deterministic, no relaxation escape hatch, no fixture
migration needed for today's default pairing.)

## Decisions

**D1 — `BackendClass` compatibility check: every `tiers.<rung>` entry
must match its rung's expected class.**

```rust
/// The I/O CAPABILITY CLASS a `Backend` belongs to — orthogonal to
/// `Backend::read_directly_by_report` (D2): this is a COMPATIBILITY
/// classification `TierPolicy::from_yaml` validates a rung's
/// configured backend against, never a report-readability fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendClass {
    /// A local, diffable file store — no live connection, no bucket.
    LocalFile,
    /// A live, queryable database connection.
    LiveDb,
    /// A live object-store bucket.
    ObjectStore,
}

impl Backend {
    pub fn class(self) -> BackendClass {
        match self {
            Backend::Git => BackendClass::LocalFile,
            Backend::Postgres => BackendClass::LiveDb,
            Backend::S3 => BackendClass::ObjectStore,
        }
    }
}

impl Rung {
    pub fn expected_backend_class(self) -> BackendClass {
        match self {
            Rung::Local => BackendClass::LocalFile,
            Rung::Hot => BackendClass::LiveDb,
            Rung::Cold => BackendClass::ObjectStore,
        }
    }
}
```

`TierPolicy::from_yaml`'s `tiers` decode loop gains one check per
entry, immediately after `Rung::parse`/`decode_backend_config`
succeed:

```rust
let actual = cfg.backend();
let expected_class = rung.expected_backend_class();
if actual.class() != expected_class {
    return Err(PolicyError(format!(
        "canon.yaml `tiers.{rung_key}`: backend `{}` is {}, but the `{rung_key}` rung expects {} (`{}`)",
        actual.as_str(), actual.class().describe(),
        expected_class.describe(), expected_class.example_backend().as_str(),
    )));
}
```

producing, for `tiers.local: { backend: postgres, ... }`:
`` canon.yaml `tiers.local`: backend `postgres` is a live-database
backend, but the `local` rung expects a local-file backend (`git`) ``.
Two shapes were weighed for WHERE this lives: (a) inside
`decode_backend_config` (the function that already decodes one
`tiers.<rung_key>` raw value); (b) inline in `from_yaml`'s loop, after
`decode_backend_config` returns. **(b) is accepted** —
`decode_backend_config` takes only `rung_key: &str` (a string, pre-
`Rung::parse`) and a raw YAML `Value`; giving it the parsed `Rung` just
to perform a check that belongs to `from_yaml`'s own validation
sequence (already the site of every other loud-rejection check:
unknown kind names, legacy backend-as-rung names, missing `backend:`
tag) would blur `decode_backend_config`'s single responsibility
("decode this YAML block into a typed config") for no benefit — the
caller already has both `rung` and the decoded `cfg` in scope.

The `backend:` field stays EXPLICIT, never inferred from the rung
(e.g. `tiers.local` auto-implying `git`) — a future same-class backend
swap (a second live-database vendor for `hot`, a second object-store
vendor for `cold`) must remain a config change, not a code change,
exactly as s27 D1 already established; D1 here narrows WHICH backends
are acceptable per rung, it does not re-introduce a fixed pairing.

**D2 — `Backend::offline_file_readable()` renames to
`Backend::read_directly_by_report()`; `S3` flips `true` → `false`.**

```rust
pub fn read_directly_by_report(self) -> bool {
    match self {
        Backend::Git => true,
        Backend::Postgres | Backend::S3 => false,
    }
}
```

This is a SEPARATE method from `Backend::class()` (D1) even though
both currently single out git — `class()` answers "is this backend
compatible with the rung it's tagged on" (a parse-time question,
independent of `canon-report`'s existence); `read_directly_by_report()`
answers "does `canon-report` open this backend's own store directly"
(a report-specific question, independent of rung compatibility). They
must never be merged into one method: a hypothetical future backend
whose class matches its rung but whose store the report still cannot
read directly (or vice versa) would be inexpressible if they were.

Two shapes were weighed for the RENAME: (a) keep
`offline_file_readable`, just flip S3's `bool`; (b) rename AND flip.
**(b) is accepted** — `offline_file_readable` is not merely wrong for
S3 today, it asks the wrong QUESTION. "Offline-file-readable" is a
property of a backend's storage MEDIUM in the abstract (a parquet
export is offline-file-readable wherever it happens to sit); the
actual invariant `canon-report` needs is narrower and more specific:
"is this backend's OWN store one of MY particular local read roots
right now". `read_directly_by_report()` names that precisely and
forecloses the exact confusion that produced the S3 bug (conflating
"this backend's export FORMAT is a local file format" with "this
backend's CURRENT store is a file `canon-report` reads").

**D3 — Truthful, non-absolute boundary wording; `## Kinds not read
directly`, not `## Tiers not reflected`.**
`crates/canon-report/src/tier_boundary.rs`'s public function renames
`non_offline_readable_kinds` → `kinds_not_read_directly` (matching
D2's renamed predicate); its filter becomes
`!backend.read_directly_by_report()`. The rendered heading renames
`## Tiers not reflected` → `## Kinds not read directly` — the heading
is about the LISTED KINDS (which ones the report doesn't open
directly), not a claim about "tiers" as a whole. The shared sentence
(read by both `render_note` and `warn_line` — module doc: one
derivation, so the note and the WARN can never disagree) rewords from
an absolute "canon report reflects only offline-file-readable tiers;
kinds routed to a backend it cannot read offline … are read via
`canon query --kind <kind>` instead" to:

> canon report reads its local roots directly (the git ledger + local
> `canon/r2` + `canon/learn` parquet); the kinds below route to a
> backend whose own store it does not read (a live database or
> object-store bucket), so their data appears only if materialized
> into the local report roots — it may be incomplete or stale. Read
> them live with `canon query --kind <kind>`.

Two shapes were weighed: (a) keep the absolute "not reflected"
framing, just add S3 to the excluded set; (b) reword to the
conditional "appears only if materialized … may be incomplete or
stale" framing. **(b) is accepted** — (a) would still be a lie for any
repo that DOES maintain a local `canon/r2` mirror (its S3-routed
kinds' data genuinely IS in the report in that case; the OLD sentence
under (a) would wrongly tell the operator otherwise), and this is
precisely the same category of overclaim this whole change exists to
remove — fixing the boolean while leaving an equally absolute sentence
around it would be an incomplete fix. `warn_line`'s kind-list suffix
renames from `Not reflected: …` to `Not read directly: …` to match.
Fail-soft posture (missing/malformed `canon.yaml` → empty `Vec`, never
a panic) is UNCHANGED — this is a wording/predicate correction, not a
new failure mode.

## Risks

- **A future `RecordKind` routed to an S3-backed `cold` rung will now
  appear in `## Kinds not read directly` for every repo using canon's
  own default pairing (`cold`→s3)**, where s27 silently omitted it.
  This is the INTENDED correction, not a regression — accepted, and
  exercised directly by this change's own new tests
  (`a_cold_rung_backed_by_s3_now_appears_in_kinds_not_read_directly`).
  An operator who genuinely does maintain a synced local `canon/r2`
  mirror will see a (harmless) note/WARN for data that is, in their
  case, actually present — the reworded D3 sentence is deliberately
  worded to remain true in that case too ("appears only if
  materialized … may be incomplete or stale" neither confirms nor
  denies presence, it names the uncertainty honestly).
- **D1's class check could, in principle, reject a future legitimate
  same-class backend swap if `BackendClass::example_backend`'s 1:1
  class↔backend assumption stops holding** (e.g. a second live-database
  vendor added as a new `Backend` variant). Not a risk today — `class()`
  itself only maps `Backend → BackendClass`, so a new same-class
  backend variant is accepted by D1's check with no code change to the
  check itself; only the class-mismatch error's parenthetical example
  name (`example_backend()`) would need updating to name one canonical
  choice among several, a cosmetic, non-blocking follow-up.
- **`canon gate check` is unaffected** — `canon-gate` reads nothing
  from `canon-store`'s tier vocabulary or `canon-report`'s boundary
  derivation, and no `canon-gate` source file is touched by this
  change (byte-identity re-confirmed by this change's own acceptance
  tests, unmodified from s27's).

## Sequencing

- **P1 — `crates/canon-store/src/policy.rs`: `BackendClass` +
  `Backend::class()` + `Rung::expected_backend_class()` +
  `TierPolicy::from_yaml`'s class-compatibility check (D1); rename
  `offline_file_readable` → `read_directly_by_report`, flip `S3` to
  `false` (D2).** Standalone; every later phase depends on this.
- **P2 — `crates/canon-report/src/tier_boundary.rs`: rename
  `non_offline_readable_kinds` → `kinds_not_read_directly`, filter on
  `read_directly_by_report`, reword the shared sentence + heading
  (D2/D3), after P1.**
- **P3 — `crates/canon-report/src/render.rs`, `src/lib.rs`,
  `crates/canon-cli/src/main.rs`: pure parameter/variable renames to
  match P2, after P2.**
- **P4 — `crates/canon-store/sql/views.sql`,
  `canon/skills/tiered-storage/SKILL.md`: doc-comment/prose corrections
  (D2/D3), independent of P2/P3 — no code dependency either
  direction, after P1.**
- **P5 — Tests, after P1-P3.** `crates/canon-store/src/policy.rs`'s own
  `#[cfg(test)] mod tests` (class-mismatch rejection, each
  class-correct combo, `Backend::class()`/`read_directly_by_report()`
  counter-cases); `crates/canon-report/src/tier_boundary.rs`'s own
  tests (the S3-now-included scenario, migrated
  incompatible-combo removals);
  `crates/canon-report/tests/tier_boundary.rs` +
  `crates/canon-cli/tests/report_tier_boundary.rs` (heading-text
  updates, the S3-now-included integration scenario).
- **P6 — Closure.** `bunx openspec validate --strict
  s28-rung-backend-capability`; `cargo build --workspace` + `cargo
  test --workspace --no-fail-fast` + `cargo clippy --workspace
  --all-targets -- -D warnings` green; `canon gate check`
  byte-identity re-confirmed (no `canon-gate` source file touched).
