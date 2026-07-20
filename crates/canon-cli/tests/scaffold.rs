//! Integration tests for `canon scenario new <tag> --title <label>
//! --feature <path>` + `canon feature new <area>.<surface> --title
//! <label>` (s16 `s16-plugin-extensibility`, P5
//! `corpus-authoring-scaffold`, tasks.md 5.3) — invokes the
//! actually-built `canon` binary (`env!("CARGO_BIN_EXE_canon")`)
//! against an offline git-tier fixture repo, zero network, no
//! credentials (mirrors `tests/plugin_sync.rs`'s own subprocess-
//! boundary discipline, never `canon_cli`'s library functions
//! in-process).
//!
//! Covers every scenario spec.md/tasks.md 5.3 names: a scaffolded
//! `.feature` round-trips through `canon fmt --check` clean;
//! `canon inventory sync` materializes exactly the tagged scenario,
//! identical (on `project_id`/`scenario_id`/`title`) to an
//! independently hand-authored `.feature` entry; `canon scenario new`
//! against an already-existing tag is rejected loud (nonzero exit, the
//! `.feature` file's bytes unchanged); `canon feature new` against an
//! already-existing file is rejected loud (nonzero exit, the file's
//! bytes unchanged); and neither command writes any ledger record.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const CANON_YAML: &str = "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  scenario: local\n";

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn write_canon_yaml(dir: &Path) {
    write(dir, "canon.yaml", CANON_YAML);
}

/// Every regular file under `dir`, recursively (hand-rolled — no extra
/// dev-dependency, mirroring `tests/plugin_sync.rs::files_under`).
fn files_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

fn feature_new(dir: &Path, surface: &str, title: &str) -> Output {
    run_canon(&["feature", "new", surface, "--title", title], dir)
}

fn scenario_new(dir: &Path, tag: &str, title: &str, feature: &str) -> Output {
    run_canon(&["scenario", "new", tag, "--title", title, "--feature", feature], dir)
}

fn scenario_new_no_feature(dir: &Path, tag: &str, title: &str) -> Output {
    run_canon(&["scenario", "new", tag, "--title", title], dir)
}

const HOTDEAL_FEATURE_PATH: &str = "specs/features/kind=feature/area=world/hotdeal.feature";

// ── round-trip through `canon fmt --check` clean ──

#[test]
fn a_scaffolded_feature_round_trips_through_canon_fmt_check_clean() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = feature_new(dir.path(), "world.hotdeal", "Hotdeal");
    assert!(out.status.success(), "canon feature new failed: {}", stderr(&out));

    let out = scenario_new(dir.path(), "world.hotdeal.42", "Apply a hotdeal coupon", HOTDEAL_FEATURE_PATH);
    assert!(out.status.success(), "canon scenario new failed: {}", stderr(&out));

    let out = run_canon(&["fmt", "--check", "specs"], dir.path());
    assert!(out.status.success(), "canon fmt --check must be clean over the scaffolded corpus: {}\n{}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains(", 0 violation(s)"), "expected zero violations: {}", stdout(&out));
}

#[test]
fn a_bare_feature_new_file_has_the_provenance_comment_and_zero_scenarios() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = feature_new(dir.path(), "world.checkout", "Checkout flow");
    assert!(out.status.success(), "canon feature new failed: {}", stderr(&out));

    let path = dir.path().join("specs/features/kind=feature/area=world/checkout.feature");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.starts_with("Feature: Checkout flow\n"), "content: {content:?}");
    assert!(content.contains("# canon: {"), "content missing provenance comment: {content:?}");
    assert!(!content.contains("Scenario:"), "a fresh `canon feature new` file must carry zero Scenario: blocks: {content:?}");
}

#[test]
fn a_bare_feature_new_stub_is_fmt_dirty_until_scenario_new_adds_a_tagged_scenario() {
    // ReviewS16P5 contract clarification: the `corpus-authoring-scaffold`
    // spec ties the fmt-clean round-trip to `scenario new`'s output. A
    // bare `feature new` stub (zero scenarios) is an in-progress starting
    // point, NOT yet a valid corpus entry (canon-fmt's feature resolver
    // needs a `@<area>.<surface>.<nn>` tag to derive `area` from). This
    // locks the intended dirty -> add-a-scenario -> clean transition.
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = feature_new(dir.path(), "world.checkout", "Checkout flow");
    assert!(out.status.success(), "canon feature new failed: {}", stderr(&out));

    // Bare stub: `fmt --check` flags the tagless feature (not yet valid corpus).
    let out = run_canon(&["fmt", "--check", "specs"], dir.path());
    assert!(!out.status.success(), "a bare zero-scenario feature stub must not be fmt-clean yet: {}", stdout(&out));

    // Adding the first tagged scenario makes the same file fmt-clean.
    let out = scenario_new(dir.path(), "world.checkout.01", "Start checkout", "specs/features/kind=feature/area=world/checkout.feature");
    assert!(out.status.success(), "canon scenario new failed: {}", stderr(&out));
    let out = run_canon(&["fmt", "--check", "specs"], dir.path());
    assert!(out.status.success(), "after `scenario new` the stub must be fmt-clean: {}\n{}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains(", 0 violation(s)"), "expected zero violations: {}", stdout(&out));
}

// ── inventory sync materializes exactly the tagged scenario ──

#[test]
fn inventory_sync_materializes_exactly_the_tagged_scenario_identical_to_a_hand_authored_entry() {
    let scaffolded = tempfile::tempdir().unwrap();
    write_canon_yaml(scaffolded.path());
    assert!(feature_new(scaffolded.path(), "world.hotdeal", "Hotdeal").status.success());
    assert!(scenario_new(scaffolded.path(), "world.hotdeal.42", "Apply a hotdeal coupon", HOTDEAL_FEATURE_PATH).status.success());

    let hand_authored = tempfile::tempdir().unwrap();
    write_canon_yaml(hand_authored.path());
    // Hand-typed, NEVER produced by calling into `canon_cli::scaffold` —
    // the exact style a human author would type, mirroring
    // `tests/plugin_sync.rs::write_repo`'s own `feature_text` literal.
    let prov = "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"a-human\"}}";
    let feature_text = format!("Feature: world hotdeal\n{prov}\n\n  @world.hotdeal.42\n  Scenario: Apply a hotdeal coupon\n{prov}\n    Given a step\n");
    write(hand_authored.path(), HOTDEAL_FEATURE_PATH, &feature_text);

    for dir in [scaffolded.path(), hand_authored.path()] {
        let out = run_canon(&["inventory", "sync", "--repo", "."], dir);
        assert!(out.status.success(), "canon inventory sync failed: {}", stderr(&out));
    }

    let scenario_record = |dir: &Path| -> Value {
        let out = run_canon(&["query", "--kind", "scenario", "--json"], dir);
        assert!(out.status.success(), "canon query failed: {}", stderr(&out));
        let payload: Value = serde_json::from_str(&stdout(&out)).expect("valid JSON on stdout");
        let records = payload["records"].as_array().cloned().unwrap_or_default();
        assert_eq!(records.len(), 1, "expected exactly one Scenario record, got {records:?}");
        records[0].clone()
    };

    let scaffolded_record = scenario_record(scaffolded.path());
    let hand_authored_record = scenario_record(hand_authored.path());

    for record in [&scaffolded_record, &hand_authored_record] {
        assert_eq!(record["project_id"], "root");
        assert_eq!(record["scenario_id"], "world.hotdeal.42");
        assert_eq!(record["title"], "Apply a hotdeal coupon");
    }
    // Identical on every field `canon inventory sync` derives from
    // content alone -- `source_digest`/envelope `at` legitimately
    // differ (different provenance timestamps/bytes), so those are
    // deliberately excluded from this comparison.
    assert_eq!(scaffolded_record["project_id"], hand_authored_record["project_id"]);
    assert_eq!(scaffolded_record["scenario_id"], hand_authored_record["scenario_id"]);
    assert_eq!(scaffolded_record["title"], hand_authored_record["title"]);
}

// ── writes no ledger record ──

#[test]
fn neither_command_writes_any_ledger_record() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());
    let ledger_dir = dir.path().join("canon/ledger");

    assert!(feature_new(dir.path(), "world.hotdeal", "Hotdeal").status.success());
    assert!(scenario_new(dir.path(), "world.hotdeal.42", "Apply a hotdeal coupon", HOTDEAL_FEATURE_PATH).status.success());

    assert!(!ledger_dir.exists() || files_under(&ledger_dir).is_empty(), "canon scenario new/canon feature new must write NO ledger record");
}

// ── `canon scenario new` against an existing tag is rejected loud ──

#[test]
fn scenario_new_against_an_already_existing_tag_is_rejected_loud_with_no_duplicate_written() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());
    assert!(feature_new(dir.path(), "world.hotdeal", "Hotdeal").status.success());

    let first = scenario_new(dir.path(), "world.hotdeal.42", "Apply a hotdeal coupon", HOTDEAL_FEATURE_PATH);
    assert!(first.status.success(), "first canon scenario new failed: {}", stderr(&first));

    let path = dir.path().join(HOTDEAL_FEATURE_PATH);
    let bytes_after_first = std::fs::read(&path).unwrap();

    let second = scenario_new(dir.path(), "world.hotdeal.42", "A different label entirely", HOTDEAL_FEATURE_PATH);
    assert!(!second.status.success(), "a duplicate tag must be rejected loud (nonzero exit)");
    let second_stderr = stderr(&second);
    assert!(second_stderr.contains("world.hotdeal.42"), "stderr must name the existing tag: {second_stderr}");

    let bytes_after_second = std::fs::read(&path).unwrap();
    assert_eq!(bytes_after_first, bytes_after_second, "a rejected duplicate must leave the `.feature` file byte-for-byte unchanged");

    // Never a silent duplicate: exactly one `@world.hotdeal.42` tag.
    let text = String::from_utf8(bytes_after_second).unwrap();
    assert_eq!(text.matches("@world.hotdeal.42").count(), 1, "the tag must appear exactly once: {text:?}");
}

// ── `canon feature new` against an existing file is rejected loud ──

#[test]
fn feature_new_against_an_already_existing_file_is_rejected_loud_with_the_file_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let first = feature_new(dir.path(), "world.checkout", "Checkout flow");
    assert!(first.status.success(), "first canon feature new failed: {}", stderr(&first));

    let path = dir.path().join("specs/features/kind=feature/area=world/checkout.feature");
    let bytes_after_first = std::fs::read(&path).unwrap();

    let second = feature_new(dir.path(), "world.checkout", "A completely different title");
    assert!(!second.status.success(), "an existing feature file must be rejected loud (nonzero exit)");
    let second_stderr = stderr(&second);
    assert!(second_stderr.contains("already exists"), "stderr must explain the refusal: {second_stderr}");

    let bytes_after_second = std::fs::read(&path).unwrap();
    assert_eq!(bytes_after_first, bytes_after_second, "a rejected `canon feature new` must leave the existing file byte-for-byte unchanged");
}

// ── s19 `derived-validated-scenario-feature`: --feature is optional, tag-derived, validated ──

const CANON_YAML_TWO_ROOTS: &str =
    "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  scenario: local\nspecs:\n  roots:\n    - id: root\n      root: specs\n    - id: extra\n      root: specs2\n";

#[test]
fn omitting_feature_derives_the_identical_path_feature_new_would_scaffold() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = scenario_new_no_feature(dir.path(), "world.hotdeal.42", "Apply a hotdeal coupon");
    assert!(out.status.success(), "canon scenario new (no --feature) failed: {}", stderr(&out));

    let path = dir.path().join(HOTDEAL_FEATURE_PATH);
    assert!(path.exists(), "expected the tag-derived path `{}` to exist", path.display());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.starts_with("Feature: world hotdeal\n"), "content: {content:?}");
    assert!(content.contains("@world.hotdeal.42"), "content missing the tag: {content:?}");
}

#[test]
fn omitting_feature_under_an_ambiguous_multi_root_config_refuses_loud() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "canon.yaml", CANON_YAML_TWO_ROOTS);

    let out = scenario_new_no_feature(dir.path(), "world.hotdeal.42", "Apply a hotdeal coupon");
    assert!(!out.status.success(), "an ambiguous multi-root config must refuse loud when --feature is omitted");
    assert_eq!(out.status.code(), Some(2), "must exit 2: {}", stderr(&out));
    assert!(stderr(&out).contains("2 configured"), "stderr must name the ambiguity: {}", stderr(&out));

    assert!(!dir.path().join(HOTDEAL_FEATURE_PATH).exists(), "zero bytes written on refusal");
    assert!(!dir.path().join("specs2").exists(), "zero bytes written on refusal");
}

#[test]
fn an_explicit_feature_path_outside_every_configured_root_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = scenario_new(dir.path(), "wall.render.03", "guess", "wall.render");
    assert!(!out.status.success(), "a --feature path resolving outside every configured root must be refused");
    assert_eq!(out.status.code(), Some(2), "must exit 2: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("wall.render"), "stderr must name the attempted path: {err}");
    assert!(err.contains("specs"), "stderr must name the configured root(s): {err}");

    assert!(!dir.path().join("wall.render").exists(), "the repo-root orphan must never be written");
    assert!(!dir.path().join("specs").exists(), "zero bytes written anywhere on refusal");
}

#[test]
fn an_explicit_feature_path_under_a_configured_root_in_a_non_canonical_subpath_still_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = scenario_new(dir.path(), "wall.render.04", "a hand-grouped surface", "specs/features/kind=feature/area=wall/misc.feature");
    assert!(out.status.success(), "a --feature path under a configured root must succeed even in a non-canonical subpath: {}", stderr(&out));

    let path = dir.path().join("specs/features/kind=feature/area=wall/misc.feature");
    assert!(path.exists(), "expected the explicit non-canonical path to be written");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("@wall.render.04"), "content missing the tag: {content:?}");
}

#[test]
fn a_duplicate_tag_is_still_refused_when_the_path_is_derived() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let first = scenario_new_no_feature(dir.path(), "world.hotdeal.42", "Apply a hotdeal coupon");
    assert!(first.status.success(), "first canon scenario new (no --feature) failed: {}", stderr(&first));

    let path = dir.path().join(HOTDEAL_FEATURE_PATH);
    let bytes_after_first = std::fs::read(&path).unwrap();

    let second = scenario_new_no_feature(dir.path(), "world.hotdeal.42", "A different label entirely");
    assert!(!second.status.success(), "a duplicate tag must be refused loud even when the path is derived");
    assert!(stderr(&second).contains("world.hotdeal.42"), "stderr must name the existing tag: {}", stderr(&second));

    let bytes_after_second = std::fs::read(&path).unwrap();
    assert_eq!(bytes_after_first, bytes_after_second, "a rejected duplicate must leave the `.feature` file byte-for-byte unchanged");
}

// ── s19 `wip-feature-stub-class`: next-step hint + reworded fmt message ──

#[test]
fn feature_new_prints_a_next_step_hint_and_fmt_check_reports_the_wip_stub_wording() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = feature_new(dir.path(), "world.hotdeal", "Hotdeal");
    assert!(out.status.success(), "canon feature new failed: {}", stderr(&out));
    let hint = stdout(&out);
    assert!(
        hint.contains("next: `canon scenario new world.hotdeal.01 --title '<label>' [--feature <path>]`"),
        "expected a next-step hint naming the derived `canon scenario new` invocation: {hint}"
    );

    let check_out = run_canon(&["fmt", "--check", "specs"], dir.path());
    assert!(!check_out.status.success(), "a bare stub must still be fmt-dirty (unchanged exit-code contract)");
    let report = stdout(&check_out);
    assert!(
        report.contains("empty feature stub (not yet a valid corpus entry)"),
        "expected the reworded WIP-stub violation text in `canon fmt --check`'s report: {report}"
    );
}

// ── s19 review nit: `specs` vs `specs2` is a SIBLING, not a prefix ──

/// Guards `path_under_root`'s component-wise `Path::starts_with` check
/// (design D3's own doc comment: "a root named `specs` never falsely
/// accepts a sibling `specs2`"): with a single configured root
/// (`specs`, the implicit default), `--feature specs2/x.feature` names
/// a SIBLING directory that merely shares `specs` as a string PREFIX —
/// a naive `str::starts_with`/non-component-wise check would wrongly
/// accept it as "under `specs`". It must be refused exactly like any
/// other out-of-root path: exit 2, zero bytes written anywhere under
/// `specs2/`.
#[test]
fn an_explicit_feature_path_under_a_sibling_directory_sharing_a_string_prefix_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = scenario_new(dir.path(), "wall.render.06", "guess", "specs2/x.feature");
    assert!(!out.status.success(), "`specs2/` must never be falsely accepted as under the configured `specs` root");
    assert_eq!(out.status.code(), Some(2), "must exit 2: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("specs2"), "stderr must name the attempted path: {err}");
    assert!(err.contains("specs"), "stderr must name the configured root(s): {err}");

    assert!(!dir.path().join("specs2").exists(), "zero bytes written under the sibling directory on refusal");
}

// ── s26 `repo-flag-uniformity` D3: `@`-prefixed tag == bare form ──

fn scenario_new_no_feature_with_repo(dir: &Path, tag: &str, title: &str) -> Output {
    run_canon(&["scenario", "new", tag, "--title", title, "--repo", "."], dir)
}

/// Provenance comments carry a fresh `Utc::now()` per invocation
/// (module doc, "Deterministic provenance ... never a bare
/// `Utc::now()`" -- but each SEPARATE `canon scenario new` process run
/// still stamps its OWN wall-clock time), so a cross-invocation
/// byte-identity check must normalize the `"at":"..."` provenance
/// timestamp field before comparing.
fn normalize_provenance_timestamps(content: &str) -> String {
    let marker = "\"at\":\"";
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(idx) = rest.find(marker) {
        out.push_str(&rest[..idx + marker.len()]);
        rest = &rest[idx + marker.len()..];
        out.push_str("STAMP");
        let Some(end) = rest.find('"') else { break };
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn an_at_prefixed_tag_writes_the_same_header_a_bare_tag_would() {
    let dir_at = tempfile::tempdir().unwrap();
    write_canon_yaml(dir_at.path());
    let out = scenario_new_no_feature_with_repo(dir_at.path(), "@story.x.01", "T");
    assert!(out.status.success(), "canon scenario new @tag failed: {}", stderr(&out));

    let dir_bare = tempfile::tempdir().unwrap();
    write_canon_yaml(dir_bare.path());
    let out = scenario_new_no_feature_with_repo(dir_bare.path(), "story.x.01", "T");
    assert!(out.status.success(), "canon scenario new bare tag failed: {}", stderr(&out));

    let feature_path = "specs/features/kind=feature/area=story/x.feature";
    let content_at = std::fs::read_to_string(dir_at.path().join(feature_path)).unwrap();
    let content_bare = std::fs::read_to_string(dir_bare.path().join(feature_path)).unwrap();
    assert_eq!(
        normalize_provenance_timestamps(&content_at),
        normalize_provenance_timestamps(&content_bare),
        "an `@`-prefixed tag must write the identical `.feature` content (modulo per-invocation provenance timestamps) as the bare form"
    );
    assert!(content_at.contains("\n  @story.x.01\n"), "content missing the `@story.x.01` tag line: {content_at:?}");
    assert!(content_at.contains("Scenario: T"), "content missing the `Scenario: T` header: {content_at:?}");
}

#[test]
fn the_bare_form_still_succeeds_unchanged_with_explicit_repo() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());
    let out = scenario_new_no_feature_with_repo(dir.path(), "story.x.02", "T");
    assert!(out.status.success(), "canon scenario new (bare tag, --repo .) failed: {}", stderr(&out));
    let content = std::fs::read_to_string(dir.path().join("specs/features/kind=feature/area=story/x.feature")).unwrap();
    assert!(content.contains("\n  @story.x.02\n"), "content missing the `@story.x.02` tag line: {content:?}");
    assert!(content.contains("Scenario: T"), "content missing the `Scenario: T` header: {content:?}");
}

#[test]
fn a_malformed_tag_is_still_refused_after_stripping_a_leading_at() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path());

    let out = scenario_new_no_feature_with_repo(dir.path(), "@Story.X.01", "T");
    assert!(!out.status.success(), "an uppercase segment must still be refused after stripping `@`");
    assert_eq!(out.status.code(), Some(2), "must exit 2: {}", stderr(&out));

    let dir_bare = tempfile::tempdir().unwrap();
    write_canon_yaml(dir_bare.path());
    let out_bare = scenario_new_no_feature_with_repo(dir_bare.path(), "Story.X.01", "T");
    assert!(!out_bare.status.success(), "the bare malformed form must be refused identically");
    assert_eq!(out_bare.status.code(), Some(2), "must exit 2: {}", stderr(&out_bare));
}
