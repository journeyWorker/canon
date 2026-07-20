//! `UnifiedRow`/`DirectiveRow` -> canon-model `Session`/`Run`/`Event`
//! normalization (S3 design §"Normalize every adapter's raw records
//! into canon-model's `Session`/`Run`/`Event` envelope … plus a
//! token/cost row keyed by `session_id`"; s31 design D4 folds in the
//! user-directive stream).
//!
//! canon-model (S1) has no standalone "token/cost row" record kind —
//! [`canon_model::records::Event`]'s `detail` field is deliberately
//! open (`serde_json::Value`) for exactly this: heterogeneous
//! per-event data that doesn't earn its own closed kind yet (S1's own
//! doc comment on `Event`). This module emits one `Event` per
//! `UnifiedRow` with `label: "token_usage"`, carrying the full token
//! breakdown + cost + provenance as `detail` — the token/cost row S3
//! calls for, keyed by `run_id` (which in turn carries `session_id`).
//!
//! **s31 D4 (user-directive capture)**: every `DirectiveRow` an
//! adapter extracted becomes a SECOND `Event` stream, `label:
//! "user_directive"`, folded into the SAME per-session `events` list
//! as the `token_usage` stream — one deterministic `seq` order across
//! both (`normalize_session`'s merge-then-stable-sort, see its doc
//! comment). `Session` also gains optional `workspace_key`/
//! `workspace_label` (populated here, first non-`None` in fold order)
//! and `project_key` (left `None` here — stamped on by `canon-cli`,
//! design D3: "project_key set by the CLI layer").

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{RunId, SessionId};
use canon_model::records::{Event, Run, RunStatus, Session};
use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::adapter::{DirectiveRow, UnifiedRow};

/// canon-model's envelope schema version every record this module
/// constructs carries (design D2: "per-kind schema version, bumped on
/// any breaking field change to that kind").
const SCHEMA_VERSION: u32 = 1;

/// The `Event.label` every normalized token/cost row carries.
pub const TOKEN_USAGE_LABEL: &str = "token_usage";

/// The `Event.label` every normalized user-directive row carries (s31
/// design D4).
pub const USER_DIRECTIVE_LABEL: &str = "user_directive";

/// One session's worth of normalized canon-model output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NormalizedSession {
    pub session: Session,
    pub run: Run,
    pub events: Vec<Event>,
}

/// The full result of normalizing a batch of `UnifiedRow`s/
/// `DirectiveRow`s — grouped by `session_id`, in deterministic
/// (sorted-by-session_id) order so two normalization passes over
/// unchanged input produce byte-identical output (S3 acceptance:
/// "identical normalized output across two runs").
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NormalizeOutcome {
    pub sessions: Vec<NormalizedSession>,
    /// Rows/directives dropped because their `session_id` failed
    /// `SessionId::parse`'s grammar check (design §7: skip + count,
    /// never crash). Always zero for adapters whose own `parse()`
    /// already guarantees a non-empty, control-char-free session id
    /// (e.g. omp/pi never emits a row without first validating its
    /// `session` header) — this is defense-in-depth against a future
    /// adapter that doesn't.
    pub skipped_rows: usize,
}

/// One seed for a session's merged `Event` stream (s31 design D4) —
/// either a `token_usage` row or a `user_directive` row, folded into
/// ONE deterministic order by [`normalize_session`].
enum EventSeed<'a> {
    TokenUsage(&'a UnifiedRow),
    UserDirective(&'a DirectiveRow),
}

impl EventSeed<'_> {
    fn timestamp_ms(&self) -> i64 {
        match self {
            EventSeed::TokenUsage(row) => row.timestamp_ms,
            EventSeed::UserDirective(directive) => directive.timestamp_ms,
        }
    }

    fn workspace(&self) -> (Option<String>, Option<String>) {
        match self {
            EventSeed::TokenUsage(row) => (row.workspace_key.clone(), row.workspace_label.clone()),
            EventSeed::UserDirective(directive) => (directive.workspace_key.clone(), directive.workspace_label.clone()),
        }
    }
}

/// Normalize a batch of `UnifiedRow`s (from any number of adapters/
/// files) into one [`NormalizedSession`] per distinct `session_id`.
/// Equivalent to [`normalize`] with an empty directive slice — kept as
/// its own entry point for every pre-s31 call site that has no
/// `DirectiveRow`s to fold in.
///
/// **Cross-file `dedup_key` consumption (ReviewS3Full finding 2,
/// fork-dedup fix)**: before grouping-by-`session_id` ever runs, rows
/// are deduped by `dedup_key` — PORTED consuming-side pattern from the
/// donor's `should_keep_deduped_message` + its per-source
/// `_seen: HashSet<String>` sets
/// (`codex_seen`/`hermes_seen`/etc. at each call site): a row whose
/// `dedup_key` was already seen for that `client` is dropped (first
/// occurrence, in scan order, wins — never merged); a row with
/// `dedup_key: None` always survives. Scoped per-`client`, matching
/// the donor's separate `_seen` set per source, since dedup-key
/// GRAMMAR is adapter-specific (never cross-adapter comparable) even
/// though every shipped adapter's format happens to be
/// self-namespaced already.
///
/// This closes the fork/replay double-count gap grouping-by-
/// `session_id` ALONE cannot: Codex's fork-scoped dedup_key
/// (`adapters::codex::set_codex_dedup_key`) is keyed on the FORK
/// PARENT identity, not the row's own (filename-derived) surface
/// `session_id` — so two sibling fork/replay files, each with a
/// DIFFERENT `session_id` but the SAME parent-scoped `dedup_key`,
/// would previously both survive grouping and both get summed,
/// double-counting the replayed parent history. Deduping here, before
/// grouping, collapses them to the single kept row regardless of
/// which `session_id` each carries.
pub fn normalize_rows(rows: &[UnifiedRow]) -> NormalizeOutcome {
    normalize(rows, &[])
}

/// The full normalization entry point (s31 design D4): [`normalize_rows`]'s
/// `UnifiedRow` grouping/dedup PLUS a `DirectiveRow` stream, unioned by
/// `session_id` (a session with directives but zero billable rows yet —
/// e.g. a human turn parsed before its assistant reply — still gets a
/// `Session`/`Run` and its directive events, never silently dropped)
/// and folded into each session's `events` in ONE deterministic `seq`
/// order (`normalize_session`'s merge-then-stable-sort).
pub fn normalize(rows: &[UnifiedRow], directives: &[DirectiveRow]) -> NormalizeOutcome {
    // BTreeMap, not HashMap: deterministic (lexical session_id) fold
    // order — the same reason canon-store's own registry.rs sorts its
    // aging-report iteration instead of trusting HashMap order.
    let mut by_session_rows: BTreeMap<String, Vec<&UnifiedRow>> = BTreeMap::new();
    let mut skipped_rows = 0usize;
    let mut seen_dedup_keys: HashMap<&str, HashSet<&str>> = HashMap::new();

    for row in rows {
        if let Some(dedup_key) = row.dedup_key.as_deref() {
            let first_occurrence = seen_dedup_keys.entry(row.client.as_str()).or_default().insert(dedup_key);
            if !first_occurrence {
                // Already counted under an earlier row sharing this
                // client + dedup_key — a source-level replay/
                // duplicate collapse, not a malformed-row violation,
                // so it is NOT added to `skipped_rows`.
                continue;
            }
        }

        match SessionId::parse(row.session_id.clone()) {
            Ok(_) => by_session_rows.entry(row.session_id.clone()).or_default().push(row),
            Err(_) => skipped_rows += 1,
        }
    }

    let mut by_session_directives: BTreeMap<String, Vec<&DirectiveRow>> = BTreeMap::new();
    for directive in directives {
        match SessionId::parse(directive.session_id.clone()) {
            Ok(_) => by_session_directives.entry(directive.session_id.clone()).or_default().push(directive),
            Err(_) => skipped_rows += 1,
        }
    }

    let session_ids: BTreeSet<&String> = by_session_rows.keys().chain(by_session_directives.keys()).collect();
    let empty_rows: Vec<&UnifiedRow> = Vec::new();
    let empty_directives: Vec<&DirectiveRow> = Vec::new();

    let sessions = session_ids
        .into_iter()
        .filter_map(|session_id| {
            let rows = by_session_rows.get(session_id).unwrap_or(&empty_rows);
            let directives = by_session_directives.get(session_id).unwrap_or(&empty_directives);
            normalize_session(session_id, rows, directives)
        })
        .collect();

    NormalizeOutcome { sessions, skipped_rows }
}

fn normalize_session(session_id_str: &str, rows: &[&UnifiedRow], directives: &[&DirectiveRow]) -> Option<NormalizedSession> {
    if rows.is_empty() && directives.is_empty() {
        return None;
    }
    // Already validated by the caller (`normalize`); re-validating
    // here keeps this function callable independently (e.g. from a
    // future per-session incremental path) without re-threading the
    // outer skip-count bookkeeping.
    let session_id = SessionId::parse(session_id_str.to_string()).ok()?;
    let client = rows.first().map(|r| r.client.clone()).or_else(|| directives.first().map(|d| d.client.clone()))?;

    // Merge the two seed streams into ONE deterministic order —
    // s31 D4: "timestamp, then stable tiebreak so re-parse of a grown
    // file re-emits byte-identical earlier events". Pre-sort
    // concatenation is directives-then-rows (each already in its own
    // adapter scan/append order); `sort_by_key` is a STABLE sort, so
    // a tie resolves to that pre-sort relative order — a directive
    // wins a same-millisecond tie against a token_usage row (a human
    // turn logically precedes the assistant reply it triggers). A
    // growing file only ever appends LATER-timestamped seeds at the
    // END of this pre-sort vec, so the stable sort never reorders an
    // already-parsed earlier seed relative to another — exactly the
    // digest-dedup invariant this exists to hold.
    let mut seeds: Vec<EventSeed> = Vec::with_capacity(rows.len() + directives.len());
    seeds.extend(directives.iter().copied().map(EventSeed::UserDirective));
    seeds.extend(rows.iter().copied().map(EventSeed::TokenUsage));
    seeds.sort_by_key(EventSeed::timestamp_ms);

    let started_at_ms = seeds.first()?.timestamp_ms();
    let ended_at_ms = seeds.last()?.timestamp_ms();
    let started_at = millis_to_utc(started_at_ms);
    let ended_at = millis_to_utc(ended_at_ms);

    // Session-level workspace context (s31 D3): first non-`None`
    // across the merged, chronologically-sorted seed stream.
    let (workspace_key, workspace_label) = seeds.iter().map(EventSeed::workspace).find(|(key, _)| key.is_some()).unwrap_or((None, None));

    let session_actor = Actor::new_unattributed(client.clone()).with_session(session_id.clone());
    let mut session = Session::new(
        Envelope::new(SCHEMA_VERSION, RecordKind::Session, ended_at, session_actor),
        session_id.clone(),
        client.clone(),
        started_at,
        Some(ended_at),
    );
    session.workspace_key = workspace_key;
    session.workspace_label = workspace_label;

    let run_id = deterministic_run_id(&session_id, started_at_ms);
    let run_actor = Actor::new_unattributed(client.clone()).with_session(session_id.clone());
    let run = Run::new(
        Envelope::new(SCHEMA_VERSION, RecordKind::Run, ended_at, run_actor),
        run_id,
        Some(session_id.clone()),
        None,
        RunStatus::Succeeded,
        started_at,
        Some(ended_at),
    );

    let events = seeds
        .iter()
        .enumerate()
        .map(|(idx, seed)| {
            let seq = (idx + 1) as u64;
            let at = millis_to_utc(seed.timestamp_ms());
            match seed {
                EventSeed::TokenUsage(row) => {
                    let actor = Actor::new_unattributed(client.clone()).with_session(session_id.clone()).with_model(row.model_id.clone());
                    let detail = json!({
                        "provider_id": row.provider_id,
                        "workspace_key": row.workspace_key,
                        "workspace_label": row.workspace_label,
                        "tokens": {
                            "input": row.tokens.input,
                            "output": row.tokens.output,
                            "cache_read": row.tokens.cache_read,
                            "cache_write": row.tokens.cache_write,
                            "reasoning": row.tokens.reasoning,
                            "total": row.tokens.total(),
                        },
                        "cost": row.cost,
                        "cost_source": row.cost_source,
                        "duration_ms": row.duration_ms,
                        "dedup_key": row.dedup_key,
                        "is_turn_start": row.is_turn_start,
                    });
                    Event::new(Envelope::new(SCHEMA_VERSION, RecordKind::Event, at, actor), run_id, seq, TOKEN_USAGE_LABEL, detail)
                }
                EventSeed::UserDirective(directive) => {
                    let actor = Actor::new_unattributed(client.clone()).with_session(session_id.clone());
                    let detail = json!({
                        "text": directive.text,
                        "workspace_key": directive.workspace_key,
                        "workspace_label": directive.workspace_label,
                    });
                    Event::new(Envelope::new(SCHEMA_VERSION, RecordKind::Event, at, actor), run_id, seq, USER_DIRECTIVE_LABEL, detail)
                }
            }
        })
        .collect();

    Some(NormalizedSession { session, run, events })
}

fn millis_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

/// A `RunId` (ULID) deterministically derived from `session_id` +
/// `started_at_ms` — never `RunId::new()`'s random generator, whose
/// output would differ across two ingest runs and break the S3
/// "identical normalized output across two runs" acceptance bar.
/// `Ulid::from_parts(timestamp_ms, random)` (the crate's own
/// deterministic constructor) takes the session's start time as the
/// ULID's time component and a sha256-derived value (over
/// `session_id`) as the random component, so re-ingesting the same
/// session always yields the same `run_id`.
fn deterministic_run_id(session_id: &SessionId, started_at_ms: i64) -> RunId {
    let digest = Sha256::digest(session_id.as_str().as_bytes());
    let random = u128::from_be_bytes(digest[0..16].try_into().expect("sha256 digest is >= 16 bytes"));
    let ulid = ulid::Ulid::from_parts(started_at_ms.max(0) as u64, random);
    RunId::parse(ulid.to_string()).expect("Ulid::to_string always yields a valid RunId grammar")
}

/// Canonicalize a raw workspace path string: backslash -> slash,
/// collapse `//`, trim a trailing `/`, preserving a leading UNC (`\\`
/// or `//`) prefix. Ported verbatim from the donor's
/// `normalize_workspace_key`.
pub fn normalize_workspace_key(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let preserve_unc_prefix = trimmed.starts_with("\\\\") || trimmed.starts_with("//");
    let mut normalized = trimmed.replace('\\', "/");

    if preserve_unc_prefix {
        let body = normalized.trim_start_matches('/');
        let mut collapsed = body.to_string();
        while collapsed.contains("//") {
            collapsed = collapsed.replace("//", "/");
        }
        normalized = format!("//{collapsed}");
    } else {
        while normalized.contains("//") {
            normalized = normalized.replace("//", "/");
        }
    }

    let minimum_len = if preserve_unc_prefix { 2 } else { 1 };
    if normalized.len() > minimum_len {
        normalized = normalized.trim_end_matches('/').to_string();
    }

    if normalized.is_empty() { None } else { Some(normalized) }
}

/// The last non-empty path segment of an already-normalized workspace
/// key — ported verbatim from the donor's `workspace_label_from_key`.
pub fn workspace_label_from_key(key: &str) -> Option<String> {
    key.rsplit('/').find(|segment| !segment.is_empty()).map(|segment| segment.to_string())
}

/// A stable sha256-derived content digest over a normalized record's
/// canonical JSON — canon-ingest's OWN idempotence bookkeeping
/// (logging/dedup at the ingest layer itself, independent of and in
/// addition to `canon-store`'s own digest-suffixed Hive object keys,
/// which `canon-cli`'s `TierRegistry::persist` call already applies at
/// the storage layer). `serde_json::to_value` on any of this crate's
/// output types serializes `serde_json::Map` as a `BTreeMap` (no
/// `preserve_order` feature anywhere in this workspace — same
/// invariant `canon-store::partition::content_digest12` relies on), so
/// key order never perturbs the digest.
pub fn content_digest(value: &serde_json::Value) -> String {
    let canonical = serde_json::to_vec(value).expect("serde_json::Value always serializes");
    let digest = Sha256::digest(&canonical);
    digest.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{CostSource, TokenBreakdown};

    fn row(session_id: &str, ts_ms: i64) -> UnifiedRow {
        UnifiedRow {
            client: "omp".into(),
            model_id: "claude-sonnet-5".into(),
            provider_id: "anthropic".into(),
            session_id: session_id.into(),
            workspace_key: Some("/tmp/proj".into()),
            workspace_label: Some("proj".into()),
            timestamp_ms: ts_ms,
            tokens: TokenBreakdown { input: 10, output: 5, cache_read: 0, cache_write: 0, reasoning: 0 },
            cost: 0.0,
            cost_source: CostSource::Unknown,
            duration_ms: None,
            dedup_key: None,
            is_turn_start: false,
        }
    }

    fn directive(session_id: &str, ts_ms: i64, text: &str) -> DirectiveRow {
        DirectiveRow {
            client: "omp".into(),
            session_id: session_id.into(),
            timestamp_ms: ts_ms,
            text: text.into(),
            workspace_key: Some("/tmp/proj".into()),
            workspace_label: Some("proj".into()),
        }
    }

    #[test]
    fn groups_rows_by_session_id_in_sorted_order() {
        let rows = vec![row("ses_b", 2_000), row("ses_a", 1_000), row("ses_a", 1_500)];
        let outcome = normalize_rows(&rows);
        assert_eq!(outcome.skipped_rows, 0);
        assert_eq!(outcome.sessions.len(), 2);
        assert_eq!(outcome.sessions[0].session.session_id.as_str(), "ses_a");
        assert_eq!(outcome.sessions[0].events.len(), 2);
        assert_eq!(outcome.sessions[1].session.session_id.as_str(), "ses_b");
        assert_eq!(outcome.sessions[1].events.len(), 1);
    }

    /// ReviewS3Full finding 2 (CRITICAL, fork-dedup): two codex
    /// fork/replay files carry DIFFERENT (filename-derived)
    /// `session_id`s but the SAME fork-parent-scoped `dedup_key`
    /// (`adapters::codex::set_codex_dedup_key`'s actual output
    /// shape). Grouping by `session_id` alone would count the
    /// replayed parent history TWICE (once per sibling session); the
    /// cross-file dedup must collapse it to ONE row, kept under
    /// whichever session_id scanned first.
    #[test]
    fn codex_fork_replay_across_two_files_is_not_double_counted() {
        let shared_dedup_key = "codex:token_count-total:parent-session:openai:gpt-5.1:100:50:10:0";

        let mut child_a = row("codex-fork-child-a", 1_000);
        child_a.client = "codex".into();
        child_a.dedup_key = Some(shared_dedup_key.to_string());

        let mut child_b = row("codex-fork-child-b", 1_050);
        child_b.client = "codex".into();
        child_b.dedup_key = Some(shared_dedup_key.to_string());

        let rows = vec![child_a, child_b];
        let outcome = normalize_rows(&rows);

        assert_eq!(outcome.skipped_rows, 0, "a dedup collapse is not a malformed-row skip");
        assert_eq!(outcome.sessions.len(), 1, "the second sibling's replayed row must collapse away, not surface as a second session");
        assert_eq!(outcome.sessions[0].session.session_id.as_str(), "codex-fork-child-a", "first-scanned occurrence wins");
        assert_eq!(outcome.sessions[0].events.len(), 1);
        assert_eq!(outcome.sessions[0].events[0].detail["tokens"]["input"], 10, "the shared history is summed ONCE, not once per sibling file");
    }

    /// A `dedup_key` collision NEVER crosses `client` boundaries: two
    /// different adapters that happened to produce the same literal
    /// dedup_key string must both survive — dedup is scoped per
    /// `client`, matching the donor's separate `_seen` set per source.
    #[test]
    fn dedup_key_collision_across_different_clients_does_not_collapse() {
        let mut codex_row = row("codex-session", 1_000);
        codex_row.client = "codex".into();
        codex_row.dedup_key = Some("shared-literal-key".to_string());

        let mut hermes_row = row("hermes-session", 2_000);
        hermes_row.client = "hermes".into();
        hermes_row.dedup_key = Some("shared-literal-key".to_string());

        let rows = vec![codex_row, hermes_row];
        let outcome = normalize_rows(&rows);

        assert_eq!(outcome.sessions.len(), 2, "same dedup_key string under DIFFERENT clients must not collapse");
    }

    #[test]
    fn normalization_is_deterministic_across_two_runs() {
        let rows = vec![row("ses_a", 1_000), row("ses_a", 2_000)];
        let first = normalize_rows(&rows);
        let second = normalize_rows(&rows);
        let first_json = serde_json::to_value(&first.sessions[0].run).unwrap();
        let second_json = serde_json::to_value(&second.sessions[0].run).unwrap();
        assert_eq!(first_json, second_json);
        assert_eq!(content_digest(&first_json), content_digest(&second_json));
    }

    #[test]
    fn workspace_key_normalizer_matches_donor_behavior() {
        assert_eq!(normalize_workspace_key(r"C:\repo\proj\\"), Some("C:/repo/proj".to_string()));
        assert_eq!(normalize_workspace_key("  "), None);
        assert_eq!(normalize_workspace_key("//server/share//sub/"), Some("//server/share/sub".to_string()));
        assert_eq!(workspace_label_from_key("/tmp/proj"), Some("proj".to_string()));
    }

    #[test]
    fn directive_and_token_usage_events_merge_in_timestamp_order() {
        let rows = vec![row("ses_a", 2_000)];
        let directives = vec![directive("ses_a", 1_000, "please add a retry")];
        let outcome = normalize(&rows, &directives);

        assert_eq!(outcome.skipped_rows, 0);
        assert_eq!(outcome.sessions.len(), 1);
        let events = &outcome.sessions[0].events;
        assert_eq!(events.len(), 2, "one directive + one token_usage row must merge into ONE event stream");
        assert_eq!(events[0].seq, 1);
        assert_eq!(events[0].label, USER_DIRECTIVE_LABEL);
        assert_eq!(events[0].detail["text"], "please add a retry");
        assert_eq!(events[1].seq, 2);
        assert_eq!(events[1].label, TOKEN_USAGE_LABEL);
    }

    #[test]
    fn same_millisecond_tie_resolves_directive_before_token_usage() {
        let rows = vec![row("ses_a", 1_000)];
        let directives = vec![directive("ses_a", 1_000, "same-millisecond directive")];
        let outcome = normalize(&rows, &directives);

        let events = &outcome.sessions[0].events;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].label, USER_DIRECTIVE_LABEL, "a same-millisecond tie must resolve directive-before-token (the human turn precedes the reply it triggers)");
        assert_eq!(events[0].seq, 1);
        assert_eq!(events[1].label, TOKEN_USAGE_LABEL);
        assert_eq!(events[1].seq, 2);
    }

    #[test]
    fn directive_only_session_still_produces_a_session_and_run() {
        let directives = vec![directive("ses_directive_only", 500, "hello, are you there?")];
        let outcome = normalize(&[], &directives);

        assert_eq!(outcome.sessions.len(), 1, "a session with a human turn but no billable row yet must not be silently dropped");
        let session = &outcome.sessions[0];
        assert_eq!(session.session.session_id.as_str(), "ses_directive_only");
        assert_eq!(session.session.client, "omp");
        assert_eq!(session.events.len(), 1);
        assert_eq!(session.events[0].label, USER_DIRECTIVE_LABEL);
        assert_eq!(session.events[0].detail["text"], "hello, are you there?");
        assert_eq!(session.events[0].detail["workspace_key"], "/tmp/proj");
        assert_eq!(session.events[0].detail["workspace_label"], "proj");
    }

    /// s31 D4's digest-dedup invariant: re-parsing a GROWN file (more
    /// rows/directives appended at later timestamps) must re-emit the
    /// exact same seq/content for every already-seen earlier event —
    /// otherwise canon-store's content-digest dedup would treat an
    /// unchanged earlier record as a brand-new write on every pass.
    #[test]
    fn growing_file_reparse_reemits_byte_identical_earlier_events() {
        let first_rows = vec![row("ses_a", 2_000)];
        let first_directives = vec![directive("ses_a", 1_000, "first ask")];
        let first = normalize(&first_rows, &first_directives);
        let first_events = &first.sessions[0].events;
        assert_eq!(first_events.len(), 2);

        // The file "grows": a later user turn and its reply are
        // appended, both timestamped AFTER everything already parsed.
        let grown_rows = vec![row("ses_a", 2_000), row("ses_a", 4_000)];
        let grown_directives = vec![directive("ses_a", 1_000, "first ask"), directive("ses_a", 3_000, "second ask")];
        let grown = normalize(&grown_rows, &grown_directives);
        let grown_events = &grown.sessions[0].events;
        assert_eq!(grown_events.len(), 4);

        for idx in 0..2 {
            let before = serde_json::to_value(&first_events[idx]).unwrap();
            let after = serde_json::to_value(&grown_events[idx]).unwrap();
            assert_eq!(before, after, "earlier event at index {idx} must stay byte-identical after the file grows");
        }
        assert_eq!(grown_events[2].label, USER_DIRECTIVE_LABEL);
        assert_eq!(grown_events[2].detail["text"], "second ask");
        assert_eq!(grown_events[3].label, TOKEN_USAGE_LABEL);
    }

    #[test]
    fn session_workspace_key_is_the_first_non_none_seed_in_chronological_order() {
        let mut earliest_row = row("ses_a", 1_000);
        earliest_row.workspace_key = None;
        earliest_row.workspace_label = None;
        let later_directive = directive("ses_a", 2_000, "hi");

        let outcome = normalize(&[earliest_row], &[later_directive]);
        let session = &outcome.sessions[0].session;
        assert_eq!(session.workspace_key.as_deref(), Some("/tmp/proj"), "the earliest seed has no workspace, so the next chronological seed's workspace wins");
        assert_eq!(session.workspace_label.as_deref(), Some("proj"));
        assert_eq!(session.project_key, None, "project_key is never derived inside canon-ingest — s31 design D3");
    }

    #[test]
    fn directive_with_invalid_session_id_is_skipped_and_counted() {
        // `SessionId::parse` rejects leading/trailing whitespace
        // (`is_session_id`'s `s.trim() == s` check) — a leading space
        // here is the malformed-grammar fixture.
        let directives = vec![directive(" leading-space-session", 1_000, "text")];
        let outcome = normalize(&[], &directives);
        assert!(outcome.sessions.is_empty());
        assert_eq!(outcome.skipped_rows, 1);
    }
}
