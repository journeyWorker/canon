//! Builds the live tier handles (`GitTier`/`PgTier`/`SqliteTier`/`R2Tier`)
//! a repo's `canon.yaml` configures — the CLI-only "wire `canon.yaml` to
//! live tier objects" glue every subcommand needs before reaching
//! `canon_store::registry::TierRegistry`. `crates/canon-store` ships
//! the policy parser and the tier adapters themselves (S2); this
//! module is exactly the remaining gap
//! `canon/skills/tiered-storage/SKILL.md` names ("CLI subcommand
//! wiring … is a later change's scope").
//!
//! # Rung-first construction (s27 `tier-role-backend-split`, design D1)
//! `canon.yaml`'s `tiers:` section is keyed by [`Rung`], each entry
//! tagging its own `backend:`. Every builder below resolves, for each
//! rung `policy.tiers` declares, WHICH backend it names, and attaches
//! the matching adapter (`GitTier`/`PgTier`/`SqliteTier`/`R2Tier`) into
//! `LoadedTiers`' shared `git`/`pg`/`sqlite`/`r2` slots — by today's
//! convention (design D1's own "not a type-level constraint" caveat)
//! each backend is configured for at most one rung at a time, so one
//! live adapter instance per backend suffices. `sqlite` (s32
//! `sqlite-hot-backend`) is the second `LiveDb`-class backend the s28
//! class check was built to accommodate — it needs no env-var
//! indirection at all, so it has no `attach_*` "unset var" case, only
//! a genuine unreachable/corrupt-file degrade.
//!
//! Two families of builder live here: the STRICT, all-or-nothing
//! [`build_tiers`] (`canon tier age`'s only caller — s29 design D6
//! moved `canon ingest sessions`/`canon ingest artifacts` OFF this
//! strict path onto the kind-scoped lenient one below, so `canon tier
//! age`'s destructive move+delete remains the sole caller with no
//! partial-success story, hence a declared-but-unreachable tier stays
//! a startup-time hard error here), and the LENIENT, per-tier
//! degrade-or-propagate family ([`build_lenient_tiers`]/
//! [`build_lenient_tiers_for_kind`]/[`build_lenient_tiers_for_kinds`],
//! relocated from `plans.rs`/added respectively by s22
//! `query-tier-degradation`/`uniform-lenient-tier-build`, generalized
//! to a kind SET by s29 design D6) — `canon ingest plans`/`canon
//! query` (single-kind) and `canon ingest sessions`/`canon ingest
//! artifacts` (kind-SET) all share it: an unreachable-but-not-needed
//! `hot`/`cold` rung degrades to `None` instead of failing the whole
//! command, and [`LoadedTiers::unavailable_reasons`] carries WHY (the
//! configured env-var name, design D6) for a rung that WAS needed and
//! still degraded; see [`attach_postgres`]/[`attach_s3`] for the ONE
//! per-backend decision every lenient builder reuses.

use std::collections::BTreeMap;
use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_store::git_tier::GitTier;
use canon_store::pg_tier::{validate_schema_ident, PgTier};
use canon_store::policy::{BackendConfig, PgTierConfig, R2TierConfig, Rung, SqliteTierConfig, TierPolicy};
use canon_store::r2_tier::R2Tier;
use canon_store::sqlite_tier::SqliteTier;
use canon_store::tier::{StoreError, Tier, TierQuery, TierReadResult};

/// Recognized ONLY by this CLI — never by `canon-store` itself — so an
/// offline integration test can exercise a genuine git+r2 aging move /
/// query fan-out without live R2 credentials or network. This is the
/// same "rebindable root" convention `canon/skills/tiered-storage/
/// SKILL.md` already documents for the DuckDB views (`CANON_R2_ROOT`),
/// applied here to the CLI's own tier construction. Unset in ordinary
/// use: a `canon.yaml`-configured s3-backed rung always resolves via
/// `R2Tier::connect_live` there — this env var is a test seam, never a
/// silent production fallback.
///
/// That "never a silent production fallback" claim is HARD-ENFORCED,
/// not just documented: [`build_r2_tier`] only reads this var behind
/// `#[cfg(debug_assertions)]`, which a `--release` build compiles OUT
/// entirely. `canon tier age` does a destructive move+delete against
/// whichever `r2` handle `build_tiers` resolves; a stray
/// `CANON_R2_LOCAL_ROOT` surviving into a production shell must NEVER
/// be able to silently redirect that mutation (or `canon query`'s read
/// fan-out) at a local filesystem instead of the real bucket. A
/// release `canon` binary therefore ignores this var unconditionally
/// and always calls `R2Tier::connect_live`, which itself fails loud
/// (`StoreError::BackendUnattached`) when it can't attach — never a
/// silent local substitution.
pub const LOCAL_R2_ROOT_ENV: &str = "CANON_R2_LOCAL_ROOT";

#[derive(Debug, thiserror::Error)]
pub enum TierCliError {
    #[error("reading `{path}`: {source}")]
    ReadCanonYaml { path: String, source: std::io::Error },
    #[error("parsing `{path}`: {source}")]
    Policy { path: String, source: canon_store::policy::PolicyError },
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// A `canon.yaml`'s policy plus whichever tier handles it configured —
/// `git`/`pg`/`r2`/`sqlite` (the concrete backend adapters, s27 design
/// D1; `sqlite` added by s32 `sqlite-hot-backend`) wired in wherever a
/// rung's `tiers.<rung>.backend` tag names them. `git` attaches
/// unconditionally when the `local` rung's backend is git (local disk,
/// zero network); `pg`/`r2`/`sqlite` only when their own rung's block
/// exists (design §9 local-first: an unconfigured rung is never
/// attempted).
pub struct LoadedTiers {
    pub policy: TierPolicy,
    pub git: Option<GitTier>,
    pub pg: Option<PgTier>,
    pub r2: Option<R2Tier>,
    /// s32 `sqlite-hot-backend`: the second `LiveDb`-class hot backend
    /// (s28's reserved seam), attached exactly like `pg` — via
    /// [`attach_sqlite`] in the lenient builders, or directly in
    /// [`build_tiers`]'s strict path.
    pub sqlite: Option<SqliteTier>,
    /// Per-rung reason a NEEDED rung was ATTEMPTED and degraded to
    /// unattached (s29 design D6) — populated ONLY by the LENIENT
    /// builders ([`build_lenient_tiers`]/[`build_lenient_tiers_for_kind`]/
    /// [`build_lenient_tiers_for_kinds`]), via [`attach_postgres`]/
    /// [`attach_s3`]/[`attach_sqlite`]'s own resolved reason (the
    /// configured env-var name, or the live connect/attach error) —
    /// never a rung that was simply never attempted. The STRICT
    /// [`build_tiers`] never populates this (a strict failure aborts
    /// loud with `Err` instead of degrading, so there is nothing to
    /// carry). A caller (`canon ingest sessions`/`canon ingest
    /// artifacts`) reads this to print WHY a needed rung degraded —
    /// the configured env-var name — instead of a bare "tiers
    /// unreachable" guess.
    pub unavailable_reasons: BTreeMap<Rung, String>,
}

/// `canon_yaml_path`'s own directory (not the process's CWD) -- the
/// PROJECT root every `canon.yaml`-relative path (`tiers.local.root`,
/// `tiers.<rung>.path` for a sqlite-backed rung, and s16's
/// `canon/plugins/<id>/plugin.yaml`, `resolve_plugin_
/// snapshot::PLUGINS_DIR_RELATIVE_PATH`) resolves against, so `canon`
/// behaves the same regardless of where it is invoked from within a
/// repo checkout. `.` when `canon_yaml_path` carries no parent
/// component (a bare `canon.yaml` relative to cwd).
pub fn project_dir(canon_yaml_path: &Path) -> &Path {
    canon_yaml_path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."))
}

/// Parse `canon_yaml_path` and connect every rung it configures, all-
/// or-nothing (`canon tier age`'s only caller — a declared-but-
/// unreachable rung is a startup-time hard error, never a silent
/// skip). `tiers.<rung>.root` (for a git-backed rung) resolves
/// relative to [`project_dir`], not the process's CWD; a sqlite-
/// backed rung's `path` is resolved the SAME way by
/// `TierPolicy::from_yaml_at` itself (s32), so it arrives here
/// already absolute.
pub fn build_tiers(canon_yaml_path: &Path) -> Result<LoadedTiers, TierCliError> {
    let display_path = canon_yaml_path.display().to_string();
    let yaml = std::fs::read_to_string(canon_yaml_path)
        .map_err(|source| TierCliError::ReadCanonYaml { path: display_path.clone(), source })?;
    let base = project_dir(canon_yaml_path);
    let policy = TierPolicy::from_yaml_at(&yaml, base).map_err(|source| TierCliError::Policy { path: display_path, source })?;

    let mut git = None;
    let mut pg = None;
    let mut r2 = None;
    let mut sqlite = None;

    // A stable (rung-`Ord`-sorted) iteration order so a multi-rung
    // failure always names the SAME first offender across runs.
    let mut rungs: Vec<Rung> = policy.tiers.keys().copied().collect();
    rungs.sort();

    for rung in rungs {
        match &policy.tiers[&rung] {
            BackendConfig::Git(cfg) => git = Some(GitTier::new(base.join(&cfg.root))),
            BackendConfig::Postgres(cfg) => {
                // s29 design D8: schema validated BEFORE the `dsn_env`
                // lookup — matches `attach_postgres`'s already-fixed
                // ordering (s22), so a malformed `tiers.<rung>.schema`
                // is never masked by an unset-DSN error naming the
                // wrong problem first.
                validate_schema_ident(&cfg.schema)?;
                let dsn = std::env::var(&cfg.dsn_env).map_err(|_| {
                    StoreError::tier_unavailable(rung, Some(canon_store::policy::Backend::Postgres), format!("`{}` is unset", cfg.dsn_env))
                })?;
                pg = Some(PgTier::connect(&dsn, &cfg.schema)?);
            }
            BackendConfig::S3(cfg) => {
                // Mirror the Postgres arm: a declared-but-unattached
                // cold/s3 rung names the RUNG + backend ("cold tier (s3)
                // is not attached (…)"), not the rung-less
                // `BackendUnattached` `build_r2_tier` returns on its own.
                r2 = Some(build_r2_tier(cfg).map_err(|e| match e {
                    StoreError::BackendUnattached { backend, reason } => {
                        StoreError::tier_unavailable(rung, Some(backend), reason)
                    }
                    other => other,
                })?);
            }
            BackendConfig::Sqlite(cfg) => {
                // No env indirection to resolve (s32): `cfg.path` is
                // already absolute (`from_yaml_at`), so this is a
                // direct connect — `SqliteTier::connect` itself
                // classifies an unreachable/corrupt file as
                // `StoreError::TierUnavailable` (`Rung::Hot`
                // hardcoded internally, mirroring `PgTier::connect`),
                // which the strict path here simply propagates loud
                // via `?`.
                sqlite = Some(SqliteTier::connect(&cfg.path)?);
            }
        }
    }

    Ok(LoadedTiers { policy, git, pg, r2, sqlite, unavailable_reasons: BTreeMap::new() })
}

/// Resolve `cfg`'s s3-backed tier handle. In DEBUG builds only, honors
/// [`LOCAL_R2_ROOT_ENV`] as an offline test seam (`R2Tier::local`) —
/// the `#[cfg(debug_assertions)]` gate below is load-bearing, not
/// cosmetic. `cargo build --release` compiles the whole `if` branch
/// out of the binary, so no env var — however it got into a
/// production shell — can ever substitute a local filesystem for the
/// real bucket `canon tier age`'s destructive move+delete (or `canon
/// query`'s read fan-out) targets. A release binary always takes the
/// `R2Tier::connect_live` path below, which fails loud
/// (`StoreError::BackendUnattached`) rather than silently succeeding
/// against the wrong backing store. Integration tests build in the
/// `dev`/`test` profile (`debug_assertions` on), so the override keeps
/// working for them.
pub(crate) fn build_r2_tier(cfg: &R2TierConfig) -> Result<R2Tier, StoreError> {
    #[cfg(debug_assertions)]
    if let Ok(local_root) = std::env::var(LOCAL_R2_ROOT_ENV) {
        eprintln!(
            "canon: {LOCAL_R2_ROOT_ENV} is set — substituting a LOCAL filesystem s3 tier at `{local_root}` \
             (debug-build test seam only; a release `canon` binary ignores this var and always attaches live s3)"
        );
        return R2Tier::local(local_root, cfg.prefix.clone());
    }
    R2Tier::connect_live(&cfg.bucket_env, &cfg.prefix)
}

/// Attach `cfg`'s postgres-backed tier, degrading a genuine
/// unreachability (`StoreError::TierUnavailable` — an unset
/// `dsn_env`, or `PgTier::connect`'s own live-connect-outage case,
/// s29 design D7 classifies BOTH the same way now) to `(None,
/// Some(reason))`; any OTHER error -- most notably `StoreError::Policy`
/// from [`validate_schema_ident`] (a `tiers.<rung>.schema` that fails
/// `[a-z0-9_]+`, rejected BEFORE any socket opens) or a genuine
/// `Io`/`Sql`/`Json` failure -- propagates loud. The schema is
/// validated BEFORE the `dsn_env` lookup so an unset-DSN degrade can
/// never mask a malformed schema (design.md, "lenient" describes
/// per-tier reachability, never config correctness). The returned
/// `Option<String>` is the degrade reason (s29 design D6) -- the
/// configured env-var name for an unset DSN, or the live connect
/// error's own text -- so a caller can carry WHY into its own outcome
/// instead of a bare guess; `None` on a successful attach. Shared by
/// [`build_lenient_tiers`] (whole-policy) and
/// [`build_lenient_tiers_for_kinds`] (kind-scoped) -- the ONE
/// degrade-or-propagate decision for the postgres backend in this
/// module (s22 design.md D4, renamed from `attach_pg` per s27 design D4
/// to name the BACKEND it attaches).
fn attach_postgres(cfg: &PgTierConfig) -> Result<(Option<PgTier>, Option<String>), TierCliError> {
    validate_schema_ident(&cfg.schema)?;
    match std::env::var(&cfg.dsn_env) {
        Err(_) => Ok((None, Some(format!("`{}` is unset", cfg.dsn_env)))),
        Ok(dsn) => match PgTier::connect(&dsn, &cfg.schema) {
            Ok(tier) => Ok((Some(tier), None)),
            Err(StoreError::TierUnavailable { reason, .. }) => Ok((None, Some(reason))),
            Err(err) => Err(TierCliError::Store(err)),
        },
    }
}

/// Attach `cfg`'s s3-backed tier via [`build_r2_tier`], degrading a
/// genuine `StoreError::BackendUnattached` (bucket credential
/// absent/unreachable) to `(None, Some(reason))`; any other error
/// propagates loud. The returned `Option<String>` mirrors
/// [`attach_postgres`]'s own degrade-reason contract (s29 design D6).
/// Shared by [`build_lenient_tiers`]/[`build_lenient_tiers_for_kinds`]
/// -- the ONE degrade-or-propagate decision for the s3 backend
/// (renamed from `attach_r2` per s27 design D4).
fn attach_s3(cfg: &R2TierConfig) -> Result<(Option<R2Tier>, Option<String>), TierCliError> {
    match build_r2_tier(cfg) {
        Ok(tier) => Ok((Some(tier), None)),
        Err(StoreError::BackendUnattached { reason, .. }) => Ok((None, Some(reason))),
        Err(err) => Err(TierCliError::Store(err)),
    }
}

/// Attach `cfg`'s sqlite-backed tier via `SqliteTier::connect`,
/// degrading a genuine `StoreError::TierUnavailable` (unopenable/
/// corrupt db file — `cfg.path` names it in the reason) to `(None,
/// Some(reason))`; any other error (e.g. `StoreError::Sql` from a
/// post-connect DDL failure) propagates loud. Unlike
/// [`attach_postgres`], there is no env-var indirection to resolve
/// first (s32: no `dsn_env`-shaped unset-var case) — `cfg.path` is
/// already an absolute, canon.yaml-resolved path
/// (`TierPolicy::from_yaml_at`), so this is a direct connect attempt.
/// The returned `Option<String>` mirrors [`attach_postgres`]'s own
/// degrade-reason contract (s29 design D6). Shared by
/// [`build_lenient_tiers`]/[`build_lenient_tiers_for_kinds`] -- the
/// ONE degrade-or-propagate decision for the sqlite backend in this
/// module.
fn attach_sqlite(cfg: &SqliteTierConfig) -> Result<(Option<SqliteTier>, Option<String>), TierCliError> {
    match SqliteTier::connect(&cfg.path) {
        Ok(tier) => Ok((Some(tier), None)),
        Err(StoreError::TierUnavailable { reason, .. }) => Ok((None, Some(reason))),
        Err(err) => Err(TierCliError::Store(err)),
    }
}

/// `git`/`pg`/`r2`/`sqlite` tier handles [`build_lenient_tiers`]/
/// [`build_lenient_tiers_for_kind`] resolve -- named solely to keep
/// their `Result<_, TierCliError>` signatures under clippy's
/// `type_complexity` threshold.
pub(crate) type LenientTiers = (Option<GitTier>, Option<PgTier>, Option<R2Tier>, Option<SqliteTier>);

/// Attach whichever backend each rung `policy` configured, PER RUNG,
/// each independently lenient (relocated from `plans.rs`, design.md
/// D1/D3, `uniform-lenient-tier-build` spec): a git-backed rung never
/// fails to attach (a local directory, no I/O at construction) -- it
/// is simply left `None` when no rung is git-backed. A postgres/s3-
/// backed rung degrades to `None` ONLY for genuine unreachability via
/// [`attach_postgres`]/[`attach_s3`] -- a malformed configuration
/// still propagates loud. This is the WHOLE-POLICY variant `canon
/// ingest plans` uses (a plan-import pass may persist several
/// different kinds' worth of records in one run, so it cannot scope to
/// a single kind up front) -- attempts every rung `policy` declares,
/// regardless of which kinds the current pass happens to touch. See
/// [`build_lenient_tiers_for_kinds`] for the kind-scoped sibling.
/// `attach_postgres`/`attach_s3` also resolve a degrade REASON now
/// (s29 design D6); `canon ingest plans` has no per-kind outcome shape
/// to carry one into yet, so this whole-policy variant discards it --
/// unlike [`build_lenient_tiers_for_kinds`], which threads it into
/// [`LoadedTiers::unavailable_reasons`].
pub(crate) fn build_lenient_tiers(policy: &TierPolicy, project_dir: &Path) -> Result<LenientTiers, TierCliError> {
    let mut git = None;
    let mut pg = None;
    let mut r2 = None;
    let mut sqlite = None;
    for cfg in policy.tiers.values() {
        match cfg {
            BackendConfig::Git(cfg) => git = Some(GitTier::new(project_dir.join(&cfg.root))),
            BackendConfig::Postgres(cfg) => pg = attach_postgres(cfg)?.0,
            BackendConfig::S3(cfg) => r2 = attach_s3(cfg)?.0,
            BackendConfig::Sqlite(cfg) => sqlite = attach_sqlite(cfg)?.0,
        }
    }
    Ok((git, pg, r2, sqlite))
}

/// Every [`Rung`] `kind`'s read fan-out might need: its `routing`
/// destination, PLUS its `aging.to` destination when one exists and
/// differs from the routed rung -- mirrors
/// `canon_store::registry::TierRegistry::tiers_for_read`'s own
/// two-fact combination (design.md D2), recomputed here independently
/// since `TierPolicy::tier_for`/`.routing`/`.aging` are already `pub`
/// and a `TierRegistry` does not exist yet at tier-CONSTRUCTION time
/// (a chicken-and-egg `TierRegistry::tiers_for_read` cannot resolve
/// for its own caller). `Err` (never a silent default) when `kind` has
/// no `routing` entry at all, exactly as `tier_for` itself reports.
pub(crate) fn tiers_needed_for(policy: &TierPolicy, kind: RecordKind) -> Result<Vec<Rung>, StoreError> {
    let routed = policy.tier_for(kind)?;
    let mut rungs = vec![routed];
    if let Some(rule) = policy.aging.get(&kind) {
        if rule.to != routed {
            rungs.push(rule.to);
        }
    }
    Ok(rungs)
}

/// The KIND-SET generalization of [`build_lenient_tiers_for_kind`]
/// (s29 design D6): parses `canon_yaml_path` (reusing [`build_tiers`]'s
/// own parse step), attaches the `local` rung's backend
/// UNCONDITIONALLY when configured -- never scoped by any of `kinds`'
/// own routing (design.md D2/R1: `--plugin`'s git-tree resolution
/// needs `loaded.git` regardless of which kind was queried) -- and
/// attempts the UNION of every rung ANY of `kinds`' own read fan-out
/// needs ([`tiers_needed_for`]) via the SAME [`attach_postgres`]/
/// [`attach_s3`] degrade-or-propagate core every lenient builder in
/// this module shares (design.md D4). A rung no kind in `kinds` needs
/// is never even attempted -- so an unreachable-but-irrelevant
/// `hot`/`cold` block in `canon.yaml` never affects a pass whose kinds
/// don't route (or age) to it, exactly like the single-kind sibling.
///
/// An INDIVIDUAL kind with no `routing` entry contributes NO rungs
/// (never a hard `Err`) -- this is the SAME "policy hasn't routed
/// session/run/event yet" degrade `crate::ingest`'s own module doc
/// documents, a legitimate config state, never a malformed-config
/// failure; a caller that needs every one of `kinds` to be routed
/// checks `loaded.policy.tier_for(kind)` itself afterward (`crate::
/// ingest::run`'s own `fully_routed` check is exactly this). Only a
/// genuinely malformed `canon.yaml` (unreadable, bad YAML/policy
/// syntax, an invalid pg schema, a non-forward aging rule, …)
/// propagates `Err` here -- "lenient" describes RUNG reachability
/// only, config correctness always stays loud.
///
/// [`LoadedTiers::unavailable_reasons`] carries the build-time reason
/// for every rung that WAS attempted (because some kind in `kinds`
/// needs it) and degraded -- e.g. the configured `dsn_env`/`bucket_env`
/// name for an unset var, or a live connect/attach error's own text
/// (s29 design D6) -- so a caller (`canon ingest sessions`/`canon
/// ingest artifacts`) can print WHY a needed rung is unwritten instead
/// of a bare "tiers unreachable" guess.
pub(crate) fn build_lenient_tiers_for_kinds(canon_yaml_path: &Path, kinds: &[RecordKind]) -> Result<LoadedTiers, TierCliError> {
    let display_path = canon_yaml_path.display().to_string();
    let yaml = std::fs::read_to_string(canon_yaml_path)
        .map_err(|source| TierCliError::ReadCanonYaml { path: display_path.clone(), source })?;
    let base = project_dir(canon_yaml_path);
    let policy = TierPolicy::from_yaml_at(&yaml, base).map_err(|source| TierCliError::Policy { path: display_path, source })?;

    let mut git = None;
    let mut pg = None;
    let mut r2 = None;
    let mut sqlite = None;
    let mut unavailable_reasons: BTreeMap<Rung, String> = BTreeMap::new();

    // The local rung's backend always attaches unconditionally,
    // independent of any `kind`'s own routing (design D2/R1).
    match policy.tiers.get(&Rung::Local) {
        Some(BackendConfig::Git(cfg)) => git = Some(GitTier::new(base.join(&cfg.root))),
        Some(BackendConfig::Postgres(cfg)) => {
            let (tier, reason) = attach_postgres(cfg)?;
            pg = tier;
            if let Some(reason) = reason {
                unavailable_reasons.insert(Rung::Local, reason);
            }
        }
        Some(BackendConfig::S3(cfg)) => {
            let (tier, reason) = attach_s3(cfg)?;
            r2 = tier;
            if let Some(reason) = reason {
                unavailable_reasons.insert(Rung::Local, reason);
            }
        }
        Some(BackendConfig::Sqlite(cfg)) => {
            let (tier, reason) = attach_sqlite(cfg)?;
            sqlite = tier;
            if let Some(reason) = reason {
                unavailable_reasons.insert(Rung::Local, reason);
            }
        }
        None => {}
    }

    // The UNION of every rung ANY of `kinds` needs -- an individually
    // unrouted kind contributes nothing (see doc comment above),
    // never a hard failure; a rung already resolved (`local` above,
    // or shared between two kinds) is de-duplicated so it is never
    // attempted twice.
    let mut needed: Vec<Rung> = Vec::new();
    for kind in kinds {
        if let Ok(rungs) = tiers_needed_for(&policy, *kind) {
            for rung in rungs {
                if !needed.contains(&rung) {
                    needed.push(rung);
                }
            }
        }
    }

    for rung in needed {
        if rung == Rung::Local {
            continue;
        }
        match policy.tiers.get(&rung) {
            Some(BackendConfig::Git(cfg)) if git.is_none() => git = Some(GitTier::new(base.join(&cfg.root))),
            Some(BackendConfig::Postgres(cfg)) if pg.is_none() => {
                let (tier, reason) = attach_postgres(cfg)?;
                pg = tier;
                if let Some(reason) = reason {
                    unavailable_reasons.insert(rung, reason);
                }
            }
            Some(BackendConfig::S3(cfg)) if r2.is_none() => {
                let (tier, reason) = attach_s3(cfg)?;
                r2 = tier;
                if let Some(reason) = reason {
                    unavailable_reasons.insert(rung, reason);
                }
            }
            Some(BackendConfig::Sqlite(cfg)) if sqlite.is_none() => {
                let (tier, reason) = attach_sqlite(cfg)?;
                sqlite = tier;
                if let Some(reason) = reason {
                    unavailable_reasons.insert(rung, reason);
                }
            }
            _ => {}
        }
    }

    Ok(LoadedTiers { policy, git, pg, r2, sqlite, unavailable_reasons })
}

/// The RUNG-SCOPED, single-kind sibling of
/// [`build_lenient_tiers_for_kinds`] (design.md D2,
/// `query-tier-degradation` spec) -- `canon query`/`canon query
/// --plugin` call this when a pass is scoped to exactly one kind. An
/// unrouted `kind` is tolerated at BUILD time by the plural builder
/// (see its own doc comment) but every caller of this single-kind
/// wrapper immediately queries that one kind afterward via
/// `TierRegistry::query`, so the identical `StoreError::UnroutedKind`
/// still surfaces -- just one call-frame later, byte-identical
/// message -- exactly matching this function's pre-s29 hard-fail-on-
/// unrouted contract.
pub(crate) fn build_lenient_tiers_for_kind(canon_yaml_path: &Path, kind: RecordKind) -> Result<LoadedTiers, TierCliError> {
    build_lenient_tiers_for_kinds(canon_yaml_path, &[kind])
}

/// Dispatch a read-only [`Tier::read`] to whichever backend `rung`'s
/// `canon.yaml` config names — the same one-arm-per-backend dispatch
/// `TierRegistry`'s own (private) resolver performs internally,
/// duplicated here ONLY for `canon tier age --dry-run`'s read-only
/// preview: a real run goes through `TierRegistry::age_all()` directly
/// (`crate::tier`) and never touches this function, so the
/// digest-keyed write-then-delete aging mechanism itself is never
/// reimplemented — this helper only ever reads.
pub fn read_tier(rung: Rung, loaded: &LoadedTiers, query: &TierQuery) -> Result<TierReadResult, StoreError> {
    use canon_store::policy::Backend;
    match loaded.policy.tiers.get(&rung).map(BackendConfig::backend) {
        Some(Backend::Git) => loaded
            .git
            .as_ref()
            .ok_or_else(|| StoreError::tier_unavailable(rung, Some(Backend::Git), Backend::Git.default_unattached_reason()))?
            .read(query),
        Some(Backend::Postgres) => loaded
            .pg
            .as_ref()
            .ok_or_else(|| StoreError::tier_unavailable(rung, Some(Backend::Postgres), Backend::Postgres.default_unattached_reason()))?
            .read(query),
        Some(Backend::S3) => loaded
            .r2
            .as_ref()
            .ok_or_else(|| StoreError::tier_unavailable(rung, Some(Backend::S3), Backend::S3.default_unattached_reason()))?
            .read(query),
        Some(Backend::Sqlite) => loaded
            .sqlite
            .as_ref()
            .ok_or_else(|| StoreError::tier_unavailable(rung, Some(Backend::Sqlite), Backend::Sqlite.default_unattached_reason()))?
            .read(query),
        None => Err(StoreError::tier_unavailable(rung, None, format!("no `tiers.{}` in canon.yaml", rung.as_str()))),
    }
}

/// Serializes the process-global `CANON_R2_LOCAL_ROOT` debug seam across
/// the (otherwise parallel) tests that set it — without this lock a
/// setter (`kind_scoped_build_actually_attaches_a_reachable_aged_r2_tier`,
/// `debug_build_honors_local_r2_root_env_var_as_a_test_seam`) could flip
/// the env var mid-build for a reader that asserts the opposite state
/// (`kind_scoped_build_attempts_both_tiers_...`). Poison-tolerant: a
/// panicking guarded test still releases a usable lock to the next.
#[cfg(test)]
static R2_SEAM_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    /// The debug/test path this override exists for keeps working:
    /// `cargo test`'s dev profile has `debug_assertions` on, exactly
    /// like `crates/canon-cli/tests/support::Fixture::run_canon`'s
    /// built `canon` binary — the same substitution the integration
    /// suite (`tier_age.rs`, `query.rs`) relies on offline.
    #[cfg(debug_assertions)]
    #[test]
    fn debug_build_honors_local_r2_root_env_var_as_a_test_seam() {
        let _seam = super::R2_SEAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(LOCAL_R2_ROOT_ENV, dir.path());
        let cfg = R2TierConfig { bucket_env: "CANON_R2_BUCKET_UNUSED_IN_THIS_TEST".to_string(), prefix: "canon/".to_string() };
        let result = build_r2_tier(&cfg);
        std::env::remove_var(LOCAL_R2_ROOT_ENV);
        result.expect("a dev/test-profile build must still honor the local-r2 test seam `Fixture::run_canon` relies on");
    }

    /// The production-safety gate itself, asserted directly (IMP-1):
    /// under a RELEASE build (`debug_assertions` off — `cargo test
    /// --release`), `CANON_R2_LOCAL_ROOT` must NOT substitute a local
    /// filesystem; `build_r2_tier` must fall straight through to
    /// `R2Tier::connect_live` and fail loud (never silently succeed
    /// against the wrong backing store) when it predictably can't
    /// attach in a test process with no live bucket/credentials. Only
    /// compiled and run under `cargo test --release` (a release
    /// binary is exactly what this asserts about); the default `cargo
    /// test --workspace` (dev profile) never builds this test at all
    /// — the in-process regression net for the release-binary smoke
    /// test this finding's own acceptance bar requires.
    #[cfg(not(debug_assertions))]
    #[test]
    fn release_build_ignores_local_r2_root_env_var_and_fails_loud() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(LOCAL_R2_ROOT_ENV, dir.path());
        let cfg = R2TierConfig { bucket_env: "CANON_R2_RELEASE_TEST_BUCKET_UNSET".to_string(), prefix: "canon/".to_string() };
        let result = build_r2_tier(&cfg);
        std::env::remove_var(LOCAL_R2_ROOT_ENV);
        let err = result.expect_err("a release build must NEVER silently substitute a local r2 tier from CANON_R2_LOCAL_ROOT");
        assert!(
            matches!(err, StoreError::BackendUnattached { backend: canon_store::policy::Backend::S3, .. }),
            "expected BackendUnattached (s3 backend not attached), got {err:?}"
        );
    }
}

// ── s22 `query-tier-degradation` / `uniform-lenient-tier-build` ──
#[cfg(test)]
mod lenient_tier_tests {
    use std::path::PathBuf;

    use canon_store::policy::AgingRuleConfig;

    use super::*;

    fn policy_with(routing: &[(RecordKind, Rung)], aging: &[(RecordKind, Rung)]) -> TierPolicy {
        let mut routing_map = std::collections::HashMap::new();
        for (kind, rung) in routing {
            routing_map.insert(*kind, *rung);
        }
        let mut aging_map = std::collections::HashMap::new();
        for (kind, to) in aging {
            aging_map.insert(*kind, AgingRuleConfig { after: chrono::Duration::days(30), to: *to });
        }
        TierPolicy { tiers: std::collections::HashMap::new(), routing: routing_map, aging: aging_map }
    }

    /// design.md D2/task 2.3: a local-routed, never-aged kind
    /// needs ONLY `local`.
    #[test]
    fn a_local_routed_kind_needs_local_only() {
        let policy = policy_with(&[(RecordKind::Change, Rung::Local)], &[]);
        assert_eq!(tiers_needed_for(&policy, RecordKind::Change).unwrap(), vec![Rung::Local]);
    }

    /// task 2.3: a hot-routed, never-aged kind needs ONLY `hot`.
    #[test]
    fn a_hot_routed_non_aged_kind_needs_hot_only() {
        let policy = policy_with(&[(RecordKind::Task, Rung::Hot)], &[]);
        assert_eq!(tiers_needed_for(&policy, RecordKind::Task).unwrap(), vec![Rung::Hot]);
    }

    /// task 2.3 / design.md R2: a hot-routed, cold-aged kind (e.g.
    /// `handoff` per canon.yaml) needs BOTH, routed rung first.
    #[test]
    fn a_hot_routed_cold_aged_kind_needs_both() {
        let policy = policy_with(&[(RecordKind::Handoff, Rung::Hot)], &[(RecordKind::Handoff, Rung::Cold)]);
        assert_eq!(tiers_needed_for(&policy, RecordKind::Handoff).unwrap(), vec![Rung::Hot, Rung::Cold]);
    }

    /// An aging destination IDENTICAL to the routed rung never
    /// duplicates it (mirrors `TierRegistry::tiers_for_read`).
    #[test]
    fn an_aging_destination_equal_to_the_routed_tier_is_not_duplicated() {
        let policy = policy_with(&[(RecordKind::Change, Rung::Local)], &[(RecordKind::Change, Rung::Local)]);
        assert_eq!(tiers_needed_for(&policy, RecordKind::Change).unwrap(), vec![Rung::Local]);
    }

    /// An unrouted kind fails NAMED (`StoreError::UnroutedKind`),
    /// exactly as `TierPolicy::tier_for`/`TierRegistry::tiers_for_read`
    /// already report for every other caller.
    #[test]
    fn an_unrouted_kind_fails_naming_the_kind() {
        let policy = policy_with(&[], &[]);
        let err = tiers_needed_for(&policy, RecordKind::Change).unwrap_err();
        assert!(matches!(err, StoreError::UnroutedKind { kind: RecordKind::Change }), "expected UnroutedKind, got {err:?}");
    }

    fn write_multi_tier_canon_yaml(dir: &Path) -> PathBuf {
        let path = dir.join("canon.yaml");
        std::fs::write(
            &path,
            "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S22_UNIT_UNSET, schema: canon_v1 }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_S22_UNIT_UNSET, prefix: \"canon/\" }\nrouting:\n  change: local\n  task: hot\n  handoff: hot\naging:\n  handoff: { after: 30d, to: cold }\n",
        )
        .unwrap();
        path
    }

    /// task 2.2/3.3: a local-routed kind's rung-scoped build never
    /// even attempts `hot`/`cold` — succeeds with NEITHER
    /// `CANON_PG_DSN_S22_UNIT_UNSET` nor `CANON_R2_BUCKET_S22_UNIT_UNSET`
    /// set (release-safe: no `CANON_R2_LOCAL_ROOT` test seam needed
    /// either, since `cold` is never attempted at all for this kind).
    #[test]
    fn kind_scoped_build_never_attempts_hot_or_cold_for_a_local_routed_kind() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml = write_multi_tier_canon_yaml(dir.path());
        std::env::remove_var("CANON_PG_DSN_S22_UNIT_UNSET");
        std::env::remove_var("CANON_R2_BUCKET_S22_UNIT_UNSET");
        std::env::remove_var(LOCAL_R2_ROOT_ENV);

        let loaded = build_lenient_tiers_for_kind(&canon_yaml, RecordKind::Change)
            .expect("a local-routed kind must never require hot/cold credentials to build");
        assert!(loaded.git.is_some(), "the local rung is configured, so it must attach unconditionally");
        assert!(loaded.pg.is_none(), "hot was never attempted for a local-routed kind");
        assert!(loaded.r2.is_none(), "cold was never attempted for a local-routed kind");
    }

    /// task 2.2/3.4: a hot-routed kind's rung-scoped build degrades an
    /// unset DSN to `None` (never a hard error at BUILD time — the
    /// query.rs-level failure is `TierRegistry::query`'s job, named).
    #[test]
    fn kind_scoped_build_degrades_an_unreachable_hot_to_none_for_a_hot_routed_kind() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml = write_multi_tier_canon_yaml(dir.path());
        std::env::remove_var("CANON_PG_DSN_S22_UNIT_UNSET");

        let loaded = build_lenient_tiers_for_kind(&canon_yaml, RecordKind::Task)
            .expect("an unreachable hot rung must degrade, not hard-fail, at build time");
        assert!(loaded.pg.is_none(), "the hot rung's dsn_env is unset — must degrade to None");
    }

    /// task 2.2/3.5: a hot-routed, cold-aged kind's rung-scoped build
    /// attempts BOTH rungs (design.md R2) — an unreachable `cold` alone
    /// still degrades to `None` even though `hot`'s DSN is unset too.
    #[test]
    fn kind_scoped_build_attempts_both_tiers_for_a_hot_routed_cold_aged_kind() {
        let _seam = super::R2_SEAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml = write_multi_tier_canon_yaml(dir.path());
        std::env::remove_var("CANON_PG_DSN_S22_UNIT_UNSET");
        std::env::remove_var("CANON_R2_BUCKET_S22_UNIT_UNSET");
        std::env::remove_var(LOCAL_R2_ROOT_ENV);

        let loaded = build_lenient_tiers_for_kind(&canon_yaml, RecordKind::Handoff)
            .expect("both hot and cold must be ATTEMPTED (each independently lenient), never a hard failure");
        assert!(loaded.pg.is_none(), "the hot rung's dsn_env is unset — must degrade to None");
        assert!(loaded.r2.is_none(), "the cold rung's bucket_env is unset — must degrade to None");
    }

    /// design.md R2 regression guard (ReviewS22 [important]): the sibling
    /// `..._attempts_both_tiers_...` test above asserts both handles are
    /// `None`, which cannot distinguish "cold attempted then degraded"
    /// from "cold never attempted" — narrowing `tiers_needed_for` to the
    /// routed rung only would pass it silently. This asserts the
    /// ATTEMPT POSITIVELY: with a reachable cold rung (the
    /// `CANON_R2_LOCAL_ROOT` debug seam) and hot's DSN still unset, a
    /// hot-routed cold-aged kind's build MUST attach cold to `Some` —
    /// only reachable if the aged rung stays in the needed-set.
    /// Debug-only (the seam is release-gated off).
    #[cfg(debug_assertions)]
    #[test]
    fn kind_scoped_build_actually_attaches_a_reachable_aged_r2_tier() {
        let _seam = super::R2_SEAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml = write_multi_tier_canon_yaml(dir.path());
        let r2_root = tempfile::tempdir().unwrap();
        std::env::remove_var("CANON_PG_DSN_S22_UNIT_UNSET");
        std::env::set_var(LOCAL_R2_ROOT_ENV, r2_root.path());

        let loaded = build_lenient_tiers_for_kind(&canon_yaml, RecordKind::Handoff)
            .expect("a reachable cold rung (local seam) must attach even with hot's DSN unset");
        std::env::remove_var(LOCAL_R2_ROOT_ENV);

        assert!(loaded.pg.is_none(), "the hot rung's dsn_env is unset — must degrade to None");
        assert!(
            loaded.r2.is_some(),
            "the cold-aged rung MUST be attempted (design.md R2): a reachable cold rung attaches to Some; \
             narrowing tiers_needed_for to the routed rung only would leave this None"
        );
    }

    /// A canon.yaml with ONLY local(git) + cold(s3) — no hot rung —
    /// so the strict `build_tiers` reaches the cold arm without the
    /// (rung-`Ord`-sorted-first) hot arm failing first.
    fn write_local_cold_canon_yaml(dir: &Path) -> PathBuf {
        let path = dir.join("canon.yaml");
        std::fs::write(
            &path,
            "tiers:\n  local: { backend: git, root: canon/ledger }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_S27_STRICT_UNSET, prefix: \"canon/\" }\nrouting:\n  change: local\n  handoff: cold\n",
        )
        .unwrap();
        path
    }

    /// s27 (ReviewS27 [important]): the STRICT `build_tiers` path (used
    /// by `canon tier age`) names the RUNG + backend for an unattached
    /// cold/s3 tier — "cold tier (s3) is not attached (…)" — symmetric
    /// with the hot/postgres arm, never the rung-less `BackendUnattached`
    /// `build_r2_tier` returns on its own. Debug build + `LOCAL_R2_ROOT_ENV`
    /// unset so `build_r2_tier` reaches `connect_live`, which fails loud
    /// when neither the configured `bucket_env` nor the generic
    /// `S3_BUCKET` resolves.
    #[cfg(debug_assertions)]
    #[test]
    fn strict_build_names_the_cold_rung_and_s3_backend_for_an_unattached_bucket() {
        let _seam = super::R2_SEAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml = write_local_cold_canon_yaml(dir.path());
        std::env::remove_var("CANON_R2_BUCKET_S27_STRICT_UNSET");
        std::env::remove_var("S3_BUCKET");
        std::env::remove_var(LOCAL_R2_ROOT_ENV);

        let err = match build_tiers(&canon_yaml) {
            Ok(_) => panic!("an unset cold/s3 bucket_env must be a loud strict-build failure"),
            Err(e) => e.to_string(),
        };
        let msg = err;
        assert!(
            msg.contains("cold tier (s3) is not attached"),
            "strict build must name the rung + backend, got: {msg}"
        );
    }
}
