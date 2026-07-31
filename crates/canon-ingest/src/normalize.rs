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

/// The `Event.label` every normalized token/cost row carries.
pub const TOKEN_USAGE_LABEL: &str = "token_usage";

/// The `Event.label` every normalized user-directive row carries (s31
/// design D4).
pub const USER_DIRECTIVE_LABEL: &str = "user_directive";

/// One session's worth of normalized canon-model output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NormalizedSession {
    pub session: Session,
    /// The session's ROOT run — the main agent's own run, always
    /// `parent_run_id: None`. Stays a single non-optional field (not
    /// folded into `child_runs`) because every pre-
    /// s37-execution-graph-topology consumer reads exactly this one run
    /// per session, and the root's derivation is unchanged: same
    /// `deterministic_run_id(session_id, None, started_at_ms)`, same
    /// session-wide `started_at`/`ended_at`, same events pointing at
    /// it.
    pub run: Run,
    /// One child run per distinct `UnifiedRow::agent_id`/
    /// `DirectiveRow::agent_id` present in this session
    /// (s37-execution-graph-topology), each with `parent_run_id`
    /// pointing at its dispatcher's run — another child's when
    /// `parent_agent_id` names a sibling agent (a nested subagent),
    /// [`Self::run`]'s otherwise. Sorted by `agent_id`, so two
    /// normalization passes over unchanged input emit them in
    /// byte-identical order, the same determinism bar
    /// [`NormalizeOutcome`] holds for sessions.
    ///
    /// EMPTY for a plain single-agent session — the overwhelmingly
    /// common case, whose normalized output is unchanged by this field
    /// existing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub child_runs: Vec<Run>,
    pub events: Vec<Event>,
}

impl NormalizedSession {
    /// Every run this session normalized to, root first then each
    /// child in `child_runs` order — the walk a topology consumer
    /// (e.g. the plan-vs-actual graph diff) wants, so it never has to
    /// remember that the root lives outside the child vec.
    pub fn runs(&self) -> impl Iterator<Item = &Run> {
        std::iter::once(&self.run).chain(self.child_runs.iter())
    }
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

    /// This seed's `(agent_id, parent_agent_id)` execution-lineage
    /// pair (s37-execution-graph-topology) — `(None, None)` for every
    /// row of a plain single-agent session. Borrowed, not cloned:
    /// [`normalize_session`]'s agent fold only needs to key on these.
    fn lineage(&self) -> (Option<&str>, Option<&str>) {
        match self {
            EventSeed::TokenUsage(row) => (row.agent_id.as_deref(), row.parent_agent_id.as_deref()),
            EventSeed::UserDirective(directive) => (directive.agent_id.as_deref(), directive.parent_agent_id.as_deref()),
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
        Envelope::current(RecordKind::Session, ended_at, session_actor),
        session_id.clone(),
        client.clone(),
        started_at,
        Some(ended_at),
    );
    session.workspace_key = workspace_key;
    session.workspace_label = workspace_label;

    let run_id = deterministic_run_id(&session_id, None, started_at_ms);
    let run_actor = Actor::new_unattributed(client.clone()).with_session(session_id.clone());
    let run = Run::new(
        Envelope::current(RecordKind::Run, ended_at, run_actor),
        run_id,
        Some(session_id.clone()),
        None,
        RunStatus::Succeeded,
        started_at,
        Some(ended_at),
    );

    let child_runs = child_runs_for_agents(&seeds, &session_id, &client, run_id);

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
                    Event::new(Envelope::current(RecordKind::Event, at, actor), run_id, seq, TOKEN_USAGE_LABEL, detail)
                }
                EventSeed::UserDirective(directive) => {
                    let actor = Actor::new_unattributed(client.clone()).with_session(session_id.clone());
                    let detail = json!({
                        "text": directive.text,
                        "workspace_key": directive.workspace_key,
                        "workspace_label": directive.workspace_label,
                    });
                    Event::new(Envelope::current(RecordKind::Event, at, actor), run_id, seq, USER_DIRECTIVE_LABEL, detail)
                }
            }
        })
        .collect();

    Some(NormalizedSession { session, run, child_runs, events })
}

/// Build one child [`Run`] per distinct `agent_id` in this session's
/// merged seed stream (s37-execution-graph-topology), reconstructing
/// the dispatch tree an adapter recorded via
/// `agent_id`/`parent_agent_id`.
///
/// Returns an EMPTY vec for a plain single-agent session (every seed's
/// `agent_id` is `None`) — the overwhelmingly common case, which must
/// normalize to exactly the one root run it did before this function
/// existed.
///
/// Determinism, the same bar [`NormalizeOutcome`] holds: the fold is a
/// `BTreeMap` keyed by `agent_id`, so the emitted order is lexical by
/// agent id rather than seed-arrival order, and each child's `run_id`
/// comes from [`deterministic_run_id`] — re-normalizing an unchanged
/// transcript yields byte-identical child runs, ids included.
///
/// `parent_run_id` resolution: `parent_agent_id` naming another
/// `agent_id` present in THIS session is a nested dispatch, so it
/// resolves to that sibling's child run; anything else (the parent
/// session's own id — what Claude Code's sidechain lines carry — or an
/// absent/unknown parent) resolves to `root_run_id`. An agent whose
/// dispatcher is not in this session is still a child of the session's
/// main agent, never a second root.
fn child_runs_for_agents(seeds: &[EventSeed<'_>], session_id: &SessionId, client: &str, root_run_id: RunId) -> Vec<Run> {
    /// One agent's accumulated span + declared dispatcher.
    struct AgentSpan<'a> {
        started_at_ms: i64,
        ended_at_ms: i64,
        /// First non-`None` `parent_agent_id` seen for this agent, in
        /// seed order — an adapter emits the same dispatcher on every
        /// row of one agent, so a later disagreement is source
        /// corruption, not a re-parent, and the first value wins.
        parent_agent_id: Option<&'a str>,
    }

    let mut spans: BTreeMap<&str, AgentSpan<'_>> = BTreeMap::new();
    for seed in seeds {
        let (Some(agent_id), parent_agent_id) = seed.lineage() else { continue };
        let ts = seed.timestamp_ms();
        let span = spans.entry(agent_id).or_insert(AgentSpan { started_at_ms: ts, ended_at_ms: ts, parent_agent_id });
        // No-ops on the just-inserted case, which is why one `or_insert`
        // replaces an `and_modify`/`or_insert` pair: `ts` is already
        // both bounds and `parent_agent_id` is already itself.
        span.started_at_ms = span.started_at_ms.min(ts);
        span.ended_at_ms = span.ended_at_ms.max(ts);
        span.parent_agent_id = span.parent_agent_id.or(parent_agent_id);
    }

    if spans.is_empty() {
        return Vec::new();
    }

    // Every child's id is derivable from (session_id, agent_id, its own
    // start), so a nested dispatch's parent link needs no ordering
    // between siblings — resolve each independently rather than
    // topologically sorting a tree canon never executes.
    let child_run_ids: BTreeMap<&str, RunId> =
        spans.iter().map(|(agent_id, span)| (*agent_id, deterministic_run_id(session_id, Some(*agent_id), span.started_at_ms))).collect();

    spans
        .iter()
        .map(|(agent_id, span)| {
            let parent_run_id = span.parent_agent_id.and_then(|parent| child_run_ids.get(parent)).copied().unwrap_or(root_run_id);
            let actor = Actor::new_unattributed(client).with_session(session_id.clone());
            let ended_at = millis_to_utc(span.ended_at_ms);
            Run::new(
                Envelope::current(RecordKind::Run, ended_at, actor),
                child_run_ids[*agent_id],
                Some(session_id.clone()),
                None,
                // Same hardcoded terminal status the root run carries:
                // a transcript on disk is a run that already finished,
                // and this layer has no per-agent failure signal to
                // distinguish `Failed` from `Succeeded` with.
                RunStatus::Succeeded,
                millis_to_utc(span.started_at_ms),
                Some(ended_at),
            )
            .with_parent_run_id(parent_run_id)
        })
        .collect()
}

fn millis_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

/// A `RunId` (ULID) deterministically derived from `session_id` (+ an
/// optional `agent_id`) and `started_at_ms` — never `RunId::new()`'s
/// random generator, whose output would differ across two ingest runs
/// and break the S3 "identical normalized output across two runs"
/// acceptance bar. `Ulid::from_parts(timestamp_ms, random)` (the
/// crate's own deterministic constructor) takes the run's start time as
/// the ULID's time component and a sha256-derived value as the random
/// component, so re-ingesting the same session always yields the same
/// `run_id`.
///
/// `agent_id` (s37-execution-graph-topology) separates a subagent's
/// child run from its session's root run. `None` hashes the session id
/// ALONE — byte-for-byte the pre-s37 digest input — so every existing
/// root `run_id` on disk is unchanged; `Some` appends a NUL separator
/// before the agent id, a delimiter neither id's grammar admits, so no
/// (session, agent) pair can collide with another's concatenation.
fn deterministic_run_id(session_id: &SessionId, agent_id: Option<&str>, started_at_ms: i64) -> RunId {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_str().as_bytes());
    if let Some(agent_id) = agent_id {
        hasher.update(b"\0");
        hasher.update(agent_id.as_bytes());
    }
    let digest = hasher.finalize();
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
            agent_id: None,
            parent_agent_id: None,
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
            agent_id: None,
            parent_agent_id: None,
        }
    }

    /// A [`row`] re-attributed to a dispatched subagent
    /// (s37-execution-graph-topology): same `session_id` — the
    /// dispatch and its subagents are ONE session, exactly what
    /// `adapters::claude`'s sidechain override produces — plus the
    /// agent-lineage pair the child-run fold keys on.
    fn agent_row(session_id: &str, ts_ms: i64, agent_id: &str, parent_agent_id: &str) -> UnifiedRow {
        UnifiedRow { agent_id: Some(agent_id.into()), parent_agent_id: Some(parent_agent_id.into()), ..row(session_id, ts_ms) }
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

    /// s37-execution-graph-topology, THE regression that matters most:
    /// a plain single-agent session (every row's `agent_id` is `None`,
    /// which is what omp/codex/hermes and every non-sidechain Claude
    /// transcript emit) must still normalize to EXACTLY one run, with
    /// no parent and no child runs — and that run must reserialize
    /// WITHOUT a `parent_run_id` key, so an already-persisted pre-s37
    /// run record's bytes (and its content digest) are untouched.
    #[test]
    fn a_single_agent_session_still_normalizes_to_exactly_one_parentless_run() {
        let rows = vec![row("ses_solo", 1_000), row("ses_solo", 2_000)];
        let outcome = normalize_rows(&rows);

        assert_eq!(outcome.sessions.len(), 1);
        let session = &outcome.sessions[0];
        assert!(session.child_runs.is_empty(), "no agent_id anywhere means no child runs at all");
        assert_eq!(session.runs().count(), 1, "the root run is the WHOLE run set for a single-agent session");
        assert_eq!(session.run.parent_run_id, None);

        let json = serde_json::to_value(&session.run).unwrap();
        assert!(json.get("parent_run_id").is_none(), "a root run must not introduce a spurious `parent_run_id` key");
    }

    /// s37-execution-graph-topology: rows carrying an `agent_id` (what
    /// `adapters::claude` now emits for a sidechain transcript) become
    /// one child run per distinct agent, parented to the session's root
    /// run — while session grouping and the token/cost event stream stay
    /// exactly where they were: ONE session, every event still keyed to
    /// the root run.
    #[test]
    fn subagent_rows_become_child_runs_parented_to_the_session_root_run() {
        let rows = vec![
            row("ses_dispatch", 1_000),
            agent_row("ses_dispatch", 2_000, "sub-b", "ses_dispatch"),
            agent_row("ses_dispatch", 3_000, "sub-a", "ses_dispatch"),
            agent_row("ses_dispatch", 4_000, "sub-a", "ses_dispatch"),
        ];
        let outcome = normalize_rows(&rows);

        assert_eq!(outcome.sessions.len(), 1, "a dispatch and its subagents are ONE session");
        let session = &outcome.sessions[0];
        assert_eq!(session.events.len(), 4, "every row still contributes exactly one token_usage event");
        for event in &session.events {
            assert_eq!(event.run_id, session.run.run_id, "token/cost attribution stays on the root run, unchanged by lineage capture");
        }

        assert_eq!(session.child_runs.len(), 2);
        assert_eq!(session.runs().count(), 3, "root + one child per subagent");
        let starts: Vec<i64> = session.child_runs.iter().map(|child| child.started_at.timestamp_millis()).collect();
        assert_eq!(starts, vec![3_000, 2_000], "children are emitted in lexical agent_id order (sub-a then sub-b), never seed-arrival order");

        for child in &session.child_runs {
            assert_eq!(child.parent_run_id, Some(session.run.run_id), "parent_agent_id naming the session itself resolves to the root run");
            assert_eq!(child.session_id.as_ref().map(SessionId::as_str), Some("ses_dispatch"), "a child run stays on its parent's session — grouping is untouched");
            assert_eq!(child.status, RunStatus::Succeeded);
        }

        let child_ids: BTreeSet<RunId> = session.child_runs.iter().map(|child| child.run_id).collect();
        assert_eq!(child_ids.len(), 2, "two distinct agent_ids must mint two distinct run ids");
        assert!(!child_ids.contains(&session.run.run_id), "a child run id must never collide with its root's");

        // `sub-a` has two rows (3000, 4000); its span must cover both,
        // not inherit the session's 1000..4000.
        let sub_a = session.child_runs.iter().find(|child| child.started_at.timestamp_millis() == 3_000).expect("sub-a child run");
        assert_eq!(sub_a.ended_at.map(|at| at.timestamp_millis()), Some(4_000), "a child's span is min/max over ITS OWN rows");
    }

    /// s37-execution-graph-topology: a nested dispatch (a subagent that
    /// itself dispatched a subagent) parents to the SIBLING child run
    /// its `parent_agent_id` names, not to the session root — otherwise
    /// a depth-3 tree flattens to depth 2 and the topology is still lost.
    #[test]
    fn a_nested_subagent_parents_to_its_dispatching_subagent_not_the_root() {
        let rows = vec![
            row("ses_nested", 1_000),
            agent_row("ses_nested", 2_000, "sub-outer", "ses_nested"),
            agent_row("ses_nested", 3_000, "sub-inner", "sub-outer"),
        ];
        let outcome = normalize_rows(&rows);
        let session = &outcome.sessions[0];

        let outer = session.child_runs.iter().find(|child| child.started_at.timestamp_millis() == 2_000).expect("sub-outer child run");
        let inner = session.child_runs.iter().find(|child| child.started_at.timestamp_millis() == 3_000).expect("sub-inner child run");

        assert_eq!(outer.parent_run_id, Some(session.run.run_id));
        assert_eq!(inner.parent_run_id, Some(outer.run_id), "parent_agent_id naming a sibling agent must resolve to that sibling's run");
    }

    /// s37-execution-graph-topology: an `agent_id` whose
    /// `parent_agent_id` names nobody in this session (Claude Code's
    /// sidechain lines name the PARENT SESSION, which is never an
    /// agent_id) — or names nobody at all — is still the main agent's
    /// child, never a second root. A rootless child run would be
    /// unreachable from the session's own run and drop silently out of
    /// any topology walk.
    #[test]
    fn a_subagent_with_an_unknown_dispatcher_falls_back_to_the_root_run() {
        let mut orphan = agent_row("ses_orphan", 2_000, "sub-x", "who-dispatched-me");
        let named_stranger = normalize_rows(&[row("ses_orphan", 1_000), orphan.clone()]);
        let session = &named_stranger.sessions[0];
        assert_eq!(session.child_runs[0].parent_run_id, Some(session.run.run_id));

        orphan.parent_agent_id = None;
        let no_parent_at_all = normalize_rows(&[row("ses_orphan", 1_000), orphan]);
        let session = &no_parent_at_all.sessions[0];
        assert_eq!(session.child_runs[0].parent_run_id, Some(session.run.run_id));
    }

    /// s37-execution-graph-topology idempotence: re-normalizing an
    /// unchanged multi-agent transcript must yield byte-identical child
    /// runs — ids, order, and parent links — or the watermark cursor
    /// would re-persist the same subagent as a NEW run on every ingest
    /// pass. The bar
    /// [`normalization_is_deterministic_across_two_runs`] holds for the
    /// root run, extended over the whole run set.
    #[test]
    fn re_normalizing_a_multi_agent_session_yields_identical_run_ids() {
        let rows = vec![
            row("ses_idem", 1_000),
            agent_row("ses_idem", 2_000, "sub-a", "ses_idem"),
            agent_row("ses_idem", 3_000, "sub-b", "sub-a"),
        ];
        let as_json = |outcome: &NormalizeOutcome| serde_json::to_value(outcome.sessions[0].runs().collect::<Vec<_>>()).expect("Run always serializes");

        let first = as_json(&normalize_rows(&rows));
        let second = as_json(&normalize_rows(&rows));

        assert!(first.as_array().is_some_and(|runs| runs.len() == 3), "sanity: this fixture must actually produce a root + 2 children, or it proves nothing");
        assert_eq!(first, second, "two passes over unchanged input must produce byte-identical root + child runs");
        assert_eq!(content_digest(&first), content_digest(&second));
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
