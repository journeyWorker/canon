//! The 0.30 ledger naming rule (`docs/design/ledger-layout.md`, change
//! `canon-0-30-ledger-naming`) through the stores and the built binary:
//! scenarios `ledger.naming.01`–`ledger.naming.21`
//! (`.canon/specs/kind=feature/area=ledger/naming.feature`).
//!
//! Every expected name is spelled out here from the design's component table
//! and escape rule, never computed by the code under test.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_model::{RawRecord, RecordKind};
use canon_store::git_tier::GitTier;
use canon_store::tier::{RawWrite, StoreError, Tier, TierQuery};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

mod support;

// ── helpers ──────────────────────────────────────────────────────────────

fn well_formed(kind: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../canon-model/fixtures/well-formed").join(format!("{kind}.json"));
    serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// First 8 hex digits of SHA-256 over the canonical JSON (sorted keys, compact).
fn d8(body: &Value) -> String {
    Sha256::digest(serde_json::to_vec(body).unwrap()).iter().take(4).map(|b| format!("{b:02x}")).collect()
}

/// Design R6, written independently: every byte outside `[a-z0-9.-]` is `+` and two lowercase hex digits.
fn esc(value: &str) -> String {
    value.bytes().map(|b| if matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-') { (b as char).to_string() } else { format!("+{b:02x}") }).collect()
}

fn unesc(escaped: &str) -> String {
    let bytes = escaped.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(u8::from_str_radix(&escaped[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn raw(body: &Value) -> RawWrite {
    RawWrite(RawRecord(body.clone()))
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn json_of(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", text(o)))
}

/// Every regular file below `dir`, relative, with its bytes.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn dirs_below(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if entry.path().is_dir() {
            out.push(entry.path().strip_prefix(dir).unwrap().to_path_buf());
            for inner in dirs_below(&entry.path()) {
                out.push(PathBuf::from(entry.file_name()).join(inner));
            }
        }
    }
    out
}

fn stem_of(location: &str) -> String {
    Path::new(location).file_stem().unwrap().to_string_lossy().into_owned()
}

/// The unescaped value of the single free-text component of `stem` (`<value>_<d8>`).
fn value_of_stem(stem: &str) -> String {
    unesc(stem.rsplit_once('_').unwrap().0)
}

const RUN_ULID: &str = "01J0Z9Z9Z9Z9Z9Z9Z9Z9Z9Z9Z9";
const RUN_LOWER: &str = "01j0z9z9z9z9z9z9z9z9z9z9z9";

// ── ledger.naming.01 ─────────────────────────────────────────────────────

/// ledger.naming.01: each of the fourteen core kinds is written under
/// `kind=<kind>/<components>_<digest8>.json` with the table's components in
/// table order, one directory level and no `area=`; so is a plugin overlay.
#[test]
fn ledger_naming_01_every_kind_is_stored_under_the_one_name_rule() {
    let expected: [(&str, &str); 14] = [
        ("change", "s1-state-model-join-spine"),
        ("task", "s1-state-model-join-spine+236.2"),
        ("scenario", "root_world.firstbuy-hotdeal.26"),
        ("session", "f47ac10b-58cc-4372-a567-0e02b2c3d479"),
        ("run", RUN_LOWER),
        ("event", "01j0z9z9z9z9z9z9z9z9z9z9z9_1"),
        ("handoff", "20260710-1432-ship-s1-a1b2"),
        ("review", "root_scenario_world.place-lock.01"),
        ("divergence", "root_world.place-lock.01_3_8"),
        ("trajectory", RUN_LOWER),
        ("strategy_item", "implementer+2fcanon+2fjoin-spine+2f9c93d024b1a2"),
        ("evidence", "-_task_s1-state-model-join-spine+236.2"),
        ("subject", "subject-domain-loop"),
        ("finding", "s42-close-the-open-loops_11_4"),
    ];
    let dir = tempfile::tempdir().unwrap();
    let git = GitTier::new(dir.path());
    for (kind, components) in expected {
        let body = well_formed(kind);
        let receipt = git.write(&raw(&body)).unwrap_or_else(|e| panic!("{kind}: {e}"));
        let want = format!("kind={kind}/{components}_{}.json", d8(&body));
        assert_eq!(receipt.location, want, "{kind}: the write output");
        assert!(dir.path().join(&want).is_file(), "{kind}: the stored file is {want}");
    }
    // The overlay record: `kind=<namespace>.<kind>/<join key values>_<digest8>.json`.
    let overlay = json!({"schema": 1, "kind": "porting.coverage", "at": "2026-07-10T12:00:00Z", "actor": {"agent_id": "a"}, "project_id": "root", "scenario_id": "world.place-lock.01", "covered": true});
    let receipt = git.write_namespaced("porting.coverage", &["root".to_string(), "world.place-lock.01".to_string()], RawRecord(overlay.clone())).unwrap();
    assert_eq!(receipt.location, format!("kind=porting.coverage/root_world.place-lock.01_{}.json", d8(&overlay)));
    // One directory level: every directory under the root is `kind=<kind>` itself, no `area=`.
    let dirs = dirs_below(dir.path());
    assert_eq!(dirs.len(), 15, "fourteen core kinds and one overlay, one level each: {dirs:?}");
    assert!(dirs.iter().all(|d| d.components().count() == 1 && d.to_string_lossy().starts_with("kind=")), "{dirs:?}");
    assert!(!dirs.iter().any(|d| d.to_string_lossy().contains("area=")), "{dirs:?}");
}

/// ledger.naming.01 (sqlite): the row's id is the components joined by `_`, its digest the 8 hex digits.
#[test]
fn ledger_naming_01_sqlite_rows_carry_the_same_key() {
    let dir = tempfile::tempdir().unwrap();
    let tier = canon_store::sqlite_tier::SqliteTier::connect(&dir.path().join("hot.db")).unwrap();
    let body = well_formed("evidence");
    tier.write(&raw(&body)).unwrap();
    let rows = tier.all_rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].kind.as_str(), rows[0].id.as_str(), rows[0].digest.as_str()), ("evidence", "-_task_s1-state-model-join-spine+236.2", d8(&body).as_str()));
}

// ── ledger.naming.02 ─────────────────────────────────────────────────────

fn stem_written(git: &GitTier, body: &Value) -> String {
    stem_of(&git.write(&raw(body)).unwrap().location)
}

/// ledger.naming.02: a project leads the name, an absent optional project is `-`,
/// the alternative key is tagged and evidence picks task, then scenario, then run;
/// a review's name carries no pin and the digest separates two reviews of one target.
#[test]
fn ledger_naming_02_project_leads_tags_say_which_key_and_a_review_name_has_no_pin() {
    let dir = tempfile::tempdir().unwrap();
    let git = GitTier::new(dir.path());
    let sc = well_formed("scenario");
    assert_eq!(stem_written(&git, &sc), format!("root_world.firstbuy-hotdeal.26_{}", d8(&sc)));

    let review = well_formed("review"); // scenario review, pin 9c93d024b1a2
    let first = git.write(&raw(&review)).unwrap().location;
    assert_eq!(stem_of(&first), format!("root_scenario_world.place-lock.01_{}", d8(&review)));

    let mut change_review = review.clone();
    change_review.as_object_mut().unwrap().remove("scenario_id");
    change_review["change_id"] = json!("canon-0-30");
    assert_eq!(stem_written(&git, &change_review), format!("root_change_canon-0-30_{}", d8(&change_review)));

    let div = well_formed("divergence");
    assert_eq!(stem_written(&git, &div), format!("root_world.place-lock.01_3_8_{}", d8(&div)));

    // Evidence: the project, or `-`; then task, else scenario, else run.
    let mut by_task = well_formed("evidence");
    by_task["project_id"] = json!("canon");
    assert_eq!(stem_written(&git, &by_task), format!("canon_task_s1-state-model-join-spine+236.2_{}", d8(&by_task)));
    let mut by_scenario = by_task.clone();
    by_scenario.as_object_mut().unwrap().remove("task_id");
    assert_eq!(stem_written(&git, &by_scenario), format!("canon_scenario_world.place-lock.01_{}", d8(&by_scenario)));
    let mut by_run = by_scenario.clone();
    by_run.as_object_mut().unwrap().remove("scenario_id");
    assert_eq!(stem_written(&git, &by_run), format!("canon_run_{RUN_LOWER}_{}", d8(&by_run)));
    let mut no_project = by_scenario.clone();
    no_project.as_object_mut().unwrap().remove("project_id");
    assert_eq!(stem_written(&git, &no_project), format!("-_scenario_world.place-lock.01_{}", d8(&no_project)));

    // Two reviews of one target at two pins: no commit sha in either name; only the digest differs.
    let mut other_pin = review.clone();
    other_pin["pin"] = json!("0123456789abcdef0123456789abcdef01234567");
    let second = git.write(&raw(&other_pin)).unwrap().location;
    assert_ne!(first, second);
    for location in [&first, &second] {
        assert!(!location.contains("9c93d024b1a2") && !location.contains("0123456789abcdef"), "no pin in {location}");
    }
    let (a, b) = (stem_of(&first), stem_of(&second));
    assert_eq!(a.rsplit_once('_').unwrap().0, b.rsplit_once('_').unwrap().0, "one key prefix");
    assert_ne!(a.rsplit_once('_').unwrap().1, b.rsplit_once('_').unwrap().1, "the digest separates them");
}

// ── ledger.naming.03 ─────────────────────────────────────────────────────

const ROUTING_ALL_LOCAL: &str = "  finding: local\n  event: local\n  evidence: local\n  session: local\n  run: local\n  task: local\n";

/// The records of `canon query --kind <kind> --json --all`, in the order returned.
fn queried(fixture: &support::Fixture, kind: &str) -> Vec<Value> {
    queried_with(fixture, kind, &[])
}

fn queried_with(fixture: &support::Fixture, kind: &str, extra: &[&str]) -> Vec<Value> {
    let mut args = vec!["query", "--kind", kind, "--json", "--all"];
    args.extend_from_slice(extra);
    let o = fixture.run_canon(&args);
    assert!(o.status.success(), "{}", text(&o));
    json_of(&o)["data"]["records"].as_array().unwrap().clone()
}

/// ledger.naming.03: integers are plain in names; order comes from the body's
/// `at`, then typed components (numbers as numbers), then the body digest.
#[test]
fn ledger_naming_03_integers_are_plain_and_order_comes_from_the_body() {
    let fixture = support::Fixture::new(ROUTING_ALL_LOCAL, "");
    let git = GitTier::new(fixture.git_root());
    let at = "2026-07-30T09:15:00Z";

    // Findings of rounds 10 then 2 (written in the reverse of the expected order), one shared `at`.
    let finding = |round: u64| {
        let mut f = well_formed("finding");
        f["round"] = json!(round);
        f["seq"] = json!(1);
        f["at"] = json!(at);
        f
    };
    let (r10, r2) = (finding(10), finding(2));
    let loc10 = git.write(&raw(&r10)).unwrap().location;
    let loc2 = git.write(&raw(&r2)).unwrap().location;
    assert_eq!(loc2, format!("kind=finding/s42-close-the-open-loops_2_1_{}.json", d8(&r2)));
    assert_eq!(loc10, format!("kind=finding/s42-close-the-open-loops_10_1_{}.json", d8(&r10)));

    // Events 10 then 9 of one run, one shared `at`.
    let event = |seq: u64| {
        let mut e = well_formed("event");
        e["seq"] = json!(seq);
        e["at"] = json!(at);
        e
    };
    let (e10, e9) = (event(10), event(9));
    assert_eq!(git.write(&raw(&e10)).unwrap().location, format!("kind=event/{RUN_LOWER}_10_{}.json", d8(&e10)));
    assert_eq!(git.write(&raw(&e9)).unwrap().location, format!("kind=event/{RUN_LOWER}_9_{}.json", d8(&e9)));

    // Two evidence records sharing `at`: different scenarios (components break the tie), and two with the
    // same key but different bodies (the digest breaks it).
    let evidence = |scenario: &str, summary: &str| {
        let mut e = well_formed("evidence");
        e.as_object_mut().unwrap().remove("task_id");
        e["project_id"] = json!("root");
        e["scenario_id"] = json!(scenario);
        e["at"] = json!(at);
        e["evidence_note"] = json!({"summary": summary});
        e
    };
    let (zz, aa, x1, x2) = (evidence("zz.surface.01", "z"), evidence("aa.surface.01", "a"), evidence("mm.surface.01", "one"), evidence("mm.surface.01", "two"));
    for e in [&zz, &x1, &aa, &x2] {
        git.write(&raw(e)).unwrap();
    }

    let findings = queried(&fixture, "finding");
    assert_eq!(findings.iter().map(|f| f["round"].as_u64().unwrap()).collect::<Vec<_>>(), [2, 10], "rounds compare as numbers, not as the text `10` < `2`");
    let events = queried(&fixture, "event");
    assert_eq!(events.iter().map(|e| e["seq"].as_u64().unwrap()).collect::<Vec<_>>(), [9, 10]);

    // Latest-only reads keep one version per key; `--history` returns every version, the ties among them in the rule's order.
    let evidence_order: Vec<String> = queried_with(&fixture, "evidence", &["--history"]).iter().map(|e| format!("{}/{}", e["scenario_id"].as_str().unwrap(), e["evidence_note"]["summary"].as_str().unwrap())).collect();
    let mut tied = [(d8(&x1), "mm.surface.01/one"), (d8(&x2), "mm.surface.01/two")];
    tied.sort();
    let want: Vec<String> = ["aa.surface.01/a", tied[0].1, tied[1].1, "zz.surface.01/z"].iter().map(ToString::to_string).collect();
    assert_eq!(evidence_order, want, "same `at`: components in table order, then the body digest");
}

// ── ledger.naming.04 ─────────────────────────────────────────────────────

/// ledger.naming.04: an id that needs escaping is escaped only in the name (every byte outside
/// `[a-z0-9.-]` is `+hh`), a run id is its 26 lowercase characters, the git path and the R2 object
/// key are one string that unescapes to the original, and query and its cursor show the raw value.
#[test]
fn ledger_naming_04_escaping_is_only_in_the_name_and_git_and_r2_agree() {
    let value = "Sess/One_A";
    assert_eq!(esc(value), "+53ess+2f+4fne+5f+41");
    let mut session = well_formed("session");
    session["session_id"] = json!(value);
    let mut run = well_formed("run");
    run["run_id"] = json!(RUN_ULID);
    run["session_id"] = json!(value);

    let fixture = support::Fixture::new("  session: local\n  run: local\n", "");
    let git = GitTier::new(fixture.git_root());
    let r2 = canon_store::r2_tier::R2Tier::local(fixture.r2_root(), "canon/").unwrap();

    let git_session = git.write(&raw(&session)).unwrap();
    assert_eq!(git_session.location, format!("kind=session/+53ess+2f+4fne+5f+41_{}.json", d8(&session)));
    let git_run = git.write(&raw(&run)).unwrap();
    assert_eq!(git_run.location, format!("kind=run/{RUN_LOWER}_{}.json", d8(&run)), "26 lowercase characters");
    assert_eq!(stem_of(&git_run.location).rsplit_once('_').unwrap().0.len(), 26);

    let r2_session = r2.write(&raw(&session)).unwrap();
    let r2_run = r2.write(&raw(&run)).unwrap();
    let object_key = |location: &str| location.strip_prefix("canon/").unwrap().rsplit_once('.').unwrap().0.to_string();
    assert_eq!(object_key(&r2_session.location), git_session.location.strip_suffix(".json").unwrap(), "the same string in git and R2");
    assert_eq!(object_key(&r2_run.location), git_run.location.strip_suffix(".json").unwrap());
    assert_eq!(value_of_stem(&stem_of(&git_session.location)), value, "it unescapes to the original value");
    assert!(fixture.r2_root().join(&r2_session.location).is_file());

    // A query, a cursor and a message show the value before escaping.
    let o = fixture.run_canon(&["query", "--kind", "session", "--limit", "1"]);
    assert!(o.status.success(), "{}", text(&o));
    let human = text(&o);
    assert!(human.contains(value) && !human.contains("+2f"), "the ID column shows the value: {human}");
    let o = fixture.run_canon(&["query", "--kind", "session", "--json", "--limit", "1"]);
    let doc = json_of(&o);
    assert_eq!(doc["data"]["records"][0]["session_id"], value);
    let mut second = session.clone();
    second["session_id"] = json!("Sess/Two_B");
    git.write(&raw(&second)).unwrap();
    let paged = json_of(&fixture.run_canon(&["query", "--kind", "session", "--json", "--limit", "1"]));
    let cursor = paged["meta"]["nextCursor"].as_str().expect("a second page exists");
    let decoded = String::from_utf8(cursor.as_bytes().chunks(2).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect()).unwrap();
    assert!(decoded.contains("Sess/One_A") && !decoded.contains("+2f"), "the cursor key names the value, not the escaped form: {decoded:?}");
}

// ── ledger.naming.05 ─────────────────────────────────────────────────────

/// ledger.naming.05: a name over 200 bytes replaces its free-text component with `h` and the
/// first 16 hex digits of the SHA-256 of the value; the value is still in the body.
#[test]
fn ledger_naming_05_a_long_name_hashes_its_free_text_component() {
    let value = format!("Long/{}_end", "x".repeat(240));
    assert!(esc(&value).len() + 9 > 200);
    let mut session = well_formed("session");
    session["session_id"] = json!(value);
    let dir = tempfile::tempdir().unwrap();
    let git = GitTier::new(dir.path());
    let receipt = git.write(&raw(&session)).unwrap();
    let hash: String = Sha256::digest(value.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect();
    assert_eq!(receipt.location, format!("kind=session/h{hash}_{}.json", d8(&session)));
    let stored: Value = serde_json::from_slice(&std::fs::read(dir.path().join(&receipt.location)).unwrap()).unwrap();
    assert_eq!(stored["session_id"], value, "the original value is still in the body");
    let read = git.read(&TierQuery::kind(RecordKind::Session)).unwrap();
    assert!(read.violations.is_empty(), "{:?}", read.violations);
    assert_eq!(read.records.len(), 1);
}

// ── ledger.naming.06–09: refusing a collision ────────────────────────────

fn finding_n(seq: u64) -> Value {
    let mut f = well_formed("finding");
    f["seq"] = json!(seq);
    f
}

/// ledger.naming.06: a batch whose second record would take an occupied name with other bytes is
/// refused as a key conflict naming the path, and the ledger holds exactly what it held.
#[test]
fn ledger_naming_06_a_git_key_conflict_refuses_the_batch_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let git = GitTier::new(dir.path());
    git.write(&raw(&finding_n(1))).unwrap();
    // The name the second record resolves to, occupied by other bytes.
    let (first, second) = (finding_n(2), finding_n(3));
    let occupied = dir.path().join(canon_store::partition::expected_relative_path(RecordKind::Finding, &second).unwrap());
    std::fs::create_dir_all(occupied.parent().unwrap()).unwrap();
    std::fs::write(&occupied, b"{\"other\": \"bytes\"}").unwrap();
    let before = files(dir.path());

    let err = git.write_batch(&[&raw(&first), &raw(&second)]).expect_err("the batch must be refused");
    match &err {
        StoreError::KeyConflict { location } => assert!(location.contains(&stem_of(occupied.to_str().unwrap())), "the error names the path: {location}"),
        other => panic!("expected KeyConflict, got {other:?}"),
    }
    assert_eq!(files(dir.path()), before, "the first record of the batch is not in the ledger either");
}

/// ledger.naming.07: an R2 object under the same key with other content is refused, not reported
/// deduped; the same record twice reports deduped; a batch with a conflict puts no object.
#[test]
fn ledger_naming_07_an_r2_object_with_other_content_is_a_key_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let r2 = canon_store::r2_tier::R2Tier::local(dir.path(), "canon/").unwrap();
    let (a, b) = (finding_n(1), finding_n(2));
    let first = r2.write(&raw(&a)).unwrap();
    assert!(!first.deduped);
    assert!(r2.write(&raw(&a)).unwrap().deduped, "the same record twice reports deduped");

    // The object under b's key, holding another record's row: copy a's object there.
    let b_key = canon_store::partition::hive_object_key(RecordKind::Finding, &b, "parquet").unwrap();
    let object = dir.path().join("canon").join(&b_key);
    std::fs::copy(dir.path().join(&first.location), &object).unwrap();
    let err = r2.write(&raw(&b)).expect_err("other content under the same key");
    assert!(matches!(err, StoreError::KeyConflict { .. }), "a key conflict, not `deduped`: {err:?}");

    // A batch with that conflict puts no object (the new record `c` would be the batch's first).
    let c = finding_n(3);
    let before = files(dir.path());
    let err = r2.write_batch(&[&raw(&c), &raw(&b)]).expect_err("the batch has a conflict");
    assert!(matches!(err, StoreError::KeyConflict { .. }), "{err:?}");
    assert_eq!(files(dir.path()), before, "no object was put");
}

/// ledger.naming.08 (sqlite; the Postgres tier needs a server and is covered by `ledger_naming_08_...` in
/// `crates/canon-store/tests/pg_tier_live.rs`, which CI's live-tier job runs):
/// a later record landing on a held key with other canonical JSON refuses the batch, one transaction.
#[test]
fn ledger_naming_08_sqlite_key_conflict_rolls_the_batch_back() {
    let dir = tempfile::tempdir().unwrap();
    let tier = canon_store::sqlite_tier::SqliteTier::connect(&dir.path().join("hot.db")).unwrap();
    let (first, second) = (finding_n(1), finding_n(2));
    // Plant a row at `second`'s key whose JSON differs: write another finding, then re-key it onto that key.
    let planted = finding_n(9);
    tier.write(&raw(&planted)).unwrap();
    let name = canon_store::partition::resolve_name(RecordKind::Finding, &second).unwrap();
    let planted_name = canon_store::partition::resolve_name(RecordKind::Finding, &planted).unwrap();
    tier.rekey(&[canon_store::tier::RowRekey {
        from: ("finding".into(), planted_name.key, planted_name.digest),
        to: ("finding".into(), name.key.clone(), name.digest.clone()),
        body: json!({"other": "json"}),
    }])
    .unwrap();
    let before = tier.all_rows().unwrap();
    assert_eq!(before.len(), 1);

    let err = tier.write_batch(&[&raw(&first), &raw(&second)]).expect_err("the later record lands on a held key");
    assert!(matches!(err, StoreError::KeyConflict { .. }), "{err:?}");
    assert_eq!(tier.all_rows().unwrap(), before, "no row of the batch is stored");
    // The same record twice is a no-op, not a conflict.
    assert!(!tier.write(&raw(&first)).unwrap().deduped);
    assert!(tier.write(&raw(&first)).unwrap().deduped);
}

/// ledger.naming.09: a ledger path the archive holds with other bytes is refused before anything
/// lands; an archive copy with the same bytes does not refuse.
#[test]
fn ledger_naming_09_an_archive_with_other_bytes_refuses_the_ledger_write() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join(".canon/ledger");
    let archive = dir.path().join(".canon/archive");
    let git = GitTier::new(&ledger);
    let body = finding_n(5);
    let relative = canon_store::partition::expected_relative_path(RecordKind::Finding, &body).unwrap();

    std::fs::create_dir_all(archive.join(relative.parent().unwrap())).unwrap();
    std::fs::write(archive.join(&relative), b"{\"other\": \"bytes\"}").unwrap();
    let err = git.write(&raw(&body)).expect_err("the archive holds this name with other bytes");
    assert!(matches!(err, StoreError::KeyConflict { .. }), "{err:?}");
    assert!(files(&ledger).is_empty(), "before anything lands");

    // The same bytes in the archive: write once elsewhere to learn the canonical bytes, then archive them.
    let scratch = tempfile::tempdir().unwrap();
    GitTier::new(scratch.path()).write(&raw(&body)).unwrap();
    let same = std::fs::read(scratch.path().join(&relative)).unwrap();
    std::fs::write(archive.join(&relative), same).unwrap();
    git.write(&raw(&body)).expect("an archive copy with the same bytes does not refuse");
    assert!(ledger.join(&relative).is_file());
}

// ── a repo on the 0.30 layout, built by the binary ───────────────────────

fn canon_in(dir: &Path, args: &[&str], envs: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "ln-tests").env("HOME", dir);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("spawn canon")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "gc.auto=0", "-c", "maintenance.auto=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn ok(dir: &Path, args: &[&str]) -> Output {
    let out = canon_in(dir, args, &[]);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    out
}

const FEATURE: &str = ".canon/specs/kind=feature/area=auth/login.feature";

/// A committed repo with subject `demo`, change `add-login` and scenarios `auth.login.01`..`.<count>`, all indexed.
fn repo30(scenarios: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "."]);
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    for n in 1..=scenarios {
        ok(repo, &["scenario", "new", &format!("auth.login.{n:02}"), "--title", "Case", "--feature", FEATURE]);
    }
    ok(repo, &["inventory", "sync"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "seed"]);
    dir
}

fn evidence_add(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["evidence", "add", "--override-reason", "fixture", "--class", "test-run", "--ref", "r", "--verdict", "faithful", "--actor-id", "a", "--role", "implementer"];
    args.extend_from_slice(extra);
    canon_in(repo, &args, &[])
}

fn ledger_files(repo: &Path, kind: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(repo.join(format!(".canon/ledger/kind={kind}"))).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

// ── ledger.naming.10 ─────────────────────────────────────────────────────

fn hex_decode(cursor: &str) -> String {
    String::from_utf8(cursor.as_bytes().chunks(2).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect()).unwrap()
}

/// ledger.naming.10: the ID column and the cursor key hold the typed components joined by `_`,
/// never the escaped form, and the digest is 8 hex digits.
#[test]
fn ledger_naming_10_query_shows_the_id_before_escaping() {
    let fixture = support::Fixture::new("  session: local\n  review: local\n  evidence: local\n", "");
    let git = GitTier::new(fixture.git_root());
    let mut s1 = well_formed("session");
    s1["session_id"] = json!("Sess/One_A");
    s1["at"] = json!("2026-07-10T12:00:00Z");
    let mut s2 = s1.clone();
    s2["session_id"] = json!("Sess/Two_B");
    s2["at"] = json!("2026-07-10T13:00:00Z");
    git.write(&raw(&s1)).unwrap();
    git.write(&raw(&s2)).unwrap();
    let review = well_formed("review");
    git.write(&raw(&review)).unwrap();
    let evidence = well_formed("evidence");
    git.write(&raw(&evidence)).unwrap();

    // `--json`: page through the sessions one at a time; each cursor names the raw value.
    let first = json_of(&fixture.run_canon(&["query", "--kind", "session", "--json", "--limit", "1"]));
    assert_eq!(first["data"]["records"][0]["session_id"], "Sess/One_A");
    let cursor = first["meta"]["nextCursor"].as_str().expect("a second page");
    let key = hex_decode(cursor);
    let (scope, key) = key.split_once('\u{1f}').unwrap();
    assert_eq!(scope, "query:session");
    let parts: Vec<&str> = key.split('|').collect();
    assert_eq!(parts[1], "Sess/One_A", "the cursor key holds the value, not `+53ess+2f…`: {key}");
    assert!(parts[2].trim_end_matches("#1").len() == 8 && parts[2].trim_end_matches("#1").bytes().all(|b| b.is_ascii_hexdigit()), "digest8: {key}");
    let second_page = json_of(&fixture.run_canon(&["query", "--kind", "session", "--json", "--limit", "1", "--cursor", cursor]));
    assert_eq!(second_page["data"]["records"][0]["session_id"], "Sess/Two_B");

    // Human table: the ID column.
    let human = text(&fixture.run_canon(&["query", "--kind", "session"]));
    for id in ["Sess/One_A", "Sess/Two_B"] {
        assert!(human.lines().any(|l| l.contains(id)), "{id} in the ID column: {human}");
    }
    assert!(!human.contains("+2f") && !human.contains("+5f"), "never the escaped form: {human}");
    let review_table = text(&fixture.run_canon(&["query", "--kind", "review"]));
    assert!(review_table.lines().any(|l| l.contains("root_scenario_world.place-lock.01")), "{review_table}");
    let evidence_table = text(&fixture.run_canon(&["query", "--kind", "evidence"]));
    assert!(evidence_table.lines().any(|l| l.contains("-_task_s1-state-model-join-spine#6.2") && l.contains(&d8(&evidence))), "the task id's `#` is shown as is: {evidence_table}");
}

// ── ledger.naming.11 ─────────────────────────────────────────────────────

/// ledger.naming.11: tier-age effect ids, export manifest ids and purge ids are
/// `<kind>:<components>:<digest8>` with the components unescaped.
#[test]
fn ledger_naming_11_effect_and_manifest_ids_follow_the_new_rule() {
    let fixture = support::Fixture::new("  session: local\n", "  session: { after: 1d, to: cold }\n");
    let git = GitTier::new(fixture.git_root());
    let mut session = well_formed("session");
    session["session_id"] = json!("Sess/One_A");
    session["at"] = json!("2026-01-01T00:00:00Z");
    git.write(&raw(&session)).unwrap();
    let want = format!("session:Sess/One_A:{}", d8(&session));

    let age = json_of(&fixture.run_canon(&["tier", "age", "--json"]));
    assert_eq!(age["data"]["effects"][0]["record"], want.as_str(), "tier age: {age}");

    let export = json_of(&fixture.run_canon(&["export", "--kind", "session", "--json"]));
    let entry = &export["data"]["records"][0];
    assert_eq!(format!("{}:{}:{}", entry["kind"].as_str().unwrap(), entry["id"].as_str().unwrap(), entry["digest"].as_str().unwrap()), want, "export: {export}");
    assert_eq!(export["data"]["contentDigests"][0], d8(&session).as_str());

    let purge = json_of(&fixture.run_canon(&["purge", "--kind", "session", "--before", "2026-06-01", "--json"]));
    let record = purge["data"]["effects"][0]["record"].as_str().unwrap();
    assert!(record.contains(&want), "purge names `{want}`: {record}");
    assert!(!record.contains("+2f") && !record.contains(".json"), "the components unescaped, not the path: {record}");
}

// ── ledger.naming.12 ─────────────────────────────────────────────────────

/// ledger.naming.12: `canon evidence add --scenario-id` names `kind=evidence/<project>_scenario_<id>_<digest8>.json`,
/// and the stored file holds `kind: evidence`.
#[test]
fn ledger_naming_12_the_write_result_line_names_the_file_under_the_new_rule() {
    let dir = repo30(1);
    let repo = dir.path();
    let out = evidence_add(repo, &["--project-id", "root", "--scenario-id", "auth.login.01", "--summary", "s"]);
    assert!(out.status.success(), "{}", text(&out));
    let files = ledger_files(repo, "evidence");
    assert_eq!(files.len(), 1, "{files:?}");
    let file = &files[0];
    let stem = file.strip_suffix(".json").unwrap();
    let (key, digest) = stem.rsplit_once('_').unwrap();
    assert_eq!(key, "root_scenario_auth.login.01");
    assert!(digest.len() == 8 && digest.bytes().all(|b| b.is_ascii_hexdigit()), "{file}");
    assert!(text(&out).contains(&format!("kind=evidence/{file}")), "the output line names the file: {}", text(&out));
    let body: Value = serde_json::from_slice(&std::fs::read(repo.join(".canon/ledger/kind=evidence").join(file)).unwrap()).unwrap();
    assert_eq!(body["kind"], "evidence");
    assert_eq!(digest, d8(&body), "the digest in the name is the body's");
}

// ── ledger.naming.13 ─────────────────────────────────────────────────────

/// ledger.naming.13: FINDING_SEQ_OCCUPIED quotes `<change id>_2_3`, and a promote refusal quotes the same key.
#[test]
fn ledger_naming_13_messages_that_quote_a_key_quote_the_typed_key() {
    let dir = repo30(1);
    let repo = dir.path();
    let add = |summary: &str| canon_in(repo, &["finding", "add", "--change-id", "add-login", "--round", "2", "--seq", "3", "--severity", "note", "--actor-id", "bob", "--summary", summary], &[]);
    assert!(add("first").status.success());
    let again = add("second");
    assert!(!again.status.success());
    let message = text(&again);
    assert!(message.contains("FINDING_SEQ_OCCUPIED"), "{message}");
    assert!(message.contains("add-login_2_3 is already occupied"), "the typed key: {message}");
    assert!(!message.contains("__0002") && !message.contains("0003"), "no 0.29 form: {message}");

    // A second body staged at the committed key: `gate promote` refuses it, quoting the same key.
    let committed = ledger_files(repo, "finding");
    assert_eq!(committed.len(), 1);
    let mut body: Value = serde_json::from_slice(&std::fs::read(repo.join(".canon/ledger/kind=finding").join(&committed[0])).unwrap()).unwrap();
    body["summary"] = json!("a different finding under the same key");
    let staged = repo.join(format!(".canon/ledger/_staging/kind=finding/add-login_2_3_{}.json", d8(&body)));
    std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
    std::fs::write(&staged, serde_json::to_vec_pretty(&body).unwrap()).unwrap();
    let promote = canon_in(repo, &["gate", "promote", "--json"], &[]);
    let doc = json_of(&promote);
    assert_eq!(doc["error"]["code"], "PROMOTE_REFUSED", "{}", text(&promote));
    let refused = &doc["data"]["refused"][0];
    assert_eq!(refused["subject"], "add-login_2_3");
    assert!(refused["message"].as_str().unwrap().contains("natural key `add-login_2_3` is held at"), "{refused}");
    assert!(staged.is_file(), "left staged");
}

// ── ledger.naming.14 ─────────────────────────────────────────────────────

fn copy_tree(from: &Path, to: &Path) {
    for (relative, bytes) in files(from) {
        let target = to.join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
}

/// The report text with the one line the git tier's own digest feeds (`ledger head`).
fn report_without_head(repo: &Path) -> String {
    std::fs::read_to_string(repo.join(".canon/REPORT.md")).unwrap().lines().filter(|l| !l.contains("ledger head")).collect::<Vec<_>>().join("\n")
}

/// ledger.naming.14: a ledger whose file names carry other digests than the bodies hash to gives the
/// report views the same counts as the same ledger with correct names; two evidence records of one
/// key and one `at` are told apart by the body digest, so swapping their name digests changes nothing.
#[test]
fn ledger_naming_14_a_file_name_that_disagrees_with_its_body_changes_no_count() {
    let correct = repo30(2);
    let repo = correct.path();
    // Two evidence records of one scenario sharing one `at`: only the body digest tells them apart.
    let at = "2026-10-26T10:00:00Z";
    let evidence = |verdict: &str| {
        json!({"schema": 1, "kind": "evidence", "at": at, "actor": {"agent_id": "a", "role": "implementer"}, "project_id": "root", "scenario_id": "auth.login.01", "verdict": verdict, "run_seq": 1})
    };
    let (faithful, divergent) = (evidence("faithful"), evidence("divergent"));
    let git = GitTier::new(repo.join(".canon/ledger"));
    let a = git.write(&raw(&faithful)).unwrap().location;
    let b = git.write(&raw(&divergent)).unwrap().location;
    assert_ne!(a, b);
    ok(repo, &["report", "build"]);
    let snapshot_correct = tempfile::tempdir().unwrap();
    ok(repo, &["report", "snapshot", snapshot_correct.path().to_str().unwrap()]);

    // The same ledger, every evidence file named with the OTHER record's digest (and a finding with a made-up one).
    let wrong = tempfile::tempdir().unwrap();
    copy_tree(repo, wrong.path());
    let evidence_dir = wrong.path().join(".canon/ledger/kind=evidence");
    let (a_name, b_name) = (Path::new(&a).file_name().unwrap(), Path::new(&b).file_name().unwrap());
    let (tmp_a, tmp_b) = (evidence_dir.join("a.tmp"), evidence_dir.join("b.tmp"));
    std::fs::rename(evidence_dir.join(a_name), &tmp_a).unwrap();
    std::fs::rename(evidence_dir.join(b_name), &tmp_b).unwrap();
    std::fs::rename(&tmp_a, evidence_dir.join(b_name)).unwrap();
    std::fs::rename(&tmp_b, evidence_dir.join(a_name)).unwrap();
    ok(wrong.path(), &["report", "build"]);
    let snapshot_wrong = tempfile::tempdir().unwrap();
    ok(wrong.path(), &["report", "snapshot", snapshot_wrong.path().to_str().unwrap()]);

    assert_eq!(report_without_head(repo), report_without_head(wrong.path()), "the report's counts are the same");
    for (name, bytes) in files(snapshot_correct.path()) {
        if name.extension().is_some_and(|e| e == "parquet") {
            assert_eq!(files(snapshot_wrong.path()).get(&name), Some(&bytes), "{} is the same table in both ledgers", name.display());
        }
    }
}

/// ledger.naming.14 (archive scoping): a new version of one finding archives only its own key's
/// older version: `…_1_1_…` and `…_1_10_…` share a text prefix and are different keys.
#[test]
fn ledger_naming_14_archive_scoping_groups_by_typed_components() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join(".canon/ledger");
    let git = GitTier::new(&ledger);
    let finding = |seq: u64, summary: &str| {
        let mut f = well_formed("finding");
        f["round"] = json!(1);
        f["seq"] = json!(seq);
        f["summary"] = json!(summary);
        f
    };
    let (one_v1, ten, one_v2) = (finding(1, "v1"), finding(10, "ten"), {
        let mut f = finding(1, "v2");
        f["at"] = json!("2026-08-01T00:00:00Z");
        f
    });
    git.write(&raw(&one_v1)).unwrap();
    git.write(&raw(&ten)).unwrap();
    git.write(&raw(&one_v2)).unwrap();
    let in_ledger = ledger_names(&ledger.join("kind=finding"));
    let in_archive = ledger_names(&dir.path().join(".canon/archive/kind=finding"));
    assert_eq!(in_archive, vec![format!("s42-close-the-open-loops_1_1_{}.json", d8(&one_v1))], "only the older version of `_1_1` is archived");
    assert!(in_ledger.contains(&format!("s42-close-the-open-loops_1_10_{}.json", d8(&ten))), "{in_ledger:?}");
    assert!(in_ledger.contains(&format!("s42-close-the-open-loops_1_1_{}.json", d8(&one_v2))), "{in_ledger:?}");
    assert_eq!(in_ledger.len(), 2);
}

fn ledger_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

// ── ledger.naming.15 and .20: overlay names ──────────────────────────────

const PORTING_YAML: &str = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";
const INVENTORY_YAML: &str = "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nauth.login.header:\n  upstream:\n    pin: a\n    file: f.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [auth.login.01]\n";

fn porting_repo() -> tempfile::TempDir {
    let dir = repo30(1);
    let repo = dir.path();
    std::fs::create_dir_all(repo.join(".canon/plugins/porting")).unwrap();
    std::fs::write(repo.join(".canon/plugins/porting/plugin.yaml"), PORTING_YAML).unwrap();
    let inventory = repo.join(".canon/specs/inventory/kind=inventory/area=auth/surface=login");
    std::fs::create_dir_all(&inventory).unwrap();
    std::fs::write(inventory.join("login.yaml"), INVENTORY_YAML).unwrap();
    dir
}

fn overlay_query(repo: &Path) -> Value {
    let o = canon_in(repo, &["query", "--kind", "scenario", "--plugin-id", "porting", "--json"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    json_of(&o)
}

/// ledger.naming.15: an overlay record is named by its join-key values and the scan accepts
/// a file whose name components are the body's and reports one that is not.
#[test]
fn ledger_naming_15_an_overlay_record_is_named_by_its_join_key_values() {
    let dir = porting_repo();
    let repo = dir.path();
    ok(repo, &["plugin", "sync", "porting"]);
    let files = ledger_names(&repo.join(".canon/ledger/kind=porting.coverage"));
    assert_eq!(files.len(), 1, "{files:?}");
    let body: Value = serde_json::from_slice(&std::fs::read(repo.join(".canon/ledger/kind=porting.coverage").join(&files[0])).unwrap()).unwrap();
    assert_eq!(files[0], format!("root_auth.login.01_{}.json", d8(&body)), "`kind=<namespace>.<kind>/<project>_<scenario>_<digest8>.json`");
    let doc = overlay_query(repo);
    assert!(doc["meta"]["warnings"].as_array().is_none_or(|w| w.iter().all(|m| !m.as_str().unwrap().contains("overlay scan violation"))), "the matching file is accepted: {doc}");
    assert_eq!(doc["data"]["records"][0]["overlay"]["porting.coverage"]["covered"], true);

    // A file whose name components are not the body's join key values is reported.
    let wrong = repo.join(format!(".canon/ledger/kind=porting.coverage/root_zz.surface.09_{}.json", d8(&body)));
    std::fs::copy(repo.join(".canon/ledger/kind=porting.coverage").join(&files[0]), &wrong).unwrap();
    let warnings = overlay_query(repo)["meta"]["warnings"].to_string();
    assert!(warnings.contains("root_zz.surface.09_") && warnings.contains("does not match the join-key field values"), "{warnings}");
}

/// ledger.naming.20: an overlay file whose key components are not the body's, and one named by the
/// 0.29 rule, are each reported as a violation and skipped; the matching records are returned.
#[test]
fn ledger_naming_20_overlay_files_that_do_not_match_are_reported_and_skipped() {
    let dir = porting_repo();
    let repo = dir.path();
    ok(repo, &["plugin", "sync", "porting"]);
    let overlay_dir = repo.join(".canon/ledger/kind=porting.coverage");
    let good = ledger_names(&overlay_dir).remove(0);
    let digest = good.strip_suffix(".json").unwrap().rsplit_once('_').unwrap().1.to_string();
    std::fs::copy(overlay_dir.join(&good), overlay_dir.join(format!("root_zz.surface.09_{digest}.json"))).unwrap();
    std::fs::copy(overlay_dir.join(&good), overlay_dir.join("root__auth.login.01__0123456789ab.json")).unwrap();

    let doc = overlay_query(repo);
    let warnings: Vec<String> = doc["meta"]["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect();
    let mismatch = warnings.iter().find(|w| w.contains("root_zz.surface.09_")).unwrap_or_else(|| panic!("the mismatched key is reported: {warnings:?}"));
    assert!(mismatch.contains("overlay scan violation"), "{mismatch}");
    let old = warnings.iter().find(|w| w.contains("root__auth.login.01__0123456789ab.json")).unwrap_or_else(|| panic!("the 0.29-named file is reported: {warnings:?}"));
    assert!(old.contains("old-layout") && old.contains("canon migrate"), "{old}");
    assert_eq!(doc["data"]["records"].as_array().unwrap().len(), 1);
    assert_eq!(doc["data"]["records"][0]["overlay"]["porting.coverage"]["covered"], true, "the matching record is returned");
}

// ── ledger.naming.16 ─────────────────────────────────────────────────────

/// ledger.naming.16: the kind is `evidence` in the ledger directory, the record's `kind` field,
/// `canon.yaml` routing and aging keys, the schemas, `canon context`, the report views and `--kind`.
#[test]
fn ledger_naming_16_the_kind_is_called_evidence_in_every_place() {
    let dir = repo30(1);
    let repo = dir.path();
    assert!(evidence_add(repo, &["--project-id", "root", "--scenario-id", "auth.login.01", "--summary", "s"]).status.success());

    // The ledger directory and the record's kind field.
    assert!(repo.join(".canon/ledger/kind=evidence").is_dir());
    assert!(!repo.join(".canon/ledger/kind=evidence_record").exists());
    let file = ledger_files(repo, "evidence").remove(0);
    let body: Value = serde_json::from_slice(&std::fs::read(repo.join(".canon/ledger/kind=evidence").join(file)).unwrap()).unwrap();
    assert_eq!(body["kind"], "evidence");

    // `canon.yaml` routing (written by `init`) and an aging key (read by `tier age`).
    let config = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    assert!(config.lines().any(|l| l.trim() == "evidence: local"), "routing.evidence: {config}");
    assert!(!config.contains("evidence_record"));
    let aged = format!("{config}aging:\n  evidence: {{ after: 1d, to: hot }}\n");
    std::fs::write(repo.join("canon.yaml"), aged).unwrap();
    let age = json_of(&canon_in(repo, &["tier", "age", "--json"], &[]));
    assert_eq!(age["data"]["rules"][0]["kind"], "evidence", "{age}");

    // The schemas, `canon context`, the report views, and `--kind`.
    let schemas = Path::new(env!("CARGO_MANIFEST_DIR")).join("../canon-model/schemas");
    assert!(schemas.join("evidence.schema.json").is_file());
    assert!(!schemas.join("evidence_record.schema.json").exists());
    let context = text(&canon_in(repo, &["context", "--json"], &[]));
    assert!(context.contains("\"evidence\":{") && !context.contains("evidence_record"), "canon context");
    let views = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../canon-store/sql/views.sql")).unwrap();
    assert!(views.contains("'evidence'") && !views.contains("evidence_record"), "views.sql");
    let query = canon_in(repo, &["query", "--kind", "evidence", "--json"], &[]);
    assert!(query.status.success(), "{}", text(&query));
    assert_eq!(json_of(&query)["data"]["kind"], "evidence");
    let old_kind = canon_in(repo, &["query", "--kind", "evidence_record"], &[]);
    assert!(!old_kind.status.success(), "`--kind evidence_record` is not a kind");
    let kinds = text(&canon_in(repo, &["query", "--help"], &[]));
    assert!(!kinds.contains("evidence_record"), "the --kind help lists `evidence`");
}

// ── ledger.naming.17 ─────────────────────────────────────────────────────

const SKILLS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../canon/skills");

/// What makes a guide describe the 0.29 layout: the 0.29 kind name used as a current name, an `area=`
/// level in a ledger path (spec trees `kind=feature/area=` keep theirs), a 12-hex digest suffix.
/// A line that names the 0.29 layout only to send the reader to `canon migrate` is the migration prose.
fn guide_drift(guide: &str) -> Vec<String> {
    let area_in_ledger = |line: &str| {
        line.match_indices("/area=").any(|(at, _)| {
            let before = &line[..at];
            before.rfind("kind=").is_some_and(|k| {
                let kind = &before[k + 5..];
                !kind.contains('/') && !matches!(kind, "feature" | "inventory" | "<kind>")
            })
        })
    };
    let twelve_hex = |line: &str| {
        let bytes = line.as_bytes();
        (0..bytes.len().saturating_sub(13)).any(|i| bytes[i] == b'_' && bytes[i + 1..i + 13].iter().all(u8::is_ascii_hexdigit) && !bytes.get(i + 13).is_some_and(u8::is_ascii_hexdigit))
    };
    let mut found = Vec::new();
    for (n, line) in guide.lines().enumerate() {
        let migration_prose = line.contains("0.29") || line.contains("canon migrate");
        if line.contains("evidence_record") && !migration_prose {
            found.push(format!("line {}: names `evidence_record`", n + 1));
        }
        if area_in_ledger(line) && !migration_prose {
            found.push(format!("line {}: an `area=` ledger level", n + 1));
        }
        if twelve_hex(line) && !migration_prose {
            found.push(format!("line {}: a 12-hex digest suffix", n + 1));
        }
    }
    found
}

fn shipped_guides() -> Vec<(String, String)> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push((path.display().to_string(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    walk(Path::new(SKILLS), &mut out);
    assert!(out.len() > 20, "the guides were found: {}", out.len());
    out
}

/// ledger.naming.17: a guide that still describes the 0.29 layout is reported as drift; the guides canon
/// ships and installs are clean, and the storage guide describes the 0.30 name and `canon migrate`.
#[test]
fn ledger_naming_17_a_guide_that_describes_the_029_layout_is_drift() {
    // The check catches each stale form.
    let stale = "Evidence lives in `kind=evidence_record/`.\nReviews: `kind=review/area=gate/canon__x__<pin>__e7f52dd152fe.json`.\nA file `a__e7f52dd152fe.json`.";
    let found = guide_drift(stale);
    assert!(found.iter().any(|f| f.contains("evidence_record")), "{found:?}");
    assert!(found.iter().any(|f| f.contains("area=")), "{found:?}");
    assert!(found.iter().any(|f| f.contains("12-hex")), "{found:?}");
    assert!(guide_drift("Specs live under `kind=feature/area=<area>/`; a record is `kind=<kind>/<components>_<digest8>.json`.").is_empty());

    // The guides canon ships are clean.
    let drift: Vec<String> = shipped_guides().into_iter().flat_map(|(path, text)| guide_drift(&text).into_iter().map(move |f| format!("{path}: {f}"))).collect();
    assert!(drift.is_empty(), "{drift:#?}");
    // So is the guide the binary installs and shows.
    let empty = tempfile::tempdir().unwrap();
    let shown = text(&canon_in(empty.path(), &["skills", "show", "canon", "--full"], &[]));
    assert!(guide_drift(&shown).is_empty(), "{:?}", guide_drift(&shown));

    // The storage guide describes the 0.30 name and names `canon migrate` for a 0.29 repo.
    let storage = std::fs::read_to_string(Path::new(SKILLS).join("reference/canon-storage.md")).unwrap();
    assert!(storage.contains("kind=<kind>/") && storage.contains("_<digest8>.json"), "the storage guide states the name rule");
    assert!(storage.contains("canon migrate"), "the storage guide names `canon migrate`");
    let tier = text(&canon_in(empty.path(), &["skills", "show", "canon-storage"], &[]));
    assert!(tier.contains("<digest8>") && tier.contains("canon migrate"), "the installed storage/tier guide: {}", &tier[..tier.len().min(300)]);
}

// ── ledger.naming.18 ─────────────────────────────────────────────────────

fn keyless_evidence() -> Value {
    json!({"schema": 1, "kind": "evidence", "at": "2026-07-10T12:00:00Z", "actor": {"agent_id": "a", "role": "implementer"}, "verdict": "faithful"})
}

/// ledger.naming.18: evidence naming no task, scenario or run is refused by the model, whichever store
/// it is written to, and no store changes; the CLI's own requirement is the same rule.
#[test]
fn ledger_naming_18_keyless_evidence_is_refused_by_the_model_for_every_writer() {
    let body = keyless_evidence();

    // git tier
    let dir = tempfile::tempdir().unwrap();
    let git = GitTier::new(dir.path().join(".canon/ledger"));
    let err = git.write(&raw(&body)).expect_err("git");
    assert!(matches!(err, StoreError::Layout(_)), "{err:?}");
    assert!(err.to_string().contains("task_id") && err.to_string().contains("scenario_id") && err.to_string().contains("run_id"), "{err}");
    assert!(files(dir.path()).is_empty(), "no store changes");
    // a batch that holds one keyless record writes none of it
    let err = git.write_batch(&[&raw(&well_formed("scenario")), &raw(&body)]).expect_err("git batch");
    assert!(matches!(err, StoreError::Layout(_)), "{err:?}");
    assert!(files(dir.path()).is_empty());

    // sqlite tier
    let sqlite = canon_store::sqlite_tier::SqliteTier::connect(&dir.path().join("hot.db")).unwrap();
    let err = sqlite.write(&raw(&body)).expect_err("sqlite");
    assert!(matches!(err, StoreError::Layout(_)), "{err:?}");
    assert!(sqlite.all_rows().unwrap().is_empty());

    // r2 tier
    let r2_dir = tempfile::tempdir().unwrap();
    let r2 = canon_store::r2_tier::R2Tier::local(r2_dir.path(), "canon/").unwrap();
    let err = r2.write(&raw(&body)).expect_err("r2");
    assert!(matches!(err, StoreError::Layout(_)), "{err:?}");
    assert!(files(r2_dir.path()).is_empty(), "no object was put");

    // the typed record: the same refusal
    let typed = canon_model::EvidenceRecord::new(
        canon_model::Envelope::new(1, RecordKind::Evidence, "2026-07-10T12:00:00Z".parse().unwrap(), canon_model::Actor::new("a", canon_model::RoleId::parse("implementer").unwrap())),
        None,
        None,
        None,
        canon_model::EvidenceVerdict::Faithful,
    );
    assert!(matches!(git.write(&typed), Err(StoreError::Layout(_))));

    // the CLI's own requirement of a task or scenario id
    let repo = repo30(1);
    let before = files(&repo.path().join(".canon/ledger"));
    let out = canon_in(repo.path(), &["evidence", "add", "--class", "test-run", "--ref", "r", "--verdict", "faithful", "--summary", "s", "--actor-id", "a", "--role", "implementer"], &[]);
    assert!(!out.status.success(), "{}", text(&out));
    let message = text(&out);
    assert!(message.contains("--task-id") && message.contains("--scenario-id"), "the CLI names the same requirement: {message}");
    assert_eq!(files(&repo.path().join(".canon/ledger")), before);
}

// ── ledger.naming.19 ─────────────────────────────────────────────────────

fn hex_encode(text: &str) -> String {
    text.bytes().map(|b| format!("{b:02x}")).collect()
}

/// ledger.naming.19: a cursor `canon query` issued in 0.29 names a record by its 0.29 id and 12-hex
/// digest; it is refused INVALID_CURSOR, and the hint says to drop `--cursor` and start from the first page.
#[test]
fn ledger_naming_19_a_029_cursor_is_refused_with_a_way_to_start_over() {
    let dir = repo30(1);
    let repo = dir.path();
    for seq in ["1", "2"] {
        ok(repo, &["finding", "add", "--change-id", "add-login", "--round", "2", "--seq", seq, "--severity", "note", "--actor-id", "bob", "--summary", "s"]);
    }
    // What 0.29 issued for the first of two findings: `query:finding`, then `<at>|<id>__<round:04>__<seq:04>|<12 hex>#1`.
    let at = json_of(&canon_in(repo, &["query", "--kind", "finding", "--json", "--limit", "1"], &[]))["data"]["records"][0]["at"].as_str().unwrap().to_string();
    let cursor_029 = hex_encode(&format!("query:finding\u{1f}{at}|add-login__0002__0001|d992d8cd12ab#1"));
    let out = canon_in(repo, &["query", "--kind", "finding", "--json", "--limit", "1", "--cursor", &cursor_029], &[]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let doc = json_of(&out);
    assert_eq!(doc["error"]["code"], "INVALID_CURSOR", "{doc}");
    let message = doc["error"]["message"].as_str().unwrap();
    assert!(message.contains("0.29") && message.contains("names"), "the message says the cursor names records by 0.29 names: {message}");
    let hint = doc["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("drop --cursor") && hint.contains("first page"), "{hint}");
    // Starting over works, and the cursor 0.30 issues is accepted.
    let first = json_of(&canon_in(repo, &["query", "--kind", "finding", "--json", "--limit", "1"], &[]));
    let next = first["meta"]["nextCursor"].as_str().unwrap();
    let second = canon_in(repo, &["query", "--kind", "finding", "--json", "--limit", "1", "--cursor", next], &[]);
    assert!(second.status.success(), "{}", text(&second));
}

// ── ledger.naming.21 ─────────────────────────────────────────────────────

/// ledger.naming.21: staged divergences (and evidence) sharing every sort field but their body are named
/// `<project>_<scenario>_<run seq>_<round>_<digest8>` with no padding, and commit in the order of their
/// typed components then body digest, whatever order staging was scanned in.
#[test]
fn ledger_naming_21_promotion_breaks_ties_by_components_then_digest() {
    let dir = repo30(1);
    let repo = dir.path();
    for detail in ["alpha", "beta", "gamma"] {
        ok(repo, &["divergence", "add", "auth.login.01", "--actor-id", "r", "--round", "1", "--role", "reviewer", "--no-promote", "--detail", detail]);
        let out = evidence_add(repo, &["--project-id", "root", "--scenario-id", "auth.login.01", "--summary", detail, "--no-promote"]);
        assert!(out.status.success(), "{}", text(&out));
    }
    // The staged bodies, by the digest in each staged name.
    let staged = |kind: &str, field: &dyn Fn(&Value) -> String| -> Vec<(String, String)> {
        let dir = repo.join(format!(".canon/ledger/_staging/kind={kind}"));
        let mut out: Vec<(String, String)> = ledger_names(&dir)
            .into_iter()
            .map(|name| {
                let body: Value = serde_json::from_slice(&std::fs::read(dir.join(&name)).unwrap()).unwrap();
                let digest = name.strip_suffix(".json").unwrap().rsplit_once('_').unwrap().1.to_string();
                assert_eq!(digest, d8(&body));
                (digest, field(&body))
            })
            .collect();
        out.sort();
        out
    };
    let divergences = staged("divergence", &|b| b["detail"].as_str().unwrap().to_string());
    let evidence = staged("evidence", &|b| b["evidence_note"]["summary"].as_str().unwrap().to_string());
    assert_eq!((divergences.len(), evidence.len()), (3, 3));
    for (digest, _) in &divergences {
        assert!(!digest.is_empty());
    }

    ok(repo, &["gate", "promote"]);

    // Every committed version, ledger and archive: its run_seq tells the commit order.
    let committed = |kind: &str, field: &dyn Fn(&Value) -> String| -> Vec<(u64, String, String)> {
        let mut out = Vec::new();
        for root in [".canon/ledger", ".canon/archive"] {
            let dir = repo.join(root).join(format!("kind={kind}"));
            for name in ledger_names(&dir) {
                let body: Value = serde_json::from_slice(&std::fs::read(dir.join(&name)).unwrap()).unwrap();
                out.push((body["run_seq"].as_u64().unwrap(), field(&body), name));
            }
        }
        out.sort();
        out
    };
    let committed_div = committed("divergence", &|b| b["detail"].as_str().unwrap().to_string());
    assert_eq!(committed_div.len(), 3);
    for (run_seq, detail, name) in &committed_div {
        let body_digest = name.strip_suffix(".json").unwrap().rsplit_once('_').unwrap().1;
        assert_eq!(name, &format!("root_auth.login.01_{run_seq}_1_{body_digest}.json"), "`<project>_<scenario>_<run seq>_<round>_<digest8>`, no padding ({detail})");
    }
    assert_eq!(committed_div.iter().map(|c| c.1.clone()).collect::<Vec<_>>(), divergences.iter().map(|d| d.1.clone()).collect::<Vec<_>>(), "run_seq 1, 2, 3 follow the staged body digests in ascending order");
    assert_eq!(committed_div.iter().map(|c| c.0).collect::<Vec<_>>(), [1, 2, 3]);

    let committed_ev = committed("evidence", &|b| b["evidence_note"]["summary"].as_str().unwrap().to_string());
    assert_eq!(committed_ev.iter().map(|c| c.1.clone()).collect::<Vec<_>>(), evidence.iter().map(|d| d.1.clone()).collect::<Vec<_>>(), "evidence commits in the same order");
    assert!(committed_ev.iter().all(|(seq, _, name)| name.starts_with("root_scenario_auth.login.01_") && *seq >= 1));
}

/// ledger.naming.22: a `canon.yaml` key still spelled `evidence_record` is
/// read only to send the user to `canon migrate`.
#[test]
fn ledger_naming_22_a_canon_yaml_key_spelled_evidence_record_sends_the_user_to_migrate() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let canon = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env_remove("CANON_ROLE").output().unwrap();
    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(repo).output().unwrap().status.success());
    git(&["init", "-q"]);
    assert!(canon(&["init", "--repo", ".", "--no-policy"]).status.success());
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    std::fs::write(repo.join("canon.yaml"), format!("{yaml}routing:\n  evidence_record: local\n")).unwrap();
    git(&["add", "-A"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "0.30 repo with a 0.29 key"]);

    let o = canon(&["context", "--json"]);
    assert!(o.status.success(), "{}", text(&o));
    let doc = json_of(&o);
    let warnings = doc["meta"]["warnings"].as_array().expect("a warning");
    assert!(warnings.iter().any(|w| w.as_str().is_some_and(|w| w.contains("`evidence_record` spelled the 0.29 way"))), "{doc}");
    assert_eq!(doc["meta"]["next"][0]["command"], "canon migrate --json", "{doc}");

    let before = files(&repo.join(".canon"));
    let o = canon(&["evidence", "add", "--scenario-id", "cart.add.01", "--class", "test-run", "--ref", "x", "--verdict", "faithful", "--role", "implementer", "--actor-id", "a", "--session-id", "s", "--json"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    let doc = json_of(&o);
    assert_eq!(doc["error"]["code"], "OLD_LAYOUT", "{doc}");
    assert!(doc["error"]["hint"].as_str().is_some_and(|h| h.contains("canon migrate")), "{doc}");
    assert_eq!(files(&repo.join(".canon")), before, "nothing is written");
}

// ── ledger.naming.23 ─────────────────────────────────────────────────────

/// ledger.naming.23: a divergence file whose name says another round than its body is a layout violation:
/// the reads skip it, and with `spec_coverage` on `canon gate check` names the file and exits 1. With no
/// `spec_coverage` section the gate stays green and `canon status` points at `canon query`.
#[test]
fn ledger_naming_23_a_misnamed_divergence_is_a_layout_violation_that_gate_check_names() {
    let dir = repo30(1);
    let repo = dir.path();
    ok(repo, &["divergence", "add", "auth.login.01", "--actor-id", "r", "--round", "1", "--role", "reviewer", "--detail", "d"]);
    let ledger = repo.join(".canon/ledger/kind=divergence");
    let name = ledger_names(&ledger).remove(0);
    assert!(name.starts_with("root_auth.login.01_1_1_"), "{name}");
    let wrong = name.replacen("_1_1_", "_1_2_", 1);
    std::fs::rename(ledger.join(&name), ledger.join(&wrong)).unwrap();
    assert!(std::fs::read_to_string(repo.join(".canon/policy.yaml")).unwrap().contains("spec_coverage:"), "the scaffolded policy turns spec coverage on");

    let gate = canon_in(repo, &["gate", "check", "--json"], &[]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    let doc = json_of(&gate);
    let violations = doc["data"]["violations"].as_array().unwrap();
    let named = violations.iter().find(|v| v["subject"] == format!("kind=divergence/{wrong}")).unwrap_or_else(|| panic!("the file is named: {doc}"));
    let message = named["message"].as_str().unwrap();
    assert!(message.contains("layout: expected") && message.contains(&name), "{message}");

    let query = json_of(&canon_in(repo, &["query", "--kind", "divergence", "--json"], &[]));
    assert_eq!(query["data"]["records"].as_array().unwrap().len(), 0, "the misnamed divergence is not read: {query}");
    assert_eq!(query["data"]["violations"], 1, "{query}");

    // No `spec_coverage` section: the corpus is not being judged, the gate stays green, and status says what reports the file.
    std::fs::remove_file(repo.join(".canon/policy.yaml")).unwrap();
    let gate = canon_in(repo, &["gate", "check", "--json"], &[]);
    assert_eq!(gate.status.code(), Some(0), "{}", text(&gate));
    assert!(!text(&gate).contains(&wrong), "{}", text(&gate));
    let status = text(&canon_in(repo, &["status"], &[]));
    assert!(status.contains("`canon query --kind divergence`") && status.contains("does not judge them without a `spec_coverage` section"), "{status}");
    assert!(!status.contains("`canon gate check` names them"), "{status}");
}

/// ledger.naming.23, findings: under the scaffolded policy (`block_on_findings`
/// on) a misnamed finding file is named once, by the review gate's unreadable
/// finding check, not a second time by the spec-coverage layout check.
#[test]
fn ledger_naming_23b_a_misnamed_finding_is_named_once() {
    let dir = repo30(1);
    let repo = dir.path();
    ok(repo, &["finding", "add", "--change-id", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor-id", "bob", "--summary", "s"]);
    let ledger = repo.join(".canon/ledger/kind=finding");
    let name = ledger_names(&ledger).remove(0);
    assert!(name.starts_with("add-login_1_1_"), "{name}");
    let wrong = name.replacen("_1_1_", "_1_2_", 1);
    std::fs::rename(ledger.join(&name), ledger.join(&wrong)).unwrap();

    let gate = canon_in(repo, &["gate", "check", "--json"], &[]);
    let doc = json_of(&gate);
    let naming: Vec<&Value> = doc["data"]["violations"].as_array().unwrap().iter().filter(|v| v.to_string().contains(&wrong)).collect();
    assert_eq!(naming.len(), 1, "the misnamed finding is named exactly once: {doc}");
}
