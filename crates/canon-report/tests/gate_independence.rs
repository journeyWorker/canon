//! Acceptance (s24 spec.md, "The scope-status panel is read-only
//! reporting and never a `canon gate` input"): adding the `##
//! Scope status` panel / `SNAPSHOT_TABLES` entry must not change
//! `canon gate check`'s verdicts, before or after this change lands —
//! `canon-gate` reads nothing produced by `canon-report`. This crate
//! never depends on `canon-gate` (so it cannot even shell out to a
//! `canon gate check` byte-diff), so the guard this test provides is
//! the STRUCTURAL half of that promise: `canon-gate`'s own
//! `Cargo.toml` never depends on `canon-report`, and no `canon-gate`
//! source file references this crate's crate name, its `mart_*`
//! output, or `mart_scope_status` specifically — mirroring
//! `crates/canon-cli/tests/plugin_sync.rs`'s established
//! `gate_never_reads_porting_specific_names`-shaped forbidden-word
//! scan for the `porting` plugin's own connector-never-authority
//! posture. The BEHAVIORAL half (`canon gate check` verdicts
//! byte-identical before/after) is covered by
//! `crates/canon-cli/tests/plugin_sync.rs`/`plans_ingest.rs`
//! (tasks.md 6.4) — this change touches no `canon-gate` file at all,
//! so those acceptance tests are unmodified and still green.

use std::path::{Path, PathBuf};

fn canon_gate_src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("crates/ parent").join("canon-gate").join("src")
}

fn canon_gate_cargo_toml() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("crates/ parent").join("canon-gate").join("Cargo.toml")
}

fn rust_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap_or_else(|e| panic!("read_dir {}: {e}", d.display())) {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn canon_gate_cargo_toml_never_depends_on_canon_report() {
    let text = std::fs::read_to_string(canon_gate_cargo_toml()).expect("read canon-gate/Cargo.toml");
    assert!(!text.contains("canon-report"), "canon-gate/Cargo.toml must never depend on canon-report:\n{text}");
}

#[test]
fn no_canon_gate_source_file_references_canon_report_or_its_marts() {
    let gate_src = canon_gate_src_dir();
    let forbidden = ["canon_report", "canon-report", "mart_scope_status", "mart_trust_matrix", "mart_session_costs", "mart_role_memory", "mart_flywheel_funnel", "mart_review_burndown"];
    for path in rust_files_under(&gate_src) {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for needle in forbidden {
            assert!(!text.contains(needle), "canon-gate must never read a canon-report mart/output — found `{needle}` in {}", path.display());
        }
    }
}
