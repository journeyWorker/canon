//! `canon ingest sessions [--watch] [--home <dir>] [--all-workspaces]`:
//! resolve `canon-ingest`'s full adapter registry (`omp`/`pi`, Claude
//! Code, Codex, Hermes — `canon_ingest::registry`), scan + parse +
//! normalize every adapter's session transcripts into canon-model
//! `Session`/`Run`/`Event` records, and persist them through
//! canon-store's write path — `canon_store::registry::TierRegistry::
//! persist`, reached via `crate::tiers::build_lenient_tiers_for_kinds`
//! (s29 design D6 — this module builds ONLY the union of rungs
//! `session`/`run`/`event` actually route/age to, and a genuinely
//! malformed `canon.yaml` fails this command loud instead of silently
//! folding into the seam below). `canon-ingest` itself has no
//! `canon-store` dependency (pure scan/parse/normalize domain logic);
//! this module is the one place the two meet.
//!
//! **s31 D1 (file-granular watermark).** [`SourceCursor::diff`]
//! replaces the old all-or-nothing `source_unchanged` gate: each
//! present file is digested and classified independently, so a single
//! growing transcript among thousands re-parses ALONE (see [`run`]'s
//! own doc for the per-file loop). D1's own doc names why this is
//! sound for every registered adapter (each derives a session from
//! exactly one file) despite [`canon_store::cursor`]'s module doc
//! still describing source-granularity as the general-purpose default.
//!
//! **s40 4.1 (parse-versioned cursor identity) plus privacy generation.**
//! A cursor is keyed on [`session_source_cursor_id_with_privacy`] — the
//! adapter's `client_id`, [`canon_ingest::SessionAdapter::parse_version`],
//! and the exact metadata-only/capture mode and positive capture bound.
//! The gate compares byte-stable transcript digests, so either adapter
//! normalization or privacy policy changes move the identity and re-read
//! the source in full. The legacy parse-generation helper preserves the
//! v1 unsuffixed adapter id, but actual privacy-aware cursors deliberately
//! add their policy suffix so pre-boundary cursors are not reused.
//!
//! **s31 D3 (project scope).** [`ProjectScope`] resolves "this
//! project" as the repo's main `git worktree` root plus every linked
//! one (fail-soft to the repo root alone outside a git repo) and
//! scopes the scan to it BY DEFAULT — `--all-workspaces` restores the
//! machine-wide scan S3 originally shipped. See [`ProjectScope`]'s own
//! doc for the root-pruning (omp/Claude Code) vs row-filtering
//! (Codex/Hermes) split and why it never DECODES an encoded on-disk
//! directory name.
//!
//! **Privacy boundary.** Adapters may parse user-turn text into
//! `DirectiveRow` in memory, but after all adapters and scope filtering
//! the shared [`canon_ingest::normalize_with_privacy`] policy either
//! omits directive events (the default when
//! `ingest.sessions.privacy` is absent) or captures a positive,
//! Unicode-scalar-bounded prefix with redaction metadata. The source
//! transcript is never rewritten.
//!
//! **Documented seam**: when `canon.yaml` itself is missing/unreadable,
//! or the policy hasn't routed `session`/`run`/`event` yet, or a
//! NEEDED rung is configured but unreachable (`tiers.pg` configured
//! but `CANON_PG_DSN` unset, exactly the "NO cloud creds" case this
//! module's own offline tests exercise), `run()` returns candidates in
//! [`IngestOutcome::unwritten`] instead of persisting — the CLI prints
//! a metadata-only JSON summary rather than failing the whole ingest
//! pass. Directive text and raw event detail never reach that fallback.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use canon_ingest::adapter::{DirectiveRow, UnifiedRow};
use canon_ingest::normalize::{normalize_with_privacy, DirectivePrivacyPolicy, NormalizedSession, normalize_workspace_key};
use canon_ingest::{enumerate, registry, scanner};
use canon_model::envelope::RecordKind;
use canon_model::paths;
use canon_store::cursor::{CursorStore, SourceCursor, file_digest};
use canon_store::partition::content_digest12;
use canon_store::policy::BackendConfig;
use canon_store::registry::TierRegistry;
use canon_store::tier::StoreError;

use crate::tiers::{self, TierCliError};

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error(transparent)]
    Tiers(#[from] TierCliError),
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A PRESENT `canon.yaml` `ingest:` section is malformed or names
    /// an unknown source — fail loud rather than silently scanning
    /// default roots (which would ingest the wrong corpus).
    #[error("{0}")]
    Config(String),
}

/// Per-adapter scan counts for the run summary (task 5.2).
#[derive(Debug, Clone)]
pub struct AdapterSummary {
    pub client_id: &'static str,
    pub roots: Vec<PathBuf>,
    /// Every present file this source matched THIS pass, after s31 D3
    /// root pruning (never includes a pruned-out cwd-partitioned
    /// subdirectory's files) but BEFORE the s31 D1 per-file gate —
    /// `reparsed + skipped_unchanged` whenever every present file was
    /// readable (a read failure excludes a file from both).
    pub files_scanned: usize,
    /// Files actually (re-)parsed this pass — s31 D1's per-file diff:
    /// new files, or files whose digest changed since the persisted
    /// cursor. `0` on a fully steady-state pass. See
    /// [`canon_store::cursor::SourceCursor::diff`].
    pub reparsed: usize,
    pub rows_parsed: usize,
    /// This adapter's sum of `ParseOutcome::skipped` across every
    /// (re-)parsed file (Wave-2 amendment, ReviewS3Full finding 3) —
    /// corrupt JSON lines / malformed dbs the adapter hit but could
    /// not extract a row from.
    pub malformed_records: usize,
    /// Files skipped by the s31 D1 per-file watermark gate because
    /// THAT FILE's digest was byte-identical to its persisted cursor
    /// entry (never handed to parse). Supersedes S3 §3's source-
    /// granular all-or-nothing count of the same name — see
    /// [`canon_store::cursor::SourceCursor::diff`].
    pub skipped_unchanged: usize,
}

/// One `canon ingest sessions` pass's outcome.
#[derive(Debug, Clone)]
pub struct IngestOutcome {
    pub adapters: Vec<AdapterSummary>,
    /// The active s31 D3 project scope, human-readable — either
    /// `"project <key> (N roots)"` or `"all workspaces"`
    /// (`--all-workspaces`). See [`ProjectScope::summary`].
    pub scope_summary: String,
    pub sessions_normalized: usize,
    pub runs_written: usize,
    pub events_written: usize,
    /// Rows dropped for a malformed/unparseable `session_id` (design
    /// §7: skip + count, never crash) — see
    /// `canon_ingest::NormalizeOutcome::skipped_rows`.
    pub skipped_rows: usize,
    /// Sum of every adapter's `AdapterSummary::malformed_records`
    /// (Wave-2 amendment, ReviewS3Full finding 3) — the "malformed
    /// adapter record is skipped as a violation" scenario the s3
    /// `session-adapter-registry` spec requires the run summary to
    /// count (`openspec/changes/s3-session-ingest/specs/
    /// session-adapter-registry/spec.md`'s "Malformed adapter record"
    /// scenario). Parse-level (corrupt line/db), NOT the same
    /// violation category as `skipped_rows` (normalize-level,
    /// malformed session_id) above.
    pub malformed_records: usize,
    /// `Some(sessions)` when the store's tiers weren't reachable (see
    /// module doc's "documented seam") — the caller prints these as
    /// JSON instead. `None` means every normalized record was
    /// persisted through canon-store.
    pub unwritten: Option<Vec<NormalizedSession>>,
    /// Stable class for the metadata-only degraded JSON seam.
    pub degrade_class: Option<&'static str>,
    /// `Some(reason)` when [`Self::unwritten`] is `Some` because a
    /// NEEDED rung (`session`/`run`/`event`'s routed or aged-to rung)
    /// was ATTEMPTED and degraded (s29 design D6). The reason preserves
    /// a safe configured env-var name when that is the failure class, but
    /// collapses live backend diagnostics to a generic availability reason
    /// so DSNs, usernames, and passwords never enter fallback metadata.
    /// `None` when [`Self::unwritten`] is `None` too, or when the degrade
    /// has no single specific rung reason (canon.yaml missing/unreadable,
    /// or `session`/`run`/`event` simply not routed yet) — those cases keep
    /// [`format_human`]'s generic fallback prose.
    pub degrade_reason: Option<String>,
}

/// Optional per-source scan-root overrides parsed from `canon.yaml`'s
/// `ingest.sources.<client_id>.roots` (S3 task 1.2). A source with a
/// PRESENT `roots` field scans EXACTLY those roots (relative paths
/// resolved against the `canon.yaml` directory, absolute as-is) — an
/// explicit `roots: []` scans ZERO roots, it never falls back. A
/// source whose key OR `roots` field is ABSENT falls back to the
/// adapter's OWN env-override + documented-default resolution
/// (`SessionAdapter::scan_roots`). This override lives entirely at the
/// canon-cli layer — parsing is still the adapter's (`parse_files`),
/// so the `SessionAdapter` contract itself is untouched (S3 froze it
/// at three methods; s40 added `parse_version`, and this override
/// touches neither).
///
/// Fail-soft ONLY for a missing / unreadable / non-YAML `canon.yaml`,
/// or one with no `ingest:` section (→ zero overrides). A PRESENT
/// `ingest:` section is parsed STRICTLY (`deny_unknown_fields`) and
/// fails loud on a typo (`root:` for `roots:`) or an unknown source id
/// (`claude` for `claude-code`): a silent fallback there would scan
/// the adapter's default home roots and ingest the wrong corpus.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIngest {
    #[serde(default)]
    sources: BTreeMap<String, RawIngestSource>,
    #[serde(default)]
    sessions: RawIngestSessions,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIngestSessions {
    #[serde(default)]
    privacy: RawDirectivePrivacy,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDirectivePrivacy {
    #[serde(default)]
    capture_user_directives: bool,
    #[serde(default)]
    max_directive_chars: Option<usize>,
}

impl Default for RawDirectivePrivacy {
    fn default() -> Self {
        Self { capture_user_directives: false, max_directive_chars: None }
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIngestSource {
    /// `None` = field absent (→ default resolution); `Some(vec)` =
    /// explicit override, including `Some([])` (scan zero roots).
    #[serde(default)]
    roots: Option<Vec<PathBuf>>,
}

struct IngestSourceConfig {
    /// Only sources whose `roots` field was PRESENT; an absent key or
    /// absent `roots` is omitted so its `.get()` misses and the source
    /// falls back to default resolution.
    sources: BTreeMap<String, Vec<PathBuf>>,
    /// The one policy applied after every adapter has parsed.
    privacy: DirectivePrivacyPolicy,
}

impl IngestSourceConfig {
    /// Parse `canon.yaml`'s `ingest.sources` (see the struct doc for
    /// the fail-soft vs fail-loud split). Unknown TOP-level keys
    /// (`tiers`/`routing`/…) are ignored — only the `ingest:` subtree
    /// is parsed — so this coexists with the `TierPolicy` parse.
    fn load(canon_yaml: &Path) -> Result<Self, IngestError> {
        let empty = || Self { sources: BTreeMap::new(), privacy: DirectivePrivacyPolicy::METADATA_ONLY };
        // Missing / unreadable canon.yaml: fail-soft (no config at all
        // is a legitimate first-run / minimal state).
        let Ok(text) = std::fs::read_to_string(canon_yaml) else {
            return Ok(empty());
        };
        // A PRESENT but non-YAML canon.yaml fails LOUD: `run()` swallows
        // a later `build_tiers` YAML error into the unwritten/JSON seam,
        // so a syntax typo in a canon.yaml meant to set `ingest.sources`
        // would otherwise silently scan default home roots.
        let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| {
            IngestError::Config(format!(
                "canon.yaml is not valid YAML (fail-loud so an intended \
                 `ingest.sources` override is never silently dropped to \
                 default roots): {e}"
            ))
        })?;
        let Some(ingest_val) = doc.get("ingest") else {
            return Ok(empty());
        };
        // PRESENT `ingest:` — strict from here on.
        let ingest: RawIngest = serde_yaml::from_value(ingest_val.clone()).map_err(|e| {
            IngestError::Config(format!(
                "canon.yaml `ingest:` section is malformed (fail-loud — a \
                 silent fallback would scan default roots and ingest the \
                 wrong corpus): {e}"
            ))
        })?;
        let known: BTreeSet<&str> = registry().iter().map(|e| e.client_id()).collect();
        if let Some(id) = ingest.sources.keys().find(|id| !known.contains(id.as_str())) {
            let mut names: Vec<&str> = known.into_iter().collect();
            names.sort_unstable();
            return Err(IngestError::Config(format!(
                "canon.yaml `ingest.sources.{id}` names no registered adapter \
                 (known: {}); fix the source id or remove the entry",
                names.join(", ")
            )));
        }
        let privacy = if ingest.sessions.privacy.capture_user_directives {
            let max = ingest.sessions.privacy.max_directive_chars.ok_or_else(|| {
                IngestError::Config(
                    "canon.yaml `ingest.sessions.privacy.max_directive_chars` is required and positive when `capture_user_directives: true`".to_string(),
                )
            })?;
            if max == 0 {
                return Err(IngestError::Config(
                    "canon.yaml `ingest.sessions.privacy.max_directive_chars` must be positive when `capture_user_directives: true`".to_string(),
                ));
            }
            DirectivePrivacyPolicy { capture_user_directives: true, max_directive_chars: max }
        } else {
            DirectivePrivacyPolicy::METADATA_ONLY
        };
        Ok(Self {
            sources: ingest
                .sources
                .into_iter()
                .filter_map(|(k, v)| v.roots.map(|r| (k, r)))
                .collect(),
            privacy,
        })
    }

    /// Resolve `entry`'s `(roots, matching_files)`: the `canon.yaml`-
    /// configured roots when this source has a PRESENT `roots` override
    /// (relative paths resolved against `repo_root`; an explicit empty
    /// list scans nothing), else the adapter's own `scan_roots` (env +
    /// defaults) via [`enumerate`].
    fn enumerate(&self, entry: &canon_ingest::AdapterEntry, repo_root: &Path, home: &Path, use_env_roots: bool) -> (Vec<PathBuf>, Vec<PathBuf>) {
        match self.sources.get(entry.client_id()) {
            Some(configured) => {
                let roots: Vec<PathBuf> = configured.iter().map(|r| if r.is_absolute() { r.clone() } else { repo_root.join(r) }).collect();
                let files = scanner::scan_roots(&roots, |p| entry.file_matches(p));
                (roots, files)
            }
            None => enumerate(entry, home, use_env_roots),
        }
    }
}

/// s31 D3: cwd-partitioned adapters whose on-disk session store is
/// physically split into one subdirectory per project (`omp`/`pi`,
/// Claude Code) — see [`ProjectScope`]'s own doc for the root-pruning
/// vs row-filtering split this drives.
const CWD_PARTITIONED_CLIENT_IDS: &[&str] = &["omp", "claude-code"];

fn is_cwd_partitioned(client_id: &str) -> bool {
    CWD_PARTITIONED_CLIENT_IDS.contains(&client_id)
}

/// s31 D3's default corpus: this repo's main `git worktree` root plus
/// every linked one, resolved once per [`run`] pass and consulted by
/// every adapter's scan.
///
/// **Root pruning vs row filtering.** omp/pi and Claude Code partition
/// their session store by cwd — one on-disk subdirectory per project,
/// named by a LOSSY forward-encoding of the cwd (every `/` collapses
/// to `-`; the convention `adapters::omp`'s `<encoded-cwd>` layout doc
/// and Claude Code's `claude_workspace_from_path` 3-window match both
/// already assume). DECODING that name back into a real path is
/// ambiguous — a literal `-` inside one path segment is
/// indistinguishable from an encoded `/` — so this scope never
/// decodes: it FORWARD-encodes each of its OWN roots the same way and
/// matches on-disk subdirectory names by exact string equality
/// ([`Self::encoded_dirnames`], consulted by
/// [`Self::keep_cwd_partitioned_file`] before any file under a pruned
/// subdirectory is ever read or digested). Claude Code's OWN
/// `UnifiedRow::workspace_key` is already that same undecoded encoded
/// string (never a real path — ported faithfully from
/// `claude_workspace_from_path`), so the identical set also answers
/// [`Self::project_key_for`]'s membership check for Claude Code
/// sessions, with no special case.
///
/// Codex and Hermes carry no on-disk cwd partition (every session file
/// mixes every project) — `codex_workspace_from_cwd` DOES decode a
/// real absolute path, so those two adapters are filtered POST-PARSE
/// by ROW `workspace_key` membership in [`Self::roots`] instead
/// ([`Self::keep_row`]), never by `encoded_dirnames`. A row with
/// `workspace_key: None` (Hermes always; Codex only if `cwd` was never
/// captured) is KEPT, fail-soft: an unknown workspace is ordinary
/// ambiguity, never proof a session is foreign.
struct ProjectScope {
    /// Stamped onto every in-scope `NormalizedSession` — the MAIN
    /// worktree's own normalized key.
    project_key: String,
    /// Normalized (`normalize_workspace_key`) absolute paths for the
    /// main worktree + every linked worktree (or the repo root alone,
    /// fail-soft outside a git repo).
    roots: BTreeSet<String>,
    /// Forward-encoded per-root cwd dirname, one per `roots` entry.
    encoded_dirnames: BTreeSet<String>,
    /// `--all-workspaces`: the scope above is still resolved (so
    /// `project_key` stamping stays correct) but pruning/filtering are
    /// both disabled.
    all_workspaces: bool,
}

impl ProjectScope {
    /// Resolve `repo_root`'s project scope. Fail-soft: `git worktree
    /// list --porcelain` failing for ANY reason (not a repo, git
    /// absent, non-zero exit, non-UTF8 output) degrades to the repo
    /// root alone, never an error (s31 D3).
    ///
    /// `home` feeds the omp/pi dirname convention below; pass the same
    /// scan home `run()` scans (so tests with a fixture `--home` prune
    /// against the fixture's own layout, never the real `$HOME`).
    fn resolve(repo_root: &Path, home: &Path, all_workspaces: bool) -> Self {
        let abs_roots = git_worktree_roots(repo_root).unwrap_or_else(|| vec![absolutize(repo_root)]);
        // `git worktree list` reports the main worktree FIRST — this
        // order is git's own guarantee, so `project_key` is captured
        // from the Vec before it collapses into an unordered set below.
        let normalized: Vec<String> = abs_roots.iter().filter_map(|p| normalize_workspace_key(&p.to_string_lossy())).collect();
        let project_key = normalized.first().cloned().unwrap_or_else(|| absolutize(repo_root).to_string_lossy().into_owned());
        let roots: BTreeSet<String> = normalized.into_iter().collect();
        // TWO on-disk dirname conventions coexist (verified against the
        // real stores, 2026-07-14): Claude Code encodes the FULL
        // absolute cwd (`/Users/j/Workspace/canon` →
        // `-Users-j-Workspace-canon`), while omp/pi encode the cwd with
        // the HOME prefix stripped (same cwd, home `/Users/j` →
        // `-Workspace-canon`). Both candidates go into one match set —
        // exact-equality matching keeps a collision harmless (it can
        // only ADD an in-scope-looking subdir, and every kept file
        // still resolves workspace/project keys from its own content).
        let home_key = normalize_workspace_key(&absolutize(home).to_string_lossy());
        let mut encoded_dirnames: BTreeSet<String> = roots.iter().map(|r| encode_cwd_dirname(r)).collect();
        if let Some(home_key) = home_key {
            for root in &roots {
                if let Some(rel) = root.strip_prefix(home_key.as_str()) {
                    if rel.starts_with('/') {
                        encoded_dirnames.insert(encode_cwd_dirname(rel));
                    }
                }
            }
        }
        Self { project_key, roots, encoded_dirnames, all_workspaces }
    }

    /// s31 3.2's summary line: `"project <key> (N roots)"` or `"all
    /// workspaces"` (the caller prefixes `"scope: "`).
    fn summary(&self) -> String {
        if self.all_workspaces { "all workspaces".to_string() } else { format!("project {} ({} roots)", self.project_key, self.roots.len()) }
    }

    /// Root pruning for `omp`/`claude-code` (see the struct doc): does
    /// `file`'s per-project subdirectory — the first path component
    /// strictly beneath whichever of `scan_roots` contains it — name
    /// an in-scope project? A file this scope cannot classify (not
    /// under any of `scan_roots`, or living directly IN a scan root
    /// with no subdirectory nesting — e.g. a `canon.yaml`-configured
    /// override root that already points AT one project) is KEPT,
    /// never silently dropped by a root-matching miss.
    fn keep_cwd_partitioned_file(&self, file: &Path, scan_roots: &[PathBuf]) -> bool {
        if self.all_workspaces {
            return true;
        }
        let Some(root) = scan_roots.iter().find(|r| file.starts_with(r)) else { return true };
        let Ok(rel) = file.strip_prefix(root) else { return true };
        let components: Vec<_> = rel.components().collect();
        if components.len() < 2 {
            return true;
        }
        let dirname = components[0].as_os_str().to_string_lossy();
        self.encoded_dirnames.contains(dirname.as_ref())
    }

    /// Row-level filter for `codex`/`hermes` (see the struct doc).
    fn keep_row(&self, workspace_key: Option<&str>) -> bool {
        if self.all_workspaces {
            return true;
        }
        match workspace_key {
            Some(key) => self.roots.contains(key) || self.encoded_dirnames.contains(key),
            None => true,
        }
    }

    /// `Some(project_key)` iff `workspace_key` names an in-scope
    /// project, in EITHER representation [`Self::resolve`] indexes —
    /// independent of `all_workspaces` (D3: stamping tracks ACTUAL
    /// membership, never the flag; `--all-workspaces` widens what gets
    /// SCANNED, not what counts as "this project").
    fn project_key_for(&self, workspace_key: Option<&str>) -> Option<String> {
        let key = workspace_key?;
        (self.roots.contains(key) || self.encoded_dirnames.contains(key)).then(|| self.project_key.clone())
    }
}

/// Forward-encode an already-normalized absolute path the same lossy
/// way omp/pi and Claude Code encode a cwd into their per-project
/// on-disk directory name: every `/` becomes `-`. See
/// [`ProjectScope`]'s own doc for why this is ONE-DIRECTION only
/// (never decoded back into a path).
fn encode_cwd_dirname(normalized_path: &str) -> String {
    normalized_path.chars().map(|c| if c == '/' { '-' } else { c }).collect()
}

/// Best-effort absolute form of `path` — canonicalized when possible
/// (resolves symlinks, matches what real transcripts record as
/// `cwd`), else joined onto the process cwd when relative, else
/// returned as-is. Never fails; a `path` that genuinely can't be
/// resolved just stays whatever was given.
fn absolutize(path: &Path) -> PathBuf {
    if let Ok(canon) = std::fs::canonicalize(path) {
        return canon;
    }
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir().map(|cwd| cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())
}

/// `git worktree list --porcelain` -> every worktree root, main
/// worktree FIRST (git's own listing order). `None` on ANY failure
/// (not a repo, git absent, non-zero exit, non-UTF8 output) — the ONE
/// subprocess boundary this module crosses, swallowing every failure
/// rather than raising (mirrors `canon-gate`'s own `staleness.rs`
/// `run_git` convention).
fn git_worktree_roots(repo_root: &Path) -> Option<Vec<PathBuf>> {
    let output = Command::new("git").arg("-C").arg(repo_root).args(["worktree", "list", "--porcelain"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let roots: Vec<PathBuf> = text.lines().filter_map(|line| line.strip_prefix("worktree ")).map(|p| absolutize(Path::new(p))).collect();
    if roots.is_empty() { None } else { Some(roots) }
}

/// One scan -> gate -> parse -> normalize -> persist pass over every
/// registered adapter.
///
/// **s31 D1 file-granular watermark gate.** Before parsing, each
/// present file (after D3 project pruning below) is content-digested
/// and diffed against its source's persisted [`SourceCursor`] (under
/// `<repo>/.canon/ingest/cursors/`, resolved from `canon_yaml`'s
/// directory) via [`SourceCursor::diff`]: a file whose digest exactly
/// matches is SKIPPED (never parsed), one that's new or changed is
/// parsed — so a single growing transcript among thousands re-parses
/// ALONE, turning `--watch`'s steady state from "re-parse the whole
/// corpus every poll" into "read + hash only, parse what changed".
/// [`canon_store::cursor`]'s own module doc still frames the general
/// gate as source-granular for soundness against a multi-file session;
/// s31 D1's own doc names why every registered adapter today derives a
/// session from exactly one file, making the finer grain safe here. A
/// missing/corrupt/absent cursor (first run, `full_rescan`, or a lost
/// cursor file) degrades to treating every present file as new — never
/// an error, since a full re-parse stays correct under the digest-
/// idempotent write path (S3 4.2). Cursors advance ONLY after the
/// whole pass persists durably (S3 3.2); the advance is best-effort —
/// a failed cursor write leaves the source to be re-parsed next pass,
/// never failing an ingest whose records already landed. The FRESH
/// cursor built each pass records every READABLE present file
/// regardless of whether it changed, so next pass's diff stays sound.
///
/// **s31 D3 project scope.** [`ProjectScope::resolve`] runs once,
/// before the adapter loop, and prunes/filters every source against it
/// (`all_workspaces` disables both without changing resolution) — see
/// its own doc for the root-pruning (omp/Claude Code) vs row-filtering
/// (Codex/Hermes) split. `project_key` is stamped onto every
/// normalized session whose workspace resolves into the scope,
/// independent of `all_workspaces`.
pub fn run(canon_yaml: &Path, home: &Path, use_env_roots: bool, full_rescan: bool, all_workspaces: bool) -> Result<IngestOutcome, IngestError> {
    let repo_root = canon_yaml.parent().unwrap_or_else(|| Path::new("."));
    let cursors = CursorStore::open(repo_root.join(paths::INGEST_CURSORS_DIR));
    let scope = ProjectScope::resolve(repo_root, home, all_workspaces);
    let scope_summary = scope.summary();

    let mut adapters = Vec::new();
    let mut all_rows: Vec<UnifiedRow> = Vec::new();
    let mut all_directives: Vec<DirectiveRow> = Vec::new();
    let mut malformed_records = 0usize;
    // A fresh cursor per source with at least one readable present
    // file, advanced iff this whole pass persists durably (below).
    let mut pending_cursors: Vec<SourceCursor> = Vec::new();
    let source_config = IngestSourceConfig::load(canon_yaml)?;

    for entry in registry() {
        let (roots, files) = source_config.enumerate(entry, repo_root, home, use_env_roots);

        // s31 D3 root pruning (omp/claude-code only, see ProjectScope's
        // doc): drop a candidate file BEFORE it is ever read or
        // digested when its per-project subdirectory is out of scope.
        let files: Vec<PathBuf> = if is_cwd_partitioned(entry.client_id()) {
            files.into_iter().filter(|f| scope.keep_cwd_partitioned_file(f, &roots)).collect()
        } else {
            files
        };

        // Read + digest every present (post-prune) file. A read
        // failure just excludes that one file from this pass — it is
        // absent from the fresh cursor too, so it is always retried.
        let mut present_digests: BTreeMap<String, String> = BTreeMap::new();
        let mut readable: Vec<(PathBuf, String, i64, u64, String)> = Vec::new();
        for path in &files {
            if let Ok(bytes) = std::fs::read(path) {
                let digest = file_digest(&bytes);
                let (mtime_ms, size) = file_stat(path);
                let key = path.to_string_lossy().into_owned();
                present_digests.insert(key.clone(), digest.clone());
                readable.push((path.clone(), key, mtime_ms, size, digest));
            }
        }

        // s31 D1: per-file diff, against the cursor stored at THIS
        // source's identity (s40 task 4.1 plus the privacy policy
        // generation): `client_id` + adapter parse version + the exact
        // capture mode/bound. A policy change must not reuse a cursor
        // whose normalized records were produced under another boundary.
        let cursor_id = session_source_cursor_id_with_privacy(entry.client_id(), entry.adapter.parse_version(), source_config.privacy);
        let base_cursor = base_session_cursor(&cursors, &cursor_id, full_rescan);
        let diff = base_cursor.diff(&present_digests);

        let to_parse: Vec<PathBuf> = readable.iter().filter(|(_, key, ..)| diff.changed_or_new.contains(key)).map(|(path, ..)| path.clone()).collect();

        // s31 D4: parse each changed/new file directly (bypassing
        // `canon_ingest::parse_files`, which doesn't carry
        // `ParseOutcome::directives`) so both rows AND directive rows
        // are collected in one pass, in file order.
        let mut rows: Vec<UnifiedRow> = Vec::new();
        let mut directives: Vec<DirectiveRow> = Vec::new();
        let mut skipped = 0usize;
        for path in &to_parse {
            let outcome = entry.adapter.parse(path);
            skipped += outcome.skipped;
            rows.extend(outcome.rows);
            directives.extend(outcome.directives);
        }

        adapters.push(AdapterSummary {
            client_id: entry.client_id(),
            roots,
            files_scanned: files.len(),
            reparsed: to_parse.len(),
            rows_parsed: rows.len(),
            malformed_records: skipped,
            skipped_unchanged: diff.unchanged.len(),
        });
        malformed_records += skipped;

        // s31 D3: codex/hermes carry no on-disk cwd partition — filter
        // rows (and their directives) post-parse by workspace
        // membership instead. Ordinary filtering, never malformed: no
        // counter for it (mirrors the spec's "dropped rows are
        // ordinary filtering" wording).
        if is_cwd_partitioned(entry.client_id()) {
            all_rows.extend(rows);
            all_directives.extend(directives);
        } else {
            all_rows.extend(rows.into_iter().filter(|r| scope.keep_row(r.workspace_key.as_deref())));
            all_directives.extend(directives.into_iter().filter(|d| scope.keep_row(d.workspace_key.as_deref())));
        }

        // s31 D1: the fresh cursor records EVERY readable present file
        // (unchanged AND changed_or_new alike), never gated on whether
        // this pass changed anything. It is written at the SAME
        // identity the gate above read from, so a version bump's
        // re-read lands its cursor at the new id and the pre-bump one
        // is left orphaned, never mutated (s40 task 4.2).
        if !readable.is_empty() {
            let mut fresh = SourceCursor::empty(cursor_id.as_str());
            for (path, _, mtime_ms, size, digest) in &readable {
                fresh.record(path, *mtime_ms, *size, digest.clone());
            }
            pending_cursors.push(fresh);
        }
    }

    // Apply the single privacy boundary only after every adapter's
    // rows have been collected and scope-filtered.
    let mut normalized = normalize_with_privacy(&all_rows, &all_directives, source_config.privacy);
    for session in &mut normalized.sessions {
        session.session.project_key = scope.project_key_for(session.session.workspace_key.as_deref());
    }
    let sessions_normalized = normalized.sessions.len();
    let skipped_rows = normalized.skipped_rows;

    const SESSION_KINDS: [RecordKind; 3] = [RecordKind::Session, RecordKind::Run, RecordKind::Event];

    // s29 design D6: build ONLY the union of rungs `session`/`run`/
    // `event` actually route (or age) to — a genuinely malformed
    // `canon.yaml` (bad YAML/policy syntax, an invalid pg schema, a
    // non-forward aging rule, …) fails this WHOLE command loud;
    // `canon.yaml` simply missing/unreadable stays the pre-existing
    // graceful "documented seam" (a legitimate first-run state, not a
    // malformed one).
    let loaded = match tiers::build_lenient_tiers_for_kinds(canon_yaml, &SESSION_KINDS) {
        Ok(loaded) => loaded,
        Err(TierCliError::ReadCanonYaml { .. }) => {
            return Ok(IngestOutcome {
                adapters,
                scope_summary,
                sessions_normalized,
                runs_written: 0,
                events_written: 0,
                skipped_rows,
                malformed_records,
                unwritten: Some(normalized.sessions),
                degrade_class: Some("config_unavailable"),
                degrade_reason: None,
            });
        }
        Err(other) => return Err(other.into()),
    };

    // Check every kind this ingest writes is actually routed BEFORE
    // persisting anything, so a policy gap (e.g. `event` unrouted while
    // `session`/`run` are) never leaves a partial write for this pass —
    // either the whole batch persists, or none of it does and the
    // caller gets the documented-seam JSON fallback instead.
    let fully_routed = SESSION_KINDS.into_iter().all(|kind| loaded.policy.tier_for(kind).is_ok());
    if !fully_routed {
        return Ok(IngestOutcome {
            adapters,
            scope_summary,
            sessions_normalized,
            runs_written: 0,
            events_written: 0,
            skipped_rows,
            malformed_records,
            unwritten: Some(normalized.sessions),
            degrade_class: Some("unrouted"),
            degrade_reason: None,
        });
    }

    // s29 design D6: a routed-but-unattached rung carries its
    // build-time reason (the configured env-var name) in
    // `loaded.unavailable_reasons` — surface it here instead of the
    // bare "tiers unreachable" guess `format_human` used to print.
    let mut degrade_reasons: Vec<String> = Vec::new();
    for kind in SESSION_KINDS {
        if let Ok(rung) = loaded.policy.tier_for(kind) {
            if let Some(reason) = loaded.unavailable_reasons.get(&rung) {
                let backend = loaded.policy.tiers.get(&rung).map(BackendConfig::backend);
                let message = safe_tier_degrade_reason(rung, backend, reason);
                if !degrade_reasons.contains(&message) {
                    degrade_reasons.push(message);
                }
            }
        }
    }
    if !degrade_reasons.is_empty() {
        return Ok(IngestOutcome {
            adapters,
            scope_summary,
            sessions_normalized,
            runs_written: 0,
            events_written: 0,
            skipped_rows,
            malformed_records,
            unwritten: Some(normalized.sessions),
            degrade_class: Some("tier_unavailable"),
            degrade_reason: Some(degrade_reasons.join("; ")),
        });
    }

    let store = TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite);
    let mut runs_written = 0usize;
    let mut events_written = 0usize;
    // s31 D2: batch the persist path. Records are grouped per kind and
    // handed to `TierRegistry::persist_many` (one rung resolution + one
    // `Tier::write_batch` per flush) instead of one `persist` round
    // trip per record — the dogfood finding that put ~23 minutes of a
    // full first pass inside `PgTier::write_row`. Events flush in
    // bounded chunks so an `--all-workspaces` pass never clones the
    // whole corpus into memory at once.
    const EVENT_FLUSH: usize = 2_000;
    let mut session_buf: Vec<canon_model::records::Session> = Vec::new();
    let mut run_buf: Vec<canon_model::records::Run> = Vec::new();
    let mut event_buf: Vec<canon_model::records::Event> = Vec::new();
    for session in &normalized.sessions {
        session_buf.push(session.session.clone());
        // s37 (`execution-graph-topology`): persist the session's ROOT
        // run AND every child run `normalize` derived for a distinct
        // `agent_id` — `NormalizedSession::runs()` yields root-first,
        // then children, each child carrying `parent_run_id`. Pushing
        // only `session.run` (the pre-s37 shape) would silently drop the
        // whole subagent tier: `normalize` would build the lineage and
        // nothing would ever write it, so a real multi-agent transcript
        // still landed as one flat run. A single-agent session yields
        // exactly one run here, unchanged.
        for run in session.runs() {
            run_buf.push(run.clone());
            runs_written += 1;
        }
        for event in &session.events {
            event_buf.push(event.clone());
            events_written += 1;
            if event_buf.len() >= EVENT_FLUSH {
                persist_many_idempotent(&store, &std::mem::take(&mut event_buf))?;
            }
        }
    }
    persist_many_idempotent(&store, &session_buf)?;
    persist_many_idempotent(&store, &run_buf)?;
    persist_many_idempotent(&store, &event_buf)?;

    // Durable-write succeeded (S3 3.2): advance each (re-)parsed
    // source's cursor. Best-effort — a cursor write failure only costs
    // a re-parse next pass (digest-idempotent, S3 4.2), never the
    // just-persisted records.
    for mut cursor in pending_cursors {
        cursor.refresh_summary();
        let _ = cursors.write(&cursor);
    }

    Ok(IngestOutcome {
        adapters,
        scope_summary,
        sessions_normalized,
        runs_written,
        events_written,
        skipped_rows,
        malformed_records,
        unwritten: None,
        degrade_class: None,
        degrade_reason: None,
    })
}

/// A file's `(mtime_ms, size)` for the cursor's informational summary
/// fields — best-effort (a metadata failure right after a successful
/// read is degenerate; `(0, 0)` is harmless, never a gate input — the
/// gate decides on the content digest alone).
fn file_stat(path: &Path) -> (i64, u64) {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let mtime_ms = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
            (mtime_ms, meta.len())
        }
        Err(_) => (0, 0),
    }
}

/// The parse-generation component of a [`SourceCursor`] id per
/// `(client_id, parse_version)` session source (s40
/// (`plan-vs-actual-diff`), task 4.1) — the counterpart of
/// `crate::plans::plan_source_cursor_id` for session ingest. The actual
/// cursor identity used by [`run`] is
/// [`session_source_cursor_id_with_privacy`], which appends the privacy
/// mode and bound to this component.
///
/// `parse_version` ([`canon_ingest::SessionAdapter::parse_version`])
/// joins the identity because of what this cursor's gate actually
/// compares: per-file CONTENT digests. Transcript bytes are
/// byte-stable by design — an already-ingested session re-ingests
/// idempotently — so a change to the ADAPTER is invisible to the gate
/// and every transcript is reported `skipped unchanged (watermark)`
/// even though it would now normalize differently. Folding the
/// adapter's parse generation into the cursor IDENTITY rather than
/// into its per-file digests is what makes a bump behave EXACTLY like
/// editing every transcript in the source: the lookup finds no cursor
/// at the new id at all, so every file is re-read, re-parsed and
/// re-persisted, and the stale cursor is simply orphaned rather than
/// mutated — no `--full`, no cursor deletion.
///
/// ## Why version 1 carries no suffix
///
/// The id is the bare `client_id` at version 1 and
/// `<client_id>-v<parse_version>` from 2 on. This is NOT a legacy
/// carve-out to be tidied away later: the version records WHICH
/// normalization produced a cursor's skip decision, and every session
/// cursor written before s40 was produced by the normalization that is
/// now version 1. A pre-s40 cursor and a v1 cursor are therefore not
/// merely compatible, they are the SAME cursor, and giving them
/// different ids would assert a difference that does not exist —
/// imposing a full re-read of every transcript on every existing
/// checkout to gain exactly zero information. So the unsuffixed form
/// IS the correct id for version 1, and the suffix begins at 2 because
/// 2 is the first version whose output can differ from what a stored
/// cursor was computed against. Normalizing v1 to `-v1` later would
/// silently orphan every cursor in the field; do not.
///
/// Version `0` is not folded into the unsuffixed form (it yields
/// `<client_id>-v0`): it is a distinct declared generation, and
/// colliding it with 1 would be exactly the false-equality this
/// function otherwise avoids.
///
/// ## Why this rendering is injective and filename-safe
///
/// Both properties rest on a precondition held by the ADAPTER ID, not
/// by this function: `canon_ingest::session_adapter_id_violation`'s
/// grammar (lowercase `[a-z0-9-]`, no outer `-`, and neither the
/// reserved `-v<digits>` suffix nor the `plan-` prefix), asserted over
/// the real static registry by `canon_ingest::registry`'s tests. Given
/// it, `(client_id, parse_version) -> id` is one-to-one — see that
/// function's doc for the proof, and for why `omp-v2` as an adapter
/// name is the concrete collision it rules out (`("omp", 2)` renders
/// the same string, so the two adapters would share one watermark file
/// and each report the other's transcripts `unchanged`). The same
/// grammar is what makes the result a bare filename component with no
/// separator, no `..`, no empty stem, and no case-only alias that
/// macOS's case-insensitive default volume would collapse into one
/// cursor — which matters because `CursorStore::path_for` joins this
/// string onto its root verbatim.
fn session_source_cursor_id(client_id: &str, parse_version: u32) -> String {
    if parse_version == 1 { client_id.to_string() } else { format!("{client_id}-v{parse_version}") }
}

/// Compose the actual session cursor identity. Privacy is part of the
/// identity because the same source bytes normalize to different durable
/// event sets under metadata-only, bounded capture, or a changed bound.
/// Old unsuffixed cursors are intentionally not reused: they may have been
/// produced before the pre-store privacy boundary existed.
fn session_source_cursor_id_with_privacy(client_id: &str, parse_version: u32, privacy: DirectivePrivacyPolicy) -> String {
    let generation = session_source_cursor_id(client_id, parse_version);
    if privacy.capture_user_directives {
        format!("{generation}-privacy-capture-{}", privacy.max_directive_chars)
    } else {
        format!("{generation}-privacy-metadata-{}", privacy.max_directive_chars)
    }
}
/// `changed_or_new`) when `full_rescan` is set, when this source has
/// never been ingested, or when nothing is stored AT THIS IDENTITY.
/// That last case is the whole mechanism behind s40 task 4.2: an
/// adapter's `parse_version` bump moves its cursor id, so the read
/// misses and the source re-reads in full — reached without `--full`
/// and without deleting anything.
fn base_session_cursor(cursors: &CursorStore, cursor_id: &str, full_rescan: bool) -> SourceCursor {
    if full_rescan {
        return SourceCursor::empty(cursor_id);
    }
    cursors.read(cursor_id).unwrap_or_else(|| SourceCursor::empty(cursor_id))
}

/// Persist `record`, treating an already-identical git-tier record
/// (`StoreError::DuplicatePath`) as a successful no-op rather than a
/// fatal error — canon-ingest's own idempotence guarantee (S3 design
/// D3: "a full re-scan … never double-writes or double-counts a
/// record already stored") for tiers whose `Tier::write` REJECTS a
/// byte-identical duplicate path instead of silently deduping it
/// (`canon_store::partition` module doc: git-tier's Hive path is
/// `{natural_key}__{digest12}`, so a byte-identical resubmission
/// resolves to the SAME path and `GitTier::write` hard-errors on it
/// by design, "never silently overwriting"). PG/R2 tiers instead
/// report `WriteReceipt::deduped` and never raise `DuplicatePath` at
/// all, so this helper is safe unconditionally, not git-tier-specific
/// by construction.
///
/// s42 (`close-the-open-loops`) made it `pub(crate)`: `canon dispatch
/// begin`/`end` now persist their `Run` through the SAME
/// `TierRegistry` this module writes sessions/runs/events through
/// (`crate::dispatch::persist_run`), and reuse this one
/// duplicate-tolerance decision rather than restating it — so the two
/// run writers can never disagree about whether an already-stored
/// record is an error.
pub(crate) fn persist_idempotent<T: canon_model::envelope::CanonRecord>(store: &TierRegistry, record: &T) -> Result<(), StoreError> {
    match store.persist(record) {
        Ok(_) => Ok(()),
        Err(StoreError::DuplicatePath { .. }) => Ok(()),
        Err(err) => Err(err),
    }
}

/// Batched counterpart to [`persist_idempotent`] (s31 D2). PG/R2 tiers
/// dedup byte-identical resubmissions inside `write_batch` itself
/// (`WriteReceipt::deduped`), but the git tier's default `write_batch`
/// loops `GitTier::write`, which hard-errors `DuplicatePath` on a
/// byte-identical path by design — and one such error would abort the
/// whole batch. Falling back to the per-record [`persist_idempotent`]
/// loop for that batch keeps the pre-s31 idempotence contract exactly
/// (S3 design D3: a re-scan never double-writes or fails on records
/// already stored), while every non-duplicate batch keeps the one-
/// round-trip fast path.
fn persist_many_idempotent<T: canon_model::envelope::CanonRecord>(store: &TierRegistry, records: &[T]) -> Result<(), StoreError> {
    match store.persist_many(records) {
        Ok(_) => Ok(()),
        Err(StoreError::DuplicatePath { .. }) => {
            for record in records {
                persist_idempotent(store, record)?;
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

/// Build a degraded-store reason without copying backend diagnostics into
/// metadata-only output. Unset env-var names are safe, stable configuration
/// identifiers; live backend errors are intentionally collapsed to the
/// backend's generic availability reason because they may contain DSNs,
/// usernames, or passwords.
fn safe_tier_degrade_reason(rung: canon_store::policy::Rung, backend: Option<canon_store::policy::Backend>, raw: &str) -> String {
    let safe_reason = raw.strip_suffix(" is unset").and_then(|name| {
        let name = name.strip_prefix('`').and_then(|name| name.strip_suffix('`')).unwrap_or(name);
        (!name.is_empty() && name.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')).then_some(name)
    });
    let reason = safe_reason.map(|name| format!("{name} is unset")).unwrap_or_else(|| backend.map(|value| value.default_unattached_reason().to_string()).unwrap_or_else(|| "unavailable".to_string()));
    StoreError::tier_unavailable(rung, backend, reason).to_string()
}

/// Human-readable run summary (task 5.2: "records scanned, records
/// skipped as violations, per-adapter counts").
pub fn format_human(outcome: &IngestOutcome) -> String {
    let mut out = String::new();
    out.push_str(&format!("scope: {}\n", outcome.scope_summary));
    for adapter in &outcome.adapters {
        out.push_str(&format!(
            "{}: {} file(s) scanned, {} reparsed, {} skipped unchanged (watermark), {} row(s) parsed, {} malformed record(s)\n",
            adapter.client_id, adapter.files_scanned, adapter.reparsed, adapter.skipped_unchanged, adapter.rows_parsed, adapter.malformed_records
        ));
    }
    out.push_str(&format!("sessions normalized: {}\n", outcome.sessions_normalized));
    out.push_str(&format!("malformed records (corrupt line/db, counted as violations): {}\n", outcome.malformed_records));
    out.push_str(&format!("rows skipped (malformed session_id): {}\n", outcome.skipped_rows));
    match &outcome.unwritten {
        None => {
            out.push_str(&format!("runs written: {}\n", outcome.runs_written));
            out.push_str(&format!("events written: {}\n", outcome.events_written));
        }
        Some(sessions) => {
            let reason = outcome
                .degrade_reason
                .as_deref()
                .unwrap_or("`canon.yaml` missing/unreadable, or session/run/event unrouted");
            out.push_str(&format!(
                "store tiers unreachable ({reason}) — {} normalized session(s) NOT persisted; printing JSON instead\n",
                sessions.len()
            ));
        }
    }
    out
}

/// `--json`: metadata-only representation of the unwritten normalized
/// candidates. Directive text and every open event `detail` are omitted
/// even when bounded capture was explicitly enabled.
pub fn format_json(outcome: &IngestOutcome) -> Option<String> {
    outcome.unwritten.as_ref().map(|sessions| serde_json::to_string_pretty(&degraded_metadata(outcome, sessions)).expect("metadata JSON always serializes"))
}

fn degraded_metadata(outcome: &IngestOutcome, sessions: &[NormalizedSession]) -> serde_json::Value {
    let session_entries: Vec<serde_json::Value> = sessions
        .iter()
        .map(|normalized| {
            let body = serde_json::to_value(&normalized.session).expect("Session always serializes");
            serde_json::json!({
                "session_id": normalized.session.session_id,
                "client": normalized.session.client,
                "workspace_key": normalized.session.workspace_key,
                "workspace_label": normalized.session.workspace_label,
                "project_key": normalized.session.project_key,
                "digest": content_digest12(&body),
            })
        })
        .collect();
    let run_entries: Vec<serde_json::Value> = sessions
        .iter()
        .flat_map(|normalized| normalized.runs().map(|run| {
            let body = serde_json::to_value(run).expect("Run always serializes");
            serde_json::json!({
                "run_id": run.run_id,
                "session_id": run.session_id,
                "parent_run_id": run.parent_run_id,
                "digest": content_digest12(&body),
            })
        }))
        .collect();
    let event_entries: Vec<serde_json::Value> = sessions
        .iter()
        .flat_map(|normalized| normalized.events.iter().map(|event| {
            let body = serde_json::to_value(event).expect("Event always serializes");
            serde_json::json!({
                "run_id": event.run_id,
                "seq": event.seq,
                "at": event.envelope.at,
                "label": event.label,
                "digest": content_digest12(&body),
            })
        }))
        .collect();
    let adapters: Vec<serde_json::Value> = outcome
        .adapters
        .iter()
        .map(|adapter| serde_json::json!({
            "client_id": adapter.client_id,
            "files_scanned": adapter.files_scanned,
            "reparsed": adapter.reparsed,
            "rows_parsed": adapter.rows_parsed,
            "malformed_records": adapter.malformed_records,
            "skipped_unchanged": adapter.skipped_unchanged,
        }))
        .collect();
    let failure_class = outcome.degrade_class.unwrap_or("degraded");
    let failure_reason = outcome.degrade_reason.as_deref().unwrap_or(match failure_class {
        "config_unavailable" => "canon.yaml missing or unreadable",
        "unrouted" => "session/run/event routing is incomplete",
        _ => "session ingest did not reach a durable tier",
    });
    serde_json::json!({
        "scope": outcome.scope_summary,
        "adapters": adapters,
        "counts": {
            "sessions": session_entries.len(),
            "runs": run_entries.len(),
            "events": event_entries.len(),
            "sessions_normalized": outcome.sessions_normalized,
            "runs_written": outcome.runs_written,
            "events_written": outcome.events_written,
            "malformed_records": outcome.malformed_records,
            "rows_skipped": outcome.skipped_rows,
        },
        "sessions": session_entries,
        "runs": run_entries,
        "events": event_entries,
        "failure": {
            "class": failure_class,
            "reason": failure_reason,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn omp_parse_version() -> u32 {
        canon_ingest::registry::find("omp").expect("the omp adapter is registered").adapter.parse_version()
    }

    fn omp_cursor_id(privacy: DirectivePrivacyPolicy) -> String {
        session_source_cursor_id_with_privacy("omp", omp_parse_version(), privacy)
    }

    fn omp_bumped_cursor_id(privacy: DirectivePrivacyPolicy) -> String {
        session_source_cursor_id_with_privacy("omp", omp_parse_version() + 1, privacy)
    }

    fn write_fixture_home(root: &Path) {
        let session_dir = root.join(".omp/agent/sessions/-tmp-proj");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("s1.jsonl"),
            "{\"type\":\"session\",\"id\":\"ing_ses_1\",\"cwd\":\"/tmp/proj\"}\n\
             {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"model\":\"gpt-4o-mini\",\"provider\":\"openai\",\"usage\":{\"input\":10,\"output\":5}}}\n",
        )
        .unwrap();
    }

    fn write_canon_yaml(root: &Path, routed: bool) -> PathBuf {
        let routing = if routed { "  session: local\n  run: local\n  event: local\n" } else { "" };
        let yaml = format!("tiers:\n  local: {{ backend: git, root: .canon/ledger }}\nrouting:\n{routing}\n");
        let path = root.join("canon.yaml");
        std::fs::write(&path, yaml).unwrap();
        path
    }

    #[test]
    fn absent_privacy_config_defaults_to_metadata_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canon.yaml");
        std::fs::write(&path, "tiers: {}\n").unwrap();

        let config = IngestSourceConfig::load(&path).unwrap();
        assert_eq!(config.privacy, DirectivePrivacyPolicy::METADATA_ONLY);
    }

    #[test]
    fn capture_privacy_requires_a_positive_unicode_bound() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canon.yaml");
        std::fs::write(
            &path,
            "ingest:\n  sessions:\n    privacy:\n      capture_user_directives: true\n      max_directive_chars: 4096\n",
        )
        .unwrap();

        let config = IngestSourceConfig::load(&path).unwrap();
        assert_eq!(config.privacy, DirectivePrivacyPolicy { capture_user_directives: true, max_directive_chars: 4096 });
    }

    #[test]
    fn capture_privacy_rejects_missing_or_zero_bound() {
        let dir = tempfile::tempdir().unwrap();
        for privacy in [
            "capture_user_directives: true\n",
            "capture_user_directives: true\n      max_directive_chars: 0\n",
        ] {
            let path = dir.path().join("canon.yaml");
            std::fs::write(&path, format!("ingest:\n  sessions:\n    privacy:\n      {privacy}")).unwrap();
            assert!(IngestSourceConfig::load(&path).is_err());
        }
    }

    #[test]
    fn unrouted_policy_falls_back_to_unwritten_documented_seam() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let canon_yaml = write_canon_yaml(dir.path(), false);

        let outcome = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(outcome.sessions_normalized, 1);
        assert_eq!(outcome.runs_written, 0);
        assert!(outcome.unwritten.is_some());
        assert!(format_json(&outcome).unwrap().contains("ing_ses_1"));
    }

    #[test]
    fn malformed_record_is_counted_and_surfaced_in_the_run_summary() {
        // ReviewS3Full finding 3: a corrupt JSON line in an otherwise
        // valid session file must be COUNTED (not just silently
        // skipped) and surfaced through to the CLI's human-readable
        // run summary as the malformed-record violation the s3
        // `session-adapter-registry` spec's "Malformed adapter record
        // is skipped as a violation" scenario requires.
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join(".omp/agent/sessions/-tmp-proj");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("s1.jsonl"),
            "{\"type\":\"session\",\"id\":\"ing_ses_malformed\",\"cwd\":\"/tmp/proj\"}\n\
             not valid json at all\n\
             {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"model\":\"gpt-4o-mini\",\"provider\":\"openai\",\"usage\":{\"input\":10,\"output\":5}}}\n",
        )
        .unwrap();
        let canon_yaml = write_canon_yaml(dir.path(), false);

        let outcome = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(outcome.malformed_records, 1, "the corrupt line must be counted, not just silently skipped");
        assert_eq!(outcome.adapters[0].malformed_records, 1);
        assert_eq!(outcome.sessions_normalized, 1, "the two valid records around the corrupt line still normalize");

        let human = format_human(&outcome);
        assert!(human.contains("malformed records"), "the run summary must surface the malformed-record count: {human}");
        assert!(human.contains("1 malformed record(s)"), "the per-adapter line must surface it too: {human}");
    }

    #[test]
    fn routed_policy_persists_through_canon_store_git_tier() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let canon_yaml = write_canon_yaml(dir.path(), true);

        let outcome = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(outcome.sessions_normalized, 1);
        assert_eq!(outcome.runs_written, 1);
        assert_eq!(outcome.events_written, 1);
        assert!(outcome.unwritten.is_none());

        // S3 §3 watermark: a second pass over the UNCHANGED fixture
        // home skips the source wholesale — nothing is re-parsed or
        // re-persisted (the cursor written after pass 1 matches every
        // present file's digest).
        let second = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(second.runs_written, 0, "unchanged source is watermark-skipped, not re-parsed");
        assert_eq!(second.events_written, 0);
        assert_eq!(second.adapters[0].reparsed, 0, "unchanged source is not re-parsed");
        assert!(second.adapters[0].skipped_unchanged >= 1, "the skip must be surfaced in the summary");
        let cursor_id = omp_cursor_id(DirectivePrivacyPolicy::METADATA_ONLY);
        assert!(dir.path().join(paths::INGEST_CURSORS_DIR).join(format!("{cursor_id}.json")).exists(), "the pass-1 cursor persisted");

        // S3 6.8 (watermark reset): `--full` (full_rescan) ignores the
        // cursor and re-parses every source — the reset re-scan. The S3
        // 4.2 digest-idempotent write path means the git tier's record
        // COUNT is unchanged (a byte-identical resubmission resolves to
        // the same `{natural_key}__{digest12}` path, a no-op), so a
        // forced re-ingest never duplicates.
        let count_records = || -> usize {
            fn walk(dir: &Path, n: &mut usize) {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            walk(&p, n);
                        } else if p.extension().is_some_and(|x| x == "json") {
                            *n += 1;
                        }
                    }
                }
            }
            let mut n = 0;
            walk(&dir.path().join(".canon/ledger"), &mut n);
            n
        };
        let before = count_records();
        assert!(before >= 3, "pass 1 persisted session + run + event records");

        let forced = run(&canon_yaml, dir.path(), false, true, true).unwrap();
        assert_eq!(forced.runs_written, 1, "forced full rescan re-parses");
        assert_eq!(forced.events_written, 1);
        assert_eq!(count_records(), before, "the forced reset re-scan must not duplicate any record (S3 6.8)");
    }

    #[test]
    fn a_changed_source_reingests_while_unchanged_ones_stay_skipped() {
        // Source-granular gating: after pass 1 writes a cursor per
        // source, appending a session to omp's transcript makes ONLY
        // omp's present set diverge from its cursor, so omp re-parses
        // while the (empty) other sources stay skipped.
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let canon_yaml = write_canon_yaml(dir.path(), true);

        assert_eq!(run(&canon_yaml, dir.path(), false, false, true).unwrap().runs_written, 1);
        assert_eq!(run(&canon_yaml, dir.path(), false, false, true).unwrap().runs_written, 0, "unchanged -> skipped");

        // Append a NEW session -> omp's file digest changes.
        let s = dir.path().join(".omp/agent/sessions/-tmp-proj/s1.jsonl");
        let mut body = std::fs::read_to_string(&s).unwrap();
        body.push_str(
            "{\"type\":\"session\",\"id\":\"ing_ses_2\",\"cwd\":\"/tmp/proj2\"}\n\
             {\"type\":\"message\",\"timestamp\":\"2026-07-02T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"model\":\"gpt-4o-mini\",\"provider\":\"openai\",\"usage\":{\"input\":3,\"output\":2}}}\n",
        );
        std::fs::write(&s, body).unwrap();

        let third = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert!(third.runs_written >= 1, "the changed omp source re-parses");
        assert_eq!(third.adapters[0].skipped_unchanged, 0, "the changed omp source is NOT skipped");
    }

    #[test]
    fn resetting_a_cursor_reingests_the_full_corpus_without_duplicating() {
        // S3 6.8 (literal cursor reset): delete a source's persisted
        // cursor, run again, and assert the source is fully re-parsed
        // (cursor gone -> no gate) yet the git-tier record COUNT is
        // unchanged (S3 4.2 digest-idempotent write -> no duplicates).
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let canon_yaml = write_canon_yaml(dir.path(), true);

        assert_eq!(run(&canon_yaml, dir.path(), false, false, true).unwrap().runs_written, 1);
        let count = |dir: &Path| -> usize {
            fn walk(dir: &Path, n: &mut usize) {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            walk(&p, n);
                        } else if p.extension().is_some_and(|x| x == "json") {
                            *n += 1;
                        }
                    }
                }
            }
            let mut n = 0;
            walk(&dir.join(".canon/ledger"), &mut n);
            n
        };
        let before = count(dir.path());
        assert!(before >= 3);

        // literally reset: remove the persisted cursor file.
        let cursor_id = omp_cursor_id(DirectivePrivacyPolicy::METADATA_ONLY);
        let cursor = dir.path().join(paths::INGEST_CURSORS_DIR).join(format!("{cursor_id}.json"));
        assert!(cursor.exists(), "pass 1 wrote the cursor");
        std::fs::remove_file(&cursor).unwrap();

        // next pass finds no cursor -> re-parses omp in full ...
        let reset = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(reset.adapters[0].skipped_unchanged, 0, "reset source is NOT skipped");
        assert!(reset.runs_written >= 1, "reset source is re-parsed");
        // ... but the digest-idempotent write path adds no duplicate.
        assert_eq!(count(dir.path()), before, "a cursor reset re-scan must not duplicate any record (S3 6.8)");
    }

    #[test]
    fn missing_canon_yaml_falls_back_to_unwritten_documented_seam() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let missing_canon_yaml = dir.path().join("does-not-exist.yaml");

        let outcome = run(&missing_canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(outcome.sessions_normalized, 1);
        assert!(outcome.unwritten.is_some());
    }

    #[test]
    fn canon_yaml_source_roots_replace_not_union_the_default_scan() {
        // S3 1.2: `ingest.sources.omp.roots` REPLACES omp's default scan
        // (relative to the canon.yaml dir). A different valid omp session
        // seeded under the default `<home>/.omp/...` must NOT appear —
        // proving the override replaces rather than unions with defaults.
        let dir = tempfile::tempdir().unwrap();
        // Default-home fixture (session `ing_ses_1`) that must be shadowed.
        write_fixture_home(dir.path());
        // Configured custom root (session `cfg_ses_1`).
        let custom = dir.path().join("custom-omp");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(
            custom.join("s1.jsonl"),
            "{\"type\":\"session\",\"id\":\"cfg_ses_1\",\"cwd\":\"/tmp/proj\"}\n\
             {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"model\":\"gpt-4o-mini\",\"provider\":\"openai\",\"usage\":{\"input\":10,\"output\":5}}}\n",
        )
        .unwrap();
        let path = dir.path().join("canon.yaml");
        std::fs::write(&path, "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\ningest:\n  sources:\n    omp:\n      roots: [custom-omp]\n").unwrap();

        let outcome = run(&path, dir.path(), false, false, true).unwrap();
        assert_eq!(outcome.sessions_normalized, 1, "only the configured-root session is scanned (replace, not union)");
        let json = format_json(&outcome).unwrap();
        assert!(json.contains("cfg_ses_1"), "the configured-root session appears: {json}");
        assert!(!json.contains("ing_ses_1"), "the default-home session is shadowed, not unioned in: {json}");
    }

    #[test]
    fn canon_yaml_explicit_empty_roots_scans_zero_not_defaults() {
        // An explicit `roots: []` is a real override — scan NOTHING for
        // omp — never a silent fallback to the default home corpus.
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let path = dir.path().join("canon.yaml");
        std::fs::write(&path, "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\ningest:\n  sources:\n    omp:\n      roots: []\n").unwrap();

        let outcome = run(&path, dir.path(), false, false, true).unwrap();
        assert_eq!(outcome.sessions_normalized, 0, "explicit `roots: []` scans zero; the default home fixture is NOT ingested");
    }

    #[test]
    fn canon_yaml_unknown_source_id_fails_loud() {
        // `claude` is not a registered client id (it is `claude-code`) —
        // a typo'd source key would otherwise be silently dropped and
        // that adapter would scan its default home roots.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canon.yaml");
        std::fs::write(&path, "tiers:\n  local: { backend: git, root: .canon/ledger }\ningest:\n  sources:\n    claude:\n      roots: [x]\n").unwrap();

        let err = run(&path, dir.path(), false, false, true).unwrap_err();
        assert!(matches!(err, IngestError::Config(_)), "unknown source id must fail loud, got {err:?}");
        let msg = err.to_string();
        assert!(msg.contains("claude"), "names the offending id: {msg}");
        assert!(msg.contains("claude-code"), "lists the known ids: {msg}");
    }

    #[test]
    fn canon_yaml_singular_root_typo_fails_loud() {
        // `root:` (singular) is a typo for `roots:` — deny_unknown_fields
        // rejects it rather than silently scanning the default home roots.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canon.yaml");
        std::fs::write(&path, "tiers:\n  local: { backend: git, root: .canon/ledger }\ningest:\n  sources:\n    omp:\n      root: [x]\n").unwrap();

        let err = run(&path, dir.path(), false, false, true).unwrap_err();
        assert!(matches!(err, IngestError::Config(_)), "a `root:` typo must fail loud, got {err:?}");
    }

    #[test]
    fn present_but_invalid_yaml_canon_fails_loud() {
        // `run()` swallows a later `build_tiers` YAML error into the
        // unwritten seam, so a syntax error in a canon.yaml meant to set
        // `ingest.sources` must fail loud here rather than silently
        // scanning default roots.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canon.yaml");
        std::fs::write(&path, "[unterminated flow sequence\n").unwrap();

        let err = run(&path, dir.path(), false, false, true).unwrap_err();
        assert!(matches!(err, IngestError::Config(_)), "present-but-invalid YAML must fail loud, got {err:?}");
    }

    /// s29 design D6, spec scenario "An unrelated unset cold bucket no
    /// longer blocks session persistence": `session`/`run`/`event`
    /// route to `local` while an UNRELATED kind (`handoff`) routes to
    /// `cold` with an unset `bucket_env` — the kind-scoped lenient
    /// build must never even ATTEMPT the cold rung (it is not in the
    /// union `session`/`run`/`event` need), so sessions persist
    /// normally instead of degrading to the unwritten seam.
    #[test]
    fn an_unrelated_unset_cold_bucket_never_blocks_session_persistence() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let yaml = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_INGEST_S29_D6_UNSET, prefix: \"canon/\" }\nrouting:\n  session: local\n  run: local\n  event: local\n  handoff: cold\n";
        let canon_yaml = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml, yaml).unwrap();
        std::env::remove_var("CANON_R2_BUCKET_INGEST_S29_D6_UNSET");

        let outcome = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(outcome.sessions_normalized, 1);
        assert_eq!(outcome.runs_written, 1, "the unrelated unattached cold rung must never be attempted for a session/run/event pass");
        assert_eq!(outcome.events_written, 1);
        assert!(outcome.unwritten.is_none(), "an unrelated kind's unattached cold rung must never degrade session persistence");
    }

    /// s29 design D6, spec scenario "A degraded ingest names the
    /// variable an operator must set": `session`/`run`/`event` all
    /// route to `hot` with an unset `dsn_env` — the degraded outcome's
    /// printed text must name the actual configured env-var, never the
    /// old bare "tiers unreachable" guess-string.
    #[test]
    fn a_degraded_hot_rung_outcome_names_the_unset_dsn_env_var() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let yaml = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_INGEST_S29_D6_UNSET, schema: canon_v1 }\nrouting:\n  session: hot\n  run: hot\n  event: hot\n";
        let canon_yaml = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml, yaml).unwrap();
        std::env::remove_var("CANON_PG_DSN_INGEST_S29_D6_UNSET");

        let outcome = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert!(outcome.unwritten.is_some(), "the hot rung's dsn_env is unset -- session/run/event must degrade to unwritten");
        assert!(
            outcome.degrade_reason.as_deref().is_some_and(|r| r.contains("CANON_PG_DSN_INGEST_S29_D6_UNSET")),
            "the outcome must name the unset env var, got {:?}",
            outcome.degrade_reason
        );
        let human = format_human(&outcome);
        assert!(human.contains("CANON_PG_DSN_INGEST_S29_D6_UNSET"), "the printed outcome must name the unset env var, not a bare guess: {human}");
    }

    /// s29 design D6: unlike the pre-existing "unrouted"/"missing
    /// canon.yaml" degrade seams above, a PRESENT but genuinely
    /// malformed `tiers:`/`routing:` config (here, an invalid pg
    /// schema) must fail the WHOLE `canon ingest sessions` command
    /// loud — "lenient" describes rung reachability only, never config
    /// correctness.
    #[test]
    fn a_malformed_pg_schema_fails_the_whole_command_loud_not_a_silent_degrade() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let yaml = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_INGEST_S29_D6_SCHEMA, schema: Bad-Schema }\nrouting:\n  session: hot\n  run: hot\n  event: hot\n";
        let canon_yaml = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml, yaml).unwrap();

        let err = run(&canon_yaml, dir.path(), false, false, true).unwrap_err();
        assert!(matches!(err, IngestError::Tiers(_)), "a malformed pg schema must fail loud as a Tiers error, got {err:?}");
        assert!(err.to_string().contains("Bad-Schema"), "must name the offending schema: {err}");
    }

    /// s31 3.4: a two-"project" fixture — `project_a` (a real git repo
    /// with one linked `git worktree`) and `project_b` (an unrelated
    /// plain directory, no git repo of its own) — each with its own
    /// omp session under `<home>/.omp/agent/sessions/<encoded-cwd>/`,
    /// carrying one user-role message (a directive) and one assistant
    /// message (a token_usage row). Every path is canonicalized so it
    /// matches exactly what `ProjectScope::resolve`'s own `absolutize`
    /// produces — see that fn's own doc for why this must agree
    /// byte-for-byte regardless of a platform's own symlinked tmp root
    /// (macOS's `/var` -> `/private/var`).
    struct TwoProjectFixture {
        _root: tempfile::TempDir,
        home: tempfile::TempDir,
        canon_yaml: PathBuf,
        project_a: PathBuf,
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git").arg("-C").arg(dir).args(args).status().expect("git must be on PATH for this test");
        assert!(status.success(), "git {args:?} failed in {dir:?}");
    }

    fn write_omp_session(home: &Path, cwd: &str, session_id: &str, user_text: &str, ts_prefix: &str) {
        let dirname = encode_cwd_dirname(&normalize_workspace_key(cwd).unwrap());
        let session_dir = home.join(".omp/agent/sessions").join(dirname);
        std::fs::create_dir_all(&session_dir).unwrap();
        let body = format!(
            "{{\"type\":\"session\",\"id\":\"{session_id}\",\"cwd\":\"{cwd}\"}}\n\
             {{\"type\":\"message\",\"timestamp\":\"{ts_prefix}T00:00:00Z\",\"message\":{{\"role\":\"user\",\"content\":\"{user_text}\"}}}}\n\
             {{\"type\":\"message\",\"timestamp\":\"{ts_prefix}T00:00:01Z\",\"message\":{{\"role\":\"assistant\",\"model\":\"gpt-4o-mini\",\"provider\":\"openai\",\"usage\":{{\"input\":10,\"output\":5}}}}}}\n"
        );
        std::fs::write(session_dir.join(format!("{session_id}.jsonl")), body).unwrap();
    }

    fn build_two_project_fixture(routed: bool) -> TwoProjectFixture {
        let root = tempfile::tempdir().unwrap();

        let project_a_raw = root.path().join("project-a");
        std::fs::create_dir_all(&project_a_raw).unwrap();
        git(&project_a_raw, &["init", "-q"]);
        git(&project_a_raw, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-q", "-m", "init"]);
        let worktree_raw = root.path().join("project-a-wt");
        git(&project_a_raw, &["worktree", "add", "-q", "-b", "wt-branch", worktree_raw.to_str().unwrap()]);

        let project_b_raw = root.path().join("project-b");
        std::fs::create_dir_all(&project_b_raw).unwrap();

        let project_a = std::fs::canonicalize(&project_a_raw).unwrap();
        let project_a_worktree = std::fs::canonicalize(&worktree_raw).unwrap();
        let project_b = std::fs::canonicalize(&project_b_raw).unwrap();

        let home = tempfile::tempdir().unwrap();
        write_omp_session(home.path(), &project_a.to_string_lossy(), "ing_ses_a", "do the a thing", "2026-07-01");
        write_omp_session(home.path(), &project_a_worktree.to_string_lossy(), "ing_ses_a_wt", "do the worktree thing", "2026-07-02");
        write_omp_session(home.path(), &project_b.to_string_lossy(), "ing_ses_b", "do the b thing", "2026-07-03");

        let routing = if routed { "  session: local\n  run: local\n  event: local\n" } else { "" };
        let canon_yaml = project_a.join("canon.yaml");
        std::fs::write(&canon_yaml, format!("tiers:\n  local: {{ backend: git, root: .canon/ledger }}\nrouting:\n{routing}\n")).unwrap();

        TwoProjectFixture { _root: root, home, canon_yaml, project_a }
    }

    #[test]
    fn default_scope_uses_metadata_only_unwritten_output() {
        let fixture = build_two_project_fixture(false);

        let outcome = run(&fixture.canon_yaml, fixture.home.path(), false, false, false).unwrap();
        assert_eq!(outcome.sessions_normalized, 2, "project A's own session + its linked worktree's session; project B excluded");
        assert!(outcome.scope_summary.contains("2 roots"), "scope: {}", outcome.scope_summary);

        let json = format_json(&outcome).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        let by_id = |id: &str| payload["sessions"].as_array().unwrap().iter().find(|s| s["session_id"] == id).cloned();

        let expected_project_key = fixture.project_a.to_string_lossy().replace('\\', "/");
        let a = by_id("ing_ses_a").expect("project A's own session is ingested");
        assert_eq!(a["project_key"], expected_project_key);
        let wt = by_id("ing_ses_a_wt").expect("the linked worktree's session is ingested");
        assert_eq!(wt["project_key"], expected_project_key, "the worktree session carries the MAIN worktree's project_key (spec: aggregation by project)");
        assert!(by_id("ing_ses_b").is_none(), "the foreign project's session must be excluded: {json}");
        assert!(!json.contains("do the a thing"), "default degraded JSON must not contain directive text: {json}");
        assert!(payload["events"].as_array().unwrap().iter().all(|event| event["label"] != "user_directive"));
    }


    #[test]
    fn degraded_json_omits_captured_directive_text() {
        let fixture = build_two_project_fixture(false);
        std::fs::write(
            &fixture.canon_yaml,
            "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n\
             ingest:\n  sessions:\n    privacy:\n      capture_user_directives: true\n      max_directive_chars: 4096\n",
        )
        .unwrap();

        let outcome = run(&fixture.canon_yaml, fixture.home.path(), false, false, false).unwrap();
        let json = format_json(&outcome).unwrap();
        assert!(!json.contains("do the a thing"), "degraded metadata must omit captured directive text: {json}");
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(payload["failure"]["class"], "unrouted");
        assert!(payload["events"].as_array().unwrap().iter().any(|event| event["label"] == "user_directive"));
    }
    #[test]
    fn all_workspaces_flag_restores_the_machine_wide_scan() {
        let fixture = build_two_project_fixture(false);

        let outcome = run(&fixture.canon_yaml, fixture.home.path(), false, false, true).unwrap();
        assert_eq!(outcome.sessions_normalized, 3, "all three sessions are ingested with --all-workspaces");
        assert_eq!(outcome.scope_summary, "all workspaces");

        let json = format_json(&outcome).unwrap();
        assert!(json.contains("ing_ses_a") && json.contains("ing_ses_a_wt") && json.contains("ing_ses_b"), "{json}");
    }

    #[test]
    fn default_scope_second_pass_is_steady_state_zero_reparsed() {
        let fixture = build_two_project_fixture(true);

        let first = run(&fixture.canon_yaml, fixture.home.path(), false, false, false).unwrap();
        assert_eq!(first.runs_written, 2);
        assert!(first.unwritten.is_none());

        let second = run(&fixture.canon_yaml, fixture.home.path(), false, false, false).unwrap();
        assert_eq!(second.runs_written, 0, "steady state: nothing new to persist");
        let total_reparsed: usize = second.adapters.iter().map(|a| a.reparsed).sum();
        assert_eq!(total_reparsed, 0, "every present in-scope file was unchanged this pass");
    }

    /// s40 (`plan-vs-actual-diff`) task 4.1, the migration decision:
    /// version 1's id is the pre-s40 id VERBATIM. Every session cursor
    /// written before s40 was produced by the normalization that is now
    /// version 1, so a pre-s40 cursor and a v1 cursor are the same
    /// cursor; suffixing v1 would orphan every cursor in the field to
    /// gain nothing. This asserts the shape the rest of the suite
    /// depends on (`.canon/ingest/cursors/omp.json` in
    /// `routed_policy_persists_through_canon_store_git_tier`).
    #[test]
    fn version_one_keeps_the_pre_s40_cursor_id_verbatim() {
        assert_eq!(session_source_cursor_id("omp", 1), "omp");
        assert_eq!(session_source_cursor_id("claude-code", 1), "claude-code");
    }

    /// The bump must LAND SOMEWHERE ELSE — that is the entire
    /// mechanism (s40 task 4.2): a different id means `cursors.read`
    /// finds nothing and every transcript is re-read.
    #[test]
    fn a_bumped_parse_version_yields_a_different_cursor_id_for_the_same_adapter() {
        assert_ne!(session_source_cursor_id("omp", 1), session_source_cursor_id("omp", 2));
        assert_ne!(session_source_cursor_id("omp", 2), session_source_cursor_id("omp", 3));
        assert_eq!(session_source_cursor_id("omp", 2), "omp-v2");
    }

    /// Version `0` is a declared generation of its own, not a synonym
    /// for 1 — folding it into the unsuffixed form would make two
    /// different declarations share one cursor.
    #[test]
    fn version_zero_is_not_folded_into_version_ones_id() {
        assert_ne!(session_source_cursor_id("omp", 0), session_source_cursor_id("omp", 1));
        assert_eq!(session_source_cursor_id("omp", 0), "omp-v0");
    }
    #[test]
    fn privacy_policy_is_part_of_cursor_identity() {
        let metadata = DirectivePrivacyPolicy::METADATA_ONLY;
        let captured_128 = DirectivePrivacyPolicy { capture_user_directives: true, max_directive_chars: 128 };
        let captured_256 = DirectivePrivacyPolicy { capture_user_directives: true, max_directive_chars: 256 };
        let metadata_128 = DirectivePrivacyPolicy { capture_user_directives: false, max_directive_chars: 128 };

        assert_ne!(session_source_cursor_id_with_privacy("omp", 1, metadata), session_source_cursor_id_with_privacy("omp", 1, captured_128));
        assert_ne!(session_source_cursor_id_with_privacy("omp", 1, captured_128), session_source_cursor_id_with_privacy("omp", 1, captured_256));
        assert_ne!(session_source_cursor_id_with_privacy("omp", 1, metadata), session_source_cursor_id_with_privacy("omp", 1, metadata_128));
        assert!(session_source_cursor_id_with_privacy("omp", 1, metadata).ends_with("-privacy-metadata-0"));
        assert!(session_source_cursor_id_with_privacy("omp", 1, captured_128).ends_with("-privacy-capture-128"));
    }

    #[test]
    fn degraded_reason_drops_credential_laden_backend_diagnostics() {
        let reason = safe_tier_degrade_reason(
            canon_store::policy::Rung::Hot,
            Some(canon_store::policy::Backend::Postgres),
            "connect failed for postgres://alice:super-secret@example.invalid/canon",
        );
        assert!(reason.contains("no live DSN"));
        assert!(!reason.contains("super-secret"));
        assert!(!reason.contains("alice@"));
    }

    #[test]
    fn degraded_reason_preserves_backtick_wrapped_env_name() {
        let reason = safe_tier_degrade_reason(
            canon_store::policy::Rung::Hot,
            Some(canon_store::policy::Backend::Postgres),
            "`CANON_PG_DSN_1` is unset",
        );
        assert!(reason.contains("CANON_PG_DSN_1 is unset"));
        assert!(!reason.contains('`'));
        assert!(!reason.contains("no live DSN"));
    }

    /// A cursor id is used as a bare filename component
    /// (`CursorStore::path_for` joins it onto the cursor root
    /// verbatim), and one adapter must never resolve to another's
    /// cursor. Both properties are consequences of the adapter-id
    /// grammar (`canon_ingest::session_adapter_id_violation`), so this
    /// drives the REAL registry through the rendering and asserts the
    /// consequences directly rather than the grammar again: every id
    /// lowercase `[a-z0-9-]` with no outer `-` and a non-empty stem
    /// (an empty id would render the stemless dotfile `.json`), and
    /// every `(adapter, version)` pair landing on its OWN filename
    /// across a version range wide enough to cross digit lengths
    /// (`-v9` vs `-v10`, where a careless concatenation could alias).
    #[test]
    fn cursor_ids_stay_distinct_per_adapter_and_safe_as_bare_filenames() {
        let mut seen: BTreeMap<String, (&str, u32)> = BTreeMap::new();
        for entry in registry() {
            assert_eq!(
                canon_ingest::session_adapter_id_violation(entry.client_id()),
                None,
                "adapter id `{}` breaks the grammar this rendering's injectivity depends on",
                entry.client_id()
            );
            for version in 0u32..=20 {
                let id = session_source_cursor_id(entry.client_id(), version);
                assert!(!id.is_empty(), "an empty cursor id renders the stemless dotfile `.json`");
                assert!(!id.starts_with('-') && !id.ends_with('-'), "cursor id `{id}` must not start or end with `-`");
                assert!(
                    id.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-')),
                    "cursor id `{id}` must stay a lowercase, separator-free bare filename component"
                );
                if let Some(prior) = seen.insert(id.clone(), (entry.client_id(), version)) {
                    panic!("cursor id `{id}` is claimed by both {prior:?} and ({:?}, {version})", entry.client_id());
                }
            }
        }
    }

    /// The grammar rule that CARRIES the injectivity, stated as the
    /// collision it prevents rather than as a property: an adapter named
    /// `omp-v2` at version 1 renders exactly the id `omp` renders at
    /// version 2. Nothing inside [`session_source_cursor_id`] can detect
    /// that — both adapters would gate on `omp-v2.json`, find the
    /// other's per-file digests there, and report their own transcripts
    /// `unchanged`. The reserved `-v<digits>` suffix (and, for the same
    /// reason against `plans::plan_source_cursor_id`'s family, the
    /// reserved `plan-` prefix) is what makes such an adapter
    /// unregisterable.
    #[test]
    fn the_reserved_suffix_and_prefix_are_what_stop_two_sources_sharing_one_cursor() {
        assert_eq!(
            session_source_cursor_id("omp-v2", 1),
            session_source_cursor_id("omp", 2),
            "the collision is real — which is exactly why the id grammar must forbid the name"
        );
        assert_eq!(
            canon_ingest::session_adapter_id_violation("omp-v2"),
            Some(canon_ingest::SessionAdapterIdViolation::ReservedVersionSuffix),
            "an adapter that could steal another's bumped cursor must not be registerable"
        );
        assert_eq!(
            canon_ingest::session_adapter_id_violation("plan-openspec"),
            Some(canon_ingest::SessionAdapterIdViolation::ReservedPlanNamespace),
            "session and plan cursors share one directory, so the plan family's prefix is not a legal session id"
        );
    }

    /// s37 (`execution-graph-topology`), the case a blanket "every
    /// adapter is at 1" hid: the Claude adapter's parse output CHANGED
    /// for identical bytes — sidechain rows and directives now carry
    /// `agent_id`/`parent_agent_id`, which `canon_ingest::normalize`
    /// turns into child runs — so the `claude-code.json` cursor every
    /// existing installation already holds must NOT gate this build's
    /// parse. Drives the production composition (`registry()`'s DECLARED
    /// `parse_version` → [`session_source_cursor_id`] →
    /// [`base_session_cursor`] → [`SourceCursor::diff`], exactly as
    /// [`run`] composes them) against a stored legacy cursor that
    /// already covers the transcript. With the adapter left at `1` this
    /// fails on the `changed_or_new` assertion: the legacy cursor IS the
    /// v1 cursor, every untouched sidechain returns `unchanged`, and the
    /// lineage this change ships is never backfilled.
    #[test]
    fn a_pre_s37_claude_cursor_cannot_suppress_the_lineage_backfill() {
        let dir = tempfile::tempdir().unwrap();
        let cursors = CursorStore::open(dir.path().join(paths::INGEST_CURSORS_DIR));
        let transcript = dir.path().join(".claude/projects/-tmp-proj/sidechain.jsonl");
        let digest = "a".repeat(64);
        let present = BTreeMap::from([(transcript.to_string_lossy().into_owned(), digest.clone())]);

        // The cursor in the field: written at the unsuffixed v1 id by
        // the PRE-s37 normalization, already covering this transcript at
        // the very digest it still has.
        let mut legacy = SourceCursor::empty("claude-code");
        legacy.record(&transcript, 0, 0, digest);
        legacy.refresh_summary();
        cursors.write(&legacy).unwrap();
        assert_eq!(
            base_session_cursor(&cursors, "claude-code", false).diff(&present).unchanged.len(),
            1,
            "control: the legacy cursor really does cover this transcript, so only the identity can save the backfill"
        );

        let entry = canon_ingest::registry::find("claude-code").expect("the claude-code adapter is registered");
        let cursor_id = session_source_cursor_id(entry.client_id(), entry.adapter.parse_version());
        assert_ne!(cursor_id, "claude-code", "changed normalization must not reuse the identity the old cursors were computed under");

        let diff = base_session_cursor(&cursors, &cursor_id, false).diff(&present);
        assert_eq!(diff.changed_or_new.len(), 1, "an untouched sidechain must be re-read once so its agent lineage is backfilled");
        assert!(diff.unchanged.is_empty());
        assert!(dir.path().join(".canon/ingest/cursors/claude-code.json").exists(), "the pre-s37 cursor is ORPHANED, never deleted or mutated");
    }

    /// s40 task 4.2, end to end on the REAL cursor a real pass wrote:
    /// ingest at the shipped version, ingest again (watermark skip),
    /// then read the same corpus at version N+1 and watch every
    /// transcript come back as `changed_or_new` — with no `--full` and
    /// no cursor deletion. The registry is static, so the third pass
    /// drives the production gate
    /// ([`session_source_cursor_id`] + [`base_session_cursor`] +
    /// [`SourceCursor::diff`], exactly as [`run`] composes them)
    /// against the bumped identity rather than mutating the registry.
    #[test]
    fn a_parse_version_bump_re_reads_a_source_the_watermark_would_have_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_home(dir.path());
        let canon_yaml = write_canon_yaml(dir.path(), true);

        let first = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(first.adapters[0].client_id, "omp");
        assert_eq!(first.runs_written, 1, "pass 1 parses and persists");

        let second = run(&canon_yaml, dir.path(), false, false, true).unwrap();
        assert_eq!(second.adapters[0].reparsed, 0, "pass 2 at the SAME parse version is watermark-skipped");
        assert!(second.adapters[0].skipped_unchanged >= 1);

        let cursors = CursorStore::open(dir.path().join(paths::INGEST_CURSORS_DIR));
        let transcript = dir.path().join(".omp/agent/sessions/-tmp-proj/s1.jsonl");
        let present = BTreeMap::from([(transcript.to_string_lossy().into_owned(), file_digest(&std::fs::read(&transcript).unwrap()))]);

        let privacy = DirectivePrivacyPolicy::METADATA_ONLY;
        let at_shipped = base_session_cursor(&cursors, &omp_cursor_id(privacy), false).diff(&present);
        assert_eq!(at_shipped.unchanged.len(), 1, "control: at the shipped version the transcript is still skipped");
        assert!(at_shipped.changed_or_new.is_empty());

        let at_bumped = base_session_cursor(&cursors, &omp_bumped_cursor_id(privacy), false).diff(&present);
        assert_eq!(at_bumped.changed_or_new.len(), 1, "a bumped parse version must re-read the transcript with no --full and no cursor deletion");
        assert!(at_bumped.unchanged.is_empty());

        let cursor_id = omp_cursor_id(privacy);
        assert!(dir.path().join(paths::INGEST_CURSORS_DIR).join(format!("{cursor_id}.json")).exists(), "the pre-bump cursor is ORPHANED, never deleted or mutated");
    }
}
