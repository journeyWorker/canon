//! The L1 constraint, domain and validator members of the command documents, the numbers and the order of the export
//! window (0.34 `canon-0-34-atip-constraints`, atip/1 spec 10.7.4, 10.7.5, 10.8). Tests are named by scenario id
//! (`atip.constraints.NN`); the sweeps that hold every declaration to the real parser are in `tests/arg_rules.rs`.

mod harness;
mod rule_support;

use std::collections::BTreeSet;

use atip::constraint::{document_problems, Rule};
use atip::doc::{OptionDoc, ValueType};
use harness::{canon_in, fixture, leaf, leaves, options};
use serde_json::{json, Value};
use tempfile::TempDir;

fn names(rule: &Value) -> Vec<&str> {
    rule["names"].as_array().into_iter().flatten().filter_map(Value::as_str).collect()
}

fn rules<'a>(doc: &'a Value, kind: &str) -> Vec<&'a Value> {
    doc["constraints"].as_array().into_iter().flatten().filter(|rule| rule["rule"] == kind).collect()
}

fn has_names(doc: &Value, kind: &str, wanted: &[&str]) -> bool {
    rules(doc, kind).iter().any(|rule| {
        let found = names(rule);
        wanted.iter().all(|name| found.contains(name))
    })
}

struct Run {
    exit: i32,
    body: Value,
}

/// The story repo, built once; every call runs in its own copy.
struct Story {
    repo: TempDir,
    home: TempDir,
}

static STORY: std::sync::LazyLock<Story> = std::sync::LazyLock::new(|| {
    let home = TempDir::new().expect("a home");
    let repo = fixture(home.path());
    Story { repo, home }
});

fn copy_of_story() -> TempDir {
    let copy = TempDir::new().expect("a scratch directory");
    harness::copy_dir(STORY.repo.path(), copy.path());
    copy
}

fn run(args: &[&str]) -> Run {
    let home = &STORY.home;
    let dir = copy_of_story();
    let mut argv: Vec<String> = args.iter().map(ToString::to_string).collect();
    argv.push("--json".to_string());
    let output = canon_in(dir.path(), home.path(), &argv, &[("CANON_ROLE", "implementer")]);
    Run { exit: output.status.code().unwrap_or(-1), body: serde_json::from_slice(&output.stdout).unwrap_or(Value::Null) }
}

fn refused(run: &Run, code: &str, field: &str) {
    assert_eq!(run.exit, 2, "{}", run.body);
    assert_eq!(run.body["ok"], false, "{}", run.body);
    assert_eq!(run.body["error"]["code"], code, "{}", run.body);
    assert_eq!(run.body["error"]["field"], field, "{}", run.body);
}

/// atip.constraints.01: what clap enforces is in `constraints`: the options that conflict with `--check`, `--policy` with
/// `--no-policy`, the exactly-one pair of `review scenario`.
#[test]
fn atip_constraints_01_clap_relations_are_constraints_of_the_document() {
    let init = leaf("init");
    for other in ["no-agents-md", "policy", "no-policy", "providers", "language", "dry-run"] {
        assert!(has_names(init, "atMostOne", &["check", other]), "init: --check and --{other} are one atMostOne: {:?}", init["constraints"]);
    }
    assert!(has_names(init, "atMostOne", &["policy", "no-policy"]), "init: {:?}", init["constraints"]);
    assert!(has_names(leaf("review scenario"), "exactlyOne", &["upstream-ref", "original-spec-ref"]));
    assert!(has_names(leaf("review change"), "exactlyOne", &["upstream-ref", "original-spec-ref"]));
    // The safety pair is a pair of `requires`, one each way.
    let purge = leaf("purge");
    assert!(rules(purge, "requires").iter().any(|rule| rule["if"]["name"] == "commit" && rule["then"] == json!(["plan-digest"])));
    assert!(rules(purge, "requires").iter().any(|rule| rule["if"]["name"] == "plan-digest" && rule["then"] == json!(["commit"])));
    // The same relation is stated once per command (10.8.2), for every command of the spec.
    for doc in leaves() {
        let mut seen = BTreeSet::new();
        for rule in doc["constraints"].as_array().into_iter().flatten() {
            let mut key = rule.clone();
            key.as_object_mut().expect("a rule is an object").remove("code");
            key.as_object_mut().expect("a rule is an object").remove("help");
            for member in ["names", "then"] {
                if let Some(list) = key.get_mut(member).and_then(Value::as_array_mut) {
                    list.sort_by_key(ToString::to_string);
                }
            }
            assert!(seen.insert(key.to_string()), "`{}` states {rule} twice", doc["command"]);
        }
    }
}

/// atip.constraints.02: a declaration that states one relation twice is found by the document check, and `--commit` without
/// `--plan-digest` is `USAGE` naming `--plan-digest`.
#[test]
fn atip_constraints_02_a_relation_stated_twice_is_found_and_a_broken_pair_names_its_argument() {
    refused(&run(&["purge", "--kind", "session", "--before", "2020-01-01", "--commit"]), "USAGE", "--plan-digest");
    refused(&run(&["purge", "--kind", "session", "--before", "2020-01-01", "--plan-digest", "abc"]), "USAGE", "--commit");
    let flag = |name: &str| OptionDoc {
        name: name.to_string(),
        short: None,
        value_name: None,
        value_type: ValueType::Switch,
        possible_values: None,
        required: false,
        repeatable: false,
        default: json!(false),
        default_note: None,
        env: None,
        flag_only: false,
        domain: None,
        help: "h".to_string(),
    };
    let options = [flag("a"), flag("b")];
    let twice = [Rule::at_most_one(&["a", "b"]).doc(), Rule::at_most_one(&["b", "a"]).doc()];
    let problems = document_problems(&options, &[], &twice, &[], &["USAGE".to_string()], &[]);
    assert!(problems.iter().any(|problem| problem.contains("stated twice")), "{problems:?}");
}

/// atip.constraints.03: the relations the handlers enforce are in the documents with their codes.
#[test]
fn atip_constraints_03_handler_relations_are_constraints_with_their_codes() {
    let retrieve = leaf("retrieve");
    assert!(has_names(retrieve, "exactlyOne", &["regime", "domain"]), "{:?}", retrieve["constraints"]);
    assert!(rules(retrieve, "requires").iter().any(|rule| rule["if"]["name"] == "subject" && rule["then"] == json!(["domain"])), "{:?}", retrieve["constraints"]);
    let close = leaf("finding close");
    let reason = rules(close, "requires").into_iter().find(|rule| rule["then"] == json!(["reason"])).unwrap_or_else(|| panic!("no rule demands --reason: {:?}", close["constraints"]));
    assert_eq!(reason["if"]["name"], "disposition");
    assert_eq!(reason["if"]["in"], json!(["rejected", "deferred"]));
    assert_eq!(reason["code"], "FINDING_REASON_REQUIRED");
    let forbids = rules(close, "forbids").into_iter().find(|rule| rule["then"] == json!(["resolution-rev"])).unwrap_or_else(|| panic!("no rule forbids --resolution-rev: {:?}", close["constraints"]));
    assert_eq!(forbids["code"], "FINDING_RESOLUTION_REV_UNEXPECTED");
    let add = leaf("finding add");
    assert!(has_names(add, "atMostOne", &["introduced-rev", "introduced-unknown"]), "{:?}", add["constraints"]);
    let validator = add["validators"].as_array().into_iter().flatten().find(|validator| validator["code"] == "FINDING_INTRODUCED_REQUIRED").unwrap_or_else(|| panic!("no validator for the required origin: {:?}", add["validators"]));
    assert!(validator["arguments"].as_array().is_some_and(|arguments| arguments.contains(&json!("severity"))));
    let evidence = leaf("evidence add");
    assert!(has_names(evidence, "atLeastOne", &["task", "scenario", "all-scenarios"]), "{:?}", evidence["constraints"]);
    // Failure paths, through the binary: a missing reason and a missing origin are refused with their codes.
    refused(&run(&["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "rejected", "--actor", "unit-1"]), "FINDING_REASON_REQUIRED", "--reason");
    refused(&run(&["retrieve", "--for-role", "reviewer"]), "USAGE", "--regime");
}

/// atip.constraints.06: every domain is in the contract's profile, every pattern is a restricted I-Regexp that the Rust
/// `regex` crate compiles, every default is inside its domain, and a domain that breaks one of these is found.
#[test]
fn atip_constraints_06_every_domain_is_in_the_profile_and_holds_the_defaults() {
    let mut problems = Vec::new();
    let mut count = 0;
    for doc in leaves() {
        for argument in options(doc).iter().chain(doc["operands"].as_array().into_iter().flatten()) {
            let Some(domain) = argument.get("domain") else { continue };
            count += 1;
            let value_type = match argument["valueType"].as_str().unwrap_or_default() {
                "integer" => ValueType::Integer,
                "count" => ValueType::Count,
                "number" => ValueType::Number,
                "path" => ValueType::Path,
                "string" => ValueType::String,
                other => panic!("`{}` --{}: a {other} has a domain", doc["command"], argument["name"]),
            };
            for problem in atip::domain::profile_problems(value_type, domain) {
                problems.push(format!("`{}` {}: {problem}", doc["command"], argument["name"]));
            }
            let defaults: Vec<&Value> = match &argument["default"] {
                Value::Null => Vec::new(),
                Value::Array(items) => items.iter().collect(),
                one => vec![one],
            };
            for value in defaults {
                if !atip::domain::admits(domain, value).unwrap_or(false) {
                    problems.push(format!("`{}` {}: the default {value} is outside {domain}", doc["command"], argument["name"]));
                }
            }
        }
    }
    assert!(count > 40, "the documents declare {count} domains");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // Failure paths: a default outside the domain, an unrestricted pattern and a keyword outside the profile are found.
    let option = |domain: Value, default: Value, value_type: ValueType| OptionDoc {
        name: "n".to_string(),
        short: None,
        value_name: Some("N".to_string()),
        value_type,
        possible_values: None,
        required: false,
        repeatable: false,
        default,
        default_note: None,
        env: None,
        flag_only: false,
        domain: Some(domain),
        help: "h".to_string(),
    };
    let found = |options: &[OptionDoc]| document_problems(options, &[], &[], &[], &["USAGE".to_string()], &[]);
    assert!(found(&[option(json!({ "minimum": 1 }), json!(0), ValueType::Integer)]).iter().any(|problem| problem.contains("default")));
    assert!(found(&[option(json!({ "pattern": "^a|b$" }), Value::Null, ValueType::String)]).iter().any(|problem| problem.contains("| outside parentheses")));
    assert!(found(&[option(json!({ "pattern": "^\\d+$" }), Value::Null, ValueType::String)]).iter().any(|problem| problem.contains("not I-Regexp")));
    assert!(found(&[option(json!({ "minLength": 1 }), Value::Null, ValueType::Integer)]).iter().any(|problem| problem.contains("outside the domain profile")));
    assert!(found(&[option(json!({ "minimum": 9_007_199_254_740_992_i64 }), Value::Null, ValueType::Integer)]).iter().any(|problem| problem.contains("safe integer")));
    assert!(found(&[option(json!({ "minimum": 1 }), json!(1), ValueType::Integer)]).is_empty());
}

/// atip.constraints.06: a pattern is exactly what the Rust parser of the id kind accepts (a necessary condition for every
/// kind, equality for the kebab and the ULID kinds): random strings over the grammar's own alphabet agree.
#[test]
fn atip_constraints_06_the_id_patterns_agree_with_the_rust_parsers() {
    use canon_cli::arg_rules;
    use canon_model::ids::{ChangeId, ProjectId, ReviewId, RoleId, RunId, ScenarioId, SessionId, SubjectId, TaskId};
    type Parse = fn(&str) -> bool;
    let kinds: Vec<(&str, &str, &str, Parse)> = vec![
        ("CHANGE_ID", arg_rules::KEBAB, "abz019-- #.", |s| ChangeId::parse(s).is_ok()),
        ("SUBJECT_ID", arg_rules::KEBAB, "abz019-- #.", |s| SubjectId::parse(s).is_ok()),
        ("ROLE", arg_rules::KEBAB, "abz019-- #.", |s| RoleId::parse(s).is_ok()),
        ("PROJECT_ID", arg_rules::PROJECT_ID, "abz019-- #.", |s| ProjectId::parse(s).is_ok()),
        ("SCENARIO_ID", arg_rules::SCENARIO_ID, "abz019-..#_", |s| ScenarioId::parse(s).is_ok()),
        ("TASK_ID", arg_rules::TASK_ID, "abz019-#..", |s| TaskId::parse(s).is_ok()),
        ("RUN_ID", arg_rules::ULID, "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefilou", |s| RunId::parse(s).is_ok()),
        ("REVIEW_ID", arg_rules::ULID, "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefilou", |s| ReviewId::parse(s).is_ok()),
        ("SESSION_ID", arg_rules::SESSION_ID, "ab1 -\t\n\u{a0}\u{2003}\u{7}\u{200b}", |s| SessionId::parse(s).is_ok()),
    ];
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = move |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        usize::try_from(state % bound as u64).unwrap_or(0)
    };
    let mut problems = Vec::new();
    for (name, pattern, alphabet, accepts) in kinds {
        assert_eq!(atip::domain::pattern_problem(pattern), None, "{name}");
        let alphabet: Vec<char> = alphabet.chars().collect();
        let ulid = matches!(name, "RUN_ID" | "REVIEW_ID");
        let compiled = atip::domain::compile(&json!({ "pattern": pattern })).expect("the pattern compiles");
        let mut checked = 0;
        for round in 0..20_000 {
            let length = if ulid { 24 + next(5) } else { next(9) };
            let candidate: String = (0..length).map(|_| alphabet[next(alphabet.len())]).collect();
            // Seeded successes: a ULID candidate of 26 characters is common enough only when built from valid ones.
            let candidate = if ulid && round % 2 == 0 { candidate.chars().chain("0123456789".chars().cycle()).take(26).collect() } else { candidate };
            let admitted = compiled.admits(&json!(candidate));
            let parsed = accepts(&candidate);
            checked += 1;
            if name == "SESSION_ID" {
                // The session pattern is a necessary condition only when the parser accepts; equality is not claimed for exotic scalars.
                if parsed && !admitted {
                    problems.push(format!("{name}: the parser accepts {candidate:?} and the pattern refuses it"));
                }
            } else if parsed != admitted {
                problems.push(format!("{name}: the parser says {parsed} and the pattern {admitted} for {candidate:?}"));
            }
        }
        assert!(checked > 0);
    }
    assert!(problems.is_empty(), "{} problems, the first ten:\n{}", problems.len(), problems.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
}

/// atip.constraints.06: the time pattern admits everything the parser accepts (it states the shape; the calendar is a validator's).
#[test]
fn atip_constraints_06_the_time_pattern_admits_everything_the_parser_accepts() {
    let pattern = canon_cli::arg_rules::TIMESTAMP;
    assert_eq!(atip::domain::pattern_problem(pattern), None);
    let alphabet: Vec<char> = "0123456789-:TtZz+ .\u{2212}\u{a0}\n".chars().collect();
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        usize::try_from(state % bound as u64).unwrap_or(0)
    };
    let compiled = atip::domain::compile(&json!({ "pattern": pattern })).expect("the pattern compiles");
    let mut problems = Vec::new();
    let mut seeds: Vec<String> = ["2030-01-01", "2030-1-1", " 2030-01-01", "2030-01-01T10:00:00Z", "2030-01-01t10:00:00z", "2030-01-01 10:00:00+02:00", "2030-01-01T10:00:00.123456789\u{2212}02:00", "0001-01-01T00:00:00+00:00", "2030-01-01T10:00:60Z", "+2030-01-01", "-0001-01-01", "99999-12-31", "2030-01-01T24:00:00Z"].iter().map(ToString::to_string).collect();
    for _ in 0..60_000 {
        let length = 6 + next(26);
        seeds.push((0..length).map(|_| alphabet[next(alphabet.len())]).collect());
    }
    for _ in 0..60_000 {
        // Near-misses of a valid time: one character changed, dropped or inserted.
        let base: Vec<char> = "2030-01-01T10:00:00.5+02:00".chars().collect();
        let mut mutated = base.clone();
        match next(3) {
            0 => mutated[next(base.len())] = alphabet[next(alphabet.len())],
            1 => {
                mutated.remove(next(base.len()));
            }
            _ => mutated.insert(next(base.len()), alphabet[next(alphabet.len())]),
        }
        seeds.push(mutated.into_iter().collect());
    }
    let mut accepted = 0;
    for candidate in seeds {
        if canon_cli::timestamp::parse(&candidate).is_ok() {
            accepted += 1;
            if !compiled.admits(&json!(candidate)) {
                problems.push(format!("the parser accepts {candidate:?} and the pattern refuses it"));
            }
        }
    }
    assert!(accepted > 1000, "the corpus reaches the parser's accepted set ({accepted})");
    assert!(problems.is_empty(), "{} problems, the first ten:\n{}", problems.len(), problems.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
}

/// atip.constraints.07: a date that is not on the calendar is refused by every option that reads a time, with `USAGE` and the option
/// as `field`; the validator of each command that has one names its options and refuses a call that is otherwise complete.
#[test]
fn atip_constraints_07_every_time_option_declares_the_calendar_validator_and_refuses_an_impossible_date() {
    let mut options = 0;
    for doc in leaves() {
        let time: Vec<&str> = harness::options(doc).iter().filter(|option| option["valueName"] == "TIMESTAMP").filter_map(|option| option["name"].as_str()).collect();
        if time.is_empty() {
            continue;
        }
        options += time.len();
        let validator = doc["validators"].as_array().into_iter().flatten().find(|validator| time.iter().all(|name| validator["arguments"].as_array().is_some_and(|arguments| arguments.contains(&json!(name)))));
        let validator = validator.unwrap_or_else(|| panic!("`{}` reads {time:?} and declares no calendar validator: {:?}", doc["command"], doc["validators"]));
        assert_eq!(validator["code"], "USAGE", "`{}`", doc["command"]);
        assert!(validator["description"].as_str().is_some_and(|text| text.contains("calendar")), "`{}`: {}", doc["command"], validator["description"]);
        for name in &time {
            let call = validator["rejects"].as_array().into_iter().flatten().filter_map(Value::as_str).find(|call| call.contains(&format!("--{name}")) && call.contains("2026-02-30"));
            assert!(call.is_some(), "`{}`: no rejected call gives --{name} 2026-02-30: {:?}", doc["command"], validator["rejects"]);
        }
    }
    assert_eq!(options, 12, "the contract names 12 time options");
    // The pattern states the shape only: the impossible date matches it, and is the validator's.
    let domain = json!({ "pattern": canon_cli::arg_rules::TIMESTAMP });
    assert_eq!(atip::domain::admits(&domain, &json!("2026-02-30")), Ok(true));
    refused(&run(&["query", "--kind", "change", "--since", "2026-02-30"]), "USAGE", "--since");
}

/// atip.constraints.08: `export --after` later than `--before` is refused with `USAGE` naming `--after` and writes nothing;
/// an equal pair, an earlier `--after` and an empty window stay as they were.
#[test]
fn atip_constraints_08_an_export_window_that_ends_before_it_starts_is_refused() {
    let doc = leaf("export");
    let validator = doc["validators"].as_array().into_iter().flatten().find(|validator| validator["rejects"].as_array().is_some_and(|calls| calls.contains(&json!("canon export --kind change --before 2026-01-02 --after 2026-02-01 --json")))).unwrap_or_else(|| panic!("no window validator: {:?}", doc["validators"]));
    assert_eq!(validator["code"], "USAGE");
    assert!(validator["rejects"].as_array().is_some_and(|calls| calls.contains(&json!("canon export --kind change --before 2026-01-02 --after 2026-02-01 --json"))), "{validator}");
    let late = run(&["export", "--kind", "change", "--before", "2026-01-02", "--after", "2026-02-01"]);
    refused(&late, "USAGE", "--after");
    // Before any read or write: with `--dry-run` and with `--output` too.
    refused(&run(&["export", "--kind", "change", "--before", "2026-01-02", "--after", "2026-02-01", "--dry-run"]), "USAGE", "--after");
    let home = &STORY.home;
    let dir = copy_of_story();
    let out = dir.path().join("out.ndjson");
    let output = canon_in(dir.path(), home.path(), &["export".into(), "--kind".into(), "change".into(), "--before".into(), "2026-01-02".into(), "--after".into(), "2026-02-01".into(), "--output".into(), out.to_string_lossy().into_owned(), "--json".into()], &[("CANON_ROLE", "implementer")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!out.exists(), "a refused export wrote its output");
    // The same instant on both sides, and an earlier --after, are still an (empty or not) window that succeeds.
    for (after, before) in [("2026-02-01", "2026-02-01"), ("2026-01-01", "2026-02-01")] {
        let ok = run(&["export", "--kind", "change", "--before", before, "--after", after]);
        assert_eq!(ok.exit, 0, "--after {after} --before {before}: {}", ok.body);
    }
    // An RFC 3339 time with an offset is compared as an instant: 10:00 at +02:00 is 08:00Z, before 09:00Z.
    refused(&run(&["export", "--kind", "change", "--after", "2026-02-01T10:00:00+02:00", "--before", "2026-02-01T07:00:00Z"]), "USAGE", "--after");
    assert_eq!(run(&["export", "--kind", "change", "--after", "2026-02-01T10:00:00+02:00", "--before", "2026-02-01T09:00:00Z"]).exit, 0);
}

/// atip.constraints.10: the edge of the safe range is accepted by every 64-bit integer option, and no declared domain bound is above it.
#[test]
fn atip_constraints_10_the_edge_of_the_safe_range_is_accepted_and_no_domain_goes_beyond_it() {
    for (base, option) in INTEGER_OPTIONS {
        let mut edge = base.to_vec();
        let joined = format!("{option}=9007199254740991");
        edge.push(&joined);
        let accepted = run(&edge);
        assert_ne!(accepted.body["error"]["code"], "USAGE", "`{}` {option} 9007199254740991 is the edge and is accepted: {}", base.join(" "), accepted.body);
    }
    for doc in leaves() {
        for option in options_of(doc) {
            if option["valueType"] != "integer" {
                continue;
            }
            if let Some(maximum) = option["domain"]["maximum"].as_i64() {
                assert!(maximum <= 9_007_199_254_740_991, "`{}` --{}: {maximum}", doc["command"], option["name"]);
            }
        }
    }
}

const INTEGER_OPTIONS: [(&[&str], &str); 5] = [
    (&["artifacts", "store", "--dry-run"], "--max-artifact-mib"),
    (&["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "x"], "--max-artifact-mib"),
    (&["query", "--kind", "change"], "--limit"),
    (&["retrieve", "--for-role", "reviewer", "--domain", "dev"], "--limit"),
    (&["ingest", "sessions", "--dry-run"], "--interval-secs"),
];

/// atip.constraints.11: an integer option outside ±(2^53-1) is refused, never rounded.
#[test]
fn atip_constraints_11_a_sixty_four_bit_integer_outside_the_safe_range_is_refused_and_never_rounded() {
    for (base, option) in INTEGER_OPTIONS {
        for value in ["9007199254740992", "9007199254740993", "18446744073709551616", "-9007199254740992", "-1"] {
            let mut args = base.to_vec();
            let joined = format!("{option}={value}");
            args.push(&joined);
            let result = run(&args);
            assert_eq!(result.exit, 2, "`{}` {option} {value}: {}", base.join(" "), result.body);
            assert_eq!(result.body["error"]["code"], "USAGE", "`{}` {option} {value}: {}", base.join(" "), result.body);
            assert_eq!(result.body["error"]["field"], option, "`{}` {option} {value}: {}", base.join(" "), result.body);
        }
    }
    // A negative value written after its option as a separate word is not read as the option's value: the call is refused too.
    for (base, option) in INTEGER_OPTIONS {
        let mut args = base.to_vec();
        args.push(option);
        args.push("-9007199254740992");
        let result = run(&args);
        assert_eq!(result.exit, 2, "`{}` {option} -9007199254740992: {}", base.join(" "), result.body);
        assert_eq!(result.body["error"]["code"], "USAGE", "{}", result.body);
    }
    // Failure path: a domain bound above the safe range is found by the profile check.
    assert!(!atip::domain::profile_problems(ValueType::Integer, &json!({ "maximum": 9_007_199_254_740_992_i64 })).is_empty());
}

fn options_of(doc: &Value) -> &Vec<Value> {
    options(doc)
}

/// atip.constraints.12: the guides and the checked-in spec say what the documents say: relations are read from the
/// `constraints`, and a generated page lists them.
#[test]
fn atip_constraints_12_the_guides_and_the_spec_read_the_constraints_of_the_documents() {
    let retrieve = leaf("retrieve");
    let relations = atip::project::Relations::from_json(&retrieve["constraints"]);
    assert!(relations.required_one_of.iter().any(|set| set.contains(&"regime".to_string()) && set.contains(&"domain".to_string())));
    assert_eq!(relations.requires["subject"], ["domain"]);
    let init = atip::project::Relations::from_json(&leaf("init")["constraints"]);
    assert!(init.conflicts_with["check"].contains(&"policy".to_string()));
    assert!(init.conflicts_with["policy"].contains(&"no-policy".to_string()));
    // Every document member is one the contract names; nothing else of a rule is written.
    for doc in leaves() {
        for rule in doc["constraints"].as_array().into_iter().flatten() {
            for key in rule.as_object().expect("a rule is an object").keys() {
                assert!(["rule", "names", "if", "then", "code", "help"].contains(&key.as_str()) || key.starts_with("x-"), "`{}`: rule member {key}", doc["command"]);
            }
        }
        for validator in doc["validators"].as_array().into_iter().flatten() {
            for key in validator.as_object().expect("a validator is an object").keys() {
                assert!(["arguments", "code", "description", "rejects"].contains(&key.as_str()) || key.starts_with("x-"), "`{}`: validator member {key}", doc["command"]);
            }
        }
    }
}

// ------------------------------------------------------------------------------------------ the follow-up round
// (the ATIP author's answers: a relay is a relation, the environment is a stateless input, one code with reasons, the shape of `--input`)

fn words(line: &str) -> Vec<String> {
    let mut all = atip::split_argv(line);
    assert_eq!(all.remove(0), "canon");
    all
}

fn validator_of<'a>(doc: &'a Value, arguments: &[&str]) -> Option<&'a Value> {
    doc["validators"].as_array().into_iter().flatten().find(|validator| validator["arguments"] == json!(arguments))
}

/// atip.constraints.14: `--approver` and `--note` are no domain (the code depends on `--relayed-via`): a validator per code, and the
/// environment's role and actor are validators over `--relayed-via` with `env` declared on `--as` and `--actor`.
#[test]
fn atip_constraints_14_a_relayed_approval_is_a_relation_with_its_validators_and_the_environment_is_declared() {
    for command in ["brief approve", "policy approve", "divergence accept", "migrate"] {
        let doc = leaf(command);
        let role = validator_of(doc, &["relayed-via", "as"]).unwrap_or_else(|| panic!("`{command}`: no role validator: {:?}", doc["validators"]));
        assert_eq!(role["code"], "USAGE");
        let answer = validator_of(doc, &["relayed-via", "approver", "note"]).unwrap_or_else(|| panic!("`{command}`: no answer validator"));
        assert_eq!(answer["code"], "USAGE");
        assert_eq!(harness::option(doc, "as").expect("--as")["env"], "CANON_ROLE", "`{command}`");
    }
    for command in ["brief approve", "policy approve", "migrate"] {
        let doc = leaf(command);
        assert!(validator_of(doc, &["relayed-via", "actor"]).is_some_and(|validator| validator["code"] == "USAGE"), "`{command}`");
        assert_eq!(harness::option(doc, "actor").expect("--actor")["env"], "CANON_ACTOR", "`{command}`");
        // The environment is not "given" for a constraint: `--actor` still requires `--relayed-via` as a rule.
        assert!(rules(doc, "requires").iter().any(|rule| rule["if"]["name"] == "actor" && rule["then"] == json!(["relayed-via"])), "`{command}`: {:?}", doc["constraints"]);
    }
    // The human's own answer has a code of its own, and no domain claims one code for both paths.
    let brief = leaf("brief approve");
    assert!(validator_of(brief, &["approver", "note"]).is_some_and(|validator| validator["code"] == "APPROVAL_INCOMPLETE"));
    assert!(validator_of(leaf("policy approve"), &["approver"]).is_some_and(|validator| validator["code"] == "POLICY_APPROVAL_REFUSED"));
    for (command, argument) in [("brief approve", "approver"), ("brief approve", "note"), ("policy approve", "approver")] {
        assert!(harness::option(leaf(command), argument).expect("the option").get("domain").is_none(), "`{command}` --{argument} has a domain");
    }
}

/// atip.constraints.15: each of them refuses in an empty directory with no repository, with its code and the argument at fault;
/// the environment fills the actor and the role; the human's blank answer is `APPROVAL_INCOMPLETE`.
#[test]
fn atip_constraints_15_a_relayed_approval_without_role_actor_or_answer_is_refused_before_any_state_is_read() {
    let digest = "--digest add-login=sha256:0123456789abcdef";
    let check = |line: &str, envs: &[(&str, &str)], code: &str, field: &str| {
        let outcome = rule_support::run_empty_with(&words(line), envs);
        assert!(!outcome.ok && outcome.exit == 2, "{line}: {outcome:?}");
        assert_eq!(outcome.code.as_deref(), Some(code), "{line}: {outcome:?}");
        assert_eq!(outcome.field.as_deref(), Some(field), "{line}: {outcome:?}");
        assert!(outcome.changed.is_empty(), "{line}: wrote {:?}", outcome.changed);
    };
    for command in ["canon brief approve add-login", "canon policy approve"] {
        let extra = if command.contains("brief") { digest } else { "" };
        let base = format!("{command} --approver alice --note ok --relayed-via ask {extra}");
        check(&format!("{base} --actor impl"), &[], "USAGE", "--relayed-via");
        check(&base, &[("CANON_ROLE", "implementer")], "USAGE", "--actor");
        check(&format!("{base} --as implementer --actor impl --approver \" \""), &[], "USAGE", "--approver");
        check(&format!("{base} --as implementer --actor impl --note \" \""), &[], "USAGE", "--note");
        // The environment is a stateless input: CANON_ACTOR and CANON_ROLE fill them, and the call goes on to the repository (`CONFIG_MISSING`).
        let outcome = rule_support::run_empty_with(&words(&base), &[("CANON_ROLE", "implementer"), ("CANON_ACTOR", "impl")]);
        assert_eq!(outcome.code.as_deref(), Some("CONFIG_MISSING"), "{command}: {outcome:?}");
    }
    check("canon brief approve add-login --approver \" \" --note ok", &[], "APPROVAL_INCOMPLETE", "--approver");
    check("canon brief approve add-login --approver alice --note \" \"", &[], "APPROVAL_INCOMPLETE", "--note");
    check("canon policy approve --approver \" \"", &[], "POLICY_APPROVAL_REFUSED", "--approver");
    check("canon migrate --commit --plan-digest abc --relayed-via ask --actor impl --approver alice --note ok", &[], "USAGE", "--relayed-via");
    check("canon divergence accept auth.login.01 --actor alice --reason why --tracking t --relayed-via ask --approver alice --note ok", &[], "USAGE", "--relayed-via");
}

/// atip.constraints.16: the shape of `--input` is a validator: a batch on a command that takes one object, and `--input -` beside another
/// stdin reader. Each is refused in an empty directory.
#[test]
fn atip_constraints_16_the_shape_of_input_is_a_validator_and_is_refused_before_any_state_is_read() {
    for (command, line) in [
        ("artifacts move", "canon artifacts move --tier cold --input '[{},{}]'"),
        ("dashboard", "canon dashboard --input '[{},{}]'"),
        ("ledger compact", "canon ledger compact --input '[{},{}]'"),
        ("migrate", "canon migrate --input '[{},{}]'"),
        ("purge", "canon purge --kind session --before 2020-01-01T00:00:00Z --input '[{},{}]'"),
        ("tier age", "canon tier age --input '[{},{}]'"),
    ] {
        let validator = validator_of(leaf(command), &["input"]).unwrap_or_else(|| panic!("`{command}`: no `input` validator"));
        assert_eq!(validator["code"], "INPUT_INVALID");
        assert!(validator["rejects"].as_array().is_some_and(|calls| calls.contains(&json!(line))), "{validator}");
        let outcome = rule_support::run_empty(&words(line), None);
        assert_eq!((outcome.exit, outcome.code.as_deref(), outcome.reason.as_deref(), outcome.field.as_deref()), (2, Some("INPUT_INVALID"), Some("BATCH_UNSUPPORTED"), Some("--input")), "{command}: {outcome:?}");
        assert!(outcome.changed.is_empty());
        // One object is the way in: the same call with one object is not refused for its shape.
        let one = line.replace("'[{},{}]'", "'{}'");
        assert_ne!(rule_support::run_empty(&words(&one), None).reason.as_deref(), Some("BATCH_UNSUPPORTED"), "{one}");
    }
    for (command, line) in [
        ("evidence add", "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --report junit:- --input -"),
        ("evidence add", "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer --artifact - --input -"),
    ] {
        assert!(validator_of(leaf(command), &["input", "report", "artifact"]).is_some_and(|validator| validator["rejects"].as_array().is_some_and(|calls| calls.contains(&json!(line)))));
        let outcome = rule_support::run_empty(&words(line), None);
        assert_eq!((outcome.exit, outcome.code.as_deref(), outcome.field.as_deref()), (2, Some("USAGE"), Some("--input")), "{outcome:?}");
        assert!(outcome.message.is_some_and(|message| message.contains("both read stdin")));
    }
}

/// atip.constraints.17: `EVIDENCE_APPROVAL_INVALID` is one code with two reasons, both exit 2: `MALFORMED` from the arguments alone (before
/// any state is read), `SIGNATURE_MISMATCH` from what reading the signature decides.
#[test]
fn atip_constraints_17_one_approval_code_has_a_reason_for_the_arguments_and_one_for_the_signature() {
    let registry = run(&["schema", "--error", "EVIDENCE_APPROVAL_INVALID"]);
    assert_eq!(registry.exit, 0, "{}", registry.body);
    let reasons: Vec<&str> = registry.body["data"]["error"]["reasons"].as_array().into_iter().flatten().filter_map(|row| row["reason"].as_str()).collect();
    assert_eq!(reasons, ["MALFORMED", "SIGNATURE_MISMATCH"], "{}", registry.body);
    assert_eq!(registry.body["data"]["error"]["exit"], 2);
    // The arguments alone, in an empty directory: each member of the four missing, and a blank approver.
    let add = "canon evidence add --scenario auth.login.01 --class test-run --ref \"npm test\" --role implementer";
    for (extra, field) in [
        ("--approver alice", "--approval-role"),
        ("--approver alice --approval-role human", "--signature-file"),
        ("--approver alice --approval-role human --signature-file nope.sig", "--approval-at"),
        ("--approval-role human --signature-file nope.sig --approval-at 2030-01-01T00:00:00Z", "--approver"),
        ("--approver \" \" --approval-role human --signature-file nope.sig --approval-at 2030-01-01T00:00:00Z", "--approver"),
    ] {
        let outcome = rule_support::run_empty(&words(&format!("{add} {extra}")), None);
        assert_eq!((outcome.exit, outcome.code.as_deref(), outcome.reason.as_deref(), outcome.field.as_deref()), (2, Some("EVIDENCE_APPROVAL_INVALID"), Some("MALFORMED"), Some(field)), "{extra}: {outcome:?}");
        assert!(outcome.changed.is_empty());
    }
    // A well-formed set whose signature file is not there: the precondition, in the repository.
    let signed = run(&["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "npm test", "--approver", "alice", "--approval-role", "human", "--signature-file", "nope.sig", "--approval-at", "2030-01-01T00:00:00Z"]);
    refused(&signed, "EVIDENCE_APPROVAL_INVALID", "--signature-file");
    assert_eq!(signed.body["error"]["reason"], "SIGNATURE_MISMATCH", "{}", signed.body);
}
