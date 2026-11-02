//! `atip::split_argv` reads what `atip::quote` writes (canon-0-35-crate-shell), seen from the checker: a tool whose next
//! step carries a word with a single quote is not reported for it by `L0.next`.

use serde_json::{json, Value};

use atip_check::checks::rules::next_findings;
use atip_check::claim::Claim;

/// A success envelope of the tool `acme` whose one next step runs `argv`, written the way the contract advises (6.8):
/// the tool name and the words of `argv`, each quoted by `atip::quote`.
fn envelope_with_next(argv: &[&str]) -> Value {
    let command = std::iter::once("acme").chain(argv.iter().copied()).map(atip::quote).collect::<Vec<_>>().join(" ");
    json!({
        "ok": true,
        "data": {},
        "meta": {
            "tool": "acme",
            "next": [{ "command": command, "argv": argv }]
        }
    })
}

#[test]
fn atip_crateshell_06_a_single_quoted_value_in_meta_next_passes_l0_next() {
    for value in ["it's", "'", "''", "a'b'c", "back\\slash", "it's a \"quoted\" $value", "line\nbreak", ""] {
        let argv = ["member", "tag", "--name", value, "--json"];
        let document = envelope_with_next(&argv);
        let problems: Vec<String> = next_findings(&document, Claim::V1_1, true, "acme").into_iter().filter_map(|(_, problem)| problem).collect();
        assert!(problems.is_empty(), "{value:?}: {problems:?}");
    }
}

#[test]
fn atip_crateshell_06_a_command_that_does_not_split_to_its_argv_still_fails() {
    // The check is not weakened: a command that names another word than the argv does is still found.
    let mut document = envelope_with_next(&["member", "tag", "--name", "it's", "--json"]);
    document["meta"]["next"][0]["command"] = json!("acme member tag --name 'it' --json");
    let found: Vec<(&str, Option<String>)> = next_findings(&document, Claim::V1_1, true, "acme");
    let broken: Vec<&str> = found.iter().filter(|(_, problem)| problem.is_some()).map(|(condition, _)| *condition).collect();
    assert_eq!(broken, ["command-matches-argv"], "{found:?}");
}
