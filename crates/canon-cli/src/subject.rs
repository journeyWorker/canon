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
//! `fold_latest_by_key` (winner = greatest `(at, schema, digest)`) — the
//! SAME fold `canon-gate::ledger` and `canon query`'s pg-routed reader
//! already apply, so adopt/status re-writes read back as ONE latest
//! row. `Subject`'s `at` is wall-clock, so its two re-writes never tie
//! and the `schema`/`digest` rungs below `at` never engage here — the
//! bumped `at` is load-bearing, not cosmetic. (The `schema` rung exists
//! for the kinds whose `at` is byte-stable and therefore CAN tie —
//! plan-derived `Task`/`Change`, `s38-evidence-bearing-memory`.)
//!
//! # `verifying → shipped` is evidence-gated, fail-closed
//! That ONE transition additionally requires the subject to own at least
//! one scenario, and every scenario it owns to carry a latest
//! NON-`Divergent` verdict in the ledger; with `spec_coverage.
//! require_cases`, each owned feature surface must also specify a
//! scenario of every required case. A subject owns the scenarios
//! whose `@subject:<id>` Gherkin tag `canon inventory sync` indexed onto
//! `Scenario.subject_id` — read through
//! [`canon_gate::spec_coverage::subject_scenarios`], the same join the
//! gate's `spec_coverage.scope` and `canon report`'s Subjects panel use.
//! Verdicts resolve by REUSING [`canon_gate::latest_verdicts`] over a
//! [`canon_gate::GateContext`] loaded exactly as `canon gate check`
//! loads it (never a second verdict fold). A violation prints by failure
//! class ([`canon_gate::FailureClass`]), exits `1`, and leaves the record
//! UNCHANGED (fail closed). Every other transition is the pure
//! [`is_valid_transition`] chain; an off-chain transition is refused
//! (exit `2`), the record likewise unchanged.
//!
//! # `spec_coverage.require_review` guards every transition into its scope
//! With the policy present (issue #2), a transition whose target status
//! `require_review.scope` covers also runs
//! [`canon_gate::review_gate::subject_guard`]: every owned scenario needs
//! a qualifying review, and no adopted change may carry an open blocker
//! finding. A refusal exits `1` with the record unchanged, like the ship
//! gate. With `block_on_findings`, a Finding row the ledger read refused
//! is a `malformed-evidence` refusal naming its file
//! ([`canon_gate::review_gate::malformed_findings`]), never waivable.
//! `--override-reason` lets ONLY the review checks through; the
//! reason, the waived classes, and the actor are recorded on the new
//! Subject record ([`canon_model::StatusOverride`]), and `canon gate
//! check` keeps reporting the gaps as advisories. The guard prints, on
//! stderr, which checks it ran and which it skipped. Without the policy
//! none of this runs and the output is unchanged.

use std::path::Path;

use canon_gate::review_gate::{active_require_review, malformed_findings, subject_guard, unreadable_review_kinds, unreadable_violation};
use canon_gate::spec_coverage::{case_gaps, subject_scenarios};
use canon_gate::{latest_verdicts, FailureClass, GateContext, GateCtx, LedgerEntry, PolicyResolution, RequireReview, SpecCoverage, Violation};
use canon_model::{
    Actor, Change, ChangeId, Envelope, EvidenceVerdict, RawRecord, RecordKind, RoleId, StatusOverride, Subject, SubjectId, SubjectStatus, WaivedViolation,
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
/// key — winner = greatest `(at, envelope.schema, content_digest12)`,
/// via the shared [`canon_store::fold_latest_by_key`] every
/// multi-version reader uses, keyed by the SAME `resolve_partition`
/// natural key `canon query` derives. This is how an `adopt`/`status`
/// re-write (a new append at a bumped `at`) reads back as the one
/// current record; the `schema` rung
/// (`s38-evidence-bearing-memory`) is what keeps a same-`at` pair of
/// format generations from resolving by digest luck.
fn fold_latest(kind: RecordKind, records: Vec<RawRecord>) -> Vec<RawRecord> {
    struct Candidate {
        key: String,
        at: DateTime<Utc>,
        schema: u32,
        digest: String,
        record: RawRecord,
    }
    let candidates = records.into_iter().map(|record| {
        let key = canon_store::partition::resolve_partition(kind, &record.0).map(|p| p.natural_key).unwrap_or_default();
        let at = canon_store::tier::raw_record_at(&record);
        let schema = canon_store::tier::raw_record_schema(&record);
        let digest = canon_store::partition::content_digest12(&record.0);
        Candidate { key, at, schema, digest, record }
    });
    canon_store::fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.schema, |c| c.digest.as_str())
        .into_values()
        .map(|c| c.record)
        .collect()
}

/// The current, folded-to-latest set of `kind` records read through
/// `registry` (also `crate::plans`' adoption read).
pub(crate) fn latest_records(registry: &TierRegistry, kind: RecordKind) -> Result<Vec<RawRecord>, StoreError> {
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
        println!(
            "canon subject {verb}: {} ({}, {}) — {}",
            subject.subject_id.as_str(),
            subject.domain,
            status_str(subject.status),
            crate::write_mode::DIRECT
        );
    }
}

/// The `domain` enum name `canon.core` declares and a consumer repo
/// extends — the SAME key `canon context` renders under `vocab.enums`.
const DOMAIN_ENUM: &str = "domain";

/// Is `value` a member of this repo's ACTIVATED `enum_name` vocabulary?
/// `None` = acceptable (including when no such enum is declared),
/// `Some(message)` = the operator-facing refusal. Shared by `subject
/// new --domain` (`domain`) and `scenario new --lane` (`lane`, s49) —
/// one write-time membership rule, one refusal grammar.
///
/// # Why this exists
/// `canon.core`'s `enums.yaml` has always declared the `domain` set, and
/// its own comment said it was "surfaced by `canon context` so an author
/// can see the activated domain set before writing a Subject" — surfaced,
/// never checked. No `Type::Domain` attr referenced it and this command
/// did not read it, so `domain` was the one declared enum in the
/// vocabulary that nothing enforced: a typo silently minted a new
/// category, and every `--domain`-filtered read
/// (`canon query --kind subject --domain …`, `canon report`'s Subjects
/// panel) then disagreed about how many categories the repo has.
/// `task-status` is checked through `Type::Domain` and `handoff-domain`
/// through its directive tag plus `canon.yaml`'s `handoff_templates`;
/// this closes the third case at its single write point, and `lane`
/// reuses the closure.
///
/// # Why it stays dynamic
/// The member set is resolved from the vocabulary, never hardcoded here
/// — a team extends it in its own `.canon/vocab/<id>/enums.yaml` with no
/// canon-model or canon-cli change, exactly as
/// `canon_model::records::deserialize_domain_slug`'s doc promises
/// ("canon-model deliberately does NOT encode which domains a repo
/// activates"). A repo cutting categories by feature, by discipline, or
/// by anything else declares its own set and gets it enforced.
///
/// # Fail-soft when nothing is declared
/// A repo with no vocabulary plugin, or one whose plugins declare no
/// such enum, resolves to no constraint and every kebab slug is
/// accepted. `canon init` scaffolds no `.canon/vocab`, so the alternative
/// would make a freshly-inited repo unable to author its first Subject
/// or lane. This mirrors the empty-`risk_routing`-derives-zero-cells
/// default canon-gate already uses for policy.
///
/// This is a WRITE-time check at the one place a value is ever set, so
/// an already-persisted record whose value predates its vocabulary is
/// untouched and still reads back — the check tightens authoring, never
/// invalidates history.
pub fn enum_membership_violation(repo: &Path, enum_name: &str, value: &str) -> Option<String> {
    let (snapshot, _diags) = canon_vocab::resolve_snapshot(repo, None);
    let members = snapshot.enums.get(enum_name)?;
    if members.is_empty() || members.iter().any(|m| m == value) {
        return None;
    }
    Some(format!("`{value}` is not a valid value for `{enum_name}` (expected one of: {})", members.join(", ")))
}

/// Is `domain` a member of this repo's ACTIVATED domain vocabulary?
/// `None` = acceptable, `Some(message)` = the operator-facing refusal.
fn domain_membership_violation(repo: &Path, domain: &str) -> Option<String> {
    enum_membership_violation(repo, DOMAIN_ENUM, domain)
}

/// `canon subject new <id> --domain <d> --title <t> [--summary <s>]
/// [--owner-role <r>]` (module doc): author a fresh [`Subject`] at
/// status `proposed`. The envelope is attributed to `actor_id` (default
/// `canon`, the same source `canon review add` uses) in the role
/// `owner_role`. `domain` is validated in two independent steps: SHAPE
/// here (kebab slug, the SAME grammar the model's
/// `deserialize_domain_slug` enforces — reused via `SubjectId`'s
/// identical grammar so the CLI never panics on the model's
/// debug-assert), then MEMBERSHIP against the repo's activated `domain`
/// enum ([`domain_membership_violation`]). A `subject_id` already
/// present in the store is a loud refusal (exit `2`), never a silent
/// second append.
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

    // Membership is a SEPARATE question from shape, and needs the
    // resolved repo root to find the vocabulary — so it runs after
    // `resolve_repo_root` below, not here.

    let repo = resolve_repo_root(repo);
    if let Some(violation) = domain_membership_violation(&repo, domain) {
        eprintln!("canon subject new: refused — {violation}");
        return EXIT_REFUSED;
    }
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
/// an imported plan [`Change`] to a [`Subject`] via [`adopt_change`],
/// printing the result. Exit `2` on any refusal or store failure.
/// Rerunning it on a linked pair writes nothing and exits `0`.
pub fn run_adopt(repo: &Path, change_id: &ChangeId, subject_id: &SubjectId, json: bool) -> i32 {
    match adopt_change(repo, change_id, subject_id) {
        Ok((subject, wrote)) => {
            if json {
                report_subject(&subject, "adopt", true);
            } else if wrote {
                println!(
                    "canon subject adopt: linked change `{}` to subject `{}` — {}",
                    change_id.as_str(),
                    subject_id.as_str(),
                    crate::write_mode::DIRECT
                );
            } else {
                println!("canon subject adopt: change `{}` is already linked to subject `{}`; nothing written", change_id.as_str(), subject_id.as_str());
            }
            0
        }
        Err(e) => {
            eprintln!("canon subject adopt: {e}");
            EXIT_REFUSED
        }
    }
}

/// Look up, then write, an adoption: `canon subject adopt`'s whole
/// effect. Refuses (`Err` with the operator-facing message, no command
/// prefix) when either record is absent; see [`persist_adoption`] for
/// the write. `Ok` carries the subject and whether anything was written
/// (`false`: both sides already carried the link).
pub(crate) fn adopt_change(repo: &Path, change_id: &ChangeId, subject_id: &SubjectId) -> Result<(Subject, bool), String> {
    let (subject, change) = lookup_adoption(repo, subject_id, change_id)?;
    let Some(subject) = subject else {
        return Err(format!("refused — subject `{}` does not exist (author it with `canon subject new` first)", subject_id.as_str()));
    };
    let Some(change) = change else {
        return Err(format!("refused — change `{}` does not exist (import it with `canon ingest plans` first)", change_id.as_str()));
    };
    persist_adoption(repo, change, subject).map_err(|e| e.describe(change_id, subject_id, &adopt_command(change_id, subject_id)))
}

/// The latest `subject_id` Subject and `change_id` Change (fold-latest),
/// each `None` when absent — read through the routed tiers the adopt
/// write uses.
pub(crate) fn lookup_adoption(repo: &Path, subject_id: &SubjectId, change_id: &ChangeId) -> Result<(Option<Subject>, Option<Change>), String> {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo, None);
    let registry = registry_for(&canon_yaml_path, &[RecordKind::Change, RecordKind::Subject]).map_err(|e| e.to_string())?;
    Ok((find_subject(&registry, subject_id)?, find_change(&registry, change_id)?))
}

/// `canon subject adopt <change> --subject <subject>`: the command that
/// completes a half-written adoption. Safe to rerun: [`persist_adoption`]
/// writes only the side that still lacks the link, and nothing once both
/// carry it.
pub(crate) fn adopt_command(change_id: &ChangeId, subject_id: &SubjectId) -> String {
    format!("canon subject adopt {} --subject {}", change_id.as_str(), subject_id.as_str())
}

/// A failed [`persist_adoption`]: the store error, and whether the
/// subject record had already been written when the change write failed.
pub(crate) struct AdoptionWriteError {
    pub(crate) subject_written: bool,
    pub(crate) message: String,
}

impl AdoptionWriteError {
    /// The operator-facing message (no command prefix). Before any write:
    /// the error and that nothing was written. Between the two writes:
    /// which record exists, and `repair`, the command that completes the
    /// link.
    pub(crate) fn describe(&self, change_id: &ChangeId, subject_id: &SubjectId, repair: &str) -> String {
        if self.subject_written {
            format!(
                "subject `{}` now lists change `{}`, but the change record could not be written: {}; complete the link with `{repair}`",
                subject_id.as_str(),
                change_id.as_str(),
                self.message
            )
        } else {
            format!("failed to record the adoption: {}; nothing was written", self.message)
        }
    }
}

/// The adopt write, shared by `canon subject adopt` and `canon change
/// new`: the subject with the change appended to `change_ids`, and the
/// change with `subject_id` set (design D3 — stamped at adoption time,
/// never derived in canon-model), each re-stamped with a fresh envelope
/// `at` so it deterministically supersedes the prior version in the
/// query fold, persisted through the routed tiers. Returns the subject
/// and whether anything was written.
///
/// # Two records, no transaction
/// The ledger is append-only and the two kinds may route to different
/// tiers, so the pair cannot be written atomically. Two rules make a
/// failure between the writes recoverable:
///
/// - **Subject first.** `subject.change_ids` is the side the gate joins
///   on: `spec_coverage.require_review`'s `open-blocker` check and the
///   subject status guard read a subject's adopted changes from it. A
///   subject that lists the change while the change record lacks the
///   link still has that change's open blockers counted, so a half-done
///   adoption errs toward enforcing. The other order would leave a
///   change claiming a subject whose gate never looks at it.
/// - **Only what is missing.** A side that already carries the link is
///   not rewritten, and nothing is written once both do, so
///   [`adopt_command`] completes a half-written adoption and is a no-op
///   on a whole one.
pub(crate) fn persist_adoption(repo: &Path, mut change: Change, mut subject: Subject) -> Result<(Subject, bool), AdoptionWriteError> {
    let needs_subject = !subject.change_ids.contains(&change.change_id);
    let needs_change = change.subject_id.as_ref() != Some(&subject.subject_id);
    if !needs_subject && !needs_change {
        return Ok((subject, false));
    }

    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo, None);
    let registry = registry_for(&canon_yaml_path, &[RecordKind::Change, RecordKind::Subject])
        .map_err(|e| AdoptionWriteError { subject_written: false, message: e.to_string() })?;

    let now = Utc::now();
    if needs_subject {
        subject.change_ids.push(change.change_id.clone());
        subject.envelope.at = now;
        registry.persist(&subject).map_err(|e| AdoptionWriteError { subject_written: false, message: e.to_string() })?;
    }
    if needs_change {
        change.subject_id = Some(subject.subject_id.clone());
        change.envelope.at = now;
        registry.persist(&change).map_err(|e| AdoptionWriteError { subject_written: needs_subject, message: e.to_string() })?;
    }
    Ok((subject, true))
}

/// `canon subject status <id> <state>` (module doc): apply a lifecycle
/// transition. Refuses an off-chain step ([`is_valid_transition`], exit
/// `2`); for `verifying → shipped` additionally runs
/// [`ship_gate_violations`], and for a target `require_review.scope`
/// covers, the review guard (module doc). On any violation it prints
/// each by failure class and exits `1` with the record UNCHANGED (fail
/// closed), unless every violation is a review check and
/// `override_reason` waives them. A successful transition re-stamps the
/// subject (fresh `at`) and persists it through the routed tier.
pub fn run_status(repo: &Path, subject_id: &SubjectId, target: SubjectStatus, override_reason: Option<&str>, actor_id: &str, json: bool) -> i32 {
    if let Some(reason) = override_reason {
        if reason.trim().is_empty() || reason.contains(['\n', '\r']) {
            eprintln!("canon subject status: refused — --override-reason must be one non-empty line; it is recorded on the subject as the accountable reason");
            return EXIT_REFUSED;
        }
    }

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

    let schemas = SchemaRegistry::load();
    let policy = PolicyResolution::resolve(&repo, &schemas);
    let ships = current == SubjectStatus::Verifying && target == SubjectStatus::Shipped;
    let reviews = match &policy.spec_coverage {
        Some(SpecCoverage::Active { require_review: Some(rr), .. }) => rr.covers(target),
        _ => false,
    };
    let gate_context = if ships || reviews {
        match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &schemas, Utc::now()).map_err(|e| e.to_string())) {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                eprintln!("canon subject status: {e}");
                return EXIT_REFUSED;
            }
        }
    } else {
        None
    };

    let mut blocking = match (ships, &gate_context) {
        (true, Some(ctx)) => ship_gate_violations(ctx, subject_id),
        _ => Vec::new(),
    };
    let mut waivable = Vec::new();
    report_review_guard(&policy, current, target, subject.change_ids.len());
    if let (true, Some(ctx)) = (reviews, &gate_context) {
        let (rr, exclude_lanes) = active_require_review(ctx).expect("`reviews` implies an active require_review");
        let unreadable = unreadable_review_kinds(ctx, rr);
        if unreadable.is_empty() {
            // An unreadable finding blocks, unwaivably: it might be an
            // open blocker on this subject's changes.
            blocking.extend(malformed_findings(ctx, rr));
            waivable = subject_guard(ctx, rr, exclude_lanes, subject_id, &subject.change_ids);
            for violation in &mut waivable {
                violation.detail = format!("{} → {}: {}", status_str(current), status_str(target), violation.detail);
            }
        } else {
            blocking.push(unreadable_violation(&unreadable));
        }
    }

    if !blocking.is_empty() {
        for v in blocking.iter().chain(&waivable) {
            eprintln!("canon subject status: {}", v.line());
        }
        if override_reason.is_some() {
            eprintln!("canon subject status: --override-reason waives only the review checks (unreviewed-promotion, open-blocker); the violations above that are not review checks still refuse this transition");
        }
        return EXIT_GATED;
    }

    // A waiver belongs to the transition that needed it: every write
    // starts without one, so an old waiver never carries forward.
    subject.status_override = None;
    if !waivable.is_empty() {
        let Some(reason) = override_reason else {
            for v in &waivable {
                eprintln!("canon subject status: {}", v.line());
            }
            eprintln!(
                "canon subject status: refused — record an independent review (`canon review add`) and close blocker findings, or pass --override-reason <text> to move anyway; the waiver is recorded on the subject and `canon gate check` keeps listing these gaps"
            );
            return EXIT_GATED;
        };
        for v in &waivable {
            eprintln!("canon subject status: waived {}", v.line());
        }
        let waived: std::collections::BTreeSet<WaivedViolation> =
            waivable.iter().map(|v| WaivedViolation { class: v.class.as_str().to_string(), subject: v.subject.clone() }).collect();
        let count = waived.len();
        subject.status_override = Some(StatusOverride { to: target, reason: reason.to_string(), waived: waived.into_iter().collect(), actor: Actor::new_unattributed(actor_id) });
        eprintln!(
            "canon subject status: override recorded by `{actor_id}` for the {count} violation(s) above: {reason}; a gap that appears later is not covered"
        );
    } else if override_reason.is_some() {
        eprintln!("canon subject status: --override-reason not recorded — no review check refused this transition");
    }

    subject.status = target;
    subject.envelope.at = Utc::now();

    match registry.persist(&subject) {
        Ok(_) => {
            if json {
                report_subject(&subject, "status", true);
            } else {
                println!("canon subject status: {} → {} — {}", subject_id.as_str(), status_str(target), crate::write_mode::DIRECT);
            }
            0
        }
        Err(e) => {
            eprintln!("canon subject status: {e}");
            EXIT_REFUSED
        }
    }
}

/// Print, on stderr, which review checks this transition runs and which
/// it skips. Silent when `policy.yaml` has no `require_review`, so a repo
/// that never opted in sees exactly the pre-0.12 output.
fn report_review_guard(policy: &PolicyResolution, from: SubjectStatus, to: SubjectStatus, changes: usize) {
    let rr: &RequireReview = match &policy.spec_coverage {
        Some(SpecCoverage::Active { require_review: Some(rr), .. }) => rr,
        Some(SpecCoverage::Invalid { .. }) => {
            eprintln!("canon subject status: review guard skipped — policy.yaml's `spec_coverage` section is invalid, so whether review is required cannot be read; `canon gate check` reports why");
            return;
        }
        _ => return,
    };
    let scope = if rr.scope.is_empty() { "every status".to_string() } else { rr.scope.iter().map(|s| status_str(*s)).collect::<Vec<_>>().join(", ") };
    eprintln!("canon subject status: review guard for {} → {} (spec_coverage.require_review, scope: {scope})", status_str(from), status_str(to));
    if !rr.covers(to) {
        eprintln!("canon subject status:   skipped unreviewed-promotion — `{}` is not in require_review.scope", status_str(to));
        eprintln!("canon subject status:   skipped open-blocker — `{}` is not in require_review.scope", status_str(to));
        return;
    }
    let rule = if rr.distinct_actor { "a review by an actor, and from a session, other than its evidence's" } else { "a review record" };
    eprintln!("canon subject status:   ran unreviewed-promotion — every owned scenario needs {rule}");
    if rr.block_on_findings {
        eprintln!("canon subject status:   ran open-blocker — {changes} adopted change(s) checked for open blocker findings");
    } else {
        eprintln!("canon subject status:   skipped open-blocker — require_review.block_on_findings is false");
    }
}

/// The `verifying → shipped` evidence gate (contract, fail-closed). The
/// scenarios checked are the ones the subject OWNS through their
/// `@subject:` tag ([`subject_scenarios`]); there is no second,
/// hand-maintained link list to drift from the spec corpus.
///
/// Refuses, each as `uncovered-cell` with a distinguishing detail (the
/// CLOSED [`FailureClass`] set has no "divergent" or "unlinked" member —
/// by canon-gate design those are reported facts, never their own gate
/// class):
/// - the `scenario` kind routes away from the rung the gate reads, so the
///   owned set would read as empty for a reason other than "none";
/// - the subject owns NO scenario: shipping with zero evidence is the
///   exact pass-by-seeing-nothing this gate exists to stop;
/// - an owned scenario has no ledger verdict;
/// - an owned scenario's latest verdict (for any authoring role) is
///   `Divergent`;
/// - `spec_coverage.require_cases` is set and an owned feature surface
///   carries no scenario of a required case (through the SAME
///   [`case_gaps`] rule `canon gate check` applies), so a subject cannot
///   ship on a spec that only describes its golden path.
fn ship_gate_violations(gate_context: &GateContext, subject_id: &SubjectId) -> Vec<Violation> {
    let subject = subject_id.as_str();

    if gate_context.unreadable_kinds.contains(&RecordKind::Scenario) {
        return vec![Violation::new(
            FailureClass::UncoveredCell,
            subject,
            "verifying → shipped: the `scenario` kind routes away from the rung the gate reads, so this subject's scenarios cannot be counted; route `scenario` to `local`",
        )];
    }

    let owned = subject_scenarios(gate_context, subject_id);
    if owned.is_empty() {
        return vec![Violation::new(
            FailureClass::UncoveredCell,
            subject,
            format!("verifying → shipped: no scenario is tagged `@subject:{subject}` — tag its scenarios and run `canon inventory sync`; shipping needs evidence, not an empty set"),
        )];
    }

    let verdicts = latest_verdicts(gate_context);
    let mut violations = Vec::new();
    for scenario in &owned {
        let sid = scenario.scenario_id.as_str().to_string();
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

    if let Some(SpecCoverage::Active { exclude_lanes, require_cases, .. }) = &gate_context.policy.spec_coverage {
        let counted = owned.iter().copied().filter(|s| !s.lane.as_ref().is_some_and(|lane| exclude_lanes.contains(lane)));
        for gap in case_gaps(counted, require_cases) {
            let mut violation = gap.violation();
            violation.detail = format!("verifying → shipped: {}", violation.detail);
            violations.push(violation);
        }
    }
    violations
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
