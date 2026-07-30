//! The `ArtifactAdapter` trait + `ArtifactEvent` normalization target
//! (S4 FOUNDATION wave, frozen for Wave 2's ledger/divergence/handoff/
//! openspec-task adapters).
//!
//! Distinct from [`crate::adapter::SessionAdapter`] (S3): a
//! `SessionAdapter` normalizes billable-model-call token rows keyed by
//! `session_id`; an `ArtifactAdapter` is a **verdict-deriving** adapter
//! — it reads a review/CI/handoff/task-state artifact and normalizes it
//! into an [`ArtifactEvent`] keyed by the S1 join spine's
//! `scenario_id`/`handoff_id`/`task_id`, which `crate::verdict` then
//! folds (a pure, table-driven step, never per-adapter logic — design
//! D5) into an optional `{role, polarity, becomes}` verdict.
//!
//! **Rescope (operator directive, 2026-07-11): every adapter's source
//! root is `canon.yaml`-configured, GENERIC — never a hardcoded
//! sibling-repo path or a live hosted-Postgres connection.**
//! [`ArtifactSourceConfig`] carries that configuration surface;
//! [`ArtifactSourceHandle`] is what an adapter's `parse` call actually
//! reads (a filesystem root for the ledger/divergence/openspec-task
//! adapters, or already-fetched [`RawRecord`]s for the handoff adapter
//! — `canon-ingest` has no `canon-store` dependency, so a DB-backed
//! adapter never opens its own connection; its wave-2 driver resolves
//! the live query through `canon-store::Tier::read` and hands the rows
//! in here).

use std::path::PathBuf;

use canon_model::evidence::RawRecord;
use canon_model::ids::{HandoffId, RoleId, ScenarioId, TaskId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The generic, `canon.yaml`-sourced configuration every
/// `ArtifactAdapter` resolves its source location from. Every path
/// field defaults to `None` — an unconfigured source is simply not
/// scanned, NEVER silently defaulted to a hardcoded sibling repo (the
/// exact violation this rescope removes: the S4 design as originally
/// authored pointed the ledger/divergence adapters at a hardcoded
/// donor-consumer-repo `spec/**` path and the handoff adapter at a
/// prior session/event store's live hosted-Postgres `handoffs`
/// table). The one non-path field,
/// `native_records` (S15 P4, design D7), defaults `false` for the
/// same reason — a native source is never silently scanned either.
///
/// Parsing this struct out of a repo's actual `canon.yaml` file is
/// wave-2/CLI wiring (mirrors how `SessionAdapter::scan_roots` takes an
/// already-resolved `home: &Path` rather than reading YAML itself) —
/// this type is the frozen SHAPE that wiring populates, deriving
/// `Deserialize` so a future `serde_yaml::from_str::<ArtifactSourceConfig>`
/// (or a field nested inside a larger `canon.yaml` document) needs no
/// bespoke parser.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSourceConfig {
    /// Root of a Hive-partitioned ledger tree
    /// (`kind=<kind>/[area=<area>/]*.json`, S4 design D1). The donor
    /// consumer repo's `spec/ledger/` is the reference donor and
    /// fixture-corpus origin — never a compiled-in default.
    #[serde(default)]
    pub ledger_root: Option<PathBuf>,
    /// Root of a Hive-partitioned divergence tree
    /// (`lane=<l>/area=<a>/surface=<s>/*.jsonl`, S4 design D2).
    #[serde(default)]
    pub divergences_root: Option<PathBuf>,
    /// Root under which `openspec/changes/*/tasks.md` files are scanned
    /// (S4 design D4). Ordinarily the consumer repo's own root.
    #[serde(default)]
    pub openspec_root: Option<PathBuf>,
    /// Enables the S15 P4 NATIVE verdict records-source adapters
    /// (`review`/`divergence-native`, design D7) against canon's OWN
    /// tiers. XOR-exclusive with `ledger_root`/`divergences_root`/
    /// `openspec_root`: the two source families' verdict rows differ
    /// slightly, so `trajectory_content_digest` (canon-cli) would not
    /// dedupe them, silently double-counting the same underlying
    /// evidence — `canon-cli::artifact_ingest`'s config-load step
    /// rejects a config that sets both before any read runs (spec
    /// `native-record-flywheel` Requirement 3). Defaults `false`, like
    /// every other field here — never silently on.
    #[serde(default)]
    pub native_records: bool,
}

/// What one [`ArtifactAdapter::parse`] call actually reads — resolved
/// from [`ArtifactSourceConfig`] (path-based sources) or supplied
/// directly by a caller that already ran its own query (handle-based
/// sources, e.g. the handoff adapter's `Tier::read` result).
#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactSourceHandle {
    /// A filesystem root (a directory to walk, or — for the openspec
    /// adapter — a single `tasks.md` file) this adapter scans directly.
    Path(PathBuf),
    /// Already-fetched raw candidate records (e.g. rows a `Tier::read`
    /// call against canon's own Postgres-tier `Handoff` table already
    /// resolved). `canon-ingest` never opens the connection itself —
    /// see this module's doc comment.
    Records(Vec<RawRecord>),
}

/// The S1 join-spine identifier an [`ArtifactEvent`] is keyed by — the
/// three key kinds design §5 S4 names (`scenario_id`, `handoff_id`,
/// `task_id`; `change_id` is not listed separately because
/// `TaskId::change_id()` already decomposes it, join-spine spec).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArtifactJoinKey {
    Scenario(ScenarioId),
    Handoff(HandoffId),
    Task(TaskId),
}

impl ArtifactJoinKey {
    /// The concrete join-spine id string this key names
    /// (`"platformer.hud.01"`, a `HandoffId`, `"my-change#1.2"`) —
    /// deliberately NOT the source-kind-TAGGED identity string
    /// (`canon-cli::artifact_ingest::join_key_identity`'s
    /// `scenario:`/`handoff:`/`task:` prefixing, which exists so two
    /// same-spelled ids of different kinds can never collide inside a
    /// digest).
    ///
    /// Added by `s38-evidence-bearing-memory`: the artifact-ingest
    /// driver names the CONCRETE artifact each trajectory (and thus
    /// every strategy distilled from it) is about, so retrieved
    /// guidance reads `platformer.hud.01: review promotion` instead of
    /// a sentence describing the ingest driver's own plumbing. Without
    /// this accessor every such caller re-matches all three variants
    /// just to reach the id it already knows is there.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Scenario(id) => id.as_str(),
            Self::Handoff(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

/// The closed classification an [`ArtifactEvent`] carries — the ONLY
/// vocabulary [`crate::verdict::derive_verdict`] reads (S4 design §5,
/// reproduced verbatim in `specs/review-verdict-mapping/spec.md`).
/// Every wave-2 adapter's job is to map its own raw record shape onto
/// one of these variants; `derive_verdict` never sees adapter-specific
/// JSON.
///
/// The seven `*Finding`/`*Promotion`/`*Resolved`/`*Revert`/`*Merge`
/// variants are exactly the design table's seven rows. `NonVerdict`
/// collapses every explicit non-verdict case the design names: a
/// divergence manifest line (D2), a ledger `run`/`drill` record (D1), a
/// handoff state transition alone (D3 — "a handoff is management
/// plumbing, not a review/CI/merge signal"), and an openspec task flip
/// with no parseable merge/CI evidence or a `**DEFERRED**`/`**DROPPED**`
/// rewrite (D4 — "malformed evidence is no evidence… at the verdict
/// layer").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactEventKind {
    /// Ledger `kind=code-review`, `verdict` absent or not `faithful` —
    /// an open/still-divergent finding (table row 1).
    CodeReviewFinding,
    /// Ledger `kind=design-review`, same verdict condition (row 2).
    DesignReviewFinding,
    /// Ledger `kind=review` promoting a scenario to `@reviewed` (row 3).
    ReviewPromotion,
    /// Ledger `kind=clear` clearing a previously `@flagged` scenario
    /// (row 4).
    ClearAfterFlagged,
    /// Divergence `type=remediation` followed by a `resolved` status
    /// (row 5).
    RemediationResolved,
    /// A CI failure or PR revert observed via the openspec/handoff-
    /// joined event stream (row 6).
    CiFailOrPrRevert,
    /// A PR merge with no revert recorded within the configured revert
    /// window (row 7).
    PrMergeNoRevert,
    /// Every explicit non-verdict case (see variant-group doc above) —
    /// `derive_verdict` always returns `None` for this variant.
    NonVerdict,
}

impl ArtifactEventKind {
    /// A short human phrase naming this classification, written to read
    /// as prose inside a sentence an AGENT is shown — never the Rust
    /// variant name.
    ///
    /// `s38-evidence-bearing-memory`: these strings travel all the way
    /// into `canon retrieve`'s injected guidance (event ->
    /// `canon_learn::Trajectory::task` ->
    /// `canon_learn::StrategyItem::title`), so `"review promotion"` is
    /// deliberate where `"ReviewPromotion"` would leak canon's
    /// internals into a dispatched agent's context window. Returns
    /// `&'static str` because every phrase is a literal — labelling an
    /// event never allocates.
    pub fn label(&self) -> &'static str {
        match self {
            Self::CodeReviewFinding => "code-review finding",
            Self::DesignReviewFinding => "design-review finding",
            Self::ReviewPromotion => "review promotion",
            Self::ClearAfterFlagged => "clear after flagged",
            Self::RemediationResolved => "remediation resolved",
            Self::CiFailOrPrRevert => "CI failure or PR revert",
            Self::PrMergeNoRevert => "PR merge with no revert",
            Self::NonVerdict => "non-verdict",
        }
    }
}

/// The shared normalization target every [`ArtifactAdapter::parse`]
/// call emits — mirrors [`crate::adapter::UnifiedRow`]'s role for
/// `SessionAdapter`, one event per artifact-ingest record.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactEvent {
    /// The emitting adapter's stable identity (`"ledger"` |
    /// `"divergence"` | `"handoff"` | `"openspec-task"`, wave-2).
    pub adapter_id: &'static str,
    /// The S1 join-spine key this event is keyed by.
    pub join_key: ArtifactJoinKey,
    /// What kind of thing happened — the only field
    /// `crate::verdict::derive_verdict` inspects.
    pub kind: ArtifactEventKind,
    /// The role that authored the underlying artifact, when the source
    /// record makes it derivable (required for `ReviewPromotion`'s "the
    /// authoring role of the scenario"; `None` for every other kind).
    pub authoring_role: Option<RoleId>,
    /// The source artifact's area/severity tag (ledger `scenario_id`'s
    /// `<area>` component, divergence `area=`/`surface=` partition
    /// keys, …) — folded into a verdict's `regime_key` by the emitting
    /// adapter (task 5.2), never recomputed inside `derive_verdict`.
    pub area: Option<String>,
    /// A passthrough trust-level tag (`@reviewed`/`@ratified` where
    /// applicable, task 5.3) — carried on the event so the adapter can
    /// copy it onto the emitted verdict without a second source lookup.
    pub trust_level: Option<String>,
    /// When the source record itself was authored/observed.
    pub at: DateTime<Utc>,
    /// The full normalized detail this event carries — mirrors
    /// `canon_model::records::Event.detail`'s deliberately open
    /// `serde_json::Value` shape; the field this event's eventual
    /// `canon_model::records::Event` conversion copies verbatim.
    pub detail: serde_json::Value,
}

/// The `detail` keys [`ArtifactEvent::evidence_line`] mines for prose,
/// in priority order — every one is a field an adapter in
/// `crate::artifact_adapters` genuinely emits
/// (`s38-evidence-bearing-memory`):
///
/// - `detail` — a `canon_model::records::Divergence`'s own review
///   narrative (the "SHIP-BLOCKER `<file>:<line>` … `<remedy>`" finding
///   text canon's own divergence corpus carries), plus any raw
///   divergence JSONL line that spells its finding out inline. First
///   because it is the only field that is prose by CONSTRUCTION.
/// - `pin` — the reviewed commit pin every ledger/`Review` record
///   carries; not sentences, but it names WHAT was attested, which is
///   the whole evidentiary content of a promotion.
/// - `evidence` — an openspec `tasks.md` checkbox flip's evidence note
///   (`crate::artifact_adapters::openspec_task`).
/// - `reason` — a deferral/exemption rationale.
/// - `title` — an openspec task row's human title: for a flip with no
///   evidence note this is the only prose the row has.
/// - `disposition` — a divergence remediation's "what was done about
///   it" tag.
///
/// Array- and object-valued fields (a divergence review line's
/// `aspects[]`, a handoff body) are deliberately NOT mined: this list
/// exists to produce ONE line, and structured payloads can only be
/// rendered by serializing them — the exact failure
/// `s38-evidence-bearing-memory` removes.
const EVIDENCE_PROSE_FIELDS: [&str; 6] = ["detail", "pin", "evidence", "reason", "title", "disposition"];

/// The char cap [`ArtifactEvent::evidence_line`] applies to mined prose.
///
/// 512 is measured, not guessed: the 14 real reviewer findings in
/// canon's own `.canon/ledger/kind=divergence` corpus run 157–485
/// chars (a locator + symptom + remedy sentence), so 512 keeps every
/// genuine finding INTACT while bounding one pathological record — a
/// pasted stack trace, diff, or log dump — to roughly six terminal
/// lines. The cap matters because these lines are newline-joined into a
/// strategy's content and several strategies are injected into a
/// dispatched agent's context at once: without it, a single fat record
/// bloats every retrieval that touches its regime.
const EVIDENCE_TEXT_MAX_CHARS: usize = 512;

impl ArtifactEvent {
    /// The label this event is NAMED by in agent-facing text — normally
    /// its [`ArtifactEventKind::label`], but native-verdict adapters
    /// (`review`/`divergence-native`, S15 P4) are the exception this
    /// method exists for (`s38-evidence-bearing-memory`).
    ///
    /// Those two adapters deliberately set `kind = NonVerdict`, because
    /// the frozen S4 table is not what scores them — their verdicts come
    /// from `crate::verdict::derive_native_review_verdict`/
    /// `derive_native_divergence_verdict`, dispatched on `adapter_id`.
    /// So `kind.label()` alone would title every native-derived
    /// strategy `"non-verdict"`: canon-internal vocabulary, and flatly
    /// misleading about a record that DID produce a verdict. Dispatch
    /// mirrors `canon-cli::artifact_ingest::derive_verdict_for_event`
    /// exactly — on the adapter-controlled `&'static str` `adapter_id`,
    /// never on artifact-supplied `detail` content, so a raw record
    /// carrying a stray `native_kind` cannot relabel itself.
    ///
    /// A native divergence additionally names its own status, since
    /// `resolved` (a `Success`, "this is how it was fixed") and
    /// `still_divergent` (a `Failure`, "this is what stayed broken")
    /// distill into opposite kinds of memory and read as different
    /// advice. `DivergenceStatus` is a closed enum, so every phrase
    /// stays a literal.
    pub fn display_label(&self) -> &'static str {
        match (self.kind, self.adapter_id) {
            (ArtifactEventKind::NonVerdict, "review") => "review attestation",
            (ArtifactEventKind::NonVerdict, "divergence-native") => match self.detail.get("status").and_then(state_token) {
                Some("resolved") => "resolved divergence",
                Some("still_divergent") => "still-divergent divergence",
                Some("open") => "open divergence",
                Some("deferred") => "deferred divergence",
                _ => "divergence",
            },
            _ => self.kind.label(),
        }
    }

    /// ONE compact, human-readable line describing what this event
    /// actually says: its [`Self::display_label`] plus the most
    /// salient free text the event carries (`EVIDENCE_PROSE_FIELDS`,
    /// first present non-empty string wins), whitespace-collapsed and
    /// capped at `EVIDENCE_TEXT_MAX_CHARS` (512 chars — see that
    /// constant for the measurement behind the number).
    ///
    /// `s38-evidence-bearing-memory` exists because the artifact-ingest
    /// driver used to describe ITSELF ("N verdict(s) derived from
    /// canon-ingest artifact adapters for regime …") when building the
    /// trajectory text that becomes a retrieved strategy's content —
    /// grinding metadata while the real evidence sat unread in
    /// `detail`. This is the accessor that turns `detail` into the one
    /// line a distilled strategy can carry.
    ///
    /// It NEVER serializes `detail` itself. Dumping the blob is how the
    /// pre-s38 output became unreadable, and a strategy's text is
    /// agent-facing prose, not a record dump. When no prose field is
    /// present the line degrades to naming the event's `status`/`state`
    /// (an externally-tagged status object contributes its variant tag
    /// only, e.g. `deferred`), and failing that to the bare label —
    /// short and honest beats long and unreadable. A native label
    /// already ENCODES the record's status ([`Self::display_label`]), so
    /// the status step is skipped there rather than rendering `resolved
    /// divergence: status resolved`, which says the same thing twice.
    pub fn evidence_line(&self) -> String {
        let label = self.display_label();
        if let Some(text) = self.salient_prose() {
            return format!("{label}: {text}");
        }
        if label == self.kind.label() {
            for field in ["status", "state"] {
                if let Some(token) = self.detail.get(field).and_then(state_token) {
                    return format!("{label}: {field} {token}");
                }
            }
        }
        label.to_string()
    }

    /// Whether [`Self::evidence_line`] will carry genuine NARRATIVE —
    /// i.e. whether [`Self::salient_prose`] finds one of
    /// `EVIDENCE_PROSE_FIELDS` — rather than degrading to the bare
    /// [`Self::display_label`] or to the `<label>: status <token>`
    /// fallback (s39 `joined-evidence-grounding`).
    ///
    /// `canon-cli::artifact_ingest`'s antecedent join needs exactly this
    /// distinction: a NON-verdict event contributes its prose as
    /// antecedent evidence to the verdict that later closed it, and a
    /// bare kind label ("open divergence", "non-verdict") is not
    /// evidence of anything — it would pad every retrieved strategy with
    /// canon's own vocabulary, the precise regression
    /// `s38-evidence-bearing-memory` removed.
    ///
    /// Exposed as a predicate BESIDE `salient_prose` rather than
    /// re-deriving the judgement in the caller, for two reasons.
    /// `EVIDENCE_PROSE_FIELDS`' priority order is a contract stated once
    /// here (its own doc comment), and a second copy in another crate
    /// would drift. And the obvious caller-side alternative —
    /// `evidence_line() != display_label()` — silently passes the
    /// status/state fallback line, which names a state token, not a
    /// narrative: `non-verdict: status still_divergent` would be
    /// absorbed as though it were a reviewer's finding.
    pub fn has_salient_prose(&self) -> bool {
        self.salient_prose().is_some()
    }

    /// The first present, non-blank `EVIDENCE_PROSE_FIELDS` string,
    /// compacted ([`compact_evidence_text`]). Scans in the constant's
    /// declared order and stops at the first hit, so at most one
    /// allocation ever happens.
    fn salient_prose(&self) -> Option<String> {
        EVIDENCE_PROSE_FIELDS
            .iter()
            .filter_map(|field| self.detail.get(*field)?.as_str())
            .find(|raw| !raw.trim().is_empty())
            .map(compact_evidence_text)
    }
}

/// A short token naming a `status`/`state` value —
/// [`ArtifactEvent::evidence_line`]'s no-prose fallback
/// (`s38-evidence-bearing-memory`). A plain string is the token; a
/// single-key object is serde's externally-tagged enum form
/// (`canon_model::records::DivergenceStatus::Deferred` serializes as
/// `{"deferred": {"reason": …, "expiry": …}}`), whose ONE key is the
/// variant tag — the payload stays unread on purpose, because rendering
/// it means serializing a blob. Anything else (a number, an array, a
/// multi-key object) has no honest one-token rendering and yields
/// `None`.
fn state_token(value: &serde_json::Value) -> Option<&str> {
    match value {
        serde_json::Value::String(s) => Some(s.trim()).filter(|s| !s.is_empty()),
        serde_json::Value::Object(map) if map.len() == 1 => map.keys().next().map(String::as_str),
        _ => None,
    }
}

/// Collapses every whitespace run (newlines and tabs included — a
/// reviewer's finding is frequently multi-line) into one space, trims,
/// and caps the result at [`EVIDENCE_TEXT_MAX_CHARS`] CHARS (never
/// bytes — canon's corpora carry Korean prose, and a byte cut would
/// split a codepoint), marking a cut with a trailing `…`
/// (`s38-evidence-bearing-memory`).
fn compact_evidence_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(EVIDENCE_TEXT_MAX_CHARS + 4));
    for word in raw.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if let Some((cut, _)) = out.char_indices().nth(EVIDENCE_TEXT_MAX_CHARS) {
        out.truncate(cut);
        let trimmed = out.trim_end().len();
        out.truncate(trimmed);
        out.push('…');
    }
    out
}

/// The result of one [`ArtifactAdapter::parse`] call: the events it
/// successfully extracted, plus a count of records it could not parse
/// at all — mirrors [`crate::adapter::ParseOutcome`]'s "malformed
/// evidence is no evidence" discipline (design §7): a record this
/// adapter's format doesn't recognize is skipped AND counted, never
/// silently dropped, never a panic.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArtifactParseOutcome {
    pub events: Vec<ArtifactEvent>,
    pub skipped: usize,
}

impl ArtifactParseOutcome {
    pub fn empty() -> Self {
        Self::default()
    }
}

/// One artifact-ingest adapter (S4 FOUNDATION, frozen for Wave 2's
/// ledger/divergence/handoff/openspec-task adapters — mirrors
/// `SessionAdapter`'s "trait + static table" shape, S3 design D1,
/// generalized to a source that is sometimes a path and sometimes an
/// already-fetched record batch). `adapter_id()` names the adapter;
/// `resolve_source` turns the generic, `canon.yaml`-sourced
/// [`ArtifactSourceConfig`] into this adapter's own
/// [`ArtifactSourceHandle`] (returning `None` when this adapter's
/// config field is unset — an unconfigured source is never scanned);
/// `parse` converts one resolved handle into an [`ArtifactParseOutcome`].
///
/// A handle-based adapter (the handoff adapter) has no meaningful
/// `resolve_source` output from `ArtifactSourceConfig` alone — its
/// wave-2 driver constructs an `ArtifactSourceHandle::Records(..)`
/// directly (after resolving canon's own Postgres-tier `Handoff` table
/// through `canon-store::Tier::read`, entirely outside this crate) and
/// calls `parse` with it, skipping `resolve_source`.
pub trait ArtifactAdapter: Send + Sync {
    /// The adapter's stable identity (`"ledger"` | `"divergence"` |
    /// `"handoff"` | `"openspec-task"`, wave-2).
    fn adapter_id(&self) -> &'static str;

    /// Resolve this adapter's source from the generic config surface.
    /// `None` when this adapter's config field is unset, or when this
    /// adapter is handle-based (see trait doc comment) — never a
    /// hardcoded fallback path.
    fn resolve_source(&self, config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle>;

    /// Parse one already-resolved source into an [`ArtifactParseOutcome`].
    /// Malformed/unparseable content is skipped AND counted (design
    /// §7), never a crash.
    fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome;
}

/// Trivial accessor mirroring `SessionAdapter::scan_roots`'s
/// `home`-join convenience for a path-based [`ArtifactSourceConfig`]
/// field — joins a configured root against nothing (it is already
/// absolute or repo-root-relative, unlike `SessionAdapter`'s
/// `home`-relative roots) and simply clones it into an owned
/// [`PathBuf`], the shape [`ArtifactSourceHandle::Path`] wraps.
pub fn resolve_path_source(root: &Option<PathBuf>) -> Option<ArtifactSourceHandle> {
    root.as_ref().map(|p| ArtifactSourceHandle::Path(p.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_source_config_defaults_to_unconfigured() {
        // No field defaults to a hardcoded donor-repo path — an
        // unconfigured source stays `None`; `native_records` stays
        // `false` (never silently on).
        let config = ArtifactSourceConfig::default();
        assert_eq!(config.ledger_root, None);
        assert_eq!(config.divergences_root, None);
        assert_eq!(config.openspec_root, None);
        assert!(!config.native_records);
    }

    #[test]
    fn native_records_deserializes_and_defaults_false() {
        let bare = serde_json::json!({});
        let config: ArtifactSourceConfig = serde_json::from_value(bare).unwrap();
        assert!(!config.native_records);

        let on = serde_json::json!({"native_records": true});
        let config: ArtifactSourceConfig = serde_json::from_value(on).unwrap();
        assert!(config.native_records);
    }

    #[test]
    fn artifact_source_config_deserializes_from_json() {
        let json = serde_json::json!({
            "ledger_root": "canon/fixtures/ledger",
            "divergences_root": "canon/fixtures/divergences",
        });
        let config: ArtifactSourceConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.ledger_root, Some(PathBuf::from("canon/fixtures/ledger")));
        assert_eq!(config.divergences_root, Some(PathBuf::from("canon/fixtures/divergences")));
        assert_eq!(config.openspec_root, None);
    }

    #[test]
    fn resolve_path_source_is_none_when_unconfigured() {
        assert_eq!(resolve_path_source(&None), None);
        assert_eq!(resolve_path_source(&Some(PathBuf::from("a/b"))), Some(ArtifactSourceHandle::Path(PathBuf::from("a/b"))));
    }

    /// A minimal fixture adapter proving `ArtifactAdapter` is
    /// dyn-compatible and that a path-based `resolve_source` +
    /// `parse` round trip works end to end — the shape wave-2's real
    /// adapters implement against.
    struct FixtureAdapter;

    impl ArtifactAdapter for FixtureAdapter {
        fn adapter_id(&self) -> &'static str {
            "fixture"
        }

        fn resolve_source(&self, config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle> {
            resolve_path_source(&config.ledger_root)
        }

        fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome {
            match source {
                ArtifactSourceHandle::Path(p) if p.exists() => ArtifactParseOutcome { events: Vec::new(), skipped: 0 },
                _ => ArtifactParseOutcome { events: Vec::new(), skipped: 1 },
            }
        }
    }

    #[test]
    fn artifact_adapter_is_dyn_compatible_and_round_trips_config() {
        let adapter: &dyn ArtifactAdapter = &FixtureAdapter;
        assert_eq!(adapter.adapter_id(), "fixture");

        let unconfigured = ArtifactSourceConfig::default();
        assert!(adapter.resolve_source(&unconfigured).is_none());

        let dir = tempfile::tempdir().unwrap();
        let configured = ArtifactSourceConfig { ledger_root: Some(dir.path().to_path_buf()), ..Default::default() };
        let source = adapter.resolve_source(&configured).unwrap();
        let outcome = adapter.parse(&source);
        assert_eq!(outcome.skipped, 0);
    }

    #[test]
    fn artifact_source_handle_records_variant_carries_raw_records() {
        let raw = RawRecord(serde_json::json!({"id": "20260710-1432-fix-a1b2"}));
        let handle = ArtifactSourceHandle::Records(vec![raw.clone()]);
        match handle {
            ArtifactSourceHandle::Records(rows) => assert_eq!(rows, vec![raw]),
            ArtifactSourceHandle::Path(_) => panic!("expected Records variant"),
        }
    }

    fn event_with(kind: ArtifactEventKind, detail: serde_json::Value) -> ArtifactEvent {
        ArtifactEvent {
            adapter_id: "fixture",
            join_key: ArtifactJoinKey::Scenario(ScenarioId::parse("platformer.hud.01").unwrap()),
            kind,
            authoring_role: None,
            area: Some("platformer".to_string()),
            trust_level: None,
            at: "2026-07-14T20:35:50Z".parse().unwrap(),
            detail,
        }
    }

    fn native_event(adapter_id: &'static str, detail: serde_json::Value) -> ArtifactEvent {
        ArtifactEvent { adapter_id, ..event_with(ArtifactEventKind::NonVerdict, detail) }
    }

    #[test]
    fn display_label_is_the_kind_label_for_every_s4_table_event() {
        // The S4 raw-path adapters classify by `kind`, so nothing is
        // remapped for them — including a `NonVerdict` one.
        assert_eq!(event_with(ArtifactEventKind::CodeReviewFinding, serde_json::json!({})).display_label(), "code-review finding");
        assert_eq!(event_with(ArtifactEventKind::NonVerdict, serde_json::json!({"status": "resolved"})).display_label(), "non-verdict");
    }

    #[test]
    fn display_label_names_a_native_verdict_event_by_its_adapter_and_status() {
        // The native adapters set `kind = NonVerdict` (their verdicts
        // come from the native derivation path, dispatched on
        // `adapter_id`), so labelling them off `kind` alone would title
        // every native-derived strategy "non-verdict" — internal
        // vocabulary, and wrong about a record that DID score.
        assert_eq!(native_event("review", serde_json::json!({"native_kind": "review", "pin": "6be8cc2b"})).display_label(), "review attestation");

        let status = |s: serde_json::Value| native_event("divergence-native", serde_json::json!({"native_kind": "divergence", "status": s}));
        assert_eq!(status(serde_json::json!("resolved")).display_label(), "resolved divergence");
        assert_eq!(status(serde_json::json!("still_divergent")).display_label(), "still-divergent divergence");
        assert_eq!(status(serde_json::json!("open")).display_label(), "open divergence");
        assert_eq!(status(serde_json::json!({"deferred": {"reason": "waiting on design"}})).display_label(), "deferred divergence");
        // An unreadable/absent status still names the record kind, never
        // "non-verdict" and never a fabricated status.
        assert_eq!(native_event("divergence-native", serde_json::json!({"native_kind": "divergence"})).display_label(), "divergence");
    }

    #[test]
    fn a_spoofed_native_kind_cannot_relabel_an_s4_event() {
        // Mirrors `canon-cli::artifact_ingest`'s ReviewP4 discipline:
        // the raw-path adapters copy artifact JSON verbatim into
        // `detail`, so labelling must key off the adapter-controlled
        // `adapter_id`, never source content.
        let spoofed = event_with(ArtifactEventKind::CodeReviewFinding, serde_json::json!({"native_kind": "divergence", "status": "resolved"}));
        assert_eq!(spoofed.display_label(), "code-review finding");
    }

    #[test]
    fn evidence_line_does_not_repeat_a_status_its_native_label_already_names() {
        let resolved = native_event("divergence-native", serde_json::json!({"native_kind": "divergence", "status": "resolved"}));
        assert_eq!(resolved.evidence_line(), "resolved divergence", "never `resolved divergence: status resolved`");

        // With prose the line still quotes it — the skip is only of the
        // redundant status step.
        let with_prose = native_event(
            "divergence-native",
            serde_json::json!({"native_kind": "divergence", "status": "resolved", "detail": "Fixed and re-verified at 505a668e"}),
        );
        assert_eq!(with_prose.evidence_line(), "resolved divergence: Fixed and re-verified at 505a668e");
    }

    #[test]
    fn join_key_as_str_names_the_concrete_artifact_id_for_every_variant() {
        assert_eq!(ArtifactJoinKey::Scenario(ScenarioId::parse("platformer.hud.01").unwrap()).as_str(), "platformer.hud.01");
        assert_eq!(ArtifactJoinKey::Handoff(HandoffId::parse("20260710-1432-fix-a1b2").unwrap()).as_str(), "20260710-1432-fix-a1b2");
        assert_eq!(ArtifactJoinKey::Task(TaskId::parse("frozen-fixture-change#1.4").unwrap()).as_str(), "frozen-fixture-change#1.4");
    }

    #[test]
    fn every_event_kind_label_reads_as_prose_never_as_the_variant_name() {
        let labels = [
            (ArtifactEventKind::CodeReviewFinding, "code-review finding"),
            (ArtifactEventKind::DesignReviewFinding, "design-review finding"),
            (ArtifactEventKind::ReviewPromotion, "review promotion"),
            (ArtifactEventKind::ClearAfterFlagged, "clear after flagged"),
            (ArtifactEventKind::RemediationResolved, "remediation resolved"),
            (ArtifactEventKind::CiFailOrPrRevert, "CI failure or PR revert"),
            (ArtifactEventKind::PrMergeNoRevert, "PR merge with no revert"),
            (ArtifactEventKind::NonVerdict, "non-verdict"),
        ];
        for (kind, expected) in labels {
            assert_eq!(kind.label(), expected);
            // These strings are read by agents, not by rustc: a label
            // that still spelled the variant name would leak canon's
            // internals into a retrieved strategy's title.
            assert!(!kind.label().contains(&format!("{kind:?}")), "{expected:?} must not be the Debug spelling");
            assert!(!kind.label().is_empty());
        }
    }

    #[test]
    fn evidence_line_prefers_detail_then_pin_then_evidence_then_reason() {
        // Priority order, proven by removing one field at a time: the
        // constant's order IS the contract, because a `Divergence`'s
        // `detail` narrative outranks the commit `pin` that merely
        // names what was attested.
        let all = serde_json::json!({"detail": "d-prose", "pin": "p-prose", "evidence": "e-prose", "reason": "r-prose"});
        assert_eq!(event_with(ArtifactEventKind::CodeReviewFinding, all).evidence_line(), "code-review finding: d-prose");

        let no_detail = serde_json::json!({"pin": "p-prose", "evidence": "e-prose", "reason": "r-prose"});
        assert_eq!(event_with(ArtifactEventKind::ReviewPromotion, no_detail).evidence_line(), "review promotion: p-prose");

        let no_pin = serde_json::json!({"evidence": "e-prose", "reason": "r-prose"});
        assert_eq!(event_with(ArtifactEventKind::PrMergeNoRevert, no_pin).evidence_line(), "PR merge with no revert: e-prose");

        let only_reason = serde_json::json!({"reason": "r-prose"});
        assert_eq!(event_with(ArtifactEventKind::NonVerdict, only_reason).evidence_line(), "non-verdict: r-prose");

        let task_row = serde_json::json!({"title": "wire the reward writeback", "disposition": "port-fixed"});
        assert_eq!(event_with(ArtifactEventKind::NonVerdict, task_row).evidence_line(), "non-verdict: wire the reward writeback");
    }

    #[test]
    fn evidence_line_skips_blank_and_non_string_prose_fields() {
        // A `Divergence` with no narrative serializes `detail` away
        // entirely, but a whitespace-only or wrongly-typed field must
        // not win the scan and produce a dangling `"<label>: "`.
        let blank = serde_json::json!({"detail": "   \n ", "pin": "9c93d024b"});
        assert_eq!(event_with(ArtifactEventKind::ReviewPromotion, blank).evidence_line(), "review promotion: 9c93d024b");

        let wrong_type = serde_json::json!({"detail": {"nested": "object"}, "evidence": "note"});
        assert_eq!(event_with(ArtifactEventKind::NonVerdict, wrong_type).evidence_line(), "non-verdict: note");
    }

    #[test]
    fn evidence_line_falls_back_to_naming_status_then_state_when_no_prose_field_is_present() {
        let status = serde_json::json!({"native_kind": "divergence", "status": "still_divergent", "run_seq": 3});
        assert_eq!(event_with(ArtifactEventKind::NonVerdict, status).evidence_line(), "non-verdict: status still_divergent");

        // `DivergenceStatus::Deferred`'s externally-tagged form: the
        // variant tag is named, its payload deliberately unread.
        let tagged = serde_json::json!({"status": {"deferred": {"reason": "waiting on design", "expiry": "2026-08-01T00:00:00Z"}}});
        assert_eq!(event_with(ArtifactEventKind::NonVerdict, tagged).evidence_line(), "non-verdict: status deferred");

        let state = serde_json::json!({"transition": "claimed", "state": "in_review"});
        assert_eq!(event_with(ArtifactEventKind::NonVerdict, state).evidence_line(), "non-verdict: state in_review");
    }

    #[test]
    fn evidence_line_degrades_to_the_bare_label_when_the_detail_carries_neither_prose_nor_state() {
        let opaque = serde_json::json!({"run_seq": 3, "round": 1, "files": ["a.rs", "b.rs"]});
        assert_eq!(event_with(ArtifactEventKind::CiFailOrPrRevert, opaque).evidence_line(), "CI failure or PR revert");
    }

    #[test]
    fn evidence_line_never_serializes_the_detail_blob() {
        // The pre-s38 regression this whole accessor exists to prevent:
        // a strategy's text must never contain a JSON dump. Every
        // unmined key stays out of the line entirely.
        let detail = serde_json::json!({"native_kind": "divergence", "reviewer": "review-voice", "detail": "SHIP-BLOCKER teardown races HMR", "sha": "a".repeat(40)});
        let line = event_with(ArtifactEventKind::CodeReviewFinding, detail).evidence_line();
        assert_eq!(line, "code-review finding: SHIP-BLOCKER teardown races HMR");
        assert!(!line.contains('{') && !line.contains('"'), "no JSON punctuation may reach a strategy's text: {line}");
        assert!(!line.contains("reviewer") && !line.contains("native_kind"), "unmined detail keys must not leak: {line}");
    }

    #[test]
    fn evidence_line_collapses_whitespace_and_caps_pathological_prose() {
        let multiline = serde_json::json!({"detail": "  SHIP-BLOCKER PixiStage.tsx:28-32\n\tassigns app before\n\n awaiting loadTextures.  "});
        assert_eq!(
            event_with(ArtifactEventKind::CodeReviewFinding, multiline).evidence_line(),
            "code-review finding: SHIP-BLOCKER PixiStage.tsx:28-32 assigns app before awaiting loadTextures."
        );

        // One pathological record (a pasted log dump) must not bloat
        // every strategy retrieved for its regime.
        let fat = "x".repeat(EVIDENCE_TEXT_MAX_CHARS * 3);
        let line = event_with(ArtifactEventKind::CodeReviewFinding, serde_json::json!({"detail": fat})).evidence_line();
        let quoted = line.strip_prefix("code-review finding: ").expect("the label prefix always survives capping");
        assert_eq!(quoted.chars().count(), EVIDENCE_TEXT_MAX_CHARS + 1, "capped text is exactly the cap plus the ellipsis marker");
        assert!(quoted.ends_with('…'));

        // A finding exactly at the cap is left untouched — canon's own
        // corpus tops out at 485 chars, inside the cap by design.
        let exact = "y".repeat(EVIDENCE_TEXT_MAX_CHARS);
        let line = event_with(ArtifactEventKind::CodeReviewFinding, serde_json::json!({"detail": exact.clone()})).evidence_line();
        assert_eq!(line, format!("code-review finding: {exact}"));
    }

    #[test]
    fn compact_evidence_text_caps_on_chars_never_bytes() {
        // Multi-byte prose (canon's corpora carry Korean) must never be
        // cut mid-codepoint — a byte cap would panic in `truncate`.
        let korean = "가".repeat(EVIDENCE_TEXT_MAX_CHARS + 10);
        let capped = compact_evidence_text(&korean);
        assert_eq!(capped.chars().count(), EVIDENCE_TEXT_MAX_CHARS + 1);
        assert!(capped.ends_with('…'));
    }

    #[test]
    fn compact_evidence_text_does_not_leave_a_dangling_space_before_the_ellipsis() {
        // `EVIDENCE_TEXT_MAX_CHARS` is a multiple of 4, so a 4-char
        // `"abc "` cycle puts a SPACE at the char right before the cut
        // — the one input shape that would otherwise render `"abc …"`.
        let words = "abc ".repeat(EVIDENCE_TEXT_MAX_CHARS);
        let capped = compact_evidence_text(&words);
        assert!(capped.ends_with("abc…"), "the cut is trimmed before the marker: {capped}");
        assert!(!capped.contains(" …"));
    }

}
