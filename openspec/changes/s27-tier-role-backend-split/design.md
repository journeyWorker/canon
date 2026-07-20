## Context

S2 (`tiered-storage`) fixed the tier split as three concrete backends
(`TierKind { Git, Pg, R2 }`) and let `canon.yaml`'s `routing`/`aging`
sections name them directly. That was correct at the time — S2 had
exactly one backend candidate per rung and no reason to separate the
two concepts. Two later changes exposed the conflation as a real
cost, not a hypothetical one:

- **s25 `report-pg-tier-boundary`** had to hardcode `TierKind::Pg` as
  the report's exclusion boundary, when the actual reason `pg` is
  excluded is "live database connection, not a file scan" — a BACKEND
  property, not a fact about the `Pg` variant's identity.
  `crates/canon-report/src/tier_boundary.rs`'s own module doc already
  states the real invariant in prose ("no live `pg` read anywhere in
  this module") without the type system enforcing it generically.
- **s22 `query-tier-degradation`**'s `StoreError::TierUnavailable
  { tier: TierKind, reason }` names a backend ("tiers.pg not attached
  (no live DSN)") when what an operator actually needs to know is
  *which rung of the storage ladder* their query needed and *why the
  backend behind it* isn't reachable — the current message conflates
  both into one un-decomposed string.

This change performs the split S2 deferred: `Rung` (the capability
role `canon.yaml`'s `routing`/`aging` name) separated from `Backend`
(the vendor implementation `canon.yaml`'s `tiers.<rung>.backend` tag
names). It is a HARD, non-additive revision of S2's `TierPolicy` shape
— per operator directive, canon has never been pushed, so there is no
external consumer to preserve backward compatibility for.

## Goals / Non-Goals

**Goals:**
- One `Rung` enum (`Local`/`Hot`/`Cold`) is the ONLY vocabulary
  `canon.yaml`'s `routing`/`aging` sections and every `canon-store`/
  `canon-cli` internal routing decision use.
- One `Backend` enum (`Git`/`Postgres`/`S3`) is the ONLY vocabulary a
  `tiers.<rung>.backend` tag and any backend-capability-conditioned
  decision (report inclusion) use.
- `Backend::offline_file_readable()` is the single source of truth
  every backend-capability decision reads — `canon-report`'s boundary
  derivation included — so "which backends can the report see" is
  never re-decided ad hoc at a second call site.
- Every `canon.yaml` in the repository speaks the new shape; no dual-
  vocabulary transition period, no alias.

**Non-Goals:** (mirrors proposal.md's Explicit non-goals verbatim —
no new backend, no tier-adapter read/write/age logic change, no pg→
report materialization, no closed-kind-set change, connector-never-
authority preserved, report stays offline/deterministic, no DuckLake,
no backward-compat shim.)

## Decisions

**D1 — Role/backend split: `Rung` names the capability, `Backend`
names the vendor, `canon.yaml`'s `tiers.<rung>` block tags its own
backend.**

```rust
/// The capability RUNG a record kind routes/ages to — canon's storage
/// ladder's role (S2 design §4's own diagram, made literal): local
/// diffable files → hot live-queryable state → cold bulk archive.
/// Independent of which vendor backend currently implements it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rung {
    /// Local, diffable files — git-native history. Default backend: git.
    Local,
    /// Hot, live-queryable state.
    Hot,
    /// Cold, bulk object-store archive.
    Cold,
}

impl Rung {
    pub fn as_str(self) -> &'static str {
        match self {
            Rung::Local => "local",
            Rung::Hot => "hot",
            Rung::Cold => "cold",
        }
    }

    /// Loud, hint-carrying rejection of a legacy backend name used
    /// where a rung is expected (D3) — `git`/`pg`/`r2` are BACKEND
    /// names now, never a valid `routing`/`aging`/`tiers` key.
    pub fn parse(s: &str) -> Result<Self, PolicyError> {
        match s {
            "local" => Ok(Rung::Local),
            "hot" => Ok(Rung::Hot),
            "cold" => Ok(Rung::Cold),
            "git" | "pg" | "r2" => Err(PolicyError(format!(
                "`{s}` is a BACKEND name, not a rung — canon.yaml's `routing`/`aging`/`tiers` keys now name a capability rung (local/hot/cold); declare the backend separately via `tiers.<rung>.backend: {s}`"
            ))),
            other => Err(PolicyError(format!(
                "unknown rung `{other}` (expected one of local/hot/cold)"
            ))),
        }
    }
}

/// The vendor BACKEND currently implementing a rung (D1/D4) — the
/// identity `TierKind` used to conflate with the rung itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    Git,
    Postgres,
    S3,
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Git => "git",
            Backend::Postgres => "postgres",
            Backend::S3 => "s3",
        }
    }

    pub fn parse(s: &str) -> Result<Self, PolicyError> {
        match s {
            "git" => Ok(Backend::Git),
            "postgres" => Ok(Backend::Postgres),
            "s3" => Ok(Backend::S3),
            other => Err(PolicyError(format!(
                "unknown backend `{other}` (expected one of git/postgres/s3)"
            ))),
        }
    }

    /// D2: can this backend's data be read via a plain offline file
    /// scan (DuckDB `read_text`/`read_parquet`, no live connection)?
    /// Git's Hive-laid-out JSON and S3's parquet exports: yes.
    /// Postgres — a live queryable connection — would break
    /// `canon-report`'s offline/deterministic contract (S9 decision
    /// 11) if read directly: no. The ONE method every backend-
    /// capability-conditioned decision in the codebase reads; no
    /// second ad hoc "is this backend live" check exists anywhere
    /// else.
    pub fn offline_file_readable(self) -> bool {
        match self {
            Backend::Git | Backend::S3 => true,
            Backend::Postgres => false,
        }
    }
}
```

`canon.yaml`'s new shape (proposal.md's own worked example, repeated
here as the normative shape):
```yaml
tiers:
  local: { backend: git,      root: canon/ledger }
  hot:       { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }
  cold:      { backend: s3,       bucket_env: CANON_R2_BUCKET, prefix: "canon/" }
routing: { task: hot, change: local, handoff: hot }
aging:   { handoff: { after: 0s, to: cold } }
```
Each `tiers.<rung>` entry is a serde enum INTERNALLY tagged on its own
`backend:` key — the existing per-backend field sets are reused
byte-for-byte, only relocated:
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case")]
enum BackendConfigRaw {
    Git { root: PathBuf },
    Postgres { dsn_env: String, #[serde(default = "default_pg_schema")] schema: String },
    S3 { bucket_env: String, #[serde(default = "default_r2_prefix")] prefix: String },
}
```
`TiersRaw` (the old three-named-optional-field struct) becomes a
`HashMap<String, serde_yaml::Value>` — kept as raw `Value`, not decoded
straight into `BackendConfigRaw`, so `TierPolicy::from_yaml` can
validate the MAP KEY (a rung name) via `Rung::parse` FIRST, before
attempting to decode the value — this is what lets a legacy
`tiers.git`/`tiers.pg`/`tiers.r2` top-level key produce the SAME loud,
hint-carrying `Rung::parse` error a legacy `routing`/`aging` value
produces (D3), rather than an opaque "missing field `backend`" serde
error from deserializing straight into the tagged enum. `TierPolicy`
itself becomes:
```rust
pub struct TierPolicy {
    pub tiers: HashMap<Rung, BackendConfig>, // BackendConfig{Git(GitTierConfig)|Postgres(PgTierConfig)|S3(R2TierConfig)}
    pub routing: HashMap<RecordKind, Rung>,
    pub aging: HashMap<RecordKind, AgingRuleConfig>, // AgingRuleConfig.to: Rung
}
```
The per-backend config PAYLOAD structs (`GitTierConfig{root}`,
`PgTierConfig{dsn_env,schema}`, `R2TierConfig{bucket_env,prefix}`) are
UNCHANGED — same fields, same defaults (`default_pg_schema`,
`default_r2_prefix`) — only their home moves from three top-level
`Option<_>` struct fields to variants of one `BackendConfig` enum keyed
by `Rung` in a map. Any rung MAY be tagged with any backend — the enum
does not statically pin `local→git`/`hot→postgres`/`cold→s3`; that
pairing is today's convention, not a type-level constraint (D2's own
robustness claim depends on this: a `cold` rung backed by Postgres, or
a `hot` rung backed by S3, must remain EXPRESSIBLE even if unusual).

**D2 — Report inclusion keys on BACKEND CAPABILITY, never on rung/tier
identity.** `canon-report`'s marts (`stg_records` and everything built
on it) reflect exactly the `RecordKind`s whose ROUTED RUNG's configured
backend is `offline_file_readable()`. The `## Tiers not reflected`
section + stderr `WARN` (s25) name exactly the complement: kinds whose
routed rung's backend is NOT offline-file-readable. Both derivations
call `Backend::offline_file_readable()` — ONE source of truth, matching
the codebase's existing "one derivation, both surfaces read it"
discipline (s25 design D3/R2, already established for
`pg_routed_kinds`/`render_note`/`warn_line`; this change generalizes
the SAME discipline onto the new capability axis, not a new one). This
is robust to ANY future rung↔backend assignment: today `cold` is always
S3 and `hot` is always Postgres, so "backend-capability-keyed" and
"rung-identity-keyed" happen to select the identical kind set — but a
repo that ever configures `cold: { backend: postgres, ... }` (an
unusual but expressible choice under D1) would have that rung's kinds
correctly EXCLUDED from the marts (they'd need a live connection,
exactly like today's `hot` rung), which a rung-identity-keyed
derivation (hardcoding "cold is always visible") would get wrong. This
REFRAMES `crates/canon-report/src/tier_boundary.rs` from "pg-routed
kinds" to "kinds routed to a non-offline-file-readable backend" —
`crates/canon-store/sql/views.sql`'s `stg_records` doc comment is
reframed identically ("backends without `offline_file_readable()` are
intentionally not staged", generalizing today's "the `pg` tier is
intentionally not staged" sentence).

**D3 — HARD MIGRATION, no backward-compatibility alias, no
deprecation path.** Operator directive, not re-litigated: canon has
never been pushed, nothing external depends on the old shape.
`TierPolicy::from_yaml` accepts ONLY the rung/backend shape described
in D1. Three distinct legacy-usage surfaces all resolve through the
SAME `Rung::parse` hint (D1's mechanism above), so there is exactly
ONE hint-carrying error message a migrating operator ever sees,
regardless of which part of the old shape they left unmigrated:
1. A `routing.<kind>` or `aging.<kind>.to` value of `git`/`pg`/`r2`.
2. A top-level `tiers.<key>` where `<key>` is `git`/`pg`/`r2` (the old
   backend-named section) instead of `local`/`hot`/`cold`.
3. (Distinct failure mode, still loud, not silently accepted) — a
   rung-named `tiers.<rung>` block MISSING its `backend:` tag (the
   old shape's `{ root: ... }`/`{ dsn_env: ..., schema: ... }` bodies
   had no such key) fails `BackendConfigRaw`'s internally-tagged-enum
   deserialization with a wrapping `PolicyError` naming the required
   `backend: git|postgres|s3` key explicitly, rather than propagating
   `serde_yaml`'s raw "missing field" text unmodified.
Every `canon.yaml` in the tree is rewritten as part of this change's
implementation phase: every Rust test fixture (26 files, proposal.md's
Impact) plus `target/usage-review/loom/canon.yaml` (the one live
multi-tier dogfood dummy). `target/usage-review/eno-drift/canon.yaml`
and `najun-art-dummy/canon.yaml` are git-only scratch checkouts,
exercised by no test — migrating them is optional/best-effort, not a
blocking task.

**D4 — `Backend` is the vendor-name home; the concrete tier ADAPTERS
are untouched.** `GitTier`/`PgTier`/`R2Tier`'s read/write/age
implementations are not rewritten, not renamed, not moved — they
remain exactly what they are today, `crate::tier::Tier` conformers.
What changes is how a `TierRegistry`/CLI builder SELECTS one: instead
of matching on `TierKind::{Git,Pg,R2}` (identity), the selection is
"this rung's configured `backend:` tag says `git`/`postgres`/`s3`, so
construct/reuse a `GitTier`/`PgTier`/`R2Tier`". The adapters become
*backend implementations* a rung's config selects, never the tier's
own identity. `crates/canon-cli/src/tiers.rs`'s `attach_pg`/
`attach_r2` (the ONE shared per-backend degrade-or-propagate core, s22
`uniform-lenient-tier-build` D4) rename to `attach_postgres`/
`attach_s3` to match — their attach-or-degrade CONTRACT (a bare
`StoreError::TierUnavailable` degrades to `None`; a malformed config,
e.g. `validate_schema_ident` rejecting `tiers.hot.schema`, propagates
loud) is unchanged, only the function names and the concrete
`canon.yaml` paths their doc comments/tests cite move from `tiers.pg.*`
/`tiers.r2.*` to the rung-keyed equivalents.

**D5 — `TierRegistry` rekeys to `Rung`.** Three named optional fields,
one per rung (not per backend):
```rust
pub struct TierRegistry {
    policy: TierPolicy,
    local: Option<Arc<dyn Tier>>,
    hot: Option<Arc<dyn Tier>>,
    cold: Option<Arc<dyn Tier>>,
}
```
each populated with whichever concrete backend adapter (`GitTier`/
`PgTier`/`R2Tier`) `canon.yaml`'s `tiers.<rung>.backend` selected —
`Arc<dyn Tier>` erases which adapter it is at this layer, exactly as
`TierRegistry::git()`'s existing `Arc<dyn Tier>`-adjacent pattern
already does for callers that only need the trait surface.
`handle(rung: Rung) -> Result<Arc<dyn Tier>, StoreError>`,
`tiers_for_read`, `query`, `age_all` all rekey their `TierKind`
parameter/return types to `Rung` with no other logic change (routed-
rung-plus-aging-destination-if-different remains the exact read-fan-
out rule; aging-source-to-aging-destination remains the exact age
rule). `StoreError::TierUnavailable` grows a `backend: Option<Backend>`
field alongside `rung: Rung` — `None` when the rung was never
configured at all ("hot tier is not configured (no `tiers.hot` in
canon.yaml)"), `Some(backend)` when it was configured but its live
attach failed ("hot tier (postgres) is not attached (no live DSN)") —
so `canon query`'s failure names WHICH RUNG the requested kind needed
and, whenever it's knowable, WHICH BACKEND was behind it, matching s22
`query-tier-degradation`'s existing "named, never generic" contract
(`crates/canon-cli/src/tiers.rs`'s `TierCliError`/`read_tier`'s
`"tiers.pg not attached (no live DSN)"`-style messages) at the new,
correctly-decomposed granularity. `crates/canon-cli/src/tiers.rs`'s
s22 kind-scoped lenient attach (`build_lenient_tiers_for_kind`,
`tiers_needed_for`) rekeys its `TierKind` computation to `Rung`
identically — its lenient-reads/strict-aging semantics (D3 of s22's
own design: `canon tier age` keeps the STRICT all-or-nothing builder,
never the lenient one) are UNCHANGED, only the key type moves.

**D6 — Non-goals** (restated from proposal.md for design-doc
completeness): no new backend (still exactly git/postgres/S3); no
tier-adapter read/write/age logic change; closed 12-`RecordKind` set
unchanged; connector-never-authority preserved (`canon gate check`
byte-identical — `canon-gate` reads nothing from `canon-store`'s tier
vocabulary or `canon-report`'s boundary derivation, and this change
touches no `canon-gate` source file); `canon report` stays offline/
deterministic/drift-checkable (no live Postgres read added anywhere);
no pg→report materialization (a future opt-in "materialize the hot
rung for reporting" is explicitly out of scope); DuckLake is NOT
introduced — the cold rung is object-store parquet written by
`R2Tier` and read via DuckDB's plain `read_parquet`, exactly as today;
it is never renamed to, or reimplemented as, a DuckLake catalog.

## Risks / Trade-offs

- **[Risk] The hard migration (D3) touches 26+ test fixture files in
  one change** — a large mechanical diff with real regression surface
  if any fixture is missed or mistranslated. → **Mitigation**: the
  `git`→`local`, `pg`→`hot`, `r2`→`cold` mapping is a pure,
  reversible value substitution today (each backend currently backs
  exactly one rung) — no fixture's ROUTING SEMANTICS change, only the
  literal string each fixture's `canon.yaml` text uses. `cargo test
  --workspace` after the mechanical rewrite is the acceptance gate;
  a fixture whose semantics were accidentally altered (not merely
  relabeled) fails an existing assertion, not a new one.
- **[Risk] `Backend::offline_file_readable()` becoming the ONE
  capability gate for report inclusion means a FUTURE non-default
  rung↔backend pairing (e.g. `cold` backed by `postgres`) silently
  changes report coverage** for whoever configures it that way. →
  **Mitigation**: this is the INTENDED, correct behavior (D2) — the
  report's contract was always "offline-file-readable data only", not
  "cold-and-local-rung data only"; today's coincidental 1:1
  rung↔backend pairing made the two indistinguishable, which is
  exactly the bug D2 fixes. An operator who deliberately backs `cold`
  with Postgres is choosing a live-queryable cold tier and should see
  it excluded from the offline report, identically to how `hot` is
  excluded today.
- **[Trade-off] `TierRegistry`'s rung fields hold `Arc<dyn Tier>`
  instead of three concretely-typed `Arc<GitTier>`/`Arc<PgTier>`/
  `Arc<R2Tier>` fields** (today's `git()` accessor returns the
  concrete `Arc<GitTier>` for callers needing git-specific behavior,
  e.g. `--plugin`'s git-tree resolution). → **Mitigation**: `git()`'s
  concrete-type accessor is preserved as a SEPARATE, additional field/
  method (the `local` rung's backend is git by convention in every
  configured repo today; a caller needing git-tree resolution
  specifically, not "whatever backs the local rung", keeps using
  a dedicated accessor) — this is an implementation-phase detail, not
  a design change to `--plugin`'s existing "always attach git
  unconditionally" behavior (s22 D2/R1).
- **[Risk] `serde_yaml::Value`-mediated two-step decode (D1) is more
  code than a direct typed deserialize** — a real complexity cost. →
  **Mitigation**: it's the smallest mechanism that gets ALL THREE
  legacy-shape failures (D3) onto the same `Rung::parse` hint text
  instead of two different qualities of error message (a friendly one
  for `routing`/`aging` values, an opaque `serde_yaml` one for a
  legacy `tiers.pg` key) — accepted as the right trade for a
  consistent migration experience on a HARD, one-shot cutover where
  every operator will hit this exactly once.

## Migration Plan

No live production `canon.yaml` to preserve compatibility for
(operator directive, D3) — this is a rewrite, not a phased rollout.
Every in-repo fixture is mechanically rewritten (`git→local`,
`pg→hot`, `r2→cold`, plus the `tiers:` block restructuring each entry
under its rung key with an explicit `backend:` tag) as part of this
change's own implementation tasks, gated by the full existing test
suite passing unmodified in SEMANTICS (fixture text changes; assertion
text/counts do not, except where an assertion's own string embeds the
old vocabulary, e.g. `"tiers.pg not attached"` → `"hot tier (postgres)
is not attached"`, tracked file-by-file in tasks.md). Rollback is
reverting this change's commit — no external state, no database
migration, no R2 object layout change (D6: the cold rung's on-disk
parquet shape is untouched).

## Open Questions

None — every question flagged during scoping (rung↔backend pairing
flexibility, report-inclusion axis, migration compatibility posture)
is resolved by D1-D6 above; none are deferred.
