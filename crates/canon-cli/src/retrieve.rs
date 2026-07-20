//! `canon retrieve --role <r> --regime <k> [--k <n>] [--repo <dir>]
//! [--json]` (S8 `retrieve-before-task`, part2 — design.md decisions
//! 1/3, tasks.md task 1.1): the CLI surface over S8Core's already-
//! shipped library core, `canon_learn::guidance::retrieve_guidance`
//! (`crates/canon-learn/src/guidance.rs`). This module is a thin
//! "resolve `--repo` → open the strategies store → call the library →
//! print" wrapper — it adds NO retrieval logic of its own: every fail-
//! soft/demoted-exclusion/cap-at-`k` rule already lives in
//! `retrieve_guidance` itself (that module's own doc), never
//! duplicated here.
//!
//! # s36 derived-scope surface (`--domain`/`--subject`)
//! Alongside the explicit `--regime <k>` path, [`run_scoped`] accepts a
//! DERIVED scope: `--domain <d>` (with an optional `--subject <id>`),
//! mutually exclusive with `--regime` and enforced as a loud usage
//! error. The derived path builds the ordered fallback ladder
//! `<domain>-<subject_id>` → `<domain>` ([`derive_candidates`]) and
//! serves the first non-empty rung via
//! `canon_learn::guidance::retrieve_first_nonempty` — the hierarchy is
//! encoded IN the fixed four-segment `regime_key` grammar's `<area>`
//! slot (a `-`-join), never a fifth segment or a re-parsed nested key,
//! and every candidate is DERIVED from the structured inputs through
//! canon-model's ONE `regime_key` serializer plus the SAME `<hash>`
//! derivation the shipped pre-dispatch hook uses ([`area_hash`]) — never
//! a second key or hash path. The `--json` shape stays backward
//! compatible (the raw `Vec<StrategyRef>` array); a fallback's
//! serving-regime note goes to stderr ([`serving_note`]), never into
//! that array.
//!
//! # FAIL-SOFT AT THE CLI BOUNDARY (design decision 3)
//! `retrieve_guidance`'s own signature returns `Vec<StrategyRef>` —
//! never a `Result` — so there is no store-outage/malformed-row error
//! this module could propagate even if it wanted to: a store outage,
//! an empty/nonexistent store directory, or a malformed on-disk row
//! all degrade to an empty guidance list, logged internally by
//! `retrieve_guidance` itself, never surfaced here. `main.rs`'s
//! `Retrieve` arm reflects this: the retrieval path always exits `0`.
//!
//! [`run_scoped`] returns an `Err` only for a **CLI usage precondition**,
//! never a retrieval failure: a `--regime`/`--domain` scope conflict or
//! omission (s36's `--regime` XOR the derived pair), or — on the
//! explicit-`--regime` path — a `--role` that disagrees with
//! `--regime`'s own leading segment (`regime_key.role()`).
//! `retrieve_guidance` itself only `debug_assert_eq!`s the role match (a
//! caller-contract check compiled out of release builds, its own doc
//! comment), so a naive pass-through of mismatched CLI flags would
//! either panic a debug build or silently ignore the mismatch in a
//! release one. This module checks every such precondition BEFORE ever
//! calling into `canon-learn`, so a bad invocation is always a clean,
//! reported usage error (`main.rs` exits `2`, mirroring `canon gate
//! check`'s own 0-clean/1-red/2-usage convention) — never a panic path,
//! and never silently wrong.
//!
//! # Store resolution
//! `--repo` resolves through the same [`crate::context::resolve_repo_root`]
//! nearest-`canon.yaml`-ancestor walk `canon context`/`canon fmt`/`canon
//! gate` already use (design D7) — never a second root-resolution
//! convention. The resolved repo's `canon.yaml` `learn:` section
//! (`canon_learn::LearnConfig`) names the operator-local store root
//! (`canon/learn` by default, `LearnConfig::default`'s own "a repo
//! works with zero config" ethos); a missing/unreadable/malformed
//! `canon.yaml` degrades to `LearnConfig::default()` rather than
//! erroring, matching `canon context`'s own "no canon state at all"
//! degrade-to-defaults contract — `canon retrieve` must stay usable
//! against a repo that has not configured `canon-learn` at all yet.

use std::path::Path;

use sha2::{Digest, Sha256};

use canon_learn::guidance::{retrieve_first_nonempty, retrieve_guidance};
use canon_learn::{LearnConfig, ParquetStrategyStore};
use canon_model::ids::regime_key;
use canon_model::{RegimeKey, RoleId, StrategyRef, SubjectId};

use crate::context::resolve_repo_root;

/// `--role`'s `clap` value parser.
pub fn parse_role(s: &str) -> Result<RoleId, String> {
    RoleId::parse(s).map_err(|e| e.to_string())
}

/// `--regime`'s `clap` value parser — the full `regime_key` string
/// (`<role>/<repo>/<area>/<hash>`), the SAME canonical serialization
/// S6's write side produces (design decision 1: "no second key
/// derivation").
pub fn parse_regime(s: &str) -> Result<RegimeKey, String> {
    RegimeKey::parse(s).map_err(|e| e.to_string())
}

/// `--subject`'s `clap` value parser — a [`SubjectId`] kebab slug (s36
/// `subject-domain-loop`). Validated here so a malformed subject id is
/// a clean clap usage error, never a silently-coerced regime segment.
pub fn parse_subject(s: &str) -> Result<SubjectId, String> {
    SubjectId::parse(s).map_err(|e| e.to_string())
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RetrieveCliError {
    /// `--role` disagrees with `--regime`'s own leading segment (module
    /// doc). Caught here, never left to `retrieve_guidance`'s internal
    /// `debug_assert_eq!`.
    #[error(
        "--role `{role}` does not match --regime `{regime_key}`'s own leading role segment `{regime_role}` — regime_key already embeds role as its first segment (design decision 1); pass the SAME role to both, or omit --role's disagreement entirely"
    )]
    RoleRegimeMismatch { role: String, regime_key: String, regime_role: String },

    /// Both `--regime` and the derived `--domain`/`--subject` pair were
    /// given — the two scoping modes are mutually exclusive (s36:
    /// `--regime` XOR the derived pair).
    #[error(
        "pass EITHER --regime (an explicit regime_key) OR --domain (with an optional --subject), never both — they are two mutually exclusive ways to scope the same retrieval"
    )]
    ScopeConflict,

    /// Neither `--regime` nor `--domain` was given — the retrieval has
    /// no scope to resolve (s36).
    #[error("one of --regime or --domain is required to scope the retrieval")]
    NoScope,

    /// `--subject` was given without `--domain`. A subject is scoped
    /// WITHIN its domain (the candidate area is `<domain>-<subject_id>`),
    /// so a bare `--subject` has no domain to hang off (s36).
    #[error("--subject requires --domain — a subject is retrieved within its domain (candidate area `<domain>-<subject_id>`)")]
    SubjectWithoutDomain,

    /// A derived candidate did not form a valid `regime_key` — e.g. the
    /// resolved repo directory has no usable name for the `<repo>`
    /// segment. Reports the underlying grammar error verbatim (s36).
    #[error("could not derive a valid regime_key from --domain/--subject: {segment_error}")]
    DerivationFailed { segment_error: String },
}

/// Resolve `<repo>`'s configured learn root and open the `strategies`
/// parquet tier under it (module doc's "Store resolution" section).
fn open_strategy_store(repo: &Path) -> ParquetStrategyStore {
    let canon_yaml = repo.join("canon.yaml");
    let learn_config =
        std::fs::read_to_string(&canon_yaml).ok().and_then(|text| LearnConfig::from_manifest(&text).ok()).unwrap_or_default();
    ParquetStrategyStore::open(repo.join(learn_config.root).join("strategies"))
}

/// The `<hash>` a DERIVED `--domain`/`--subject` candidate carries: the
/// byte-for-byte Rust twin of `canon/skills/canon-retrieve/
/// pre-dispatch.sh`'s `printf '%s' <area> | sha256sum | cut -c1-12`
/// area-hash — the ONE retrieval-side `<area>`→`<hash>` derivation, NOT
/// a second hash scheme. A subject/domain query has no single source
/// event to digest (the write-path `<hash>` primitive,
/// `canon_ingest::normalize::content_digest`, hashes a specific event's
/// join key), so the retrieval side derives the `<hash>` deterministically
/// from the `<area>` value itself, exactly as the shipped pre-dispatch
/// hook does — so a Rust-assembled candidate lands on the IDENTICAL
/// `<role>/<repo>/<area>/<hash>` directory a shell-assembled `canon
/// regime-key --area <a> --hash "$(printf %s <a> | sha256sum | cut
/// -c1-12)"` would. `<area>` here is already a kebab slug (a `<domain>`
/// and `<subject_id>`, both kebab) — a fixed point of `regime_key`'s own
/// segment canonicalizer — so hashing the raw `<area>` and the
/// key-canonicalized `<area>` are the same bytes.
fn area_hash(area: &str) -> String {
    Sha256::digest(area.as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// The `<repo>` segment for a derived candidate: the resolved repo
/// root's own directory name, mirroring pre-dispatch.sh's `basename
/// "$REPO_ROOT"` (the existing repo-segment derivation) — never a second
/// convention. Empty only for a pathological root (handled as a
/// [`RetrieveCliError::DerivationFailed`] downstream when the assembled
/// key fails to parse).
fn repo_segment(repo: &Path) -> String {
    repo.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_string()
}

/// Build the ordered fallback candidates for a `--domain`/`--subject`
/// query (s36 `subject-domain-loop`): `<domain>-<subject_id>` then the
/// bare `<domain>` when a subject is given, else `<domain>` alone. Each
/// `<area>` becomes a full `regime_key` through canon-model's ONE
/// serializer ([`regime_key`]) with `<role>` from `--role`, `<repo>`
/// from [`repo_segment`], and `<hash>` from [`area_hash`] — the SAME
/// segment derivations the existing (pre-dispatch) assembly path uses,
/// never a second key or hash derivation. The hierarchy is encoded IN
/// the fixed four-segment grammar's `<area>` slot (a `-`-join), never as
/// a fifth segment; candidates are always DERIVED from the structured
/// inputs, so that encoding is written but never parsed back.
pub fn derive_candidates(
    repo: &Path,
    role: &RoleId,
    domain: &str,
    subject: Option<&SubjectId>,
) -> Result<Vec<RegimeKey>, RetrieveCliError> {
    let repo_seg = repo_segment(repo);
    let mut areas = Vec::with_capacity(2);
    if let Some(subject) = subject {
        areas.push(format!("{domain}-{}", subject.as_str()));
    }
    areas.push(domain.to_string());
    areas
        .into_iter()
        .map(|area| {
            RegimeKey::parse(regime_key(role.as_str(), &repo_seg, &area, &area_hash(&area)))
                .map_err(|e| RetrieveCliError::DerivationFailed { segment_error: e.to_string() })
        })
        .collect()
}

/// The outcome of a scoped `canon retrieve` (s36): the guidance plus the
/// candidate ladder that produced it, so the CLI can note when a
/// subject-scoped query fell back to its domain.
#[derive(Debug, Clone)]
pub struct ScopedRetrieval {
    /// The ordered candidates actually queried, most-specific first: one
    /// for the `--regime` path, one or two for the derived
    /// `--domain`/`--subject` pair. Never empty once [`run_scoped`]
    /// returns `Ok`.
    pub candidates: Vec<RegimeKey>,
    /// Index into [`Self::candidates`] of the regime whose lookup
    /// returned a NON-empty result, or `None` when every candidate was
    /// empty (so an all-empty search is never misreported as a
    /// first-candidate hit).
    pub served_by: Option<usize>,
    /// The guidance served — empty when [`Self::served_by`] is `None`.
    pub guidance: Vec<StrategyRef>,
}

impl ScopedRetrieval {
    /// The regime that served the guidance, or — on an all-empty search
    /// — the most-specific candidate that was tried first (the one an
    /// operator would want named in a "0 guidance" line). Always present
    /// because `candidates` is never empty.
    pub fn serving_regime(&self) -> &RegimeKey {
        &self.candidates[self.served_by.unwrap_or(0)]
    }

    /// `true` when a NON-first candidate served — i.e. a subject-scoped
    /// query fell back to (a later) domain-scoped candidate.
    pub fn fell_back(&self) -> bool {
        self.served_by.is_some_and(|i| i > 0)
    }
}

/// `canon retrieve`'s scoped entry (s36 `subject-domain-loop`): the ONE
/// function `main.rs`'s `Retrieve` arm calls. Enforces `--regime` XOR
/// the derived `--domain`/`--subject` pair as a LOUD usage error
/// (`main.rs` exits `2`), then resolves guidance:
///
/// - `--regime`: the explicit-key path — `--role` must equal the key's
///   own leading segment (the caller-contract check
///   [`retrieve_guidance`] only `debug_assert_eq!`s), then a single
///   [`retrieve_guidance`] lookup.
/// - `--domain`[`/--subject`]: the derived path — [`derive_candidates`]
///   builds the `<domain>-<subject_id>` → `<domain>` ladder and
///   [`retrieve_first_nonempty`] serves the first non-empty rung.
///
/// Once past the usage/derivation preconditions this NEVER fails on a
/// retrieval error — every store outage/malformed row degrades to empty
/// guidance inside [`retrieve_guidance`] (design decision 3's fail-soft
/// contract), exactly as the pure `--regime` path already did.
pub fn run_scoped(
    repo: &Path,
    role: &RoleId,
    regime: Option<&RegimeKey>,
    domain: Option<&str>,
    subject: Option<&SubjectId>,
    k: Option<usize>,
) -> Result<ScopedRetrieval, RetrieveCliError> {
    match (regime, domain, subject) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(RetrieveCliError::ScopeConflict),
        (Some(regime), None, None) => {
            if regime.role() != role.as_str() {
                return Err(RetrieveCliError::RoleRegimeMismatch {
                    role: role.as_str().to_string(),
                    regime_key: regime.as_str().to_string(),
                    regime_role: regime.role().to_string(),
                });
            }
            let repo = resolve_repo_root(repo);
            let store = open_strategy_store(&repo);
            let guidance = retrieve_guidance(&store, role, regime, k);
            let served_by = (!guidance.is_empty()).then_some(0);
            Ok(ScopedRetrieval { candidates: vec![regime.clone()], served_by, guidance })
        }
        (None, Some(domain), subject) => {
            let repo = resolve_repo_root(repo);
            let candidates = derive_candidates(&repo, role, domain, subject)?;
            let store = open_strategy_store(&repo);
            let (guidance, serving) = retrieve_first_nonempty(&store, role, &candidates, k);
            let served_by = serving.map(|s| candidates.iter().position(|c| c == s).expect("serving is one of candidates"));
            Ok(ScopedRetrieval { candidates, served_by, guidance })
        }
        (None, None, Some(_)) => Err(RetrieveCliError::SubjectWithoutDomain),
        (None, None, None) => Err(RetrieveCliError::NoScope),
    }
}

/// Default human table for a scoped run (s36): a header naming the
/// SERVING regime and how many items came back — printed even for an
/// empty result (never silent emptiness, same rationale as the pure
/// `--regime` table). When a subject-scoped query fell back to its
/// domain, a second line names both the requested and serving regimes so
/// an operator sees the fallback happened, not a silent substitution.
/// Each item then renders as `- [id] title` with its content indented.
pub fn format_human_scoped(role: &RoleId, outcome: &ScopedRetrieval) -> String {
    let serving = outcome.serving_regime();
    let mut out = format!("canon retrieve: {} guidance item(s) for role {role} regime {serving}\n", outcome.guidance.len());
    if outcome.fell_back() {
        out.push_str(&format!("  (fell back from {} to {serving})\n", outcome.candidates[0]));
    }
    for item in &outcome.guidance {
        out.push_str(&format!("- [{}] {}\n", item.strategy_id, item.title));
        for line in item.content.lines() {
            out.push_str(&format!("    {line}\n"));
        }
    }
    out
}

/// The stderr advisory `main.rs` prints in `--json` mode when the
/// derived fallback engaged (s36): stdout stays the byte-identical raw
/// `Vec<StrategyRef>` array [`format_json`] produces (the pre-dispatch
/// hook's `jq 'length'`/`map(...)` array contract must not break —
/// `--json` shape stays backward compatible), so the "which regime
/// served" note goes to stderr instead of mutating that array. `None`
/// when no fallback happened (the first/only candidate served, or an
/// all-empty result), so a normal run stays silent on stderr.
pub fn serving_note(outcome: &ScopedRetrieval) -> Option<String> {
    outcome
        .fell_back()
        .then(|| format!("canon retrieve: fell back from {} to {}", outcome.candidates[0], outcome.serving_regime()))
}

/// Default human table: a header line naming the role/regime queried
/// and how many items came back — printed even for an EMPTY result
/// (never silent emptiness) so an operator debugging "why is the hook
/// silent" sees "0 guidance item(s)" rather than mistaking a blank
/// stdout for `canon retrieve` not having run at all — then one
/// `- [id] title` line per item with its content indented beneath.
pub fn format_human(role: &RoleId, regime_key: &RegimeKey, guidance: &[StrategyRef]) -> String {
    let mut out = format!("canon retrieve: {} guidance item(s) for role {role} regime {regime_key}\n", guidance.len());
    for item in guidance {
        out.push_str(&format!("- [{}] {}\n", item.strategy_id, item.title));
        for line in item.content.lines() {
            out.push_str(&format!("    {line}\n"));
        }
    }
    out
}

/// `--json`: the raw `Vec<StrategyRef>`, machine-readable — the exact
/// snapshot shape a caller would embed into `Run::injected_guidance`
/// verbatim (design decision 2), never a human-table projection of it.
/// An empty result prints `[]`, never an omitted/null value, so a
/// caller piping this into a manifest write always gets a well-formed
/// JSON array.
pub fn format_json(guidance: &[StrategyRef]) -> String {
    serde_json::to_string_pretty(guidance).expect("Vec<StrategyRef> is always serializable")
}

#[cfg(test)]
mod tests {
    use canon_learn::StrategyStore;
    use canon_model::ids::regime_key;
    use chrono::Utc;

    use super::*;

    fn role(s: &str) -> RoleId {
        RoleId::parse(s).unwrap()
    }

    fn regime(role: &str, repo: &str, area: &str, hash: &str) -> RegimeKey {
        RegimeKey::parse(regime_key(role, repo, area, hash)).unwrap()
    }

    fn seed_strategy(repo_dir: &Path, regime_key: &RegimeKey, role: &RoleId, title: &str) {
        let learn_config = LearnConfig::default();
        let store = ParquetStrategyStore::open(repo_dir.join(learn_config.root).join("strategies"));
        let item = canon_learn::StrategyItem::new(
            canon_learn::StrategyId::new(),
            regime_key.clone(),
            role.clone(),
            title,
            "description",
            "content",
            vec![canon_learn::TrajectoryId::new()],
            Utc::now(),
        );
        store.append(&item).unwrap();
    }

    /// Fail-soft (design decision 3): the explicit-`--regime` path over
    /// a repo with no store at all returns empty guidance, never errors.
    #[test]
    fn run_scoped_regime_path_over_a_nonexistent_store_is_empty_never_errors() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("dev");
        let rk = regime("dev", "canon", "join-spine", "9c93d024b1a2");
        let outcome = run_scoped(dir.path(), &r, Some(&rk), None, None, None).unwrap();
        assert!(outcome.guidance.is_empty());
        assert_eq!(outcome.served_by, None);
    }

    /// `--k` caps the explicit-`--regime` result — proves `run_scoped`
    /// threads it through to `retrieve_guidance`.
    #[test]
    fn run_scoped_regime_path_respects_a_k_cap() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("dev");
        let rk = regime("dev", "canon", "join-spine", "9c93d024b1a2");
        for i in 0..3 {
            seed_strategy(dir.path(), &rk, &r, &format!("strategy {i}"));
        }
        let outcome = run_scoped(dir.path(), &r, Some(&rk), None, None, Some(2)).unwrap();
        assert_eq!(outcome.guidance.len(), 2);
    }

    #[test]
    fn format_human_scoped_reports_an_explicit_zero_count_never_silent_emptiness() {
        let r = role("dev");
        let rk = regime("dev", "canon", "join-spine", "9c93d024b1a2");
        let outcome = ScopedRetrieval { candidates: vec![rk], served_by: None, guidance: vec![] };
        let text = format_human_scoped(&r, &outcome);
        assert!(text.contains("0 guidance item(s)"), "expected an explicit zero-count line, got:\n{text}");
    }

    #[test]
    fn format_human_scoped_lists_id_title_and_indented_content() {
        let r = role("dev");
        let rk = regime("dev", "canon", "join-spine", "9c93d024b1a2");
        let guidance = vec![StrategyRef::new("sid-1", "a title", "line one\nline two")];
        let outcome = ScopedRetrieval { candidates: vec![rk], served_by: Some(0), guidance };
        let text = format_human_scoped(&r, &outcome);
        assert!(text.contains("- [sid-1] a title"));
        assert!(text.contains("    line one"));
        assert!(text.contains("    line two"));
    }

    #[test]
    fn format_json_round_trips_an_empty_and_nonempty_vec() {
        assert_eq!(serde_json::from_str::<Vec<StrategyRef>>(&format_json(&[])).unwrap(), Vec::<StrategyRef>::new());
        let guidance = vec![StrategyRef::new("sid-1", "t", "c")];
        let round_tripped: Vec<StrategyRef> = serde_json::from_str(&format_json(&guidance)).unwrap();
        assert_eq!(round_tripped, guidance);
    }

    fn subject(s: &str) -> SubjectId {
        SubjectId::parse(s).unwrap()
    }

    /// Derived-candidates fallback (s36): with the subject-scoped
    /// namespace populated, the `<domain>-<subject_id>` candidate wins —
    /// no fallback, `served_by == 0`.
    #[test]
    fn run_scoped_serves_the_subject_scoped_candidate_when_it_is_populated() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("planning");
        let subj = subject("my-subject");
        let candidates = derive_candidates(dir.path(), &r, "planning", Some(&subj)).unwrap();
        assert_eq!(candidates.len(), 2, "subject + domain candidates");
        assert_eq!(candidates[0].area(), "planning-my-subject");
        assert_eq!(candidates[1].area(), "planning");
        // Populate BOTH rungs — order must still pick the subject one.
        seed_strategy(dir.path(), &candidates[0], &r, "subject-scoped");
        seed_strategy(dir.path(), &candidates[1], &r, "domain-scoped");

        let outcome = run_scoped(dir.path(), &r, None, Some("planning"), Some(&subj), None).unwrap();

        assert_eq!(outcome.served_by, Some(0));
        assert!(!outcome.fell_back());
        assert_eq!(outcome.serving_regime(), &candidates[0]);
        assert_eq!(outcome.guidance.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(), vec!["subject-scoped"]);
    }

    /// Derived-candidates fallback (s36): an EMPTY subject namespace
    /// falls back to the `<domain>` candidate — `served_by == 1`,
    /// `fell_back()`, and the serving regime is the domain one.
    #[test]
    fn run_scoped_falls_back_to_the_domain_candidate_for_an_empty_subject() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("planning");
        let subj = subject("my-subject");
        let candidates = derive_candidates(dir.path(), &r, "planning", Some(&subj)).unwrap();
        // Only the DOMAIN rung is populated.
        seed_strategy(dir.path(), &candidates[1], &r, "domain-scoped");

        let outcome = run_scoped(dir.path(), &r, None, Some("planning"), Some(&subj), None).unwrap();

        assert_eq!(outcome.served_by, Some(1));
        assert!(outcome.fell_back());
        assert_eq!(outcome.serving_regime(), &candidates[1]);
        assert_eq!(outcome.guidance.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(), vec!["domain-scoped"]);
        // The stderr advisory names both the requested and serving regimes.
        let note = serving_note(&outcome).expect("a fallback must produce a serving note");
        assert!(note.contains(candidates[0].as_str()) && note.contains(candidates[1].as_str()), "note names both regimes, got: {note}");
        // The human table also surfaces the fallback line.
        let human = format_human_scoped(&r, &outcome);
        assert!(human.contains("fell back from"), "human output notes the fallback, got:\n{human}");
    }

    /// A bare `--domain` (no subject) derives the single `<domain>`
    /// candidate and, when empty, degrades to empty guidance / `None`
    /// serving — never an error (fail-soft), never a spurious fallback.
    #[test]
    fn run_scoped_domain_only_over_an_empty_store_is_empty_and_serves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("planning");
        let outcome = run_scoped(dir.path(), &r, None, Some("planning"), None, None).unwrap();
        assert_eq!(outcome.candidates.len(), 1);
        assert_eq!(outcome.candidates[0].area(), "planning");
        assert!(outcome.guidance.is_empty());
        assert_eq!(outcome.served_by, None);
        assert!(!outcome.fell_back());
        assert!(serving_note(&outcome).is_none(), "no fallback ⇒ no stderr note");
    }

    /// The explicit `--regime` path still works through `run_scoped`
    /// (backward compat) and reports `served_by == 0` on a hit.
    #[test]
    fn run_scoped_regime_path_serves_the_explicit_key() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("dev");
        let rk = regime("dev", "canon", "join-spine", "9c93d024b1a2");
        seed_strategy(dir.path(), &rk, &r, "explicit");

        let outcome = run_scoped(dir.path(), &r, Some(&rk), None, None, None).unwrap();
        assert_eq!(outcome.candidates, vec![rk]);
        assert_eq!(outcome.served_by, Some(0));
        assert_eq!(outcome.guidance.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(), vec!["explicit"]);
    }

    #[test]
    fn run_scoped_rejects_regime_and_domain_together_as_a_scope_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("dev");
        let rk = regime("dev", "canon", "join-spine", "9c93d024b1a2");
        let err = run_scoped(dir.path(), &r, Some(&rk), Some("dev"), None, None).unwrap_err();
        assert_eq!(err, RetrieveCliError::ScopeConflict);
    }

    #[test]
    fn run_scoped_rejects_a_subject_without_a_domain() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("planning");
        let subj = subject("my-subject");
        let err = run_scoped(dir.path(), &r, None, None, Some(&subj), None).unwrap_err();
        assert_eq!(err, RetrieveCliError::SubjectWithoutDomain);
    }

    #[test]
    fn run_scoped_rejects_no_scope_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("planning");
        let err = run_scoped(dir.path(), &r, None, None, None, None).unwrap_err();
        assert_eq!(err, RetrieveCliError::NoScope);
    }

    #[test]
    fn run_scoped_regime_path_rejects_a_role_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let r = role("dev");
        let rk = regime("content", "canon", "join-spine", "9c93d024b1a2");
        let err = run_scoped(dir.path(), &r, Some(&rk), None, None, None).unwrap_err();
        assert!(matches!(err, RetrieveCliError::RoleRegimeMismatch { .. }));
    }

    /// The derived `<hash>` matches the shipped pre-dispatch hook's
    /// `sha256(area)[:12]` — the ONE retrieval-side area→hash derivation,
    /// locked so a Rust candidate and a shell-assembled key never drift.
    #[test]
    fn area_hash_matches_the_pre_dispatch_sha256_truncation() {
        // sha256("planning")[:12] hex, the exact `printf %s planning |
        // sha256sum | cut -c1-12` a shell hook would compute.
        let expected: String = {
            use sha2::{Digest, Sha256};
            Sha256::digest(b"planning").iter().take(6).map(|b| format!("{b:02x}")).collect()
        };
        assert_eq!(area_hash("planning"), expected);
        assert_eq!(area_hash("planning").len(), 12);
        assert!(area_hash("planning").chars().all(|c| c.is_ascii_hexdigit()));
    }
}
