//! T8 (0.28 D2): every JSON `conformance/cases/*/expected/N.out` is a correct
//! report. The corpus freezes what consumers see; this holds each frozen JSON
//! document to the contract: the envelope against the vendored atip/1 schema
//! (`atip::envelope_problems`), then `data` against
//! a report its leaf declares. An `.out` that is JSON but no envelope must be the
//! bytes of an `artifact` leaf (`export`, `completion`).

mod harness;

use std::path::Path;

use harness::*;
use serde_json::Value;

/// The leaf a step's `args` run: the longest leaf path made of the leading words.
fn leaf_of(args: &[String]) -> Option<&'static Value> {
    let words: Vec<&str> = args.iter().take_while(|a| !a.starts_with('-')).map(String::as_str).collect();
    (1..=words.len()).rev().find_map(|n| leaves().into_iter().find(|l| path_of(l) == words[..n].join(" ")))
}

#[test]
fn every_json_expectation_of_the_corpus_validates_against_its_leafs_schema() {
    let cases = repo_root().join("conformance/cases");
    let mut dirs: Vec<_> = std::fs::read_dir(&cases).expect("conformance/cases").map(|e| e.unwrap().path()).filter(|p| p.join("case.yaml").is_file()).collect();
    dirs.sort();
    assert!(dirs.len() > 50, "the corpus was found ({} cases)", dirs.len());
    let (mut envelopes, mut artifacts, mut problems) = (0, 0, Vec::new());
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let case: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(dir.join("case.yaml")).unwrap()).unwrap();
        for (i, step) in case["steps"].as_sequence().expect("steps").iter().enumerate() {
            let args: Vec<String> = step["args"].as_sequence().expect("args").iter().map(|a| a.as_str().unwrap_or_default().to_string()).collect();
            let out = dir.join("expected").join(format!("{}.out", i + 1));
            let Ok(text) = std::fs::read_to_string(&out) else { continue };
            let Ok(document) = serde_json::from_str::<Value>(&text) else { continue };
            if document.get("ok").is_some() && document.get("meta").is_some() {
                envelopes += 1;
                let problems_here = validate_document(&document);
                problems.extend(problems_here.into_iter().map(|p| format!("{name} step {} (canon {}): {p}", i + 1, args.join(" "))));
            } else {
                artifacts += 1;
                let artifact = leaf_of(&args).is_some_and(|l| l["outputs"].as_array().unwrap().iter().any(|o| o["kind"] == "artifact"));
                if !artifact {
                    problems.push(format!("{name} step {}: JSON that is no envelope, from `canon {}`, which declares no artifact output ({})", i + 1, args.join(" "), Path::new(&out).display()));
                }
            }
        }
    }
    assert!(envelopes > 5, "the corpus holds JSON envelopes ({envelopes})");
    eprintln!("validated {envelopes} envelopes and {artifacts} artifacts");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
