//! `canon subject {new,adopt,status}` (s36 `subject-domain-loop`): the
//! authoring + lifecycle surface for the reviewed 13th record kind,
//! [`canon_model::Subject`] — the durable product/management unit a
//! team plans, designs, builds, verifies, and ships across many
//! changes. Every write goes through
//! [`canon_store::registry::TierRegistry`] (the SAME routed
//! tier-resolution path every other authored kind uses via
//! [`crate::tiers`]), never a hand-rolled `GitTier`, so `subject`'s
//! `routing:` destination (`local` by default) governs where records
//! land and `canon query --kind subject` reads them back from the
//! identical rung.
//!
//! # Re-writes append; the query fold reads latest
//! `adopt`/`status` do NOT mutate a subject in place: the git tier is
//! append-only (`canon_store::partition` module doc — a logically
//! different record sharing one natural key resolves to a NEW path), so
//! each stamps a FRESH envelope `at = Utc::now()` and persists a new
//! record whose greater `at` deterministically wins `canon query`'s
//! `fold_latest_by_key` (winner = greatest `(at, digest)`) — the SAME
//! fold `canon-gate::ledger` and `canon query`'s pg-routed reader
//! already apply, so adopt/status re-writes read back as ONE latest
//! row. A same-`at` digest tiebreak would be nondeterministic, so the
//! bumped `at` is load-bearing, not cosmetic.
//!
//! # `verifying → shipped` is evidence-gated, fail-closed
//! That ONE transition additionally requires every linked
//! `scenario_ids` entry to carry a latest NON-`Divergent` verdict in
//! the ledger — resolved by REUSING `canon-gate`'s own
//! [`canon_gate::latest_verdicts`] over a [`canon_gate::GateContext`]
//! loaded exactly as `canon gate check` loads it (never a second
//! verdict fold). A violation prints by failure class
//! ([`canon_gate::FailureClass`]), exits `1`, and leaves the record
//! UNCHANGED (fail closed). Every other transition is the pure
//! [`is_valid_transition`] chain; an off-chain transition is refused
//! (exit `2`), the record likewise unchanged.

use std::path::Path;

use canon_gate::{latest_verdicts, FailureClass, GateContext, GateCtx, LedgerEntry, Violation};
use canon_model::{
    Actor, Change, ChangeId, Envelope, EvidenceVerdict, RawRecord, RecordKind, RoleId, ScenarioId, Subject, SubjectId,
    SubjectStatus,
};
use canon_policy::SchemaRegistry;
use canon_store::registry::TierRegistry;
use canon_store::tier::{StoreError, TierQuery};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::context::{resolve_canon_yaml, resolve_repo_root};
use crate::tiers::{self, TierCliError};

/// Exit code for a refused/malformed invocation (duplicate id, unknown
/// change/subject, an off-chain transition) — nothing written, mirrors
/// `canon review add`/`canon divergence stage`'s own `2`.
const EXIT_REFUSED: i32 = 2;
/// Exit code for the `verifying → shipped` evidence-gate block: the
/// transition is well-formed but the linked scenarios are not covered
/// by non-`Divergent` verdicts — record unchanged, fail closed
/// (contract: "violations print by failure class, exit 1").
const EXIT_GATED: i32 = 1;

/// `<id>` / `--subject`'s `clap` value parser — grammar-level
/// [`SubjectId::parse`] (kebab-case slug).
pub fn parse_subject_id(s: &str) -> Result<SubjectId, String> {
    SubjectId::parse(s).map_err(|e| e.to_string())
}

/// `<change_id>`'s `clap` value parser — grammar-level
/// [`ChangeId::parse`].
pub fn parse_change_id(s: &str) -> Result<ChangeId, String> {
    ChangeId::parse(s).map_err(|e| e.to_string())
}

/// `<state>`'s `clap` value parser — the closed [`SubjectStatus`]
/// vocabulary, snake_case wire spelling (mirrors the model's
/// `#[serde(rename_all = "snake_case")]`), never a second casing.
pub fn parse_status(s: &str) -> Result<SubjectStatus, String> {
    match s {
        "proposed" => Ok(SubjectStatus::Proposed),
        "specced" => Ok(SubjectStatus::Specced),
        "building" => Ok(SubjectStatus::Building),
        "verifying" => Ok(SubjectStatus::Verifying),
        "shipped" => Ok(SubjectStatus::Shipped),
        "retired" => Ok(SubjectStatus::Retired),
        _ => Err(format!(
            "unknown subject status `{s}` (expected one of: proposed, specced, building, verifying, shipped, retired)"
        )),
    }
}

/// The stable wire string for a [`SubjectStatus`] — the same value its
/// `#[serde(rename_all = "snake_case")]` serialization produces, used
/// for human-readable messages only.
fn status_str(status: SubjectStatus) -> &'static str {
    match status {
        SubjectStatus::Proposed => "proposed",
        SubjectStatus::Specced => "specced",
        SubjectStatus::Building => "building",
        SubjectStatus::Verifying => "verifying",
        SubjectStatus::Shipped => "shipped",
        SubjectStatus::Retired => "retired",
    }
}

/// The subject lifecycle transition rule (s36 design D1): the forward
/// chain `proposed → specced → building → verifying → shipped`, plus
/// any non-retired state → `retired`. Every other transition (a skip,
/// a backward step, a self-loop, or anything out of `retired`) is
/// invalid. The `verifying → shipped` EVIDENCE gate is a SEPARATE,
/// additional requirement layered on top of this pure step check (see
/// [`ship_gate_violations`]).
pub fn is_valid_transition(from: SubjectStatus, to: SubjectStatus) -> bool {
    use SubjectStatus::*;
    if to == Retired {
        return from != Retired;
    }
    matches!((from, to), (Proposed, Specced) | (Specced, Building) | (Building, Verifying) | (Verifying, Shipped))
}

/// Build a [`TierRegistry`] over exactly `kinds`' routed rungs, via the
/// SAME lenient, per-rung tier construction `canon query`/`canon
/// ingest` share ([`tiers::build_lenient_tiers_for_kinds`]) — so a
/// subject write honors `canon.yaml`'s `routing.subject` and lands
/// where `canon query --kind subject` reads.
fn registry_for(canon_yaml_path: &Path, kinds: &[RecordKind]) -> Result<TierRegistry, TierCliError> {
    let loaded = tiers::build_lenient_tiers_for_kinds(canon_yaml_path, kinds)?;
    Ok(TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite))
}

/// Fold every retained version of `kind` to one latest row per natural
/// key — winner = greatest `(at, content_digest12)`, via the shared
/// [`canon_store::fold_latest_by_key`] every multi-version reader uses,
/// keyed by the SAME `resolve_partition` natural key `canon query`
/// derives. This is how an `adopt`/`status` re-write (a new append at a
/// bumped `at`) reads back as the one current record.
fn fold_latest(kind: RecordKind, records: Vec<RawRecord>) -> Vec<RawRecord> {
    struct Candidate {
        key: String,
        at: DateTime<Utc>,
        digest: String,
        record: RawRecord,
    }
    let candidates = records.into_iter().map(|record| {
        let key = canon_store::partition::resolve_partition(kind, &record.0).map(|p| p.natural_key).unwrap_or_default();
        let at = canon_store::tier::raw_record_at(&record);
        let digest = canon_store::partition::content_digest12(&record.0);
        Candidate { key, at, digest, record }
    });
    canon_store::fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.digest.as_str())
        .into_values()
        .map(|c| c.record)
        .collect()
}

/// The current, folded-to-latest set of `kind` records read through
/// `registry`.
fn latest_records(registry: &TierRegistry, kind: RecordKind) -> Result<Vec<RawRecord>, StoreError> {
    let result = registry.query(&TierQuery::kind(kind))?;
    Ok(fold_latest(kind, result.records))
}

/// The latest [`Subject`] whose `subject_id` equals `id`, or `None`.
fn find_subject(registry: &TierRegistry, id: &SubjectId) -> Result<Option<Subject>, String> {
    let target = id.as_str();
    for raw in latest_records(registry, RecordKind::Subject).map_err(|e| e.to_string())? {
        if raw.0.get("subject_id").and_then(Value::as_str) == Some(target) {
            let subject: Subject = serde_json::from_value(raw.0.clone()).map_err(|e| format!("stored subject `{target}` is malformed: {e}"))?;
            return Ok(Some(subject));
        }
    }
    Ok(None)
}

/// The latest [`Change`] whose `change_id` equals `id`, or `None`
/// (fold-latest semantics — a re-emitted change reads as its one
/// current lifecycle state).
fn find_change(registry: &TierRegistry, id: &ChangeId) -> Result<Option<Change>, String> {
    let target = id.as_str();
    for raw in latest_records(registry, RecordKind::Change).map_err(|e| e.to_string())? {
        if raw.0.get("change_id").and_then(Value::as_str) == Some(target) {
            let change: Change = serde_json::from_value(raw.0.clone()).map_err(|e| format!("stored change `{target}` is malformed: {e}"))?;
            return Ok(Some(change));
        }
    }
    Ok(None)
}

/// Print a written subject: its full record body on `--json`, else a
/// one-line human receipt.
fn report_subject(subject: &Subject, verb: &str, json: bool) {
    if json {
        // The full merged record body — the updated record, per the
        // contract's "`--json` emits the updated record".
        println!("{}", serde_json::to_string_pretty(subject).unwrap_or_default());
    } else {
        println!("canon subject {verb}: {} ({}, {})", subject.subject_id.as_str(), subject.domain, status_str(subject.status));
    }
}

/// `canon subject new <id> --domain <d> --title <t> [--summary <s>]
/// [--owner-role <r>]` (module doc): author a fresh [`Subject`] at
/// status `proposed`. The envelope is attributed to `actor_id` (default
/// `canon`, the same source `canon review add` uses) in the role
/// `owner_role`. `domain` is validated SHAPE-only here (kebab slug, the
/// SAME grammar the model's `deserialize_domain_slug` enforces — reused
/// via `SubjectId`'s identical grammar so the CLI never panics on the
/// model's debug-assert). A `subject_id` already present in the store is
/// a loud refusal (exit `2`), never a silent second append.
#[allow(clippy::too_many_arguments)]
pub fn run_new(repo: &Path, subject_id: &SubjectId, domain: &str, title: &str, summary: &str, owner_role: &RoleId, actor_id: &str, json: bool) -> i32 {
    // `domain` shares `SubjectId`'s kebab-slug grammar (design D2: the
    // model validates the SAME shape via `is_kebab_slug`); validate it
    // BEFORE `Subject::new` so a malformed value is a clean refusal
    // here, never the model's debug-assert panic.
    if SubjectId::parse(domain).is_err() {
        eprintln!("canon subject new: refused — domain `{domain}` is not a kebab-case slug (`[a-z0-9]+(-[a-z0-9]+)*`)");
        return EXIT_REFUSED;
    }

    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo, None);
    let registry = match registry_for(&canon_yaml_path, &[RecordKind::Subject]) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("canon subject new: {e}");
            return EXIT_REFUSED;
        }
    };

    match find_subject(&registry, subject_id) {
        Ok(Some(_)) => {
            eprintln!("canon subject new: refused — subject `{}` already exists", subject_id.as_str());
            return EXIT_REFUSED;
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("canon subject new: {e}");
            return EXIT_REFUSED;
        }
    }

    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new(actor_id, owner_role.clone()));
    let subject = Subject::new(envelope, subject_id.clone(), title, summary, domain, SubjectStatus::Proposed, owner_role.clone());

    match registry.persist(&subject) {
        Ok(_) => {
            report_subject(&subject, "new", json);
            0
        }
        Err(e) => {
            eprintln!("canon subject new: {e}");
            EXIT_REFUSED
        }
    }
}

/// `canon subject adopt <change_id> --subject <id>` (module doc): link
/// an imported plan [`Change`] to a [`Subject`]. Loads the latest of
/// each (fold-latest), refusing (exit `2`) if either is absent, then
/// writes BOTH re-stamped through the routed tiers: the change with
/// `subject_id` set (design D3 — stamped at adoption time, never
/// derived in canon-model), and the subject with `change_id` appended
/// to `change_ids` (deduped). Both carry a fresh envelope `at` so the
/// update deterministically supersedes the prior version in the query
/// fold.
pub fn run_adopt(repo: &Path, change_id: &ChangeId, subject_id: &SubjectId, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo, None);
    let registry = match registry_for(&canon_yaml_path, &[RecordKind::Change, RecordKind::Subject]) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("canon subject adopt: {e}");
            return EXIT_REFUSED;
        }
    };

    let mut subject = match find_subject(&registry, subject_id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            eprintln!("canon subject adopt: refused — subject `{}` does not exist (author it with `canon subject new` first)", subject_id.as_str());
            return EXIT_REFUSED;
        }
        Err(e) => {
            eprintln!("canon subject adopt: {e}");
            return EXIT_REFUSED;
        }
    };

    let mut change = match find_change(&registry, change_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            eprintln!("canon subject adopt: refused — change `{}` does not exist (import it with `canon ingest plans` first)", change_id.as_str());
            return EXIT_REFUSED;
        }
        Err(e) => {
            eprintln!("canon subject adopt: {e}");
            return EXIT_REFUSED;
        }
    };

    let now = Utc::now();
    change.subject_id = Some(subject_id.clone());
    change.envelope.at = now;
    if !subject.change_ids.contains(change_id) {
        subject.change_ids.push(change_id.clone());
    }
    subject.envelope.at = now;

    if let Err(e) = registry.persist(&change) {
        eprintln!("canon subject adopt: {e}");
        return EXIT_REFUSED;
    }
    match registry.persist(&subject) {
        Ok(_) => {
            if json {
                report_subject(&subject, "adopt", true);
            } else {
                println!("canon subject adopt: linked change `{}` to subject `{}`", change_id.as_str(), subject_id.as_str());
            }
            0
        }
        Err(e) => {
            eprintln!("canon subject adopt: {e}");
            EXIT_REFUSED
        }
    }
}

/// `canon subject status <id> <state>` (module doc): apply a lifecycle
/// transition. Refuses an off-chain step ([`is_valid_transition`], exit
/// `2`); for `verifying → shipped` additionally runs
/// [`ship_gate_violations`] and, on any violation, prints each by
/// failure class and exits `1` with the record UNCHANGED (fail closed).
/// A successful transition re-stamps the subject (fresh `at`) and
/// persists it through the routed tier.
pub fn run_status(repo: &Path, subject_id: &SubjectId, target: SubjectStatus, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo, None);
    let registry = match registry_for(&canon_yaml_path, &[RecordKind::Subject]) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("canon subject status: {e}");
            return EXIT_REFUSED;
        }
    };

    let mut subject = match find_subject(&registry, subject_id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            eprintln!("canon subject status: refused — subject `{}` does not exist", subject_id.as_str());
            return EXIT_REFUSED;
        }
        Err(e) => {
            eprintln!("canon subject status: {e}");
            return EXIT_REFUSED;
        }
    };

    let current = subject.status;
    if !is_valid_transition(current, target) {
        eprintln!(
            "canon subject status: refused — invalid transition {} → {} (allowed: the chain proposed → specced → building → verifying → shipped, or any non-retired state → retired)",
            status_str(current),
            status_str(target)
        );
        return EXIT_REFUSED;
    }

    if current == SubjectStatus::Verifying && target == SubjectStatus::Shipped {
        match ship_gate_violations(&repo, &subject.scenario_ids) {
            Ok(violations) if !violations.is_empty() => {
                for v in &violations {
                    eprintln!("canon subject status: {}", v.line());
                }
                return EXIT_GATED;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("canon subject status: {e}");
                return EXIT_REFUSED;
            }
        }
    }

    subject.status = target;
    subject.envelope.at = Utc::now();

    match registry.persist(&subject) {
        Ok(_) => {
            if json {
                report_subject(&subject, "status", true);
            } else {
                println!("canon subject status: {} → {}", subject_id.as_str(), status_str(target));
            }
            0
        }
        Err(e) => {
            eprintln!("canon subject status: {e}");
            EXIT_REFUSED
        }
    }
}

/// The `verifying → shipped` evidence gate (contract, fail-closed):
/// every linked `scenario_ids` entry MUST carry a latest NON-`Divergent`
/// verdict in the ledger. Reuses `canon-gate`'s own
/// [`latest_verdicts`] fold over a [`GateContext`] loaded exactly as
/// `canon gate check` loads it — never a second verdict derivation. A
/// scenario with NO ledger verdict is `uncovered-cell`; a scenario
/// whose latest verdict (for any authoring role) is `Divergent` is
/// likewise refused — the CLOSED [`FailureClass`] set has no
/// "divergent" member (by canon-gate design, a divergent verdict is a
/// reported fact, never its own gate class), so both refusals surface
/// as `uncovered-cell` with a distinguishing detail. An empty
/// `scenario_ids` yields no violations (nothing linked to gate).
fn ship_gate_violations(repo: &Path, scenario_ids: &[ScenarioId]) -> Result<Vec<Violation>, String> {
    let ctx = GateCtx::from_repo(repo);
    let registry = SchemaRegistry::load();
    let now = Utc::now();
    let gate_context = GateContext::load(ctx, &registry, now).map_err(|e| e.to_string())?;
    let verdicts = latest_verdicts(&gate_context);

    let mut violations = Vec::new();
    for scenario in scenario_ids {
        let sid = scenario.as_str().to_string();
        let entries: Vec<&LedgerEntry> = verdicts.iter().filter(|((subject, _), _)| subject == &sid).map(|(_, entry)| entry).collect();
        if entries.is_empty() {
            violations.push(Violation::new(FailureClass::UncoveredCell, sid.clone(), "verifying → shipped: no ledger verdict for this linked scenario"));
        } else if let Some(divergent) = entries.iter().find(|e| e.verdict == EvidenceVerdict::Divergent) {
            violations.push(Violation::new(
                FailureClass::UncoveredCell,
                sid.clone(),
                format!("verifying → shipped: latest verdict is divergent (by {})", divergent.agent_id),
            ));
        }
    }
    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forward_chain_is_the_only_forward_path() {
        use SubjectStatus::*;
        assert!(is_valid_transition(Proposed, Specced));
        assert!(is_valid_transition(Specced, Building));
        assert!(is_valid_transition(Building, Verifying));
        assert!(is_valid_transition(Verifying, Shipped));
        // Skips, backward steps, and self-loops are all invalid.
        assert!(!is_valid_transition(Proposed, Building));
        assert!(!is_valid_transition(Building, Proposed));
        assert!(!is_valid_transition(Building, Building));
        assert!(!is_valid_transition(Shipped, Verifying));
    }

    #[test]
    fn any_non_retired_state_may_retire_but_retired_is_terminal() {
        use SubjectStatus::*;
        for from in [Proposed, Specced, Building, Verifying, Shipped] {
            assert!(is_valid_transition(from, Retired), "{from:?} must be allowed to retire");
        }
        assert!(!is_valid_transition(Retired, Retired));
        assert!(!is_valid_transition(Retired, Proposed));
    }

    #[test]
    fn status_wire_strings_round_trip_through_the_parser() {
        for status in [
            SubjectStatus::Proposed,
            SubjectStatus::Specced,
            SubjectStatus::Building,
            SubjectStatus::Verifying,
            SubjectStatus::Shipped,
            SubjectStatus::Retired,
        ] {
            assert_eq!(parse_status(status_str(status)).unwrap(), status);
        }
        assert!(parse_status("not-a-state").is_err());
    }
}
