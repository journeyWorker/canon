//! `canon dispatch begin --role <r> --regime <k> [--repo <dir>]
//! [--agent-id <id>] [--json]` (S8 `retrieve-before-task`, task 2.3):
//! the LIVE run-manifest write seam. Everything else in S8 shipped
//! standalone (design.md Migration Plan Step 1 — `canon retrieve` + the
//! pre-dispatch hook "work with zero manifest integration"); this is
//! Step 2: at the moment a run is dispatched, retrieve the role+regime
//! guidance ONCE and record it verbatim into a `Run` manifest's
//! [`canon_model::records::Run::injected_guidance`], so a later replay
//! reproduces the run's inputs byte-for-byte even after the source
//! strategies are edited or demoted (the whole point of the snapshot
//! field, `Run::injected_guidance`'s own doc).
//!
//! # Why a private side-channel, not canon-store's git tier
//! `canon-ingest`'s own `Run` constructor (`normalize.rs`) is a
//! POST-HOC reconstruction from an already-completed session transcript,
//! written through `canon-store`'s `GitTier` at a canonical Hive-keyed
//! path — and a git-tier duplicate-path write is a HARD ERROR
//! (`canon-store::tier`'s own doc), not an idempotent dedup. A live
//! dispatch-time `Run` and the later post-hoc ingest `Run` for the same
//! session would therefore collide on that path. So the dispatch record
//! lands in a private, non-canonical side-channel
//! (`<repo>/.canon/dispatch/<run_id>.json`), keyed by the freshly-minted
//! `RunId` (unique per dispatch, never colliding), for a future
//! reconciliation step to fold into the canonical tier — never fed
//! through `GitTier`'s Hive scheme here. This is exactly the seam S8's
//! own tasks.md note called "a live run-manifest write seam that does
//! not exist yet".
//!
//! FAIL-SOFT retrieval, FAIL-LOUD write: the retrieval half reuses
//! `canon_learn::retrieve_guidance`'s own fail-soft contract (a store
//! outage yields empty guidance, never an error); only a `--role`/
//! `--regime` usage mismatch (exit `2`) or a filesystem write failure
//! (exit `1`) is surfaced.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use canon_learn::guidance::retrieve_guidance;
use canon_learn::{LearnConfig, ParquetStrategyStore};
use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::RunId;
use canon_model::records::{Run, RunStatus};
use canon_model::{RegimeKey, RoleId};
use canon_store::write_atomic;

use crate::context::resolve_repo_root;

/// The `Run` kind's current envelope schema version — mirrors
/// `canon_ingest::normalize`'s own `SCHEMA_VERSION` const for the Run
/// kind (both write a `RecordKind::Run` envelope; they must agree so a
/// reconciler reads a dispatch-tier and an ingest-tier Run under one
/// schema).
const RUN_SCHEMA_VERSION: u32 = 1;

/// The private side-channel directory a dispatch record lands under,
/// relative to the repo root (module doc: never `canon-store`'s git
/// tier).
pub const DISPATCH_DIR: &str = ".canon/dispatch";

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    /// `--role` disagrees with `--regime`'s own leading segment — the
    /// same caller-contract check `canon retrieve` makes (design
    /// decision 1: regime_key already embeds role as its first segment).
    #[error(
        "--role `{role}` does not match --regime `{regime_key}`'s own leading role segment `{regime_role}` — pass the SAME role to both"
    )]
    RoleRegimeMismatch { role: String, regime_key: String, regime_role: String },

    /// The dispatch record could not be written to the side-channel.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The `Run` manifest could not be serialized (should never happen —
    /// `Run` is always `Serialize`).
    #[error("serializing the dispatch Run manifest: {0}")]
    Serialize(String),
}

/// What [`begin`] produced: the minted run id, the side-channel path the
/// manifest was written to, and the guidance snapshot recorded into it.
#[derive(Debug, Clone)]
pub struct Begun {
    pub run_id: RunId,
    pub manifest_path: PathBuf,
    pub run: Run,
}

/// Resolve `<repo>`'s configured learn root and open the `strategies`
/// parquet tier (same resolution `canon retrieve` uses — never a second
/// convention).
fn open_strategy_store(repo: &Path) -> ParquetStrategyStore {
    let canon_yaml = repo.join("canon.yaml");
    let learn_config =
        std::fs::read_to_string(&canon_yaml).ok().and_then(|text| LearnConfig::from_manifest(&text).ok()).unwrap_or_default();
    ParquetStrategyStore::open(repo.join(learn_config.root).join("strategies"))
}

/// Mint a `Run` (status `Running`), retrieve the role+regime guidance,
/// record it into the run's `injected_guidance`, and persist the
/// manifest to `<repo>/.canon/dispatch/<run_id>.json`. Returns the
/// [`Begun`] record (run id + path + the in-memory `Run`).
pub fn begin(repo: &Path, role: &RoleId, regime_key: &RegimeKey, agent_id: &str) -> Result<Begun, DispatchError> {
    if regime_key.role() != role.as_str() {
        return Err(DispatchError::RoleRegimeMismatch {
            role: role.as_str().to_string(),
            regime_key: regime_key.as_str().to_string(),
            regime_role: regime_key.role().to_string(),
        });
    }
    let repo = resolve_repo_root(repo);
    let store = open_strategy_store(&repo);
    let guidance = retrieve_guidance(&store, role, regime_key, None);

    let run_id = RunId::new();
    let now = chrono::Utc::now();
    let actor = Actor::new(agent_id.to_string(), role.clone());
    let run = Run::new(Envelope::new(RUN_SCHEMA_VERSION, RecordKind::Run, now, actor), run_id, None, None, RunStatus::Running, now, None)
        .with_injected_guidance(guidance);

    let manifest_path = repo.join(DISPATCH_DIR).join(format!("{run_id}.json"));
    let json = serde_json::to_string_pretty(&run).map_err(|e| DispatchError::Serialize(e.to_string()))?;
    // Atomic write (canon-store's shared tempfile+rename primitive): a
    // replay depends on this manifest being complete, so a mid-write
    // kill must never leave a torn `.canon/dispatch/<run_id>.json`.
    write_atomic(&manifest_path, json.as_bytes())?;

    Ok(Begun { run_id, manifest_path, run })
}

/// `canon dispatch begin`'s CLI wrapper: `0` on a written manifest, `2`
/// on a `--role`/`--regime` usage mismatch, `1` on a write/serialize
/// failure.
pub fn run_begin(repo: &Path, role: &RoleId, regime_key: &RegimeKey, agent_id: &str, json: bool) -> ExitCode {
    match begin(repo, role, regime_key, agent_id) {
        Ok(begun) => {
            if json {
                let summary = serde_json::json!({
                    "run_id": begun.run_id.to_string(),
                    "manifest": begun.manifest_path.display().to_string(),
                    "injected_guidance": begun.run.injected_guidance,
                });
                println!("{}", serde_json::to_string_pretty(&summary).expect("summary is always serializable"));
            } else {
                println!(
                    "dispatch {} begun for {} — recorded {} guidance item(s) -> {}",
                    begun.run_id,
                    regime_key.as_str(),
                    begun.run.injected_guidance.len(),
                    begun.manifest_path.display()
                );
            }
            ExitCode::SUCCESS
        }
        Err(err @ DispatchError::RoleRegimeMismatch { .. }) => {
            eprintln!("canon dispatch begin: {err}");
            ExitCode::from(2)
        }
        Err(err) => {
            eprintln!("canon dispatch begin: {err}");
            ExitCode::from(1)
        }
    }
}
