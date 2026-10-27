//! 0.30 `record-links`: the Gherkin parser reads every scenario of canon's own
//! corpora, and every scenario the line scan finds has a step index entry.
//! The scenario digest is not computed from this parser, so nothing here
//! compares digests.

use std::path::{Path, PathBuf};

use canon_fmt::gherkin::scan;
use canon_fmt::steps::scenario_steps;
use canon_fmt::util::feature_files;

fn corpus_roots() -> Vec<PathBuf> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    [".canon/specs", "examples/platformer/specs"].iter().map(|root| repo.join(root)).collect()
}

#[test]
fn every_scenario_of_the_corpora_has_steps_in_the_step_index() {
    let mut files = 0;
    let mut scenarios = 0;
    for root in corpus_roots() {
        for path in feature_files(&root) {
            let text = std::fs::read_to_string(&path).unwrap();
            let steps = scenario_steps(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            for scenario in scan(&text).scenarios {
                let indexed = steps.get(&scenario.scenario_id).unwrap_or_else(|| panic!("{}: no steps entry for `{}`", path.display(), scenario.scenario_id));
                assert!(indexed.iter().any(|step| step.inherited.is_none()), "{}: `{}` has no step of its own", path.display(), scenario.scenario_id);
                scenarios += 1;
            }
            files += 1;
        }
    }
    assert!(files > 30 && scenarios > 500, "the corpora were not found: {files} files, {scenarios} scenarios");
}
