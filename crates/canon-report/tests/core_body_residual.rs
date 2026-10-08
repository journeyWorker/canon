//! Executable pin of the ONE exception to `manifest.json`'s
//! `source_digest` guarantee (`canon_report::manifest::Manifest::
//! source_digest`, "The standing asymmetry"): a file under a CORE
//! `kind=<k>/` directory whose BODY `GitTier::read` refuses still
//! reaches DuckDB through `stg_git_records`' glob, so it can move an
//! exported mart under an unchanged `source_digest`.
//!
//! The asymmetry is structural and permanent, not a bug in either
//! reader. The Rust side VALIDATES — `scan_kind_where` checks the
//! body's `kind`, its resolved layout, and then its full schema
//! through `partition::validate_body`, soft-skipping whatever fails.
//! The SQL side GLOBS — `read_text(kind=*/**/*.json)` parses JSON and
//! nothing else. Every canon value derived from the validated read can
//! therefore disagree with every table derived from the glob, over
//! exactly the corpus the validated read refuses.
//!
//! Two tests, one per direction, because the fix that looks obvious
//! from the first is refuted by the second:
//!
//! - `a_core_kind_body_the_rust_reader_refuses_moves_a_mart_under_an_
//!   unchanged_source_digest` is the counterexample itself. Without
//!   it, `source_digest`'s doc sentence can silently re-absolutise
//!   into "a snapshot whose `mart_scope_status` rows moved cannot
//!   carry an unchanged digest", which is what it said until s43 round
//!   4 found this.
//! - `a_whitespace_only_reformat_of_a_valid_core_record_moves_neither`
//!   is why the exception is NOT closed by hashing core bytes the way
//!   the overlay half is hashed. The core half digests the VALIDATED,
//!   canonically re-serialized record, so a reformatted-but-valid file
//!   moves neither the digest nor any mart — the two agree. Byte-
//!   hashing would move the digest while every exported table stood
//!   still: a false provenance CHANGE traded for a false provenance
//!   CLAIM. `manifest.rs` records the position that follows (this is
//!   corpus hygiene, and the closure is a gate that refuses the file).
//!
//! Sibling pin, same asymmetry one rung over:
//! `multi_version_fold.rs::both_roots_fold_a_digest_the_rust_reader_
//! refuses_to_return` covers the LAYOUT gate (a renamed file). This
//! one covers the BODY-schema gate, which is the half that reaches
//! `manifest.json`'s own guarantee.

mod support;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::evidence::RawRecord;
use canon_model::ids::{RoleId, ScenarioId, TaskId};
use canon_model::records::{Task, TaskStatus};
use canon_report::digest::DigestHeader;
use canon_report::marts;
use canon_report::roots::Roots;
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier, TierQuery};
use serde_json::json;

const VALID_TASK_ID: &str = "core-residual#1";
const VALID_SCENARIO_ID: &str = "residual.valid.01";
const REFUSED_TASK_ID: &str = "core-residual#2";
const REFUSED_SCENARIO_ID: &str = "residual.refused.02";

fn at() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-03-04T09:00:00Z").unwrap().with_timezone(&chrono::Utc)
}

/// The one well-formed `Task` both tests start from — declared
/// `scenario_refs`, so `int_task_scenario_refs` (and therefore
/// `mart_scope_status`) has a row at all.
fn valid_task() -> Task {
    Task::new(
        Envelope::new(RecordKind::Task.schema_version(), RecordKind::Task, at(), Actor::new("planner", RoleId::parse("dev").unwrap())),
        TaskId::parse(VALID_TASK_ID).unwrap(),
        "a body the rust reader accepts",
        TaskStatus::Done,
        None,
    )
    .with_scenario_refs(vec![ScenarioId::parse(VALID_SCENARIO_ID).unwrap()])
}

/// A `kind=task/` body carrying `kind`, `task_id`, `title`, `status`
/// and `scenario_refs` but NO `actor` — so `Envelope`'s deserialize
/// fails and `partition::validate_body` refuses the whole record.
///
/// Every OTHER gate is deliberately satisfied: the body's own `kind`
/// agrees with its directory, and the file is written at exactly the
/// path `expected_relative_path` resolves from its own content
/// (`kind=task/{task_id}__{content_digest12}.json`). The body-schema
/// gate is the only reason this file is refused, which is what makes
/// this test a pin of THAT gate rather than a second copy of the
/// layout-gate pin in `multi_version_fold.rs`.
fn plant_refused_task(git_root: &std::path::Path) -> std::path::PathBuf {
    let body = json!({
        "schema": RecordKind::Task.schema_version(),
        "kind": "task",
        "at": at().to_rfc3339(),
        "task_id": REFUSED_TASK_ID,
        "title": "a body the rust reader refuses",
        "status": "open",
        "scenario_refs": [REFUSED_SCENARIO_ID],
    });
    let digest = canon_store::partition::content_digest12(&body);
    let dir = git_root.join("kind=task");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{REFUSED_TASK_ID}__{digest}.json"));
    std::fs::write(&path, serde_json::to_vec_pretty(&body).unwrap()).unwrap();

    // The plant is only a body-gate pin while every other gate passes.
    assert_eq!(
        canon_store::partition::expected_relative_path(RecordKind::Task, &body).unwrap(),
        path.strip_prefix(git_root).unwrap(),
        "the planted file must sit at exactly the path its own content resolves to, or it is refused by the LAYOUT gate instead"
    );
    assert!(
        canon_store::partition::validate_body(RecordKind::Task, &RawRecord(body.clone())).is_err(),
        "this test is only meaningful while a `kind=task` body with no `actor` fails its schema — if `Envelope.actor` ever becomes optional, the plant needs a new omission"
    );
    path
}

/// The scenarios `mart_scope_status` reports as CARRIED by some plan
/// task. Since s45 the mart is driven by the spec corpus, so every
/// authored scenario has a row and `task_id` is the nullable side: a
/// NULL means "specified, no plan task declares it". This helper is
/// therefore the set of scenarios whose worklist row a task has
/// claimed — which is exactly what a refused-but-globbed task moves.
fn scope_status_carried_scenarios(roots: &Roots) -> Vec<String> {
    let mut ids: Vec<String> = marts::fetch_scope_status(roots)
        .unwrap()
        .rows
        .iter()
        .filter(|row| row["task_id"].as_str().is_some())
        .map(|row| row["scenario_id"].as_str().expect("`scenario_id` is the mart's non-null driving key").to_string())
        .collect();
    ids.sort();
    ids
}

/// One `Scenario` per id the fixture's tasks declare. Required since
/// s45: the mart is driven by the spec corpus, so a declared ref
/// pointing at no authored scenario has no row to attach to.
fn write_scenarios(tier: &GitTier) {
    for id in [VALID_SCENARIO_ID, REFUSED_SCENARIO_ID] {
        let record = canon_model::Scenario::new(
            Envelope::new(RecordKind::Scenario.schema_version(), RecordKind::Scenario, at(), Actor::new("canon", RoleId::parse("dev").unwrap())),
            canon_model::ProjectId::parse("core").unwrap(),
            ScenarioId::parse(id).unwrap(),
            "t",
            "d",
            canon_model::SpecDigest::of(id.as_bytes()),
        );
        tier.write(&record).unwrap();
    }
}

/// s43 round 4, finding 3. `manifest.rs` claimed a snapshot whose
/// `mart_scope_status` rows moved could NOT carry an unchanged
/// `source_digest`, on the grounds that the mart's `porting.coverage`
/// input is a digested overlay. Its OTHER input is `Task.
/// scenario_refs` — a CORE kind, digested through the validated read —
/// and the glob feeding the mart runs no such validation.
///
/// So: plant one `kind=task/` file that `GitTier::read` refuses on its
/// body schema, and the mart grows a row while `source_digest` stands
/// still. Asserted through `combined_digest`, the value `manifest.json`
/// actually carries, and through `fetch_scope_status`, the Rust fetch
/// the report renders and `--snapshot` exports.
#[test]
fn a_core_kind_body_the_rust_reader_refuses_moves_a_mart_under_an_unchanged_source_digest() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);
    tier.write(&valid_task()).unwrap();
    write_scenarios(&tier);

    let roots = Roots::new(git_root.clone(), dir.path().join("r2"), dir.path().join("learn"));
    let digest = || DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();

    let baseline_digest = digest();
    assert_eq!(scope_status_carried_scenarios(&roots), vec![VALID_SCENARIO_ID.to_string()], "the fixture starts with exactly one scenario carried by a task");

    let planted = plant_refused_task(&git_root);

    // The Rust reader refuses it — LOUDLY, as a violation, and on the
    // BODY gate specifically (named by the refused file, not the
    // `layout` subject the sibling pin covers).
    let read = tier.read(&TierQuery::kind(RecordKind::Task)).unwrap();
    assert_eq!(read.records.len(), 1, "the planted file must be refused, leaving one readable task, got {:?}", read.records);
    assert_eq!(read.records[0].0["task_id"], json!(VALID_TASK_ID));
    assert_eq!(read.violations.len(), 1, "…and refused loudly: {:?}", read.violations);
    let planted_relative = planted.strip_prefix(&git_root).unwrap().display().to_string();
    assert_eq!(read.violations[0].subject, planted_relative, "refused by the body-schema gate, naming the file, got {:?}", read.violations[0]);

    // …so the digest, which is that same validated read, cannot see it.
    assert_eq!(baseline_digest, digest(), "a record the validated read refuses contributes nothing to `source_digest` — this is the documented exception, not a wish");

    // …and the glob, which is not that read, hands it to the mart.
    let mut expected = vec![VALID_SCENARIO_ID.to_string(), REFUSED_SCENARIO_ID.to_string()];
    expected.sort();
    assert_eq!(
        scope_status_carried_scenarios(&roots),
        expected,
        "`mart_scope_status` must attach the refused task to its declared scenario — the exception `manifest.rs` names beside its guarantee"
    );
}

/// Why the exception above is NOT closed by byte-hashing the core half
/// the way [`canon_report::digest`]'s overlay half is byte-hashed.
///
/// The core half digests `serde_json::to_string` of the VALIDATED,
/// parsed record, so it is insensitive to a file's whitespace — and so
/// is DuckDB, which parses the same JSON. Reformatting a valid record
/// file therefore moves neither side, and the two agree. A byte-based
/// core half would move `source_digest` here while every exported
/// table stood still, converting a false provenance CLAIM into a false
/// provenance CHANGE — and a fingerprint that cries wolf on a
/// reformat is one nobody reads on the run that matters.
#[test]
fn a_whitespace_only_reformat_of_a_valid_core_record_moves_neither() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);
    let receipt = tier.write(&valid_task()).unwrap();
    let path = git_root.join(&receipt.location);

    let roots = Roots::new(git_root.clone(), dir.path().join("r2"), dir.path().join("learn"));
    let digest = || DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();

    let baseline_digest = digest();
    let baseline_scope = scope_status_carried_scenarios(&roots);

    // `GitTier::write` writes `to_vec_pretty`; rewrite the SAME value
    // compactly. Different bytes, identical content — and identical
    // filename, since the `__{digest12}` suffix is computed over the
    // parsed value, so the layout gate still passes.
    let pretty = std::fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&pretty).unwrap();
    let compact = serde_json::to_vec(&value).unwrap();
    assert_ne!(pretty, compact, "the reformat must actually change the file's bytes, or this test asserts nothing");
    std::fs::write(&path, &compact).unwrap();

    let read = tier.read(&TierQuery::kind(RecordKind::Task)).unwrap();
    assert!(read.violations.is_empty(), "a reformatted record is still a valid record: {:?}", read.violations);
    assert_eq!(read.records.len(), 1);

    assert_eq!(baseline_digest, digest(), "the core half hashes the canonical re-serialization of the validated record, so whitespace cannot move `source_digest`");
    assert_eq!(baseline_scope, scope_status_carried_scenarios(&roots), "…and DuckDB parses the same JSON, so whitespace cannot move a mart either");
}
