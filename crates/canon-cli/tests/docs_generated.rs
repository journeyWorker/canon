//! T14: the derived documentation blocks (role command lists, the website's
//! CLI tables, the README's removed-spellings table) equal what the spec
//! renders. `CANON_BLESS_DOCS=1 cargo test -p canon-cli --test docs_generated`
//! rewrites them; see `tests/doc_gen/mod.rs` for the marker convention.

mod doc_gen;

use std::fs;

#[test]
fn every_generated_block_is_what_the_spec_renders() {
    let spec = canon_cli::spec::generate();
    let bless = std::env::var_os("CANON_BLESS_DOCS").is_some();
    let mut stale = Vec::new();
    for path in doc_gen::doc_files() {
        let on_disk = fs::read_to_string(&path).expect("read doc");
        let generated = doc_gen::regenerate(&spec, &path).unwrap_or_else(|e| panic!("{e}"));
        if generated != on_disk {
            if bless {
                fs::write(&path, generated).expect("write doc");
            } else {
                stale.push(doc_gen::rel_of(&path));
            }
        }
    }
    assert!(stale.is_empty(), "generated blocks are out of date in {stale:?}; run CANON_BLESS_DOCS=1 cargo test -p canon-cli --test docs_generated and commit");
}

#[test]
fn the_files_that_need_blocks_have_them() {
    let spec = canon_cli::spec::generate();
    let files: Vec<String> = doc_gen::doc_files().iter().map(|p| doc_gen::rel_of(p)).collect();
    for role in ["planner", "implementer", "reviewer", "orchestrator"] {
        let rel = format!("canon/skills/reference/role-{role}.md");
        assert!(files.contains(&rel), "{rel} is missing");
    }
    for rel in ["packages/website/src/content/docs/cli.mdx", "packages/website/src/content/docs/ko/cli.mdx"] {
        assert!(files.iter().any(|f| f == rel), "{rel} is missing");
        let groups = doc_gen::groups(&spec);
        assert!(groups.len() > 20, "{groups:?}");
    }
}
