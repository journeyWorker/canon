//! `canon subject {new,adopt,move}` (s36 `subject-domain-loop`): the
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
//! scenario of every required case; and no owned scenario may have a
//! `refuted` latest audit (`false-success`) or, under
//! `spec_coverage.require_audit`, an audit still owed. A subject owns the scenarios
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
//! # `verifying → shipped` needs the approved brief where one exists (0.16 D1, 0.17 D3)
//! The same transition also refuses when the subject's brief is
//! enforced and not approved, through [`crate::brief::brief_of`], the
//! one call `canon status` reports the brief with: its newest adopted
//! change has a brief to answer (its `.canon/changes/<change>/brief.md`,
//! or an external openspec `proposal.md` while that canon file does not
//! exist), and the canon `brief.md` carries no `Approved:` line outside
//! HTML comments and code fences. The refusal names that `brief.md`
//! (with the `Source:` pointer to write, for an external plan) and says
//! a human must answer the brief; the agent records the line only from
//! that answer. It is an
//! `unreviewed-promotion` naming the subject, a review check like those
//! below, so `--override-reason` lets it through and records it in the
//! waiver. With no adopted change or no brief to answer, the guard
//! prints an advisory saying the
//! brief is not approved and does not refuse. A brief that says the
//! human ships changes who runs the transition, not the guard. It runs
//! whatever the policy says, as status reports the brief whatever the
//! policy says.
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
//!
//! # A reason on any transition (0.21.1)
//! `--reason` is accepted on every transition and recorded with the actor
//! on the new record ([`canon_model::StatusReason`]); the reopen is the one
//! transition that requires it, and records [`canon_model::Reopen`] too.
//!
//! # Reopening a shipped subject (0.18 D3)
//! `shipped → verifying` is the one backward step: follow-up work on a
//! shipped subject (a fix, a review round, a scenario its gate now
//! misses) reopens it. It needs `--reason`, recorded with the actor on
//! the new record ([`canon_model::Reopen`], with the change the work goes
//! into). Inside the approved scope it needs no human: the subject's
//! newest adopted change has an approved brief, or there is no brief to
//! answer. Otherwise it refuses, unwaivably, naming the brief to approve.
//! The review guard does not run on it (it moves the subject back to fix
//! its gaps); shipping again runs every guard above, as the first ship
//! did.
//!
//! # A change may belong to several subjects (0.18 D4)
//! `canon subject adopt` appends: adopting a change under a second
//! subject adds that subject to the change's `subject_ids` and the
//! change to the subject's `change_ids`, and says the change now belongs
//! to both. Every guard reads a subject's changes joined from both sides
//! ([`canon_gate::spec_coverage::subject_change_ids`]), so an open
//! blocker on a shared change gates each subject it belongs to.

use std::path::Path;

use canon_gate::review_gate::{
    active_require_review, malformed_findings, subject_guard, unreadable_review_kinds, unreadable_violation,
};
use canon_gate::spec_coverage::{case_gaps, subject_change_ids, subject_scenarios};
use canon_gate::{latest_verdicts, FailureClass, GateContext, GateCtx, LedgerEntry, PolicyResolution, RequireReview, SpecCoverage, Violation};
use canon_model::{
    Change, ChangeId, Envelope, EvidenceVerdict, RawRecord, RecordKind, Reopen, RoleId, StatusOverride, StatusReason, Subject, SubjectId,
    SubjectStatus, WaivedViolation,
};
use canon_policy::SchemaRegistry;
use canon_store::registry::TierRegistry;
use canon_store::tier::{StoreError, TierQuery};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::cli_error::CliError;
use crate::context::{resolve_canon_yaml, resolve_repo_root};
use crate::tiers::{self, TierCliError};

/// Exit code for the `verifying → shipped` evidence-gate block: the
/// transition is well-formed but the linked scenarios are not covered
/// by non-`Divergent` verdicts — record unchanged, fail closed
/// (contract: "violations print by failure class, exit 1").
const EXIT_GATED: i32 = 1;

/// `<id>` / `--subject-id`'s `clap` value parser — grammar-level
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
pub fn status_parser() -> impl clap::builder::TypedValueParser<Value = SubjectStatus> {
    crate::cli_error::closed_set(["proposed", "specced", "building", "verifying", "shipped", "retired"], parse_status)
}

/// A subject move, as the CLI spells it.
pub fn parse_status(s: &str) -> Result<SubjectStatus, String> {
    match s {
        "proposed" => Ok(SubjectStatus::Proposed),
        "specced" => Ok(SubjectStatus::Specced),
        "building" => Ok(SubjectStatus::Building),
        "verifying" => Ok(SubjectStatus::Verifying),
        "shipped" => Ok(SubjectStatus::Shipped),
        "retired" => Ok(SubjectStatus::Retired),
        _ => Err(format!(
            "unknown subject move `{s}` (expected one of: proposed, specced, building, verifying, shipped, retired)"
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
/// chain `proposed → specced → building → verifying → shipped`, the one
/// backward step `shipped → verifying` that reopens a shipped subject
/// for follow-up work (0.18 D3, which [`run_move`] also requires a
/// `--reason` and the approved scope for), plus any non-retired state →
/// `retired`. Every other transition (a skip, another backward step, a
/// self-loop, or anything out of `retired`) is invalid. The `verifying →
/// shipped` EVIDENCE gate is a SEPARATE, additional requirement layered
/// on top of this pure step check (see [`ship_gate_violations`]).
pub fn is_valid_transition(from: SubjectStatus, to: SubjectStatus) -> bool {
    use SubjectStatus::*;
    if to == Retired {
        return from != Retired;
    }
    matches!((from, to), (Proposed, Specced) | (Specced, Building) | (Building, Verifying) | (Verifying, Shipped) | (Shipped, Verifying))
}

/// Build a [`TierRegistry`] over exactly `kinds`' routed rungs, via the
/// SAME lenient, per-rung tier construction `canon query`/`canon
/// ingest` share ([`tiers::build_lenient_tiers_for_kinds`]) — so a
/// subject write honors `canon.yaml`'s `routing.subject` and lands
/// where `canon query --kind subject` reads.
pub(crate) fn registry_for(canon_yaml_path: &Path, kinds: &[RecordKind]) -> Result<TierRegistry, TierCliError> {
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

/// Report a written subject: `human` and the `next:` line, or on `--json`
/// the full record body (the updated record) with `meta.next` for the
/// subject.
fn report_subject(repo: &Path, subject: &Subject, verb: &str, dry_run: bool, json: bool, human: &str) {
    report_subject_warning(repo, subject, verb, dry_run, json, human, &[]);
}

/// [`report_subject`] with `warnings`: said on stderr, and in `--json`
/// carried in `meta.warnings` ([`crate::json_out::warnings_meta`]).
#[allow(clippy::too_many_arguments)]
fn report_subject_warning(
    repo: &Path,
    subject: &Subject,
    verb: &str,
    dry_run: bool,
    json: bool,
    human: &str,
    warnings: &[String],
) {
    if !json {
        println!("{human}");
    }
    for warning in warnings {
        crate::human_err!("canon subject {verb}: warning: {warning}");
    }
    let scope = crate::next::Scope::Subject(subject.subject_id.as_str().to_string());
    if dry_run {
        // A preview marks itself: the subject as it would stand, plus `dryRun`.
        crate::next::finish_with_meta(repo, &format!("subject {verb}"), &scope, false, json, &SubjectPreviewReport { subject, dry_run: true }, crate::json_out::warnings_meta(warnings));
    } else if verb == "move" {
        let overrides = subject.status_override.iter().map(|waiver| crate::reports::Override { code: "SUBJECT_GATE_VIOLATION".to_string(), reason: waiver.reason.clone() }).collect();
        crate::next::finish_with_meta(repo, "subject move", &scope, false, json, &SubjectMoveReport { subject, overrides }, crate::json_out::warnings_meta(warnings));
    } else {
        crate::next::finish_with_meta(repo, &format!("subject {verb}"), &scope, false, json, &SubjectReport(subject), crate::json_out::warnings_meta(warnings));
    }
}

/// `data` of the subject leaves: the Subject record, embedded verbatim (it keeps the ledger record's own schema and snake_case keys).
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct SubjectReport<'a>(pub &'a Subject);

impl crate::json_out::Report for SubjectReport<'_> {}

/// One line per subject that stands in its status under a waiver (D12):
/// who waived which refusals, when, and why. `canon status` lists them as
/// warnings for the human; they never fail anything.
pub fn override_lines(ctx: &GateContext) -> Vec<String> {
    let mut lines: Vec<(chrono::DateTime<Utc>, String)> = canon_gate::spec_coverage::latest_subjects(ctx)
        .into_iter()
        .filter_map(|subject| {
            let waiver = subject.status_override.as_ref().filter(|w| w.to == subject.status)?;
            let waived = waiver.waived.iter().map(|w| format!("{} {}", w.class, w.subject)).collect::<Vec<_>>().join(", ");
            Some((
                subject.envelope.at,
                format!(
                    "subject {} was moved to {} by `{}` at {} past {} refusal(s) ({waived}) (override: {})",
                    subject.subject_id.as_str(),
                    status_str(subject.status),
                    waiver.actor.agent_id,
                    subject.envelope.at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    waiver.waived.len(),
                    waiver.reason
                ),
            ))
        })
        .collect();
    lines.sort();
    lines.into_iter().map(|(_, line)| line).collect()
}

/// `data` of `subject move`: the Subject record, plus the refusals `--override-reason` waived (D12).
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubjectMoveReport<'a> {
    #[serde(flatten)]
    pub subject: &'a Subject,
    /// The refusals this move waived under `--override-reason`; empty when nothing was waived. The record's `status_override` lists each waived violation.
    pub overrides: Vec<crate::reports::Override>,
}

impl crate::json_out::Report for SubjectMoveReport<'_> {}

/// `data` of a subject leaf under `--dry-run`: the Subject as it would stand, plus `dryRun`.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubjectPreviewReport<'a> {
    #[serde(flatten)]
    pub subject: &'a Subject,
    /// Always true.
    pub dry_run: bool,
}

impl crate::json_out::Report for SubjectPreviewReport<'_> {}

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
        return CliError::refused("SUBJECT_INVALID_ARGUMENT", format!("domain `{domain}` is not a kebab-case slug (`[a-z0-9]+(-[a-z0-9]+)*`)")).hint("`canon schema subject new` lists the accepted arguments and their forms").emit("canon subject new", json);
    }

    // Membership is a SEPARATE question from shape, and needs the
    // resolved repo root to find the vocabulary — so it runs after
    // `resolve_repo_root` below, not here.

    let repo = resolve_repo_root(repo);
    if let Some(violation) = domain_membership_violation(&repo, domain) {
        return CliError::refused("SUBJECT_INVALID_ARGUMENT", violation.to_string()).hint("`canon schema subject new` lists the accepted arguments and their forms").emit("canon subject new", json);
    }
    let canon_yaml_path = resolve_canon_yaml(&repo);
    let registry = match registry_for(&canon_yaml_path, &[RecordKind::Subject]) {
        Ok(r) => r,
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon subject new", json);
        }
    };

    match find_subject(&registry, subject_id) {
        Ok(Some(_)) => {
            return CliError::refused("SUBJECT_EXISTS", format!("subject `{}` already exists", subject_id.as_str())).hint("pick another id for a new subject, or look at this one with `canon subject move <id>`").emit("canon subject new", json);
        }
        Ok(None) => {}
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon subject new", json);
        }
    }

    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), crate::role::attributed(actor_id, Some(owner_role)));
    let subject = Subject::new(envelope, subject_id.clone(), title, summary, domain, SubjectStatus::Proposed, owner_role.clone());

    match registry.persist(&subject) {
        Ok(_) => {
            let human = format!("canon subject new: {} ({}, {}) — {}", subject.subject_id.as_str(), subject.domain, status_str(subject.status), crate::write_mode::DIRECT);
            report_subject(&repo, &subject, "new", false, json, &human);
            0
        }
        Err(e) => {
            CliError::refused("WRITE_FAILED", e.to_string()).hint("check that the path is writable and the disk has room, then run the command again").emit("canon subject new", json)
        }
    }
}

/// `canon subject adopt <subject_id> --change-id <id>` (module doc): link
/// an imported plan [`Change`] to a [`Subject`] via [`adopt_change`],
/// printing the result. Exit `2` on any refusal or store failure.
/// Rerunning it on a linked pair writes nothing and exits `0`. A change
/// may belong to several subjects (0.18 D4): adopting it under a second
/// one adds that subject and says the change now belongs to each, never
/// re-points it.
pub fn run_adopt(repo: &Path, change_id: &ChangeId, subject_id: &SubjectId, dry_run: bool, json: bool) -> i32 {
    match adopt_change(repo, change_id, subject_id, dry_run) {
        Ok((subject, owners, wrote)) => {
            let (link, now_belongs) = if dry_run { ("would link", "would belong") } else { ("linked", "now belongs") };
            let mode = if dry_run { "dry run, nothing written" } else { crate::write_mode::DIRECT };
            let human = if wrote && owners.len() > 1 {
                format!(
                    "canon subject adopt: {link} change `{}` to subject `{}` as well; the change {now_belongs} to {} — {mode}",
                    change_id.as_str(),
                    subject_id.as_str(),
                    quoted_list(&owners),
                )
            } else if wrote {
                format!("canon subject adopt: {link} change `{}` to subject `{}` — {mode}", change_id.as_str(), subject_id.as_str())
            } else {
                crate::json_out::mark_unchanged();
                format!("canon subject adopt: change `{}` is already linked to subject `{}`; nothing written", change_id.as_str(), subject_id.as_str())
            };
            let warnings: Vec<String> = crate::brief::adoption_warning(repo, &subject, change_id)
                .into_iter()
                .collect();
            report_subject_warning(repo, &subject, "adopt", dry_run, json, &human, &warnings);
            0
        }
        Err(e) => {
            CliError::refused("SUBJECT_ADOPT_REFUSED", e.to_string()).hint("fix what the message names, then run `canon subject adopt` again").emit("canon subject adopt", json)
        }
    }
}

/// `canon subject unadopt <subject_id> --change-id <id>` (0.25 D1): take
/// the link [`run_adopt`] made back, on both sides. Refused for a change
/// whose canon brief carries an `Approved:` line: it is the subject's
/// approved scope. An unlinked pair writes nothing and exits `0`.
pub fn run_unadopt(repo: &Path, change_id: &ChangeId, subject_id: &SubjectId, dry_run: bool, json: bool) -> i32 {
    match unadopt_change(repo, change_id, subject_id, dry_run) {
        Ok(UnadoptOutcome::Refused(message, hint)) => CliError::refused("SUBJECT_UNADOPT_REFUSED", message).hint(hint).emit("canon subject unadopt", json),
        Ok(UnadoptOutcome::Done(subject, wrote)) => {
            let human = if wrote {
                let (unlink, mode) = if dry_run { ("would unlink", "dry run, nothing written") } else { ("unlinked", crate::write_mode::DIRECT) };
                format!("canon subject unadopt: {unlink} change `{}` from subject `{}` — {mode}", change_id.as_str(), subject_id.as_str())
            } else {
                crate::json_out::mark_unchanged();
                format!("canon subject unadopt: change `{}` is not linked to subject `{}`; nothing written", change_id.as_str(), subject_id.as_str())
            };
            report_subject(repo, &subject, "unadopt", dry_run, json, &human);
            0
        }
        Err(e) => CliError::refused("SUBJECT_ADOPT_REFUSED", e).hint("fix what the message names, then run `canon subject unadopt` again").emit("canon subject unadopt", json),
    }
}

enum UnadoptOutcome {
    /// The subject as it now stands, and whether anything was written.
    Done(Subject, bool),
    /// The refusal message and its hint.
    Refused(String, String),
}

/// The unadopt write: the mirror of [`adopt_change`] and [`persist_adoption`].
/// Removes the change from the subject's `change_ids` and the subject from
/// the change's `subject_ids`, each re-stamped and persisted through the
/// routed tiers, only for the side that still carries the link, so a
/// half-written unadopt is finished by running it again.
fn unadopt_change(
    repo: &Path,
    change_id: &ChangeId,
    subject_id: &SubjectId,
    dry_run: bool,
) -> Result<UnadoptOutcome, String> {
    let (subject, change) = lookup_adoption(repo, subject_id, change_id)?;
    let Some(mut subject) = subject else {
        return Err(format!(
            "subject `{}` does not exist (author it with `canon subject new` first)",
            subject_id.as_str()
        ));
    };
    let Some(mut change) = change else {
        return Err(format!(
            "change `{}` does not exist (import it with `canon ingest plans` first)",
            change_id.as_str()
        ));
    };
    let on_subject = subject.change_ids.contains(change_id);
    let on_change = change.subject_ids.contains(subject_id);
    if !on_subject && !on_change {
        return Ok(UnadoptOutcome::Done(subject, false));
    }
    let resolved = resolve_repo_root(repo);
    if let Some(brief) = crate::brief::canon_brief(&resolved, change_id) {
        let approved = std::fs::read_to_string(&brief)
            .is_ok_and(|text| canon_ingest::markdown::field(&text, "Approved:").is_some());
        if approved {
            return Ok(UnadoptOutcome::Refused(
                format!(
                    "change `{}` has an approved brief ({}), so it is subject `{}`'s approved scope and cannot be unadopted",
                    change_id.as_str(),
                    brief.strip_prefix(&resolved).unwrap_or(&brief).display(),
                    subject_id.as_str()
                ),
                format!(
                    "an approved change is the subject's approved scope and stays adopted; unadopt only a change nobody approved, or keep `{}` and let its flow-tagged scenarios wait for a new approval",
                    change_id.as_str()
                ),
            ));
        }
    }
    let tagged = tagged_scenarios(&resolved, subject_id, change_id)?;
    if !tagged.is_empty() {
        return Ok(UnadoptOutcome::Refused(
            format!(
                "scenario(s) {} owned by subject `{}` carry a `@flow:{}.*` tag; unadopting the change would make them fall back to an older approval",
                tagged.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", "),
                subject_id.as_str(),
                change_id.as_str()
            ),
            format!("remove or retag the `@flow:{}.*` tags on those scenarios first, then run `canon subject unadopt` again", change_id.as_str()),
        ));
    }
    let canon_yaml_path = resolve_canon_yaml(&resolved);
    let registry = registry_for(&canon_yaml_path, &[RecordKind::Change, RecordKind::Subject])
        .map_err(|e| e.to_string())?;
    let now = Utc::now();
    if on_subject {
        subject.change_ids.retain(|id| id != change_id);
        subject.envelope.at = now;
        if dry_run {
            registry.check_persist::<Subject>().map_err(|e| format!("failed to record the unadoption: {e}; nothing was written"))?;
        } else {
            registry
                .persist(&subject)
                .map_err(|e| format!("failed to record the unadoption: {e}; nothing was written"))?;
        }
    }
    if on_change {
        change.subject_ids.retain(|id| id != subject_id);
        change.envelope.at = now;
        change.envelope.schema = RecordKind::Change.schema_version();
        let written = if dry_run { registry.check_persist::<Change>() } else { registry.persist(&change).map(drop) };
        written.map_err(|e| {
            format!(
                "subject `{}` no longer lists change `{}`, but the change record could not be written: {e}; complete the unlink with `canon subject unadopt {} --change-id {}`",
                subject_id.as_str(),
                change_id.as_str(),
                subject_id.as_str(),
                change_id.as_str()
            )
        })?;
    }
    Ok(UnadoptOutcome::Done(subject, true))
}

/// The ids of the scenarios `subject_id` owns that carry a
/// `@flow:<change_id>.*` tag.
fn tagged_scenarios(resolved: &Path, subject_id: &SubjectId, change_id: &ChangeId) -> Result<Vec<String>, String> {
    let schemas = SchemaRegistry::load();
    let ctx = GateCtx::from_repo(resolved)
        .map_err(|e| e.to_string())
        .and_then(|ctx| GateContext::load(ctx, &schemas, Utc::now()).map_err(|e| e.to_string()))?;
    Ok(subject_scenarios(&ctx, subject_id)
        .into_iter()
        .filter(|s| crate::brief_check::flow_changes_of(resolved, &s.project_id, &s.scenario_id).contains(change_id))
        .map(|s| s.scenario_id.as_str().to_string())
        .collect())
}

/// `` `a` ``, `` `a` and `b` ``, `` `a`, `b` and `c` ``.
fn quoted_list(ids: &[SubjectId]) -> String {
    let quoted: Vec<String> = ids.iter().map(|id| format!("`{}`", id.as_str())).collect();
    match quoted.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => quoted.join(""),
    }
}

/// Look up, then write, an adoption: `canon subject adopt`'s whole
/// effect. Refuses (`Err` with the operator-facing message, no command
/// prefix) when either record is absent; see [`persist_adoption`] for
/// the write. `Ok` carries the subject, every subject the change now
/// belongs to ([`change_owners`]), and whether anything was written
/// (`false`: both sides already carried the link).
pub(crate) fn adopt_change(repo: &Path, change_id: &ChangeId, subject_id: &SubjectId, dry_run: bool) -> Result<(Subject, Vec<SubjectId>, bool), String> {
    let (subject, change) = lookup_adoption(repo, subject_id, change_id)?;
    let Some(subject) = subject else {
        return Err(format!("subject `{}` does not exist (author it with `canon subject new` first)", subject_id.as_str()));
    };
    let Some(change) = change else {
        return Err(format!("change `{}` does not exist (import it with `canon ingest plans` first)", change_id.as_str()));
    };
    let adopted = if dry_run { preview_adoption(repo, change, subject) } else { persist_adoption(repo, change, subject) };
    let (subject, change, wrote) =
        adopted.map_err(|e| e.describe(change_id, subject_id, &adopt_command(change_id, subject_id)))?;
    // The write is done; an owner list that cannot be read falls back to
    // the change's own side rather than reporting a written adoption as
    // refused.
    let owners = change_owners(repo, &change).unwrap_or_else(|_| change.subject_ids.clone());
    Ok((subject, owners, wrote))
}

/// Every subject `change` belongs to, joined from both sides (0.18 D4):
/// its own `subject_ids` in adoption order, then each latest subject
/// whose `change_ids` lists it, by subject id. The same join
/// [`canon_gate::spec_coverage::subject_change_ids`] reads from the
/// subject side.
pub(crate) fn change_owners(repo: &Path, change: &Change) -> Result<Vec<SubjectId>, String> {
    let repo = resolve_repo_root(repo);
    let registry = registry_for(&resolve_canon_yaml(&repo), &[RecordKind::Subject]).map_err(|e| e.to_string())?;
    let mut owners = change.subject_ids.clone();
    let mut listing: Vec<SubjectId> = latest_records(&registry, RecordKind::Subject)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|raw| serde_json::from_value::<Subject>(raw.0).ok())
        .filter(|s| s.change_ids.contains(&change.change_id) && !owners.contains(&s.subject_id))
        .map(|s| s.subject_id)
        .collect();
    listing.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    owners.extend(listing);
    Ok(owners)
}

/// The latest `subject_id` Subject and `change_id` Change (fold-latest),
/// each `None` when absent — read through the routed tiers the adopt
/// write uses.
pub(crate) fn lookup_adoption(repo: &Path, subject_id: &SubjectId, change_id: &ChangeId) -> Result<(Option<Subject>, Option<Change>), String> {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo);
    let registry = registry_for(&canon_yaml_path, &[RecordKind::Change, RecordKind::Subject]).map_err(|e| e.to_string())?;
    Ok((find_subject(&registry, subject_id)?, find_change(&registry, change_id)?))
}

/// `canon subject adopt <subject> --change-id <change>`: the command that
/// completes a half-written adoption. Safe to rerun: [`persist_adoption`]
/// writes only the side that still lacks the link, and nothing once both
/// carry it.
pub(crate) fn adopt_command(change_id: &ChangeId, subject_id: &SubjectId) -> String {
    format!("canon subject adopt {} --change-id {}", subject_id.as_str(), change_id.as_str())
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
/// change with the subject appended to `subject_ids` (design D3 —
/// stamped at adoption time, never derived in canon-model; 0.18 D4 —
/// appended, never re-pointed, so a change adopted by a second subject
/// belongs to both), each re-stamped with a fresh envelope `at` so it
/// deterministically supersedes the prior version in the query fold,
/// persisted through the routed tiers. The change is written at its
/// kind's current schema generation, since it is written in the current
/// format. Returns the subject, the change, and whether anything was
/// written.
///
/// # Two records, no transaction
/// The ledger is append-only and the two kinds may route to different
/// tiers, so the pair cannot be written atomically. Two rules make a
/// failure between the writes recoverable:
///
/// - **Subject first.** Readers join a subject's changes from both
///   sides ([`canon_gate::spec_coverage::subject_change_ids`]), and the
///   subject side is the one every tier layout keeps readable to the
///   gate. A subject that lists the change while the change record lacks
///   the link still has that change's open blockers counted, so a
///   half-done adoption errs toward enforcing.
/// - **Only what is missing.** A side that already carries the link is
///   not rewritten, and nothing is written once both do, so
///   [`adopt_command`] completes a half-written adoption and is a no-op
///   on a whole one.
pub(crate) fn persist_adoption(repo: &Path, mut change: Change, mut subject: Subject) -> Result<(Subject, Change, bool), AdoptionWriteError> {
    let needs_subject = !subject.change_ids.contains(&change.change_id);
    let needs_change = !change.subject_ids.contains(&subject.subject_id);
    if !needs_subject && !needs_change {
        return Ok((subject, change, false));
    }

    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo);
    let registry = registry_for(&canon_yaml_path, &[RecordKind::Change, RecordKind::Subject])
        .map_err(|e| AdoptionWriteError { subject_written: false, message: e.to_string() })?;

    let now = Utc::now();
    if needs_subject {
        subject.change_ids.push(change.change_id.clone());
        subject.envelope.at = now;
        registry.persist(&subject).map_err(|e| AdoptionWriteError { subject_written: false, message: e.to_string() })?;
    }
    if needs_change {
        change.subject_ids.push(subject.subject_id.clone());
        change.envelope.at = now;
        change.envelope.schema = RecordKind::Change.schema_version();
        registry.persist(&change).map_err(|e| AdoptionWriteError { subject_written: needs_subject, message: e.to_string() })?;
    }
    Ok((subject, change, true))
}

/// [`persist_adoption`] under `--dry-run`: the same decision and the same
/// tier refusals, the records as they would stand, nothing written.
fn preview_adoption(repo: &Path, mut change: Change, mut subject: Subject) -> Result<(Subject, Change, bool), AdoptionWriteError> {
    let needs_subject = !subject.change_ids.contains(&change.change_id);
    let needs_change = !change.subject_ids.contains(&subject.subject_id);
    if !needs_subject && !needs_change {
        return Ok((subject, change, false));
    }
    let repo = resolve_repo_root(repo);
    let registry = registry_for(&resolve_canon_yaml(&repo), &[RecordKind::Change, RecordKind::Subject])
        .map_err(|e| AdoptionWriteError { subject_written: false, message: e.to_string() })?;
    if needs_subject {
        registry.check_persist::<Subject>().map_err(|e| AdoptionWriteError { subject_written: false, message: e.to_string() })?;
        subject.change_ids.push(change.change_id.clone());
    }
    if needs_change {
        registry.check_persist::<Change>().map_err(|e| AdoptionWriteError { subject_written: false, message: e.to_string() })?;
        change.subject_ids.push(subject.subject_id.clone());
    }
    Ok((subject, change, true))
}

/// `canon subject move <id> --to <state>` (module doc): apply a lifecycle
/// transition. Refuses an off-chain step ([`is_valid_transition`], exit
/// `2`); for `verifying → shipped` additionally runs
/// [`ship_gate_violations`], the brief check ([`brief_violation`]) and
/// the unrefreshable-index check ([`crate::inventory::unrefreshable_roots`],
/// unwaivable),
/// and for a target `require_review.scope` covers, the review guard
/// (module doc). On any violation it prints each by failure class and
/// exits `1` with the record UNCHANGED (fail closed), unless every
/// violation is a review check or the brief check and `override_reason`
/// waives them. The reopen `shipped → verifying` (0.18 D3) needs
/// `reason` and the approved scope ([`reopen_violation`], unwaivable),
/// runs no review guard, and records [`Reopen`]. A successful transition
/// re-stamps the subject (fresh `at`) and persists it through the routed
/// tier.
#[allow(clippy::too_many_arguments)]
pub fn run_move(
    repo: &Path,
    subject_id: &SubjectId,
    target: SubjectStatus,
    override_reason: Option<&str>,
    reason: Option<&str>,
    actor_id: &str,
    json: bool,
) -> i32 {
    for (flag, text, recorded) in [
        ("--override-reason", override_reason, "the accountable reason"),
        ("--reason", reason, "the transition note, with the actor"),
    ] {
        if text.is_some_and(|text| text.trim().is_empty() || text.contains(['\n', '\r'])) {
            return CliError::refused("SUBJECT_INVALID_ARGUMENT", format!("{flag} must be one non-empty line; it is recorded on the subject as {recorded}")).hint("`canon schema subject move` lists the accepted arguments and their forms").emit("canon subject move", json);
        }
    }

    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo);
    let registry = match registry_for(&canon_yaml_path, &[RecordKind::Subject]) {
        Ok(r) => r,
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon subject move", json);
        }
    };

    let mut subject = match find_subject(&registry, subject_id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return CliError::refused("SUBJECT_NOT_FOUND", format!("subject `{}` does not exist", subject_id.as_str())).hint("`canon subject new <id> --domain <domain> --title <title>` creates it; check the id against the subjects `canon status` lists").emit("canon subject move", json);
        }
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon subject move", json);
        }
    };

    let current = subject.status;
    if !is_valid_transition(current, target) {
        return CliError::refused("SUBJECT_INVALID_TRANSITION", format!("invalid transition {} → {} (allowed: the chain proposed → specced → building → verifying → shipped, shipped → verifying to reopen with --reason, or any non-retired state → retired)",
            status_str(current),
            status_str(target)
        )).hint("follow the chain proposed → specced → building → verifying → shipped; `canon subject move <id>` shows where the subject stands").field("--to").expected(["proposed", "specced", "building", "verifying", "shipped", "retired"].into_iter().filter(|s| parse_status(s).is_ok_and(|to| is_valid_transition(current, to)))).emit("canon subject move", json);
    }
    let reopens = current == SubjectStatus::Shipped && target == SubjectStatus::Verifying;
    if reopens && reason.is_none() {
        return CliError::refused("SUBJECT_REASON_REQUIRED", format!("reopening {} (shipped → verifying) needs --reason <text>: why it reopens, recorded on the subject with the actor",
            subject_id.as_str()
        ))
        .field("--reason")
        .hint(format!("canon subject move {} --to verifying --reason \"<why it reopens>\" --as <role>", subject_id.as_str()))
        .emit("canon subject move", json);
    }

    let schemas = SchemaRegistry::load();
    let policy = PolicyResolution::resolve(&repo, &schemas);
    let ships = current == SubjectStatus::Verifying && target == SubjectStatus::Shipped;
    // A policy that no longer matches its approval fails closed (0.22 D3):
    // shipping or reopening would grade against bytes no human approved, and
    // `--override-reason` waives only review checks.
    if ships || reopens {
        if let Some((closed, detail)) = crate::policy_approval::closed_failure(&repo) {
            let error = match closed {
                crate::policy_approval::Closed::Unreadable => CliError::failed("POLICY_APPROVAL_UNREADABLE", detail).hint(crate::policy_approval::REAPPROVE_HINT),
                crate::policy_approval::Closed::Changed => CliError::failed("POLICY_CHANGED_SINCE_APPROVAL", detail).hint(crate::policy_approval::REAPPROVE_HINT),
            };
            return error.with_exit(EXIT_GATED).emit("canon subject move", json);
        }
    }
    // A reopen moves a subject back to fix its gaps, so the review guard
    // that gates moving forward into its scope does not run on it;
    // shipping again runs it.
    let reviews = !reopens
        && match &policy.spec_coverage {
            Some(SpecCoverage::Active { require_review: Some(rr), .. }) => rr.covers(target),
            _ => false,
        };
    let gate_context = if ships || reviews || reopens {
        match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &schemas, Utc::now()).map_err(|e| e.to_string())) {
            Ok(ctx) => {
                Some(ctx)
            }
            Err(e) => {
                return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon subject move", json);
            }
        }
    } else {
        None
    };
    // The subject's adopted changes, joined from both sides (0.18 D4).
    let changes = gate_context.as_ref().map_or_else(|| subject.change_ids.clone(), |ctx| subject_change_ids(ctx, &subject));
    // The subject-level brief rules read its scope: a pending draft (no
    // scenario tagged for it) does not freeze the maintenance loop.
    let adopted = changes.len();
    let (changes, pending_drafts) = match &gate_context {
        Some(ctx) if ships || reopens => crate::brief::scope_changes(&repo, ctx, &subject, &changes),
        _ => (changes, Vec::new()),
    };
    let pending_note: Vec<String> = (!pending_drafts.is_empty()).then(|| crate::brief::pending_draft_line(&pending_drafts)).into_iter().collect();

    let mut blocking = match (ships, &gate_context) {
        (true, Some(ctx)) => ship_gate_violations(ctx, subject_id),
        _ => Vec::new(),
    };
    // The same decision `canon gate check` fails on (0.16 M6): shipping on
    // an index the `.feature` edits are not in would ship ungraded specs.
    if ships {
        blocking.extend(crate::inventory::unrefreshable_roots(&repo).into_iter().map(|root| root.violation("verifying → shipped: ")));
        blocking.extend(brief_check_violation(&repo, subject_id, &changes));
        if let Some(ctx) = &gate_context {
            blocking.extend(crate::reverify::chore_violations(&repo, ctx, &subject));
        }
    }
    if reopens {
        blocking.extend(reopen_violation(&repo, &subject, &changes));
    }
    let mut waivable = Vec::new();
    report_review_guard(&policy, current, target, adopted);
    if let (true, Some(ctx)) = (reviews, &gate_context) {
        let (rr, exclude_lanes) = active_require_review(ctx).expect("`reviews` implies an active require_review");
        let (unwaivable, gaps, readable) = review_checks(ctx, rr, exclude_lanes, &subject);
        blocking.extend(unwaivable);
        waivable.extend(gaps);
        for violation in &mut waivable {
            violation.detail = format!("{} → {}: {}", status_str(current), status_str(target), violation.detail);
        }
        if readable {
            if let Some(note) = canon_gate::review_gate::unmigrated_review_note(ctx) {
                crate::warn_err!("canon subject move: advisory: {note}");
            }
        }
    }
    let brief_gap = if ships { brief_violation(&repo, &subject, &changes) } else { None };
    let review_gaps = !waivable.is_empty();
    waivable.extend(brief_gap.iter().cloned());

    if !blocking.is_empty() {
        let lines = blocking.iter().chain(&waivable).map(Violation::line).collect::<Vec<_>>().join("\n");
        let mut hints: Vec<String> = crate::reverify::hint(&repo, gate_context.as_ref(), &subject, &blocking.iter().chain(&waivable).collect::<Vec<_>>()).into_iter().collect();
        if let Some(ctx) = &gate_context {
            let divergent = divergent_cells(ctx, subject_id, &blocking);
            if !divergent.is_empty() {
                hints.push(canon_gate::acceptance::hint(&divergent));
            }
        }
        if override_reason.is_some() {
            hints.push("--override-reason waives only the review checks (unreviewed-promotion, open-blocker); the violations above that are not review checks still refuse this transition".to_string());
        }
        if hints.is_empty() {
            hints.push("fix each violation above (`canon gate check` lists them with their causes), then run this transition again".to_string());
        }
        hints.extend(pending_note.iter().cloned());
        return CliError::failed("SUBJECT_GATE_VIOLATION", lines).with_exit(EXIT_GATED).hint(hints.join("; ")).emit("canon subject move", json);
    }

    // A waiver, a reopen note, and a transition reason belong to the
    // transition that needed them: every write starts without any, so
    // none carries forward.
    subject.status_override = None;
    subject.reopen = None;
    subject.status_reason = None;
    if !waivable.is_empty() {
        let Some(reason) = override_reason else {
            let lines = waivable.iter().map(Violation::line).collect::<Vec<_>>().join("\n");
            let mut fixes: Vec<String> = Vec::new();
            if review_gaps && !waivable.iter().all(canon_gate::review_gate::is_reverify) {
                fixes.push("record an independent review (`canon review scenario`) and close blocker findings".to_string());
            }
            if brief_gap.is_some() {
                fixes.push("get the human's answer to the brief; the human records it with `canon brief approve` in their own shell".to_string());
            }
            let reverify: Vec<&Violation> = waivable.iter().filter(|v| canon_gate::review_gate::is_reverify(v)).collect();
            if reverify.iter().any(|v| !canon_gate::review_gate::is_scoped_reverify(v)) {
                let commands = gate_context.as_ref().map(|ctx| crate::reverify::batch_commands_of(&repo, ctx, &subject)).unwrap_or_default();
                fixes.push(match commands.as_slice() {
                    [] => "record the suite again with `canon evidence add --all-scenarios` and a --report for each test report of the suite (a review stays current while the verdicts and checks are the same)".to_string(),
                    many => format!("record the suite again for the scenarios named with {} (a review stays current while the verdicts and checks are the same)", many.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join("; ")),
                });
            } else if !reverify.is_empty() {
                fixes.push("record evidence again for the scenarios named with `canon evidence add --project-id <root> --scenario-id <id> --report <junit|cucumber|swift-testing>:<path> --role implementer`, one each (a review stays current while the verdicts and checks are the same)".to_string());
            }
            return CliError::failed("SUBJECT_GATE_VIOLATION", lines)
                .with_exit(EXIT_GATED)
                .hint(format!(
                    "{}, or pass --override-reason <text> to move anyway; the waiver is recorded on the subject{}{}",
                    fixes.join("; "),
                    if review_gaps { " and `canon gate check` keeps listing these gaps" } else { "" },
                    pending_note.iter().map(|note| format!("; {note}")).collect::<String>()
                ))
                .emit("canon subject move", json);
        };
        for v in &waivable {
            crate::warn_err!("canon subject move: waived {}", v.line());
        }
        let waived: std::collections::BTreeSet<WaivedViolation> =
            waivable.iter().map(|v| WaivedViolation { class: v.class.as_str().to_string(), subject: v.subject.clone() }).collect();
        let count = waived.len();
        subject.status_override = Some(StatusOverride { to: target, reason: reason.to_string(), waived: waived.into_iter().collect(), actor: crate::role::attributed(actor_id, None) });
        crate::warn_err!(
            "canon subject move: override recorded by `{actor_id}` for the {count} violation(s) above: {reason}; a gap that appears later is not covered"
        );
    } else if override_reason.is_some() {
        crate::warn_err!("canon subject move: --override-reason not recorded — no review check refused this transition");
    }
    if let (true, Some(reason)) = (reopens, reason) {
        let change = changes.last().cloned();
        let into = match &change {
            Some(change) => format!("keep the follow-up in its approved change `{}`", change.as_str()),
            None => "keep the follow-up in its change".to_string(),
        };
        crate::warn_err!(
            "canon subject move: reopened `{}` from shipped, recorded by `{actor_id}`: {reason}; {into}, then scenario, evidence and review; shipping again runs every guard",
            subject_id.as_str()
        );
        subject.reopen = Some(Reopen { reason: reason.to_string(), change, actor: crate::role::attributed(actor_id, None) });
    }
    if let Some(reason) = reason {
        subject.status_reason = Some(StatusReason { reason: reason.to_string(), actor: crate::role::attributed(actor_id, None) });
    }

    subject.status = target;
    subject.envelope.at = Utc::now();

    match registry.persist(&subject) {
        Ok(_) => {
            report_subject_warning(&repo, &subject, "move", false, json, &format!("canon subject move: {} → {} — {}", subject_id.as_str(), status_str(target), crate::write_mode::DIRECT), &pending_note);
            0
        }
        Err(e) => {
            CliError::refused("WRITE_FAILED", e.to_string()).hint("check that the path is writable and the disk has room, then run the command again").emit("canon subject move", json)
        }
    }
}

/// `canon subject show <id>` (0.21 D1): show the subject and write
/// nothing. `--override-reason` and `--reason` belong to `canon subject move`.
pub fn run_show(repo: &Path, subject_id: &SubjectId, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo);
    let registry = match registry_for(&canon_yaml_path, &[RecordKind::Subject]) {
        Ok(r) => r,
        Err(e) => return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon subject show", json),
    };
    let subject = match find_subject(&registry, subject_id) {
        Ok(Some(s)) => s,
        Ok(None) => return CliError::refused("SUBJECT_NOT_FOUND", format!("subject `{}` does not exist", subject_id.as_str())).hint("`canon subject new <id> --domain <domain> --title <title>` creates it; check the id against the subjects `canon status` lists").emit("canon subject show", json),
        Err(e) => return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon subject show", json),
    };
    if json {
        crate::json_out::data("subject show", &SubjectReport(&subject));
    } else {
        println!("canon subject show: {} ({}, {}) — {}", subject.subject_id.as_str(), subject.domain, status_str(subject.status), subject.title);
        if !subject.summary.is_empty() {
            println!("  summary: {}", subject.summary);
        }
        println!("  owner: {}", subject.owner_role.as_str());
        if !subject.change_ids.is_empty() {
            println!("  changes: {}", subject.change_ids.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", "));
        }
        if let Some(reopen) = &subject.reopen {
            println!("  reopened: {}", reopen.reason);
        }
        if let Some(note) = &subject.status_reason {
            println!("  reason: {}", note.reason);
        }
    }
    0
}

/// Print, on stderr, which review checks this transition runs and which
/// it skips. Silent when `policy.yaml` has no `require_review`, so a repo
/// that never opted in sees exactly the pre-0.12 output.
fn report_review_guard(policy: &PolicyResolution, from: SubjectStatus, to: SubjectStatus, changes: usize) {
    let rr: &RequireReview = match &policy.spec_coverage {
        Some(SpecCoverage::Active { require_review: Some(rr), .. }) => rr,
        Some(SpecCoverage::Invalid { .. }) => {
            crate::warn_err!("canon subject move: review guard skipped — policy.yaml's `spec_coverage` section is invalid, so whether review is required cannot be read; `canon gate check` reports why");
            return;
        }
        _ => return,
    };
    let scope = if rr.scope.is_empty() { "every status".to_string() } else { rr.scope.iter().map(|s| status_str(*s)).collect::<Vec<_>>().join(", ") };
    crate::human_err!("canon subject move: review guard for {} → {} (spec_coverage.require_review, scope: {scope})", status_str(from), status_str(to));
    if from == SubjectStatus::Shipped && to == SubjectStatus::Verifying {
        crate::human_err!("canon subject move:   skipped unreviewed-promotion — a reopen moves the subject back to fix its gaps; shipping again runs it");
        crate::human_err!("canon subject move:   skipped open-blocker — a reopen moves the subject back to fix its gaps; shipping again runs it");
        return;
    }
    if !rr.covers(to) {
        crate::human_err!("canon subject move:   skipped unreviewed-promotion — `{}` is not in require_review.scope", status_str(to));
        crate::human_err!("canon subject move:   skipped open-blocker — `{}` is not in require_review.scope", status_str(to));
        return;
    }
    let rule = if rr.distinct_actor {
        "a signed review, current with the spec and evidence it pinned, by an actor, and from a session, other than its evidence's"
    } else {
        "a signed review, current with the spec and evidence it pinned"
    };
    crate::human_err!("canon subject move:   ran unreviewed-promotion — every owned scenario needs {rule}");
    if rr.block_on_findings {
        crate::human_err!("canon subject move:   ran open-blocker — {changes} adopted change(s) checked for open blocker findings");
    } else {
        crate::human_err!("canon subject move:   skipped open-blocker — require_review.block_on_findings is false");
    }
}

/// The `require_review` checks a transition into its scope runs for
/// `subject`: `(unwaivable, waivable, readable)`. When a review corpus
/// kind is unreadable, the one unwaivable [`unreadable_violation`] and
/// `readable == false`; otherwise every malformed finding (unwaivable:
/// it might be an open blocker) and [`subject_guard`]'s review and
/// open-blocker gaps (waivable).
fn review_checks(ctx: &GateContext, rr: &RequireReview, exclude_lanes: &[String], subject: &Subject) -> (Vec<Violation>, Vec<Violation>, bool) {
    let unreadable = unreadable_review_kinds(ctx, rr);
    if !unreadable.is_empty() {
        return (vec![unreadable_violation(&unreadable)], Vec::new(), false);
    }
    (malformed_findings(ctx, rr), subject_guard(ctx, rr, exclude_lanes, subject), true)
}

/// What the `verifying → shipped` guard would refuse `subject` on, from
/// `ctx` alone: [`ship_gate_violations`] (ownership, verdicts, cases,
/// audits, an unreadable scenario kind) and, when `require_review`
/// covers `shipped`, [`review_checks`], less any review gap the
/// subject's current waiver recorded (as `canon gate check` reports it
/// as an advisory). The same functions [`run_move`] runs, so `canon
/// status`'s red flag on a shipped subject (0.18 D3) and the ship guard
/// cannot disagree. The brief and the unrefreshable-index check are not
/// here: status reports both on their own.
pub(crate) fn shipped_gate_gaps(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<Violation> {
    let mut gaps = ship_gate_violations(ctx, &subject.subject_id);
    gaps.extend(crate::reverify::chore_violations(repo, ctx, subject));
    if let Some((rr, exclude_lanes)) = active_require_review(ctx).filter(|(rr, _)| rr.covers(SubjectStatus::Shipped)) {
        let (unwaivable, waivable, _) = review_checks(ctx, rr, exclude_lanes, subject);
        let waiver = subject.status_override.as_ref().filter(|waiver| waiver.to == subject.status);
        gaps.extend(unwaivable);
        gaps.extend(waivable.into_iter().filter(|v| !waiver.is_some_and(|waiver| waiver.covers(v.class.as_str(), &v.subject))));
    }
    gaps
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
///   ship on a spec that only describes its golden path;
/// - an owned scenario's latest audit is `refuted` — `false-success`
///   (0.15 D3), whatever its declared verdict — or, with
///   `spec_coverage.require_audit` covering `shipped`, an owned scenario
///   in the audit sample still owes an audit (`uncovered-cell`), through
///   [`canon_gate::audit::subject_audit_gaps`], the rule `canon gate
///   check` applies.
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
        } else if let Some(divergent) = entries.iter().find(|e| e.verdict == EvidenceVerdict::Divergent).filter(|_| !canon_gate::acceptance::is_accepted(gate_context, scenario)) {
            violations.push(Violation::new(
                FailureClass::UncoveredCell,
                sid.clone(),
                format!("verifying → shipped: {}", canon_gate::acceptance::divergent_detail(gate_context, scenario, &divergent.agent_id)),
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
    violations.extend(canon_gate::audit::subject_audit_gaps(gate_context, subject_id, SubjectStatus::Shipped).into_iter().map(|mut violation| {
        violation.detail = format!("verifying → shipped: {}", violation.detail);
        violation
    }));
    violations
}

/// The `(project_id, scenario_id)` of each divergent-verdict refusal among
/// `blocking` ([`canon_gate::acceptance::DIVERGENT_DETAIL`]), for the hint
/// that names the way out.
fn divergent_cells(ctx: &GateContext, subject_id: &SubjectId, blocking: &[Violation]) -> Vec<(String, String)> {
    let owned = subject_scenarios(ctx, subject_id);
    blocking
        .iter()
        .filter(|v| v.class == FailureClass::UncoveredCell && v.detail.contains(canon_gate::acceptance::DIVERGENT_DETAIL))
        .filter_map(|v| owned.iter().find(|s| s.scenario_id.as_str() == v.subject))
        .map(|s| (s.project_id.as_str().to_string(), s.scenario_id.as_str().to_string()))
        .collect()
}

/// The `verifying → shipped` brief-structure check (0.19 D2): the blocker
/// rules of `canon brief check` over the subject's newest adopted change's
/// brief, as one `unreviewed-promotion` naming each finding. Not waivable
/// by `--override-reason`: the brief waives its own findings, one rule in
/// one section (`canon:waive`). `None` when there is no such brief, it is a
/// pre-0.19 brief, or nothing blocks.
fn brief_check_violation(repo: &Path, subject_id: &SubjectId, changes: &[ChangeId]) -> Option<Violation> {
    let findings = crate::brief_check::guard_blockers(repo, changes.last()?);
    if findings.is_empty() {
        return None;
    }
    let listed = findings
        .iter()
        .map(|f| format!("{} [{}] line {}", f.rule, f.section.as_deref().unwrap_or("-"), f.line))
        .collect::<Vec<_>>()
        .join("; ");
    Some(Violation::new(
        FailureClass::UnreviewedPromotion,
        subject_id.as_str(),
        format!("verifying → shipped: the brief has {} blocker finding(s): {listed}; `canon brief check` shows them", findings.len()),
    ))
}

/// The `verifying → shipped` brief check (module doc): an
/// `unreviewed-promotion` naming the subject when [`crate::brief::brief_of`],
/// the decision `canon status` reports, reads its brief as enforced and
/// not approved. An unapproved brief that is not enforced (nothing to
/// answer) prints an advisory instead. `None` when the brief is
/// approved, not enforced, or the subject needs none. `changes` is the
/// subject's adopted changes, joined from both sides.
fn brief_violation(repo: &Path, subject: &Subject, changes: &[ChangeId]) -> Option<Violation> {
    brief_gap(repo, subject, changes, "verifying → shipped", "shipping")
}

/// The reopen check (0.18 D3): `shipped → verifying` needs no human
/// inside the approved scope, the subject's newest adopted change whose
/// brief is approved, or when there is no brief to answer. Otherwise it
/// refuses with the same `unreviewed-promotion` the ship guard prints,
/// naming the brief to approve; `--override-reason` does not waive it.
fn reopen_violation(repo: &Path, subject: &Subject, changes: &[ChangeId]) -> Option<Violation> {
    brief_gap(repo, subject, changes, "shipped → verifying", "reopening").map(|mut violation| {
        violation.detail = violation.detail.replacen("brief not approved", "reopening needs the approved scope, and the brief is not approved", 1);
        violation
    })
}

/// The brief check of `transition` (`verb` is how its advisory names
/// it): [`brief_violation`] and [`reopen_violation`].
fn brief_gap(repo: &Path, subject: &Subject, changes: &[ChangeId], transition: &str, verb: &str) -> Option<Violation> {
    let (roots, unreadable) = match crate::brief::plan_roots(repo) {
        Ok(roots) => (roots, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let brief = crate::brief::brief_of(repo, &roots, subject, changes).filter(|brief| !brief.approved)?;
    let id = subject.subject_id.as_str();
    if !brief.enforced {
        let why = match (unreadable, &brief.change) {
            (Some(e), Some(change)) => format!("its newest adopted change `{change}` has no brief, and canon.yaml's `plans:` section cannot be read ({e}), so no external proposal.md is found"),
            (Some(e), None) => format!("canon.yaml's `plans:` section cannot be read ({e}), and {id} adopts no change"),
            (None, Some(change)) => format!("its newest adopted change `{change}` has no brief (no {}/{change}/brief.md, no openspec proposal.md)", canon_model::paths::CHANGES_DIR),
            (None, None) => format!("{id} adopts no change"),
        };
        crate::warn_err!("canon subject move: advisory: {id}'s brief is not approved — {why}, so there is no brief to answer and {verb} is not refused");
        return None;
    }
    Some(Violation::new(
        FailureClass::UnreviewedPromotion,
        id,
        format!(
            "{transition}: brief not approved — {}; a human must answer the brief; {}",
            brief.unanswered(),
            brief.record()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forward_chain_and_the_reopen_are_the_only_paths() {
        use SubjectStatus::*;
        assert!(is_valid_transition(Proposed, Specced));
        assert!(is_valid_transition(Specced, Building));
        assert!(is_valid_transition(Building, Verifying));
        assert!(is_valid_transition(Verifying, Shipped));
        // The one backward step: reopening a shipped subject (0.18 D3).
        assert!(is_valid_transition(Shipped, Verifying));
        // Skips, other backward steps, and self-loops are all invalid.
        assert!(!is_valid_transition(Proposed, Building));
        assert!(!is_valid_transition(Building, Proposed));
        assert!(!is_valid_transition(Building, Building));
        assert!(!is_valid_transition(Shipped, Building));
        assert!(!is_valid_transition(Verifying, Building));
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
