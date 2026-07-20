//! Acceptance (s25 `report-pg-tier-boundary` spec.md, s27
//! `tier-role-backend-split` design D2, s28 `rung-backend-capability`
//! design D2/D3): a multi-tier `canon.yaml` renders `## Kinds not read
//! directly` naming exactly the kinds routed to a rung whose backend
//! is not read directly by the report, sorted, never a git-backed
//! rung's kind; two renders of an unchanged multi-tier config are
//! byte-identical; a repo with no `canon.yaml` (every other fixture in
//! this crate) renders no section at all; a `canon.yaml` whose every
//! routed rung resolves to a git-backed (directly-read) backend also
//! renders no section; an s3-backed `cold` rung — s28's correction —
//! now APPEARS in the section (s27 wrongly excluded it).

mod support;

use canon_report::{report, ReportInputs};

fn write_canon_yaml(dir: &std::path::Path, text: &str) {
    std::fs::write(dir.join("canon.yaml"), text).unwrap();
}

#[test]
fn multi_tier_config_renders_the_boundary_note_naming_exactly_the_kinds_not_read_directly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = support::corpus::build(dir.path());
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_TB1, schema: canon_v1 }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_TB1, prefix: \"canon/\" }\nrouting:\n  task: hot\n  session: hot\n  change: local\n  scenario: cold\n",
    );
    let inputs = ReportInputs::new(dir.path(), roots);

    let content = report(&inputs).unwrap();

    assert!(content.contains("## Kinds not read directly"), "{content}");
    assert!(content.contains("- `session`\n"), "{content}");
    assert!(content.contains("- `task`\n"), "{content}");
    assert!(content.contains("- `scenario`\n"), "an s3-backed (cold)-routed kind must be named (s28 D2 correction):\n{content}");
    assert!(content.contains("canon query --kind"), "{content}");
    assert!(!content.contains("- `change`\n"), "a local (git-backed)-routed kind must never be named:\n{content}");

    // Placement: after `## Inputs (digest)`, before `## Trust matrix`.
    let inputs_pos = content.find("## Inputs (digest)").unwrap();
    let note_pos = content.find("## Kinds not read directly").unwrap();
    let trust_pos = content.find("## Trust matrix").unwrap();
    assert!(inputs_pos < note_pos && note_pos < trust_pos, "note must sit between Inputs and Trust matrix:\n{content}");
}

#[test]
fn two_renders_of_an_unchanged_multi_tier_config_are_byte_identical() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = support::corpus::build(dir.path());
    write_canon_yaml(
        dir.path(),
        "tiers:\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_TB2, schema: canon_v1 }\nrouting:\n  task: hot\n  event: hot\n",
    );
    let inputs = ReportInputs::new(dir.path(), roots);

    let first = report(&inputs).unwrap();
    let second = report(&inputs).unwrap();

    assert_eq!(first, second, "two renders over an unchanged multi-tier canon.yaml must be byte-identical");
}

#[test]
fn no_canon_yaml_renders_no_boundary_section() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = support::corpus::build(dir.path());
    let inputs = ReportInputs::new(dir.path(), roots);

    let content = report(&inputs).unwrap();

    assert!(!content.contains("## Kinds not read directly"), "no canon.yaml must render no section:\n{content}");
}

#[test]
fn git_only_routing_renders_no_boundary_section() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = support::corpus::build(dir.path());
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  task: local\n  change: local\n",
    );
    let inputs = ReportInputs::new(dir.path(), roots);

    let content = report(&inputs).unwrap();

    assert!(
        !content.contains("## Kinds not read directly"),
        "a routing table whose every rung resolves to a git-backed (directly-read) backend must render no section:\n{content}"
    );
}

/// s28 `rung-backend-capability` spec scenario: a `cold` rung backed
/// by `s3` — today's class-correct pairing (s28 design D1) — is not
/// read directly by the report (design D2), so it now names the
/// boundary section even though s27's `offline_file_readable()`
/// wrongly excluded it.
#[test]
fn a_cold_rung_backed_by_s3_now_appears_in_the_boundary_section() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = support::corpus::build(dir.path());
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_TB5, prefix: \"canon/\" }\nrouting:\n  task: local\n  change: local\n  scenario: cold\n",
    );
    let inputs = ReportInputs::new(dir.path(), roots);

    let content = report(&inputs).unwrap();

    assert!(content.contains("## Kinds not read directly"), "{content}");
    assert!(content.contains("- `scenario`\n"), "an s3-backed (cold)-routed kind must be named:\n{content}");
    assert!(!content.contains("- `task`\n") && !content.contains("- `change`\n"), "local (git-backed)-routed kinds must never be named:\n{content}");
}
