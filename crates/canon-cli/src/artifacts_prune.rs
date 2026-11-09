//! `canon artifacts prune [--keep-days N] [--commit --plan-digest D]` (0.38): delete the blobs no record binds any more.
//!
//! The artifact store only grows: a report written anew on every run adds one blob per run, and nothing took them out. The gate and
//! `canon artifacts verify` read only the blobs the latest evidence and the current reviews bind
//! ([`canon_gate::artifact_store::bound_files`]); every other blob is history that nothing reads.
//!
//! # What stays
//! A blob stays for every reason below that holds, and the preview names them all:
//!
//! - `latest-evidence`: the latest evidence of a task or of a `(project, scenario)` binds it (what the gate and `artifacts verify`
//!   read);
//! - `current-review`: a review that is still in effect binds it (the same set);
//! - `staged`: a record not promoted yet (`<ledger>/_staging`) binds it: promotion would make it the latest;
//! - `within-keep-days`: with `--keep-days N`, an evidence record or a review, in the ledger or in its archive, written in the last
//!   `N` days binds it.
//!
//! Everything else in the configured store is deleted. A repo whose store is an object-store tier is pruned by the same rule in the
//! tier; a blob copy left in the local directory of such a repo is judged by it too, so a copy and its object never part ways.
//!
//! # Preview and commit
//! The bare run is the preview (the destructive leaf's handshake, [`crate::safety`]): it counts what goes and what stays with the
//! reason of each kept blob, lists the blobs to delete a page at a time (`--limit`, `--cursor`, `--all`; the totals are the whole's)
//! and writes nothing. `--commit --plan-digest <D>` recomputes the plan and acts only on the very plan the digest names: it covers the
//! blobs to delete, the blobs to keep with their reasons and a digest of every record the plan read, so evidence added since, or a
//! record that changed, is `PLAN_STALE` (exit 2) and nothing is deleted. A plan with nothing to delete prints no commit command.
//!
//! # What it refuses
//! A ledger, a staging area or an archive it cannot read in full, a review routing it cannot see and a store it cannot list: the keep
//! list is then not known, and a blob is never deleted on a guess. A commit holds the repository write lock for the whole run and the
//! artifact-store lock while it deletes.
//!
//! # Partly done
//! A delete that fails stops the run. When nothing was deleted yet it is `ARTIFACTS_STORE_FAILED` (an object) or `WRITE_FAILED` (a
//! local file); after some were deleted it is `ARTIFACTS_PRUNE_INCOMPLETE` with the counts in `data`. Deleting is idempotent: a new
//! preview of the repo that was left half pruned lists what remains.
//!
//! # Git
//! A deleted local blob leaves the working tree, so the removal needs a commit; the blob stays in git history until the repo owner
//! shrinks that history, which is not this command's call. A deleted object of a tier is gone: the tier keeps no other copy.
//!
//! # Stale temp files (0.39)
//! A blob write stages `.canon/artifacts/sha256/.tmp-<sha256>-<pid>-<nanos>` and renames it when complete; a write killed in between leaves it.
//! The preview counts the **stale** ones (a regular file of that name shape, modified more than five minutes ago, [`crate::stale_temps`]) in
//! `data.staleTemps` beside the blobs, and the commit deletes exactly the ones the plan lists, which are part of the plan digest: a plan with
//! a temp file more or fewer is `PLAN_STALE`. A temp file is no finished blob, so deleting one changes no record; one git already tracks must
//! be taken out of the commit that holds it. The commit checks each file again under the artifact-store lock and skips one that is gone, no
//! longer a regular file or no longer old enough; a plan with temp files and no blob to delete still commits.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use canon_gate::artifact_store::{artifacts_config, bound_files, is_sha256_hex, store_dir, ArtifactStore, ArtifactStoreError, Binder, ObjectArtifacts, StoreLock, OBJECT_STORE_HINT};
use canon_gate::{GateContext, GateCtx};
use canon_model::{EvidenceRecord, RecordKind, Review};
use canon_policy::SchemaRegistry;
use canon_store::artifact_blobs::ArtifactsBackend;
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier as _, TierQuery};
use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::Serialize;

use crate::artifacts::local_blobs;
use crate::cli_error::{CliError, CliErrorExt as _};
use crate::context::resolve_repo_root;
use crate::json_out::Report;
use crate::paging::{paginate, Page};
use crate::safety::{digest_of, Effect, Mode, Plan, Preview, Reads};
use crate::stale_temps;

const CMD: &str = "canon artifacts prune";
const LEAF: &str = "artifacts prune";

/// What a stale temp file's deletion leaves for git: the file was never a blob, and nothing records it.
const TEMP_GIT_NOTE: &str = "a stale temp file that git already tracks stays in the commit that holds it: take it out of the commit (`git rm`, then commit); deleting it here changes only the working tree";

/// How many temp file names a preview lists; the totals count them all.
const MAX_LISTED_TEMPS: usize = 50;

/// Why a blob stays: the names the preview gives, in the order it lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Reason {
    LatestEvidence,
    CurrentReview,
    Staged,
    WithinKeepDays,
}

impl Reason {
    const fn as_str(self) -> &'static str {
        match self {
            Reason::LatestEvidence => "latest-evidence",
            Reason::CurrentReview => "current-review",
            Reason::Staged => "staged",
            Reason::WithinKeepDays => "within-keep-days",
        }
    }
}

/// `canon artifacts prune`'s `data`: what the plan deletes and keeps, the page of the blobs to delete, and what a commit did.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsPruneReport {
    /// `local`, or the name of the artifact store tier the blobs live in.
    pub backend: String,
    /// `--keep-days`, when it was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_days: Option<u64>,
    /// What the plan deletes, over the whole store (not only the page below).
    pub delete: PruneCounts,
    /// What the plan keeps, each blob with every reason it stays.
    pub keep: PruneKeep,
    /// What this run deleted: all zero in a preview.
    pub deleted: PruneDeleted,
    /// Present when the plan deletes objects of a tier, which cannot be undone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irreversible: Option<String>,
    /// Present when the plan deletes local blobs: what stays in git.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_note: Option<String>,
    /// The stale temp files of the local store the plan deletes (a dead write's leftovers), apart from the blobs.
    pub stale_temps: PruneTemps,
    /// One page of the blobs the plan deletes, in digest order: empty in a commit (`meta.page` pages a preview).
    pub blobs: Vec<PruneBlob>,
    #[serde(flatten)]
    pub plan: Preview,
}

impl Report for ArtifactsPruneReport {
    /// 1.1: the plan is `atip::plan::Preview` (`planDigest` is `sha256:` and 64 hex digits). 1.2: `staleTemps` and `deleted.staleTemps`.
    const SCHEMA_VERSION: &'static str = "1.2";
}

/// What a run deleted: the blobs, and the stale temp files apart from them (a temp file is no blob and never counts as one).
#[derive(Debug, Clone, Copy, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PruneDeleted {
    #[serde(flatten)]
    pub counts: PruneCounts,
    /// Stale temp files this run deleted.
    pub stale_temps: TempCounts,
}

/// Files and bytes of a set of stale temp files.
#[derive(Debug, Clone, Copy, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TempCounts {
    pub files: usize,
    pub bytes: u64,
}

/// The stale temp files of the local store (`.tmp-<sha256>-<pid>-<nanos>`, a regular file modified more than five minutes ago): what a write that
/// died left. They sit beside the blobs and are not counted among them, in a tier repo too (they are local files).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PruneTemps {
    /// How many the plan deletes, over the whole store (not only the names below).
    pub files: usize,
    /// Their bytes together.
    pub bytes: u64,
    /// The first 50 of them in name order, empty in a commit: `files` says how many there are in all.
    pub items: Vec<TempFile>,
    /// Present when the plan deletes temp files: a temp file already committed must be taken out of the commit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_note: Option<String>,
}

/// One stale temp file a plan deletes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TempFile {
    /// Its name in `.canon/artifacts/sha256`.
    pub name: String,
    pub bytes: u64,
}

/// Blobs and bytes of a set of blobs of the store.
#[derive(Debug, Clone, Copy, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PruneCounts {
    /// Distinct blobs (digests).
    pub blobs: usize,
    /// Bytes of every copy of them.
    pub bytes: u64,
    /// Copies in the local directory.
    pub local_copies: usize,
    /// Objects in the tier.
    pub tier_objects: usize,
}

/// The blobs a plan keeps.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PruneKeep {
    pub blobs: usize,
    pub bytes: u64,
    /// How many kept blobs each reason holds (a blob with several reasons is counted under each).
    pub reasons: BTreeMap<String, usize>,
    /// Every kept blob, in digest order.
    pub items: Vec<KeptBlob>,
}

/// One blob that stays.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeptBlob {
    pub sha256: String,
    pub bytes: u64,
    /// `latest-evidence`, `current-review`, `staged`, `within-keep-days`: every one that holds.
    pub reasons: Vec<String>,
}

/// One blob a plan deletes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PruneBlob {
    pub sha256: String,
    pub bytes: u64,
    /// Where it is deleted from: `local`, the tier's name, or both.
    pub stored_in: Vec<String>,
}

/// One blob of the store: its copy in the local directory and its object in the tier, whichever exist.
struct Blob {
    digest: String,
    local: Option<(PathBuf, u64)>,
    object: Option<u64>,
    /// The forms its tier objects take (`zst=<bytes>`, `raw=<bytes>`), in the plan's read state.
    forms: String,
    reasons: BTreeSet<Reason>,
}

impl Blob {
    fn bytes(&self) -> u64 {
        self.local.as_ref().map_or(0, |(_, bytes)| *bytes) + self.object.unwrap_or(0)
    }
}

/// What the records bind: the blobs with their reasons, and a digest line of every record the plan read.
#[derive(Default)]
struct Bindings {
    reasons: BTreeMap<String, BTreeSet<Reason>>,
    state: BTreeSet<String>,
}

impl Bindings {
    fn bind(&mut self, digest: &str, reason: Reason) {
        // A digest that is no blob name binds nothing in the store.
        if is_sha256_hex(digest) {
            self.reasons.entry(digest.to_string()).or_default().insert(reason);
        }
    }

    fn read<T: Serialize>(&mut self, what: &str, record: &T) -> Result<(), CliError> {
        let text = serde_json::to_string(record).map_err(|e| CliError::failed("INTERNAL", format!("canon could not digest a record the plan read: {e}")).hint("report it with the command line you ran"))?;
        self.state.insert(format!("{what}:{}", digest_of(text.as_bytes())));
        Ok(())
    }

    fn evidence(&mut self, record: &EvidenceRecord, reason: Reason) {
        for attachment in &record.attachments {
            self.bind(&attachment.sha256, reason);
        }
    }

    fn review(&mut self, review: &Review, reason: Reason) {
        for artifact in &review.artifacts {
            self.bind(&artifact.sha256, reason);
        }
    }
}

/// The refusal for records the plan cannot read in full: the keep list is not known, so nothing is deleted.
fn unreadable(what: &str, why: impl std::fmt::Display) -> CliError {
    crate::read_cause::repo_error(format!("cannot read {what}: {why}")).hint("the blobs the records bind cannot be known, so nothing is deleted; fix what the message names (`canon gate check` shows it), then run the command again")
}

/// One violation the ledger read found, as a sentence.
fn problem(violation: &canon_model::EvidenceViolation) -> String {
    format!("{}: {}", violation.subject, violation.detail)
}

/// A page of raw records read as `T`: one that does not read as it is has a binding no plan can know.
fn typed<T: serde::de::DeserializeOwned>(what: &str, raw: &canon_model::RawRecord) -> Result<T, CliError> {
    <T as serde::Deserialize>::deserialize(&raw.0).map_err(|e| unreadable(what, format!("a record does not read as a {what}: {e}")))
}

/// Every blob the records bind, with why (the module doc's list), and what the plan read to know it.
fn read_bindings(context: &GateContext, keep_days: Option<u64>) -> Result<Bindings, CliError> {
    // The gate reads only what it can: a record it cannot read, or a review kind routed where it cannot look, is a binding it would miss.
    if let Some(violation) = context.violations.first() {
        return Err(unreadable("the evidence ledger", format!("{} ({} problem(s) in all)", problem(violation), context.violations.len())));
    }
    if let Some((kind, violation)) = context.corpus_violations.iter().find(|(kind, _)| matches!(kind, RecordKind::Review)) {
        return Err(unreadable("the reviews", format!("{} record: {}", kind.as_str(), problem(violation))));
    }
    if context.unreadable_kinds.contains(&RecordKind::Review) {
        return Err(unreadable("the reviews", "canon.yaml routes them off the local ledger, where the gate does not read them"));
    }
    let mut bindings = Bindings::default();

    for file in bound_files(context) {
        let reason = match file.binder {
            Binder::Evidence => Reason::LatestEvidence,
            Binder::Review { .. } => Reason::CurrentReview,
        };
        bindings.bind(&file.sha256, reason);
    }
    for record in context.evidence.iter().chain(&context.audits) {
        bindings.read("evidence", record)?;
    }
    for review in &context.reviews {
        bindings.read("review", review)?;
    }

    // Records not promoted yet: promotion makes them the latest, so what they bind stays.
    let staging = GitTier::new(crate::gate::evidence_staging_dir(&context.ctx.ledger_root));
    let staged = staging.read(&TierQuery::kind(RecordKind::Evidence)).map_err(|e| unreadable("the staged evidence", e))?;
    if let Some(violation) = staged.violations.first() {
        return Err(unreadable("the staged evidence", format!("{} ({} problem(s) in all)", problem(violation), staged.violations.len())));
    }
    for raw in &staged.records {
        let record: EvidenceRecord = typed("staged evidence record", raw)?;
        bindings.evidence(&record, Reason::Staged);
        bindings.read("staged-evidence", &raw.0)?;
    }

    if let Some(days) = keep_days {
        let cutoff = keep_cutoff(context.now, days);
        let ledger = GitTier::new(context.ctx.ledger_root.clone());
        for (kind, name) in [(RecordKind::Evidence, "evidence"), (RecordKind::Review, "review")] {
            let history = ledger.read_history(kind).map_err(|e| unreadable(&format!("the {name} history"), e))?;
            if let Some(violation) = history.violations.first() {
                return Err(unreadable(&format!("the {name} history"), format!("{} ({} problem(s) in all)", problem(violation), history.violations.len())));
            }
            for raw in &history.records {
                if kind == RecordKind::Evidence {
                    let record: EvidenceRecord = typed("evidence record", raw)?;
                    bindings.read("history-evidence", &raw.0)?;
                    if record.envelope.at >= cutoff {
                        bindings.evidence(&record, Reason::WithinKeepDays);
                    }
                } else {
                    let review: Review = typed("review", raw)?;
                    bindings.read("history-review", &raw.0)?;
                    if review.envelope.at >= cutoff {
                        bindings.review(&review, Reason::WithinKeepDays);
                    }
                }
            }
        }
    }
    Ok(bindings)
}

/// The start of the `days` the records are kept for: a record written at or after it is within them.
fn keep_cutoff(now: DateTime<Utc>, days: u64) -> DateTime<Utc> {
    let days = i64::try_from(days).unwrap_or(i64::MAX);
    Duration::try_days(days).and_then(|span| now.checked_sub_signed(span)).unwrap_or(DateTime::<Utc>::MIN_UTC)
}

/// Every blob of the configured store, plus the copies in the local directory of a tier repo.
fn store_blobs(repo: &Path, store: &ArtifactStore) -> Result<BTreeMap<String, Blob>, CliError> {
    let mut blobs: BTreeMap<String, Blob> = BTreeMap::new();
    let (local, _) = local_blobs(repo).map_err(|e| {
        let message = format!("cannot read the local artifact store: {e}");
        crate::artifacts::store_refusal(&ArtifactStoreError::Io(e), message).hint("check that .canon/artifacts/sha256 is a readable directory, then run the command again")
    })?;
    for blob in local {
        let bytes = std::fs::metadata(&blob.path).map(|meta| meta.len()).map_err(|e| {
            let message = format!("cannot read {}: {e}", blob.path.display());
            crate::artifacts::store_refusal(&ArtifactStoreError::Io(e), message).hint("check that .canon/artifacts/sha256 is a readable directory, then run the command again")
        })?;
        blobs.insert(blob.digest.clone(), Blob { digest: blob.digest, local: Some((blob.path, bytes)), object: None, forms: String::new(), reasons: BTreeSet::new() });
    }
    if let ArtifactStore::Object(object) = store {
        let listed = object.list_objects().map_err(|e| crate::artifacts::store_error(&e).hint(OBJECT_STORE_HINT))?;
        for entry in listed {
            let forms = [("zst", entry.zst), ("raw", entry.raw)].iter().filter_map(|(name, bytes)| bytes.map(|bytes| format!("{name}={bytes}"))).collect::<Vec<_>>().join(",");
            let blob = blobs.entry(entry.digest.clone()).or_insert_with(|| Blob { digest: entry.digest.clone(), local: None, object: None, forms: String::new(), reasons: BTreeSet::new() });
            blob.object = Some(entry.bytes());
            blob.forms = forms;
        }
    }
    Ok(blobs)
}

fn counts<'a>(blobs: impl IntoIterator<Item = &'a Blob>) -> PruneCounts {
    let mut total = PruneCounts::default();
    for blob in blobs {
        total.blobs += 1;
        total.bytes += blob.bytes();
        total.local_copies += usize::from(blob.local.is_some());
        total.tier_objects += usize::from(blob.object.is_some());
    }
    total
}

/// `canon artifacts prune`.
pub fn run_prune(repo: &crate::cli::RepoArgs, keep_days: Option<u64>, page: &Page, mode: &Mode, json: bool) -> u8 {
    let repo = resolve_repo_root(repo);

    let store = match ArtifactStore::open(&repo) {
        Ok(store) => store,
        Err(e) => return crate::artifacts::store_error(&e).emit("canon artifacts prune", json),
    };
    let backend = match artifacts_config(&repo).map(|config| config.backend) {
        Ok(ArtifactsBackend::Tier { name, .. }) => name,
        _ => "local".to_string(),
    };
    let context = match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())) {
        Ok(context) => context,
        Err(e) => return crate::read_cause::config_error(e).emit("canon artifacts prune", json),
    };
    let bindings = match read_bindings(&context, keep_days) {
        Ok(bindings) => bindings,
        Err(e) => return e.emit("canon artifacts prune", json),
    };
    let mut blobs = match store_blobs(&repo, &store) {
        Ok(blobs) => blobs,
        Err(e) => return e.emit("canon artifacts prune", json),
    };
    for blob in blobs.values_mut() {
        if let Some(reasons) = bindings.reasons.get(&blob.digest) {
            blob.reasons.clone_from(reasons);
        }
    }
    let (kept, doomed): (Vec<&Blob>, Vec<&Blob>) = blobs.values().partition(|blob| !blob.reasons.is_empty());
    let delete = counts(doomed.iter().copied());
    let keep = counts(kept.iter().copied());
    // The stale temp files of the local store: not blobs, so never in the keep or delete lists; a store directory that cannot be read is refused.
    let temps = match stale_temps::find(&repo, SystemTime::now()) {
        Ok(temps) => temps,
        Err(e) => {
            let message = format!("cannot read the local artifact store: {e}");
            return crate::artifacts::store_refusal(&ArtifactStoreError::Io(e), message).hint("check that .canon/artifacts/sha256 is a readable directory, then run the command again").emit("canon artifacts prune", json);
        }
    };
    let temp_bytes: u64 = temps.iter().map(|temp| temp.bytes).sum();

    // The plan: what goes (by tier, with the totals), and what it read: the blobs that go, the blobs that stay with their reasons and
    // every record the keep list came from.
    let mut effects = Vec::new();
    if delete.local_copies > 0 {
        let bytes: u64 = doomed.iter().filter_map(|blob| blob.local.as_ref().map(|(_, bytes)| *bytes)).sum();
        effects.push(Effect::path("delete", format!("{}/", canon_model::paths::ARTIFACTS_SHA256_DIR)).detail(format!("{} blob(s), {bytes} bytes; data.blobs lists them", delete.local_copies)));
    }
    if delete.tier_objects > 0 {
        let bytes: u64 = doomed.iter().filter_map(|blob| blob.object).sum();
        effects.push(Effect::record("delete", store.display_dir()).detail(format!("{} object(s), {bytes} bytes; data.blobs lists them; cannot be undone", delete.tier_objects)));
    }
    if !temps.is_empty() {
        effects.push(Effect::path("delete", format!("{}/.tmp-*", canon_model::paths::ARTIFACTS_SHA256_DIR)).detail(format!("{} stale temp file(s), {temp_bytes} bytes; data.staleTemps lists them", temps.len())));
    }
    let reads = Reads::new()
        .path(&repo, "canon.yaml")
        .set("delete", doomed.iter().map(|blob| format!("{} local={} object={}", blob.digest, blob.local.as_ref().map_or(0, |(_, bytes)| *bytes), if blob.forms.is_empty() { "-" } else { blob.forms.as_str() })))
        .set("keep", kept.iter().map(|blob| format!("{} {}", blob.digest, blob.reasons.iter().map(|reason| reason.as_str()).collect::<Vec<_>>().join(","))))
        .set("stale-temps", temps.iter().map(|temp| format!("{} {}", temp.name, temp.bytes)))
        .set("records", bindings.state.iter());
    let plan = match Plan::new(LEAF, effects, reads) {
        Ok(plan) => plan.unpaged(),
        Err(e) => return e.emit("canon artifacts prune", json),
    };

    // A commit deletes local files and takes the store lock: refuse a store that cannot be written before anything goes.
    if matches!(mode, Mode::Commit(_)) && !(doomed.is_empty() && temps.is_empty()) {
        let lock = repo.join(canon_model::paths::ARTIFACT_STORE_LOCK_FILE);
        let dir = store_dir(&repo);
        let mut targets: Vec<&Path> = vec![lock.as_path()];
        if delete.local_copies > 0 || !temps.is_empty() {
            targets.push(dir.as_path());
        }
        if let Some(code) = crate::write_mode::refuse_unwritable(&targets, CMD, json) {
            return code;
        }
    }
    let commit = match plan.gate(mode) {
        Ok(commit) => commit,
        Err(e) => return e.emit("canon artifacts prune", json),
    };

    let irreversible = (delete.tier_objects > 0).then(|| format!("deleting an object of `{backend}` cannot be undone: the tier keeps no other copy ({} object(s), {} bytes)", delete.tier_objects, store_bytes(&doomed, |blob| blob.object)));
    let git_note = (delete.local_copies > 0).then(|| "a deleted local blob stays in git history; shrinking that history is the repo owner's call, not this command's".to_string());
    let stale_temps_report = PruneTemps {
        files: temps.len(),
        bytes: temp_bytes,
        items: temps.iter().take(MAX_LISTED_TEMPS).map(|temp| TempFile { name: temp.name.clone(), bytes: temp.bytes }).collect(),
        git_note: (!temps.is_empty()).then(|| TEMP_GIT_NOTE.to_string()),
    };
    let mut kept_reasons: BTreeMap<String, usize> = BTreeMap::new();
    for blob in &kept {
        for reason in &blob.reasons {
            *kept_reasons.entry(reason.as_str().to_string()).or_default() += 1;
        }
    }
    let keep_report = PruneKeep {
        blobs: keep.blobs,
        bytes: keep.bytes,
        reasons: kept_reasons,
        items: kept.iter().map(|blob| KeptBlob { sha256: blob.digest.clone(), bytes: blob.bytes(), reasons: blob.reasons.iter().map(|reason| reason.as_str().to_string()).collect() }).collect(),
    };

    let kept_after = keep_report.blobs;
    let mut report = ArtifactsPruneReport { backend: backend.clone(), keep_days, delete, keep: keep_report, deleted: PruneDeleted::default(), irreversible, git_note, stale_temps: stale_temps_report, blobs: Vec::new(), plan: plan.preview() };
    if !commit {
        return preview(&plan, report, &doomed, page, json);
    }

    // The commit holds the artifact-store lock while it deletes (`main` holds the repository write lock for the whole run), so no writer
    // adds a blob between the plan and the deletion, and no writer that takes the lock has a temp file in flight.
    let _lock = if doomed.is_empty() && temps.is_empty() {
        None
    } else {
        match StoreLock::acquire(&repo) {
            Ok(lock) => Some(lock),
            Err(e) => return crate::artifacts::store_error(&e).emit("canon artifacts prune", json),
        }
    };
    let object = match &store {
        ArtifactStore::Object(object) => Some(object),
        ArtifactStore::Local => None,
    };
    let mut done = PruneCounts::default();
    let mut stopped: Option<Stop> = None;
    for blob in &doomed {
        if let Err(stop) = delete_blob(blob, object, &mut done) {
            stopped = Some(stop);
            break;
        }
        #[cfg(debug_assertions)]
        if done.blobs == 1 {
            if let Some(e) = crate::incomplete::injected_stop("artifacts-prune-after-delete") {
                stopped = Some(Stop { object: false, message: format!("failed to delete the next blob: {e}"), hint: "fix what the message names, then preview and commit again".to_string() });
                break;
            }
            // The same stop as an object store that gives out after its first delete.
            if let Some(e) = crate::incomplete::injected_stop("artifacts-prune-object-after-delete") {
                stopped = Some(Stop { object: true, message: format!("could not delete the next object: {e}"), hint: format!("{OBJECT_STORE_HINT}; then preview and commit again") });
                break;
            }
        }
    }
    // Then the temp files of the plan, and only those: each is checked again now that the lock is held (a file that is gone, no longer a
    // regular file or no longer old enough is skipped), and no other old temp file is looked for.
    let mut done_temps = TempCounts::default();
    if stopped.is_none() {
        let swept_at = SystemTime::now();
        for temp in &temps {
            match stale_temps::remove(temp, swept_at) {
                Ok(Some(bytes)) => {
                    done_temps.files += 1;
                    done_temps.bytes += bytes;
                }
                Ok(None) => {}
                Err(e) => {
                    stopped = Some(Stop { object: false, message: format!("failed to remove {}: {e}", temp.path.display()), hint: "fix the permissions of the blob directory, then preview and commit again".to_string() });
                    break;
                }
            }
            #[cfg(debug_assertions)]
            if done_temps.files == 1 {
                if let Some(e) = crate::incomplete::injected_stop("artifacts-prune-temp-after-delete") {
                    stopped = Some(Stop { object: false, message: format!("failed to delete the next stale temp file: {e}"), hint: "fix what the message names, then preview and commit again".to_string() });
                    break;
                }
            }
        }
    }
    report.deleted = PruneDeleted { counts: done, stale_temps: done_temps };
    report.stale_temps.items.clear();
    report.plan = plan.committed();

    if let Some(stop) = stopped {
        if done.blobs == 0 && done_temps.files == 0 {
            // Nothing was deleted: the same call can run again.
            return CliError::failed(stop.code(), format!("{}; nothing was deleted", stop.message)).hint(stop.hint).emit("canon artifacts prune", json);
        }
        crate::incomplete::read_first("canon artifacts verify --json", "read what the stopped prune left: what it deleted is gone and no blob a record binds was touched");
        crate::json_out::prepend_next(crate::status::NextStep {
            command: format!("canon artifacts prune{} --json", keep_days.map_or_else(String::new, |days| format!(" --keep-days={days}"))),
            why: "preview a new plan: what this run deleted is no longer in it (the same --plan-digest is stale now)".into(),
            actor: crate::status::StepActor::Agent,
            role: Some(crate::role::Role::Orchestrator),
        });
        let incomplete = CliError::failed(
            "ARTIFACTS_PRUNE_INCOMPLETE",
            format!("stopped after deleting {} of {} blob(s) and {} of {} stale temp file(s): {}", done.blobs, delete.blobs, done_temps.files, temps.len(), stop.message),
        )
            .hint("what was deleted stays deleted and no blob a record binds was touched; read the state, deal with what the message names, then preview and commit a new plan (the same --plan-digest is stale now)");
        // The reason is the code of the part that failed.
        let incomplete = if stop.object { incomplete.reason("ARTIFACTS_STORE_FAILED") } else { incomplete.reason("WRITE_FAILED") };
        return incomplete.emit_with_data("canon artifacts prune", json, &report);
    }

    crate::say!(
        json,
        "{CMD}: deleted {} blob(s) ({} bytes), kept {kept_after} blob(s) ({} bytes) from {}",
        done.blobs,
        done.bytes,
        keep.bytes,
        store.location()
    );
    if done.local_copies > 0 {
        crate::say!(json, "commit the removal of {} local blob(s) from {}/ with the ledger; they stay in git history until the repo owner shrinks it", done.local_copies, canon_model::paths::ARTIFACTS_SHA256_DIR);
    }
    if done.tier_objects > 0 {
        crate::say!(json, "{} object(s) of `{backend}` are deleted for good: the tier keeps no other copy", done.tier_objects);
    }
    if done_temps.files > 0 {
        crate::say!(json, "deleted {} stale temp file(s) ({} bytes) a write that died left; they were no finished blobs, so no record changed", done_temps.files, done_temps.bytes);
        crate::say!(json, "{TEMP_GIT_NOTE}");
    }
    if doomed.is_empty() && temps.is_empty() {
        crate::json_out::mark_unchanged();
        crate::say!(json, "{CMD}: nothing to delete");
    }
    crate::next::finish(&repo, LEAF, &crate::next::Scope::Repo, false, json, &report);
    0
}

fn store_bytes(blobs: &[&Blob], of: impl Fn(&Blob) -> Option<u64>) -> u64 {
    blobs.iter().filter_map(|blob| of(blob)).sum()
}

/// Why a delete stopped the run: an object of the tier (`ARTIFACTS_STORE_FAILED`) or a local file (`WRITE_FAILED`) could not be deleted.
struct Stop {
    object: bool,
    message: String,
    hint: String,
}

impl Stop {
    /// The code of the refusal a run that had deleted nothing answers, and the reason of the incomplete answer after it had.
    const fn code(&self) -> &'static str {
        if self.object {
            "ARTIFACTS_STORE_FAILED"
        } else {
            "WRITE_FAILED"
        }
    }
}

/// Deletes one blob: its object first (a tier is the call that can fail), then its local copy. `done` counts what went, a blob once
/// when any of its copies did.
fn delete_blob(blob: &Blob, object: Option<&ObjectArtifacts>, done: &mut PruneCounts) -> Result<(), Stop> {
    let mut removed = false;
    let result = (|| {
        if let (Some(bytes), Some(object)) = (blob.object, object) {
            match object.delete(&blob.digest) {
                Ok(()) => {
                    done.tier_objects += 1;
                    done.bytes += bytes;
                    removed = true;
                }
                Err(e) => return Err(Stop { object: true, message: format!("could not delete {}:{}: {e}", object.tier, blob.digest), hint: format!("{OBJECT_STORE_HINT}; then preview and commit again") }),
            }
        }
        if let Some((path, bytes)) = &blob.local {
            match std::fs::remove_file(path) {
                Ok(()) => {
                    done.local_copies += 1;
                    done.bytes += bytes;
                    removed = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Stop { object: false, message: format!("failed to remove {}: {e}", path.display()), hint: "fix the permissions of the blob directory, then preview and commit again".to_string() }),
            }
        }
        Ok(())
    })();
    done.blobs += usize::from(removed);
    result
}

/// The preview's output: the totals, the page of the blobs to delete, and, when there is something to delete, the commit.
fn preview(plan: &Plan, mut report: ArtifactsPruneReport, doomed: &[&Blob], page: &Page, json: bool) -> u8 {
    let items: Vec<PruneBlob> = doomed
        .iter()
        .map(|blob| {
            let mut stored_in = Vec::new();
            if blob.local.is_some() {
                stored_in.push("local".to_string());
            }
            if blob.object.is_some() {
                stored_in.push(report.backend.clone());
            }
            PruneBlob { sha256: blob.digest.clone(), bytes: blob.bytes(), stored_in }
        })
        .collect();
    let (listed, meta) = match paginate(LEAF, items, |blob| blob.sha256.clone(), page) {
        Ok(paged) => paged,
        Err(e) => return e.emit("canon artifacts prune", json),
    };
    crate::say!(json, "{CMD}: delete {} blob(s) ({} bytes), keep {} blob(s) ({} bytes) — nothing written", report.delete.blobs, report.delete.bytes, report.keep.blobs, report.keep.bytes);
    for (reason, count) in &report.keep.reasons {
        crate::say!(json, "  keep {count} for {reason}");
    }
    if let Some(note) = &report.irreversible {
        crate::say!(json, "  {note}");
    }
    if let Some(note) = &report.git_note {
        crate::say!(json, "  {note}");
    }
    if report.stale_temps.files > 0 {
        crate::say!(json, "  delete {} stale temp file(s) ({} bytes) a write that died left in {}/; they are no finished blobs", report.stale_temps.files, report.stale_temps.bytes, canon_model::paths::ARTIFACTS_SHA256_DIR);
        for temp in &report.stale_temps.items {
            crate::say!(json, "    {} ({} bytes)", temp.name, temp.bytes);
        }
        if report.stale_temps.items.len() < report.stale_temps.files {
            crate::say!(json, "    … and {} more (data.staleTemps counts them all)", report.stale_temps.files - report.stale_temps.items.len());
        }
        if let Some(note) = &report.stale_temps.git_note {
            crate::say!(json, "  {note}");
        }
    }
    for blob in &listed {
        crate::say!(json, "  delete {} ({} bytes, {})", blob.sha256, blob.bytes, blob.stored_in.join(" and "));
    }
    if let Some(line) = meta.human_line() {
        crate::say!(json, "  {line}");
    }
    report.blobs = listed;
    if doomed.is_empty() && report.stale_temps.files == 0 {
        // Nothing to delete: no commit command is offered.
        crate::say!(json, "  nothing to delete");
        if json {
            crate::json_out::data_with_meta(LEAF, &report, meta.to_json());
        }
        return 0;
    }
    if json {
        crate::json_out::data_with_next_and_meta(LEAF, &report, &plan.next_step("the plan only lists what it deletes; deleting takes its planDigest"), meta.to_json());
    } else {
        println!("{}", plan.commit_hint());
    }
    0
}
