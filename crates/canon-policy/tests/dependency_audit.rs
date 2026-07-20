//! Task 1.2: a workspace-level dependency audit confirming the donor CEL
//! parser crate (and the upstream `cel-parser` crate the spec excludes)
//! never appear in `canon-policy`'s dependency graph, direct or transitive
//! (design D1, spec scenario "canon-policy's dependency graph excludes the
//! donor CEL parser crate"). Automated rather than only documented — this
//! test fails loudly the moment any future `canon-policy` dependency change
//! reintroduces one of the excluded crates, without anyone remembering to
//! re-run `cargo tree` by hand.

use std::process::Command;

const BANNED: &[&str] = &["cel-parser", "cel_parser"];

#[test]
fn dependency_graph_excludes_cel_parser() {
    let output = Command::new(env!("CARGO"))
        .args(["tree", "--package", "canon-policy", "--prefix", "none", "--format", "{p}"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("`cargo tree` must be runnable in this environment");

    assert!(output.status.success(), "`cargo tree -p canon-policy` failed: {}", String::from_utf8_lossy(&output.stderr));

    let tree = String::from_utf8_lossy(&output.stdout);
    assert!(tree.contains("cel v0.14") || tree.to_lowercase().contains("cel v0."), "expected the upstream `cel` crate to be present:\n{tree}");

    for banned in BANNED {
        assert!(!tree.to_lowercase().contains(&banned.to_lowercase()), "canon-policy's dependency graph must never contain `{banned}` (design D1) — found it in:\n{tree}");
    }
}
