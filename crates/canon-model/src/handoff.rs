//! `Handoff`: models the donor handoff queue's Postgres table state
//! machine (S1 design D4) plus the per-domain body template registry
//! (D5, tasks group 5).
//!
//! The 13 non-body fields below (plus the closed 4-state enum) are a
//! fixed struct that mirrors the *matching* handoff-queue columns
//! column-for-column by name and shape — that is the state-machine core
//! this type exists to guarantee, and a canon-written row and a
//! donor-CLI-written row agree exactly on those fields. This is NOT a
//! lossless, full-row mapping onto the live table: the donor's `trigger`
//! column is `NOT NULL` with no default and has no `Handoff` field (a
//! donor `INSERT` needs a value this type cannot supply), the donor's
//! `created_at`/`created_by_session_id`/`created_by_branch`/
//! `created_by_worktree`/`created_by_host`/`refs_extra` columns have no
//! `Handoff` field (reading a real donor row drops them), and canon's own
//! envelope (`schema`/`kind`/`at`/`actor`) has no donor column at all.
//! Whichever change actually reads/writes the donor's live table (S4,
//! artifact/handoff ingest) owns bridging that gap — see this change's
//! proposal.md for the forward note. The body (`HandoffBody`) is
//! deliberately NOT fixed: a per-domain template registry, gated by the
//! active repo's `canon.yaml` (`handoff_templates:`), validates and
//! renders it.

use std::borrow::Cow;
use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::{JsonSchema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::envelope::{CanonRecord, Envelope, RecordKind};
use crate::evidence::{EvidenceViolation, FailureClass};
use crate::ids::{ChangeId, HandoffId};

/// `handoffs.state` — `text`, runtime-checked in the donor to exactly
/// these four values (donor `handoffs` table doc comment).
/// `Done`/`Abandoned` are terminal: [`HandoffState::can_transition_to`]
/// never allows a transition back out of either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HandoffState {
    Pending,
    InProgress,
    Done,
    Abandoned,
}

impl HandoffState {
    /// The exact transition set the donor CLI's own CAS SQL enforces:
    /// `claim`
    /// (pending→in-progress), `complete` (in-progress→done), `abandon`
    /// (in-progress→abandoned). Every other pair — including both
    /// terminal states' only possible source transitions back to
    /// `pending`/`in-progress` — is rejected.
    pub fn can_transition_to(self, next: HandoffState) -> bool {
        matches!(
            (self, next),
            (HandoffState::Pending, HandoffState::InProgress)
                | (HandoffState::InProgress, HandoffState::Done)
                | (HandoffState::InProgress, HandoffState::Abandoned)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, HandoffState::Done | HandoffState::Abandoned)
    }
}

/// A handoff body's domain (기획/디자인/개발/테스트/…) — the
/// per-domain template registry key. Validated only for non-emptiness;
/// domain vocabulary itself becomes typed once S10 lands (design D5).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DomainId(String);

impl DomainId {
    pub fn parse(s: impl Into<String>) -> Result<Self, &'static str> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err("domain id must be non-empty");
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DomainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl JsonSchema for DomainId {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("DomainId")
    }
    fn json_schema(_generator: &mut SchemaGenerator) -> schemars::Schema {
        json_schema!({ "type": "string", "minLength": 1 })
    }
}

/// The handoff body: a domain-scoped, template-validated payload
/// distinct from the fixed state-machine fields (design D4). Not itself
/// a `handoffs.ts` column — the registered domain's
/// [`HandoffTemplate::render`] produces the table's `body_text` column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HandoffBody {
    pub domain: DomainId,
    pub template_version: u32,
    pub fields: serde_json::Value,
}

/// A per-domain Handoff body template (design D5): validates and
/// renders `HandoffBody.fields`. Implementations are compiled Rust
/// registered into a [`TemplateRegistry`], gated per-repo by
/// `canon.yaml`'s `handoff_templates:` list.
pub trait HandoffTemplate: Send + Sync {
    fn domain(&self) -> DomainId;
    fn validate(&self, fields: &serde_json::Value) -> Result<(), Vec<EvidenceViolation>>;
    fn render(&self, fields: &serde_json::Value) -> String;
}

/// 기획 (planning) — the simplest domain, S1's registry-contract proof
/// (design D5, task 5.6): `title` + `summary` + `acceptance-criteria`
/// (a non-empty array), all required.
pub struct GihoekTemplate;

const GIHOEK_REQUIRED_STRING_FIELDS: [&str; 2] = ["title", "summary"];
const GIHOEK_ACCEPTANCE_FIELD: &str = "acceptance-criteria";

impl HandoffTemplate for GihoekTemplate {
    fn domain(&self) -> DomainId {
        DomainId::parse("기획").expect("literal domain id is non-empty")
    }

    fn validate(&self, fields: &serde_json::Value) -> Result<(), Vec<EvidenceViolation>> {
        let mut violations = Vec::new();
        let obj = fields.as_object();

        for field in GIHOEK_REQUIRED_STRING_FIELDS {
            let ok = obj.and_then(|o| o.get(field)).and_then(|v| v.as_str()).is_some_and(|s| !s.trim().is_empty());
            if !ok {
                violations.push(EvidenceViolation::new(
                    FailureClass::InvalidHandoffBody,
                    field,
                    format!("기획 template requires a non-empty string `{field}`"),
                ));
            }
        }

        let acceptance_ok = obj
            .and_then(|o| o.get(GIHOEK_ACCEPTANCE_FIELD))
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty());
        if !acceptance_ok {
            violations.push(EvidenceViolation::new(
                FailureClass::InvalidHandoffBody,
                GIHOEK_ACCEPTANCE_FIELD,
                "기획 template requires a non-empty `acceptance-criteria` array",
            ));
        }

        if violations.is_empty() { Ok(()) } else { Err(violations) }
    }

    fn render(&self, fields: &serde_json::Value) -> String {
        let title = fields.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let summary = fields.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        let criteria = fields
            .get(GIHOEK_ACCEPTANCE_FIELD)
            .and_then(|v| v.as_array())
            .map(|items| items.iter().filter_map(|v| v.as_str()).map(|s| format!("- {s}")).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();
        format!("# {title}\n\n{summary}\n\n## Acceptance Criteria\n\n{criteria}\n")
    }
}

/// `canon.yaml`'s `handoff_templates:` section — a flat list of domains
/// this repo activates. S1 owns only this narrow slice of `canon.yaml`;
/// the file's other sections (S2's `TierPolicy`, …) are out of scope
/// here.
#[derive(Debug, Clone, Default, Deserialize)]
struct HandoffTemplatesManifest {
    #[serde(default)]
    handoff_templates: Vec<String>,
}

/// Resolves `HandoffBody.domain` to its [`HandoffTemplate`] impl, or
/// reports an unregistered-domain violation (design D5, task 5.5).
/// Construct via [`TemplateRegistry::from_manifest`] (the `canon.yaml`-
/// gated path) or [`TemplateRegistry::new`] + [`TemplateRegistry::register`]
/// directly (tests, or a caller that has already resolved its own
/// domain list).
#[derive(Default)]
pub struct TemplateRegistry {
    templates: BTreeMap<DomainId, Box<dyn HandoffTemplate>>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self { templates: BTreeMap::new() }
    }

    pub fn register(&mut self, template: Box<dyn HandoffTemplate>) -> &mut Self {
        self.templates.insert(template.domain(), template);
        self
    }

    /// Parse `canon.yaml`'s `handoff_templates:` list and register only
    /// the `available` templates whose domain appears in it — a
    /// template compiled into the binary but not listed in this repo's
    /// `canon.yaml` is treated as unregistered here (design D5's
    /// "registered for a given consumer repo").
    pub fn from_manifest(canon_yaml: &str, available: Vec<Box<dyn HandoffTemplate>>) -> Result<Self, serde_yaml::Error> {
        let manifest: HandoffTemplatesManifest = serde_yaml::from_str(canon_yaml)?;
        let active: std::collections::HashSet<String> = manifest.handoff_templates.into_iter().collect();
        let mut registry = Self::new();
        for template in available {
            if active.contains(template.domain().as_str()) {
                registry.register(template);
            }
        }
        Ok(registry)
    }

    pub fn is_registered(&self, domain: &DomainId) -> bool {
        self.templates.contains_key(domain)
    }

    /// Validate a body against its domain's template. An unregistered
    /// domain and a registered-but-invalid body both return a
    /// structured [`EvidenceViolation`] — never a silent accept.
    pub fn validate_body(&self, body: &HandoffBody) -> Result<(), EvidenceViolation> {
        let Some(template) = self.templates.get(&body.domain) else {
            return Err(EvidenceViolation::new(
                FailureClass::UnregisteredHandoffDomain,
                body.domain.as_str(),
                "domain not registered in this repo's canon.yaml handoff_templates",
            ));
        };
        template.validate(&body.fields).map_err(|violations| {
            violations
                .into_iter()
                .next()
                .unwrap_or_else(|| EvidenceViolation::new(FailureClass::InvalidHandoffBody, body.domain.as_str(), "invalid body"))
        })
    }

    /// Every violation a registered domain's template reports (unlike
    /// [`Self::validate_body`], which surfaces only the first) — used
    /// where a caller wants the complete missing/invalid-field set.
    pub fn validate_body_all(&self, body: &HandoffBody) -> Result<(), Vec<EvidenceViolation>> {
        let Some(template) = self.templates.get(&body.domain) else {
            return Err(vec![EvidenceViolation::new(
                FailureClass::UnregisteredHandoffDomain,
                body.domain.as_str(),
                "domain not registered in this repo's canon.yaml handoff_templates",
            )]);
        };
        template.validate(&body.fields)
    }

    pub fn render_body(&self, body: &HandoffBody) -> Option<String> {
        self.templates.get(&body.domain).map(|t| t.render(&body.fields))
    }
}

/// The `handoffs` state machine (S1 design D4): each field below except
/// `envelope` and `body` maps 1:1 to a same-named column of the donor
/// handoff queue's `handoffs` Postgres table — the 13 fields the
/// handoff-state-machine spec fixes, on which a canon-written row and a
/// donor-CLI-written row agree exactly. This is
/// state-machine-core compatibility, not a full-row mapping onto the
/// live table: the donor's `trigger` column (`NOT NULL`, no default) has no
/// analog here, the donor's `created_at`/`created_by_*`/`refs_extra` columns
/// have no `Handoff` field, and `envelope` (`schema`/`kind`/`at`/`actor`)
/// is canon-only with no donor column — see the module doc comment for
/// the full gap and S4's ownership of bridging it. `body` is likewise
/// not a column itself — canon's own typed staging area for the table's
/// `body_text` column, rendered via [`TemplateRegistry::render_body`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Handoff {
    #[serde(flatten)]
    pub envelope: Envelope,

    pub id: HandoffId,
    pub state: HandoffState,
    pub chain_id: Uuid,
    #[serde(default)]
    pub parent_handoff_id: Option<HandoffId>,
    pub seq: i32,
    #[serde(default)]
    pub claimed_by: Option<String>,
    #[serde(default)]
    pub claimed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub abandoned_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub openspec_change_slug: Option<ChangeId>,
    #[serde(default)]
    pub research_vendor_slug: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub title: String,

    pub body: HandoffBody,
}

impl Handoff {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope: Envelope,
        id: HandoffId,
        chain_id: Uuid,
        parent_handoff_id: Option<HandoffId>,
        seq: i32,
        title: impl Into<String>,
        openspec_change_slug: Option<ChangeId>,
        body: HandoffBody,
    ) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Handoff);
        Self {
            envelope,
            id,
            state: HandoffState::Pending,
            chain_id,
            parent_handoff_id,
            seq,
            claimed_by: None,
            claimed_at: None,
            completed_at: None,
            abandoned_at: None,
            openspec_change_slug,
            research_vendor_slug: None,
            tags: Vec::new(),
            title: title.into(),
            body,
        }
    }

    /// Apply a state transition, stamping the matching timestamp column
    /// (`claimed_at`/`completed_at`/`abandoned_at`) on success.
    /// Rejects any transition [`HandoffState::can_transition_to`]
    /// disallows — in particular, `done`/`abandoned` → anything
    /// (handoff-state-machine spec, "An invalid state transition is
    /// rejected").
    pub fn transition_to(&mut self, next: HandoffState, at: DateTime<Utc>, claimed_by: Option<&str>) -> Result<(), EvidenceViolation> {
        if !self.state.can_transition_to(next) {
            return Err(EvidenceViolation::new(
                FailureClass::InvalidStateTransition,
                self.id.as_str(),
                format!("cannot transition {:?} -> {:?}", self.state, next),
            ));
        }
        match next {
            HandoffState::InProgress => {
                self.claimed_at = Some(at);
                self.claimed_by = claimed_by.map(str::to_string).or(self.claimed_by.take());
            }
            HandoffState::Done => self.completed_at = Some(at),
            HandoffState::Abandoned => self.abandoned_at = Some(at),
            HandoffState::Pending => {}
        }
        self.state = next;
        Ok(())
    }
}

impl CanonRecord for Handoff {
    const KIND: RecordKind = RecordKind::Handoff;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Actor;
    use crate::ids::RoleId;

    fn sample_envelope() -> Envelope {
        Envelope::new(1, RecordKind::Handoff, Utc::now(), Actor::new("codex-cli", RoleId::parse("implementer").unwrap()))
    }

    fn sample_body(fields: serde_json::Value) -> HandoffBody {
        HandoffBody { domain: DomainId::parse("기획").unwrap(), template_version: 1, fields }
    }

    fn valid_gihoek_fields() -> serde_json::Value {
        serde_json::json!({
            "title": "Ship S1",
            "summary": "join spine + envelope",
            "acceptance-criteria": ["round trips", "schemas emitted"],
        })
    }

    fn registry_with_gihoek() -> TemplateRegistry {
        let yaml = "handoff_templates:\n  - 기획\n";
        TemplateRegistry::from_manifest(yaml, vec![Box::new(GihoekTemplate)]).unwrap()
    }

    #[test]
    fn handoff_round_trips_and_carries_full_envelope() {
        let handoff = Handoff::new(
            sample_envelope(),
            HandoffId::parse("20260710-1432-fix-the-thing-a1b2").unwrap(),
            Uuid::new_v4(),
            None,
            1,
            "topic",
            Some(ChangeId::parse("s1-state-model-join-spine").unwrap()),
            sample_body(valid_gihoek_fields()),
        );
        let json = serde_json::to_value(&handoff).unwrap();
        for column in ["id", "state", "chain_id", "parent_handoff_id", "seq", "claimed_by", "openspec_change_slug", "tags", "title"] {
            assert!(json.get(column).is_some(), "missing column {column}");
        }
        assert_eq!(json.get("state").unwrap(), "pending");
        let round_tripped: Handoff = serde_json::from_value(json).unwrap();
        assert_eq!(handoff, round_tripped);
    }

    /// Task 5.2's mapping test: every `Handoff` field maps to a donor
    /// `handoffs` column of a compatible name/type, read directly from
    /// the donor table's own column list (the 13 fixed fields) as the
    /// fixture source of truth — no live database connection. `state`'s
    /// four wire values are asserted separately
    /// (`state_only_takes_the_four_wire_values`).
    #[test]
    fn every_field_maps_to_a_handoffs_ts_column() {
        let schema = serde_json::to_value(schemars::schema_for!(Handoff)).unwrap();
        let properties = schema.pointer("/properties").and_then(|v| v.as_object()).expect("schema has properties");
        let required: std::collections::HashSet<&str> =
            schema.pointer("/required").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_str()).collect();

        // (column, drizzle type, notNull-with-no-default) — from handoffs.ts's
        // own `pgTable` definition. `tags` is `.notNull()` but carries a
        // `.default(...)`, so canon's own required/omittable split treats
        // it as constructor-optional the same way the DB does at insert time.
        let columns: &[(&str, &str, bool)] = &[
            ("id", "text", true),
            ("state", "text", true),
            ("chain_id", "uuid", true),
            ("parent_handoff_id", "text", false),
            ("seq", "integer", true),
            ("claimed_by", "text", false),
            ("claimed_at", "timestamp", false),
            ("completed_at", "timestamp", false),
            ("abandoned_at", "timestamp", false),
            ("openspec_change_slug", "text", false),
            ("research_vendor_slug", "text", false),
            ("tags", "text[]", false),
            ("title", "text", true),
        ];

        for (column, drizzle_type, not_null_no_default) in columns {
            assert!(properties.contains_key(*column), "Handoff has no field mapping to handoffs.ts column `{column}`");
            assert_eq!(
                required.contains(column),
                *not_null_no_default,
                "Handoff.{column}'s required-ness disagrees with handoffs.ts's `{drizzle_type}` column (notNull-without-default = {not_null_no_default})"
            );
        }

        // `body`/`body_text`: not a 1:1 column (design D4) — `body` is
        // canon's typed staging area, rendered to `body_text` via the
        // domain template, so it is deliberately excluded from `columns`
        // above and asserted separately here.
        assert!(properties.contains_key("body"));
    }

    /// The registry actually reads THIS repo's own `canon.yaml`
    /// (design D5's "referenced from `canon.yaml`"), not just an
    /// inline test fixture string.
    #[test]
    fn registers_gihoek_from_the_repos_own_canon_yaml() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let canon_yaml = std::fs::read_to_string(repo_root.join("canon.yaml")).expect("repo root canon.yaml");
        let registry = TemplateRegistry::from_manifest(&canon_yaml, vec![Box::new(GihoekTemplate)]).unwrap();
        assert!(registry.is_registered(&DomainId::parse("기획").unwrap()));
    }

    #[test]
    fn state_only_takes_the_four_wire_values() {
        for (state, wire) in [
            (HandoffState::Pending, "pending"),
            (HandoffState::InProgress, "in-progress"),
            (HandoffState::Done, "done"),
            (HandoffState::Abandoned, "abandoned"),
        ] {
            assert_eq!(serde_json::to_string(&state).unwrap(), format!("\"{wire}\""));
        }
    }

    #[test]
    fn valid_transitions_succeed_and_stamp_timestamps() {
        let mut handoff = Handoff::new(
            sample_envelope(),
            HandoffId::parse("20260710-1432-fix-the-thing-a1b2").unwrap(),
            Uuid::new_v4(),
            None,
            1,
            "topic",
            None,
            sample_body(valid_gihoek_fields()),
        );
        let now = Utc::now();
        handoff.transition_to(HandoffState::InProgress, now, Some("session-1")).unwrap();
        assert_eq!(handoff.state, HandoffState::InProgress);
        assert_eq!(handoff.claimed_at, Some(now));
        handoff.transition_to(HandoffState::Done, now, None).unwrap();
        assert_eq!(handoff.state, HandoffState::Done);
        assert_eq!(handoff.completed_at, Some(now));
    }

    #[test]
    fn terminal_states_reject_transitions_back_to_pending_or_in_progress() {
        let mut handoff = Handoff::new(
            sample_envelope(),
            HandoffId::parse("20260710-1432-fix-the-thing-a1b2").unwrap(),
            Uuid::new_v4(),
            None,
            1,
            "topic",
            None,
            sample_body(valid_gihoek_fields()),
        );
        let now = Utc::now();
        handoff.transition_to(HandoffState::InProgress, now, None).unwrap();
        handoff.transition_to(HandoffState::Done, now, None).unwrap();

        let err = handoff.transition_to(HandoffState::Pending, now, None).unwrap_err();
        assert_eq!(err.class, FailureClass::InvalidStateTransition);

        let mut abandoned = Handoff::new(
            sample_envelope(),
            HandoffId::parse("20260710-1432-other-topic-c3d4").unwrap(),
            Uuid::new_v4(),
            None,
            1,
            "topic",
            None,
            sample_body(valid_gihoek_fields()),
        );
        abandoned.transition_to(HandoffState::InProgress, now, None).unwrap();
        abandoned.transition_to(HandoffState::Abandoned, now, None).unwrap();
        let err = abandoned.transition_to(HandoffState::InProgress, now, None).unwrap_err();
        assert_eq!(err.class, FailureClass::InvalidStateTransition);
    }

    #[test]
    fn gihoek_domain_with_valid_fields_validates_and_renders() {
        let registry = registry_with_gihoek();
        let body = sample_body(valid_gihoek_fields());
        assert!(registry.validate_body(&body).is_ok());
        let rendered = registry.render_body(&body).unwrap();
        assert!(rendered.contains("Ship S1"));
        assert!(rendered.contains("round trips"));
    }

    #[test]
    fn gihoek_domain_missing_acceptance_criteria_names_the_field() {
        let registry = registry_with_gihoek();
        let body = sample_body(serde_json::json!({"title": "t", "summary": "s"}));
        let err = registry.validate_body(&body).unwrap_err();
        assert_eq!(err.class, FailureClass::InvalidHandoffBody);
        assert_eq!(err.subject, "acceptance-criteria");
    }

    #[test]
    fn unregistered_domain_is_rejected_before_write() {
        let registry = registry_with_gihoek();
        let body = HandoffBody { domain: DomainId::parse("no-such-domain").unwrap(), template_version: 1, fields: valid_gihoek_fields() };
        let err = registry.validate_body(&body).unwrap_err();
        assert_eq!(err.class, FailureClass::UnregisteredHandoffDomain);
    }

    #[test]
    fn two_domains_expose_identical_state_machine_fields() {
        let a = Handoff::new(
            sample_envelope(),
            HandoffId::parse("20260710-1432-a-topic-a1b2").unwrap(),
            Uuid::new_v4(),
            None,
            1,
            "a",
            None,
            sample_body(valid_gihoek_fields()),
        );
        let mut other_body_fields = valid_gihoek_fields();
        other_body_fields["title"] = serde_json::json!("different domain body");
        let b = Handoff::new(
            sample_envelope(),
            HandoffId::parse("20260710-1432-b-topic-c3d4").unwrap(),
            Uuid::new_v4(),
            None,
            1,
            "b",
            None,
            HandoffBody { domain: DomainId::parse("디자인").unwrap(), template_version: 1, fields: other_body_fields },
        );
        let a_json = serde_json::to_value(&a).unwrap();
        let b_json = serde_json::to_value(&b).unwrap();
        for column in ["state", "chain_id", "parent_handoff_id", "seq", "claimed_by", "openspec_change_slug"] {
            assert_eq!(
                a_json.as_object().unwrap().contains_key(column),
                b_json.as_object().unwrap().contains_key(column),
                "column {column} presence differs across domains"
            );
        }
    }
}
