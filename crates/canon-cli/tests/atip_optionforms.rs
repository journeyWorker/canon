//! `canon-0-37-option-forms` (scenarios `atip.optionforms.01-16`): the time options take RFC 3339 and nothing else, and `--providers` takes the exact names, against the
//! built binary and the documents it declares.
//!
//! `--approval-at` takes a date-time (`YYYY-MM-DDTHH:MM:SS`, a fraction, `Z` or `+HH:MM`); `--expiry`, `--as-of`, `--before`, `--after` and `--since` take a full-date or the
//! same date-time; `T` and `Z` are read in either case. `--providers` takes `none`, or `claude`, `codex`, `omp`, `pi` in lowercase joined by single commas, in `init` and in
//! the three `skills` commands. `atip/option_forms_0_36.json` holds what canon 0.36.0 accepted, per option and class of value, and the CHANGELOG of the build holds a row
//! for each value of it that is refused now.
//!
//! The checks that seeded faults must trip are small pure functions in this file (`forms_problems`, `changelog_problems`): each real input passes them, each seeded fault is
//! found, and no sweep is vacuous. Tests are named by scenario id (`atip_optionforms_05_…` proves `atip.optionforms.05`). The rollback of a strategy is in
//! `spec030_reads.rs`, which holds the strategy fixture (`atip_optionforms_05_…`, `atip_optionforms_06_…`, `atip_optionforms_01_…`).

mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

use serde_json::{json, Value};
use tempfile::TempDir;

/// What canon 0.36.0 accepted, per option and class of value.
const GOLDEN: &str = "crates/canon-cli/tests/atip/option_forms_0_36.json";

/// The four commands that read `--approval-at`.
const APPROVAL_AT_COMMANDS: [&str; 4] = ["evidence add", "evidence approval-payload", "strategy approval-payload", "strategy rollback"];

/// The five time options and the commands that take each.
const DATE_OPTIONS: [(&str, &[&str]); 5] =
    [("expiry", &["divergence accept", "divergence add", "divergence defer"]), ("as-of", &["divergence list"]), ("before", &["export", "purge"]), ("after", &["export"]), ("since", &["query"])];

/// The four commands that read `--providers`.
const PROVIDER_COMMANDS: [&str; 4] = ["init", "skills check", "skills doctor", "skills install"];

/// The classes of the golden file that 0.37 still takes: the RFC 3339 spellings and the lists that were exact.
const KEPT: [&str; 5] = ["date-time", "fraction", "lowercase-t-z", "valid", "duplicate"];

/// The pattern of each option, as `arg_rules` writes it.
const TIMESTAMP: &str = canon_cli::arg_rules::TIMESTAMP;
const DATE_TIME: &str = canon_cli::arg_rules::DATE_TIME;
const PROVIDERS: &str = canon_cli::arg_rules::PROVIDERS;

/// An approval time as the same instant, in the spellings RFC 3339 reads.
const ONE_INSTANT: [&str; 7] =
    ["2030-01-01T10:00:00Z", "2030-01-01t10:00:00z", "2030-01-01T10:00:00.000Z", "2030-01-01T19:00:00+09:00", "2030-01-01T05:00:00-05:00", "2030-01-01T10:00:00.000000000+00:00", "2030-01-01T10:00:00.0-00:00"];

/// The spellings 0.36.0 took (and spellings it refused already) that a date-time option refuses now.
const NOT_A_DATE_TIME: [&str; 19] = [
    "2030-01-01",
    " 2030-01-01T10:00:00Z",
    "2030-01-01T10:00:00Z ",
    "\t2030-01-01T10:00:00Z",
    "2030-1-1T10:00:00Z",
    "2030-1-01T10:00:00Z",
    "2030-01-1T10:00:00Z",
    "+2030-01-01T10:00:00Z",
    "230-01-01T10:00:00Z",
    "2030-01-01 10:00:00Z",
    "2030-01-01T10:00:00\u{2212}02:00",
    "2030-01-01T10:00Z",
    "2030-01-01T10:00:00",
    "2030-01-01T10:00:00+0200",
    "2026-02-30T10:00:00Z",
    "2030-01-01T25:00:00Z",
    "2030-01-01T10:00:00+24:00",
    "yesterday",
    "",
];

/// The spellings of a full-date or date-time option that are refused: what 0.36.0 took, what it refused, and an impossible date.
const NOT_A_TIME: [&str; 22] = [
    "2030-1-1",
    "2030-1-01",
    "2030-01-1",
    " 2030-01-01",
    "\t2030-01-01",
    "\u{a0}2030-01-01",
    "2030-01-01 ",
    "+2030-01-01",
    "-0001-01-01",
    "230-01-01",
    "20300-01-01",
    "2030- 01-01",
    "2030-01-01 10:00:00Z",
    "2030-01-01T10:00:00\u{2212}02:00",
    "2030-01-01T10:00",
    "2030-01-01T10:00:00",
    "2030-01-01T10:00:00+0200",
    " 2030-01-01T10:00:00Z",
    "2026-02-30",
    "2030-01-01T25:00:00Z",
    "tomorrow",
    "",
];

/// The provider lists that are refused: what 0.36.0 took, what it refused, and the empty value.
const NOT_A_LIST: [&str; 16] = ["Claude", "NONE", "None", "claude,,pi", "claude,", ",omp", ",", " claude", "claude ", "claude, pi", "", "  ", "none,claude", "claude,none", "gemini", "claude,gemini"];

/// The provider lists that are taken (a name written twice among them).
const A_LIST: [&str; 6] = ["claude", "claude,omp", "none", "omp,omp", "pi,codex,claude", "claude,codex,omp,pi"];

// ---------------------------------------------------------------------------
// The story repo and the calls.

struct Story {
    repo: TempDir,
    home: TempDir,
}

static STORY: LazyLock<Story> = LazyLock::new(|| {
    let home = TempDir::new().expect("a home");
    let repo = harness::fixture(home.path());
    Story { repo, home }
});

fn base() -> &'static Path {
    STORY.repo.path()
}

fn home() -> &'static Path {
    STORY.home.path()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("`{path}` is readable: {e}"))
}

/// The result of one call on a copy of the story: its exit, its envelope, and the paths it changed.
struct Call {
    exit: i32,
    envelope: Value,
    changed: Vec<PathBuf>,
}

impl Call {
    fn code(&self) -> &str {
        self.envelope["error"]["code"].as_str().unwrap_or_default()
    }

    fn field(&self) -> &str {
        self.envelope["error"]["field"].as_str().unwrap_or_default()
    }
}

fn envelope(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

/// `canon <args> --json` on a copy of the story.
fn call(args: &[&str]) -> Call {
    let mut argv = harness::strs(args);
    argv.push("--json".to_string());
    let (changed, output) = harness::run_on_copy(base(), home(), &argv);
    Call { exit: output.status.code().unwrap_or(-1), envelope: envelope(&output), changed }
}

/// `canon <args>` on a copy of the story, without `--json`: the exit, the bytes on stdout and the paths it changed.
fn bare(args: &[&str]) -> (i32, Vec<u8>, Vec<PathBuf>) {
    let (changed, output) = harness::run_on_copy(base(), home(), &harness::strs(args));
    (output.status.code().unwrap_or(-1), output.stdout, changed)
}

/// The call, which must be refused with `code` at exit 2 naming `field`, and write nothing.
fn assert_refused(args: &[&str], code: &str, field: &str) {
    let got = call(args);
    assert_eq!((got.exit, got.code(), got.field()), (2, code, field), "canon {args:?}: {}", got.envelope);
    assert!(got.changed.is_empty(), "canon {args:?} was refused and wrote {:?}", got.changed);
}

/// The call is not refused for `field`: the stateless stage took the option (a later step may refuse the call for something else).
fn assert_taken(args: &[&str], field: &str) -> Call {
    let got = call(args);
    assert!(!(got.exit == 2 && got.field() == field), "canon {args:?} was refused for {field}: {}", got.envelope);
    got
}

fn files_under(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(files_under(&path, extension));
            } else if path.extension().is_some_and(|ext| ext == extension) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

// ---------------------------------------------------------------------------
// The documents.

fn docs() -> Vec<&'static Value> {
    harness::leaves()
}

fn option_of<'a>(doc: &'a Value, name: &str) -> Option<&'a Value> {
    doc["options"].as_array().into_iter().flatten().find(|option| option["name"] == name)
}

fn doc_of(command: &str) -> &'static Value {
    docs().into_iter().find(|doc| doc["command"] == command).unwrap_or_else(|| panic!("`{command}` is a command"))
}

/// Whether the declared domain of `--option` of `command` takes `value`.
fn admits(command: &str, option: &str, value: &Value) -> bool {
    let argument = option_of(doc_of(command), option).unwrap_or_else(|| panic!("`{command}` has `--{option}`"));
    match argument.get("domain") {
        None | Some(Value::Null) => true,
        Some(domain) => atip::domain::compile(domain).expect("the domain compiles").admits(value),
    }
}

/// The pattern each of the options must declare, in every command that has it.
fn pattern_of_option(option: &str) -> &'static str {
    match option {
        "approval-at" => DATE_TIME,
        "providers" => PROVIDERS,
        _ => TIMESTAMP,
    }
}

/// The places of the options of this change, `(option, command)`.
fn places() -> Vec<(&'static str, &'static str)> {
    let mut found: Vec<(&str, &str)> = APPROVAL_AT_COMMANDS.iter().map(|command| ("approval-at", *command)).collect();
    for (option, commands) in DATE_OPTIONS {
        found.extend(commands.iter().map(|command| (option, *command)));
    }
    found.extend(PROVIDER_COMMANDS.iter().map(|command| ("providers", *command)));
    found
}

/// What is wrong with the declarations of the options of this change: a command that lacks one, declares a domain of another pattern, or declares none.
fn forms_problems(docs: &[&Value]) -> Vec<String> {
    let mut problems = Vec::new();
    for (option, command) in places() {
        let Some(doc) = docs.iter().find(|doc| doc["command"] == command) else {
            problems.push(format!("`{command}` is not a command"));
            continue;
        };
        let Some(declared) = option_of(doc, option) else {
            problems.push(format!("`{command}` has no `--{option}`"));
            continue;
        };
        let pattern = declared["domain"]["pattern"].as_str();
        if pattern != Some(pattern_of_option(option)) {
            problems.push(format!("`{command} --{option}` declares {pattern:?}, not `{}`", pattern_of_option(option)));
        }
    }
    // One declaration for each option: the same domain in every command that has it.
    let mut domains: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (option, command) in places() {
        if let Some(doc) = docs.iter().find(|doc| doc["command"] == command) {
            if let Some(declared) = option_of(doc, option) {
                domains.entry(option).or_default().insert(declared.get("domain").map_or_else(|| "none".to_string(), ToString::to_string));
            }
        }
    }
    for (option, set) in domains {
        if set.len() > 1 {
            problems.push(format!("`--{option}` has {} domains: {set:?}", set.len()));
        }
    }
    problems
}

// ===========================================================================
// atip.optionforms.01 - 02 (F1): the signature bytes.

fn payload_args(at: &str) -> Vec<&str> {
    vec!["evidence", "approval-payload", "--scenario", "auth.login.01", "--project", "root", "--approver", "alice", "--approval-at", at]
}

/// The payload arguments of the scenario the signed `evidence add` records (`auth.login.02`).
fn signed_payload_args(at: &str) -> Vec<&str> {
    let mut args = payload_args(at);
    args[3] = "auth.login.02";
    args
}

#[test]
fn atip_optionforms_01_an_instant_in_any_rfc_3339_spelling_is_one_payload_and_the_payload_reads_no_clock() {
    let first = bare(&payload_args(ONE_INSTANT[0]));
    assert_eq!(first.0, 0, "{}", String::from_utf8_lossy(&first.1));
    assert!(first.2.is_empty(), "a payload is a read: {:?}", first.2);
    let text = String::from_utf8(first.1.clone()).expect("the payload is UTF-8");
    assert!(text.contains("\"timestamp\":\"2030-01-01T10:00:00.000000000Z\""), "the instant is UTC to the nanosecond: {text}");
    for at in ONE_INSTANT {
        let got = bare(&payload_args(at));
        assert_eq!((got.0, &got.1), (0, &first.1), "`--approval-at {at}` is the bytes of the instant");
    }
    // The same arguments give the same bytes: no clock is read. Another instant is other bytes.
    assert_eq!(bare(&payload_args(ONE_INSTANT[0])).1, first.1);
    assert_ne!(bare(&payload_args("2030-01-01T10:00:01Z")).1, first.1);
    assert_ne!(bare(&payload_args("2030-01-01T10:00:00.000000001Z")).1, first.1);
    // `--json` carries the same bytes as `payloadHex`.
    let json = call(&payload_args("2030-01-01T19:00:00+09:00"));
    let hex: String = first.1.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(json.envelope["data"]["payloadHex"], hex.as_str(), "{}", json.envelope);
    // Both approval-payload leaves and the two commands that record the time read it as the same date-time: the parser is one.
    let (utc, offset) = (canon_cli::timestamp::parse_date_time("2030-01-01T10:00:00Z"), canon_cli::timestamp::parse_date_time("2030-01-01T19:00:00+09:00"));
    assert_eq!(utc.expect("a date-time"), offset.expect("a date-time"));
}

#[test]
fn atip_optionforms_02_a_date_or_a_padded_or_loose_time_is_refused_for_approval_at_with_exit_2_and_no_bytes() {
    for at in NOT_A_DATE_TIME {
        let value = format!("--approval-at={at}");
        let mut argv = payload_args("x");
        argv.truncate(argv.len() - 2);
        argv.push(&value);
        // No `--json`: the bytes would be on stdout, and there are none.
        let (exit, stdout, changed) = bare(&argv);
        assert_eq!(exit, 2, "`{at}`");
        assert!(stdout.is_empty(), "`{at}`: a refused time prints no bytes: {}", String::from_utf8_lossy(&stdout));
        assert!(changed.is_empty(), "`{at}` wrote {changed:?}");
        let mut with_json = argv.clone();
        with_json.push("--json");
        let json = call(&with_json[..with_json.len() - 1]);
        assert_eq!((json.exit, json.code(), json.field()), (2, "USAGE", "--approval-at"), "`{at}`: {}", json.envelope);
        assert!(json.envelope["data"]["payloadHex"].is_null(), "`{at}`: {}", json.envelope);
        // The three other commands that read it refuse it with the same code and field, at the argument stage.
        let ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        assert_refused(&["strategy", "approval-payload", ulid, "--evaluation", "evaluation.json", "--approver", "alice", &value], "USAGE", "--approval-at");
        assert_refused(&["strategy", "rollback", ulid, "--reason", "r", "--actor", "alice", "--signature-file", "s.sig", &value], "USAGE", "--approval-at");
        assert_refused(
            &["evidence", "add", "--project", "root", "--scenario", "auth.login.02", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--approver", "alice", "--approval-role", "human", "--signature-file", "s.sig", &value],
            "USAGE",
            "--approval-at",
        );
    }
    // A value in an RFC 3339 spelling is taken by the argument stage: the next answer is about the strategy, which this repository does not hold.
    let ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    for at in ONE_INSTANT {
        let got = call(&["strategy", "approval-payload", ulid, "--evaluation", "evaluation.json", "--approver", "alice", "--approval-at", at]);
        assert_eq!((got.exit, got.code()), (2, "STRATEGY_NOT_FOUND"), "`{at}`: {}", got.envelope);
    }
}

// ===========================================================================
// atip.optionforms.03 - 04 (F2): the record.

/// A copy of the story with an allowed signer, and the means to sign the payload `canon evidence approval-payload` prints.
struct Signed {
    guard: TempDir,
    repo: PathBuf,
    key: PathBuf,
}

impl Signed {
    fn new() -> Signed {
        let (guard, repo) = harness::copy_of(base());
        let key = guard.path().join("approval_ed25519");
        let generated = Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-C", "canon-optionforms-test", "-f"]).arg(&key).output().expect("ssh-keygen");
        assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
        let public = std::fs::read_to_string(key.with_extension("pub")).expect("a public key");
        let mut fields = public.split_whitespace();
        let signers = repo.join(".canon/allowed_signers");
        std::fs::write(&signers, format!("alice {} {}\n", fields.next().expect("a key type"), fields.next().expect("a key"))).expect("allowed signers");
        std::fs::set_permissions(&signers, std::fs::Permissions::from_mode(0o600)).expect("owner-only verifier");
        let policy = repo.join(".canon/policy.yaml");
        let text = std::fs::read_to_string(&policy).expect("the starter policy");
        std::fs::write(&policy, format!("{text}\napproval:\n  allowed_signers: .canon/allowed_signers\n")).expect("the policy");
        harness::ok(&repo, home(), &["inventory", "sync"]);
        Signed { guard, repo, key }
    }

    fn run(&self, args: &[&str]) -> Output {
        harness::canon_in(&self.repo, home(), &harness::strs(args), &[])
    }

    /// The path of a detached signature over the payload the binary prints for `at`.
    fn signature(&self, at: &str) -> PathBuf {
        let payload = self.run(&signed_payload_args(at));
        assert!(payload.status.success(), "{}{}", String::from_utf8_lossy(&payload.stdout), String::from_utf8_lossy(&payload.stderr));
        let file = self.guard.path().join("payload.bin");
        std::fs::write(&file, &payload.stdout).expect("the payload");
        let signed = Command::new("ssh-keygen").args(["-Y", "sign", "-f"]).arg(&self.key).args(["-n", "canon-approval-v1"]).arg(&file).output().expect("ssh-keygen");
        assert!(signed.status.success(), "{}", String::from_utf8_lossy(&signed.stderr));
        PathBuf::from(format!("{}.sig", file.display()))
    }

    /// `canon evidence add` of `auth.login.02` with the approval of `alice`, signed over `signature`, and `at` as the time.
    fn add(&self, signature: &Path, at: &str) -> Output {
        let at = format!("--approval-at={at}");
        let signature = signature.to_string_lossy().into_owned();
        self.run(&[
            "evidence", "add", "--scenario", "auth.login.02", "--project", "root", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--approver", "alice", "--approval-role", "human", "--signature-file", &signature, &at, "--json",
        ])
    }

    /// The evidence records of `auth.login.02` that carry an approval, with the approval.
    fn approvals(&self) -> Vec<Value> {
        files_under(&self.repo.join(".canon/ledger/kind=evidence"), "json")
            .iter()
            .filter_map(|path| serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap_or_default()).ok())
            .filter(|record| record["scenario_id"] == "auth.login.02" && record["approval"].is_object())
            .map(|record| record["approval"].clone())
            .collect()
    }
}

#[test]
fn atip_optionforms_03_evidence_add_records_the_approval_signed_over_the_time_written_in_any_rfc_3339_spelling() {
    let signed = Signed::new();
    let signature = signed.signature("2030-01-01T10:00:00Z");
    // The time as signed, and the same instant in other spellings: each is the one record.
    let first = signed.add(&signature, "2030-01-01T19:00:00+09:00");
    assert!(first.status.success(), "{}{}", String::from_utf8_lossy(&first.stdout), String::from_utf8_lossy(&first.stderr));
    let approvals = signed.approvals();
    assert_eq!(approvals.len(), 1, "{approvals:?}");
    assert_eq!((approvals[0]["approver"].as_str(), approvals[0]["role"].as_str(), approvals[0]["at"].as_str()), (Some("alice"), Some("human"), Some("2030-01-01T10:00:00Z")), "the record holds the instant as UTC");
    assert_eq!(approvals[0]["subject"], "auth.login.02");
    for at in ONE_INSTANT {
        let again = signed.add(&signature, at);
        assert!(again.status.success(), "`{at}`: {}{}", String::from_utf8_lossy(&again.stdout), String::from_utf8_lossy(&again.stderr));
        assert_eq!(envelope(&again)["data"]["unchanged"], true, "`{at}` is the record already written");
    }
    assert_eq!(signed.approvals().len(), 1);
}

#[test]
fn atip_optionforms_04_evidence_add_with_another_instant_or_a_refused_spelling_records_nothing() {
    let signed = Signed::new();
    let signature = signed.signature("2030-01-01T10:00:00Z");
    let before = harness::tree(&signed.repo);
    // Another instant: the signature does not verify over it.
    for at in ["2030-01-01T10:00:01Z", "2030-01-01T10:00:00+01:00", "2030-01-01T10:00:00.000000001Z", "2030-01-02T10:00:00Z"] {
        let got = signed.add(&signature, at);
        let document = envelope(&got);
        assert_eq!((got.status.code(), document["error"]["code"].as_str(), document["error"]["reason"].as_str()), (Some(2), Some("EVIDENCE_APPROVAL_INVALID"), Some("SIGNATURE_MISMATCH")), "`{at}`: {document}");
        assert_eq!(harness::changed(&before, &harness::tree(&signed.repo)), Vec::<PathBuf>::new(), "`{at}` recorded something");
    }
    // A spelling the argument stage refuses, the date that the signature may have been made over included.
    for at in ["2030-01-01", "2030-01-01 10:00:00Z", " 2030-01-01T10:00:00Z", "2030-01-01T10:00", "2030-1-1T10:00:00Z", "2030-01-01T10:00:00\u{2212}02:00", "+2030-01-01T10:00:00Z", ""] {
        let got = signed.add(&signature, at);
        let document = envelope(&got);
        assert_eq!((got.status.code(), document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--approval-at")), "`{at}`: {document}");
        assert_eq!(harness::changed(&before, &harness::tree(&signed.repo)), Vec::<PathBuf>::new(), "`{at}` recorded something");
    }
    // The four approval options still come together.
    let signature = signature.to_string_lossy().into_owned();
    let got = signed.run(&[
        "evidence", "add", "--scenario", "auth.login.02", "--project", "root", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--approver", "alice", "--approval-role", "human", "--signature-file", &signature, "--json",
    ]);
    let document = envelope(&got);
    assert_eq!((got.status.code(), document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("EVIDENCE_APPROVAL_REQUIRED"), Some("--approval-at")), "{document}");
    assert!(signed.approvals().is_empty());
    assert_eq!(harness::changed(&before, &harness::tree(&signed.repo)), Vec::<PathBuf>::new());
}

// ===========================================================================
// atip.optionforms.07 - 08 (F3): the date options.

/// The call of `command` that gives `--option` the value `value`.
fn date_call(option: &str, command: &str, value: &str) -> Vec<String> {
    let given = format!("--{option}={value}");
    let mut argv: Vec<&str> = match command {
        "divergence add" => vec!["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--state", "deferred", "--reason", "r"],
        "divergence defer" => vec!["divergence", "defer", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--reason", "r"],
        "divergence accept" => vec!["divergence", "accept", "auth.login.01", "--actor", "alice", "--reason", "r", "--tracking", "T-1", "--role", "reviewer"],
        "divergence list" => vec!["divergence", "list"],
        "export" => vec!["export", "--kind", "change"],
        "purge" => vec!["purge", "--kind", "session"],
        "query" => vec!["query", "--kind", "evidence"],
        other => panic!("`{other}` takes no date option"),
    };
    argv.push(&given);
    harness::strs(&argv)
}

fn run_date(option: &str, command: &str, value: &str) -> Call {
    let argv = date_call(option, command, value);
    call(&argv.iter().map(String::as_str).collect::<Vec<_>>())
}

#[test]
fn atip_optionforms_07_the_date_options_take_a_full_date_or_a_date_time_and_a_date_is_utc_midnight() {
    for (option, commands) in DATE_OPTIONS {
        for command in commands {
            for value in ["2030-01-01", "2030-01-01T00:00:00Z", "2030-01-01t00:00:00z", "2030-01-01T10:00:00.5+09:00", "2030-01-01T10:00:00-02:00", "0001-01-01"] {
                let got = run_date(option, command, value);
                assert!(!(got.exit == 2 && got.field() == format!("--{option}")), "`canon {command} --{option} {value}` was refused for the option: {}", got.envelope);
            }
        }
    }
    // A date is that day's UTC midnight: the same instant as the date-time, and what a deferral records.
    assert_eq!(canon_cli::timestamp::parse("2030-01-01").expect("a date"), canon_cli::timestamp::parse("2030-01-01T00:00:00Z").expect("a date-time"));
    assert_eq!(canon_cli::timestamp::parse("2030-01-01").expect("a date").to_rfc3339(), "2030-01-01T00:00:00+00:00");
    let (_guard, repo) = harness::copy_of(base());
    let output = harness::canon_in(&repo, home(), &harness::strs(&["divergence", "defer", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--reason", "waiting", "--expiry", "2030-01-01", "--json"]), &[]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    let staged: String = files_under(&repo.join(".canon/ledger"), "json").iter().map(|path| std::fs::read_to_string(path).unwrap_or_default()).collect();
    assert!(staged.contains("2030-01-01T00:00:00Z"), "the deferral holds the midnight: {staged}");
    // An export window of two forms of one day is a window that is taken.
    let got = call(&["export", "--kind", "change", "--after", "2030-01-01", "--before", "2030-01-01T00:00:00Z"]);
    assert_eq!(got.exit, 0, "{}", got.envelope);
}

#[test]
fn atip_optionforms_08_a_loose_or_impossible_time_is_refused_for_each_date_option_and_nothing_is_recorded() {
    let mut cells = 0;
    for (option, commands) in DATE_OPTIONS {
        for command in commands {
            for value in NOT_A_TIME {
                let argv = date_call(option, command, value);
                let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
                assert_refused(&argv, "USAGE", &format!("--{option}"));
                cells += 1;
            }
        }
    }
    assert_eq!(cells, 8 * NOT_A_TIME.len(), "every command and every spelling was run");
    // The order of an export window is checked as before.
    assert_refused(&["export", "--kind", "change", "--before", "2030-01-01", "--after", "2030-02-01"], "USAGE", "--after");
    assert_refused(&["export", "--kind", "change", "--before", "2030-01-01T00:00:00Z", "--after", "2030-01-01T00:00:01Z"], "USAGE", "--after");
}

// ===========================================================================
// atip.optionforms.09 - 10 (F4): the provider list.

/// A new git directory, which `init` sets up.
fn fresh() -> TempDir {
    let dir = TempDir::new().expect("a directory");
    harness::git(dir.path(), &["init", "-q", "."]);
    dir
}

fn tree_of(dir: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    harness::tree(dir)
}

#[test]
fn atip_optionforms_09_init_and_the_three_skills_commands_take_none_and_the_exact_names() {
    for value in A_LIST {
        let given = format!("--providers={value}");
        // `init` records the list it is given.
        let dir = fresh();
        let output = harness::canon_in(dir.path(), home(), &harness::strs(&["init", &given, "--json"]), &[]);
        assert!(output.status.success(), "init {value}: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let yaml = std::fs::read_to_string(dir.path().join("canon.yaml")).expect("canon.yaml");
        let expected = if value == "none" { "[]".to_string() } else { format!("[{}]", canon_cli::skills::parse_provider_list(value).expect("a list").iter().map(|provider| provider.as_str()).collect::<Vec<_>>().join(", ")) };
        assert!(yaml.contains(&format!("providers: {expected}")), "init {value}: {yaml}");
        // The skills commands take it.
        for command in ["install", "check", "doctor"] {
            let mut args = vec!["skills", command, given.as_str()];
            if command == "install" {
                args.push("--dry-run");
            }
            let got = assert_taken(&args, "--providers");
            assert_ne!(got.code(), "SKILLS_ARGUMENT_INVALID", "skills {command} {value}: {}", got.envelope);
        }
    }
    // `skills install --providers none` records the empty list.
    let (_guard, repo) = harness::copy_of(base());
    let output = harness::canon_in(&repo, home(), &harness::strs(&["skills", "install", "--providers", "none", "--json"]), &[]);
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(std::fs::read_to_string(repo.join("canon.yaml")).expect("canon.yaml").contains("providers: []"));
}

#[test]
fn atip_optionforms_10_a_provider_list_that_is_not_exact_is_refused_by_the_four_commands_and_nothing_is_written() {
    for value in NOT_A_LIST {
        let given = format!("--providers={value}");
        // `init` in a new directory writes nothing.
        let dir = fresh();
        let before = tree_of(dir.path());
        let output = harness::canon_in(dir.path(), home(), &harness::strs(&["init", &given, "--json"]), &[]);
        let document = envelope(&output);
        assert_eq!((output.status.code(), document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("INIT_ARGUMENT_INVALID"), Some("--providers")), "init `{value}`: {document}");
        assert_eq!(harness::changed(&before, &tree_of(dir.path())), Vec::<PathBuf>::new(), "init `{value}` wrote");
        assert!(!dir.path().join("canon.yaml").exists());
        for command in ["install", "check", "doctor"] {
            assert_refused(&["skills", command, &given], "SKILLS_ARGUMENT_INVALID", "--providers");
        }
    }
    // A refused list leaves canon.yaml's own setting alone: it is not an option, and is read as before.
    let (_guard, repo) = harness::copy_of(base());
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).expect("canon.yaml");
    let output = harness::canon_in(&repo, home(), &harness::strs(&["skills", "install", "--providers=Claude", "--json"]), &[]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(std::fs::read_to_string(repo.join("canon.yaml")).expect("canon.yaml"), yaml);
}

// ===========================================================================
// atip.optionforms.11 - 12 (F5): the declarations.

#[test]
fn atip_optionforms_11_each_option_declares_one_domain_in_every_command_and_the_help_says_the_new_spelling() {
    let docs = docs();
    let problems = forms_problems(&docs);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    assert_eq!(places().len(), 4 + 8 + 4, "every place of the four approval commands, the five date options and the four list commands");
    // The five date options say one declaration, and `--approval-at` another; each is a pattern of digits and separators.
    let time: BTreeSet<String> = DATE_OPTIONS.iter().flat_map(|(option, commands)| commands.iter().map(move |command| option_of(doc_of(command), option).expect("a date option")["domain"].to_string())).collect();
    assert_eq!(time.len(), 1, "{time:?}");
    let approval: BTreeSet<String> = APPROVAL_AT_COMMANDS.iter().map(|command| option_of(doc_of(command), "approval-at").expect("--approval-at")["domain"].to_string()).collect();
    assert_eq!(approval.len(), 1, "{approval:?}");
    assert_ne!(time, approval);
    for pattern in [TIMESTAMP, DATE_TIME, PROVIDERS] {
        assert_eq!(atip::domain::pattern_problem(pattern, atip::Minor::parse("1.5").expect("1.5")), None, "{pattern}");
        assert!(!pattern.contains("White_Space"), "a pattern of characters alone, no white-space class: {pattern}");
    }
    // `init` declares the domain of the list the `skills` commands declare, with its own code.
    let init = option_of(doc_of("init"), "providers").expect("init --providers");
    assert_eq!(init["domain"], option_of(doc_of("skills install"), "providers").expect("skills install --providers")["domain"]);
    // The help says what is taken.
    for (command, option, needle) in [("evidence add", "approval-at", "date-time"), ("evidence approval-payload", "approval-at", "date-time"), ("strategy approval-payload", "approval-at", "date-time"), ("strategy rollback", "approval-at", "date-time"), ("divergence add", "expiry", "RFC 3339"), ("divergence list", "as-of", "RFC 3339"), ("export", "before", "RFC 3339"), ("export", "after", "RFC 3339"), ("query", "since", "RFC 3339"), ("init", "providers", "lowercase"), ("skills install", "providers", "lowercase"), ("skills check", "providers", "lowercase"), ("skills doctor", "providers", "lowercase")] {
        let help = option_of(doc_of(command), option).expect("an option")["help"].as_str().unwrap_or_default().to_string();
        assert!(help.contains(needle), "`{command} --{option}` says `{needle}`: {help}");
    }
    assert_eq!(init["domain"]["pattern"], PROVIDERS);
}

/// The values the domains judge, `(command, option, value, taken)`: the table run through the real pattern and the real parser.
fn value_table() -> Vec<(&'static str, &'static str, String, bool)> {
    let mut table = Vec::new();
    for command in APPROVAL_AT_COMMANDS {
        for value in ONE_INSTANT {
            table.push((command, "approval-at", value.to_string(), true));
        }
        for value in NOT_A_DATE_TIME.iter().filter(|value| !value.starts_with("2026-02-30") && !value.starts_with("2030-01-01T25") && !value.contains("+24:00")) {
            table.push((command, "approval-at", (*value).to_string(), false));
        }
    }
    for (option, commands) in DATE_OPTIONS {
        for command in commands {
            for value in ["2030-01-01", "2030-01-01T10:00:00Z", "2030-01-01t10:00:00z", "2030-01-01T10:00:00.5-02:00", "0001-01-01"] {
                table.push((command, option, value.to_string(), true));
            }
            for value in NOT_A_TIME.iter().filter(|value| !value.starts_with("2026-02-30") && !value.starts_with("2030-01-01T25")) {
                table.push((command, option, (*value).to_string(), false));
            }
        }
    }
    for command in PROVIDER_COMMANDS {
        for value in A_LIST {
            table.push((command, "providers", value.to_string(), true));
        }
        for value in NOT_A_LIST {
            table.push((command, "providers", value.to_string(), false));
        }
    }
    table
}

#[test]
fn atip_optionforms_12_the_table_of_values_inside_and_outside_each_domain_agrees_with_the_real_parser() {
    let table = value_table();
    assert!(table.len() > 300, "{} cells", table.len());
    for (command, option, value, taken) in &table {
        // The declared domain.
        assert_eq!(admits(command, option, &json!(value)), *taken, "`{command} --{option}` and {value:?}");
        // The parser of the value.
        let parsed = match *option {
            "approval-at" => canon_cli::timestamp::parse_date_time(value).is_ok(),
            "providers" => canon_cli::skills::parse_provider_list(value).is_ok(),
            _ => canon_cli::timestamp::parse(value).is_ok(),
        };
        assert_eq!(parsed, *taken, "the parser of `--{option}` and {value:?}");
    }
    // A date or a time that does not exist matches the pattern (it states the spelling) and is refused by the parser: the validator's.
    for value in ["2026-02-30T10:00:00Z", "2030-01-01T25:00:00Z", "2030-13-01T00:00:00Z", "2030-01-01T10:60:00Z", "2030-01-01T10:00:00+24:00"] {
        assert!(admits("evidence approval-payload", "approval-at", &json!(value)), "the pattern is the spelling: {value}");
        assert!(canon_cli::timestamp::parse_date_time(value).is_err(), "{value}");
        assert!(canon_cli::timestamp::parse(value).is_err(), "{value}");
    }
    assert!(admits("query", "since", &json!("2026-02-30")) && canon_cli::timestamp::parse("2026-02-30").is_err());
    // The seeded faults: a command that declares another domain for the same option fails the sweep, by option and command.
    let real: Vec<Value> = docs().into_iter().cloned().collect();
    let seed = |command: &str, option: &str, domain: Option<Value>| -> Vec<String> {
        let mut seeded = real.clone();
        let doc = seeded.iter_mut().find(|doc| doc["command"] == command).expect("a command");
        let declared = doc["options"].as_array_mut().expect("options").iter_mut().find(|declared| declared["name"] == option).expect("an option");
        match domain {
            Some(domain) => declared["domain"] = domain,
            None => {
                declared.as_object_mut().expect("an object").remove("domain");
            }
        }
        let refs: Vec<&Value> = seeded.iter().collect();
        forms_problems(&refs)
    };
    let found = seed("strategy rollback", "approval-at", Some(json!({ "pattern": TIMESTAMP })));
    assert!(found.iter().any(|problem| problem.contains("`strategy rollback --approval-at`") && problem.contains("declares")), "{found:?}");
    assert!(found.iter().any(|problem| problem.contains("`--approval-at` has 2 domains")), "{found:?}");
    let found = seed("skills doctor", "providers", None);
    assert!(found.iter().any(|problem| problem.contains("`skills doctor --providers`")), "{found:?}");
    let found = seed("init", "providers", Some(json!({ "pattern": "^[a-z,]+$" })));
    assert!(found.iter().any(|problem| problem.contains("`--providers` has 2 domains")), "{found:?}");
    let found = seed("export", "after", Some(json!({ "pattern": DATE_TIME })));
    assert!(found.iter().any(|problem| problem.contains("`export --after` declares")), "{found:?}");
    let found = seed("divergence add", "expiry", Some(json!({ "pattern": DATE_TIME })));
    assert!(found.iter().any(|problem| problem.contains("`--expiry` has 2 domains")), "{found:?}");
    let refs: Vec<&Value> = real.iter().collect();
    assert!(forms_problems(&refs).is_empty());
}

// ===========================================================================
// atip.optionforms.13 - 14 (F6): the CHANGELOG says what is refused now.

/// One class of value of the golden file, as an entry says it: the option, the class, a value of it, and the commands that took it in 0.36.0.
struct Entry {
    option: String,
    class: String,
    value: Value,
    commands: Vec<String>,
}

fn entries(key: &str) -> Vec<Entry> {
    let file: Value = serde_json::from_str(&read(GOLDEN)).expect("the golden file is JSON");
    file[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is a list"))
        .iter()
        .map(|entry| Entry {
            option: entry["option"].as_str().expect("an option").to_string(),
            class: entry["class"].as_str().expect("a class").to_string(),
            value: json!(entry["value"].as_str().expect("a value")),
            commands: entry["commands"].as_array().expect("commands").iter().map(|command| command.as_str().expect("a command").to_string()).collect(),
        })
        .collect()
}

/// The `(option, command, class)` triples of the golden file that this build refuses: a value 0.36.0 took.
fn refused_now(entries: &[Entry]) -> BTreeSet<(String, String, String)> {
    let mut found = BTreeSet::new();
    for entry in entries {
        for command in &entry.commands {
            if !admits(command, &entry.option, &entry.value) {
                found.insert((entry.option.clone(), command.clone(), entry.class.clone()));
            }
        }
    }
    found
}

/// The `### Breaking` table rows of the `Unreleased` entry that name an option of this change, split into cells.
fn breaking_rows(changelog: &str) -> Vec<Vec<String>> {
    let unreleased = changelog.split("\n## ").find(|part| part.starts_with("0.37.0")).unwrap_or_default();
    let breaking = unreleased.split("\n### ").find(|part| part.starts_with("Breaking")).unwrap_or_default();
    breaking
        .lines()
        .filter(|line| line.starts_with("| `--"))
        .map(|line| line.trim().trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect::<Vec<String>>())
        .filter(|row| backticked(&row[0]).iter().all(|option| is_of_this_change(option.trim_start_matches("--"))))
        .collect()
}

fn is_of_this_change(option: &str) -> bool {
    option == "approval-at" || option == "providers" || DATE_OPTIONS.iter().any(|(name, _)| *name == option)
}

fn backticked(cell: &str) -> BTreeSet<String> {
    cell.split('`').enumerate().filter(|(index, _)| index % 2 == 1).map(|(_, word)| word.to_string()).collect()
}

/// What the CHANGELOG lacks: a row for each value 0.36.0 took and this build refuses (its option in the first cell, its command in the second, its class in the third),
/// the answer and the way to move in every row, and the note that a signature is made again.
fn changelog_problems(changelog: &str, refused: &BTreeSet<(String, String, String)>) -> Vec<String> {
    let rows = breaking_rows(changelog);
    let mut problems = Vec::new();
    for (option, command, class) in refused {
        let found = rows.iter().any(|row| row.len() >= 5 && backticked(&row[0]).contains(&format!("--{option}")) && backticked(&row[1]).contains(command) && backticked(&row[2]).contains(class));
        if !found {
            problems.push(format!("no row of the table of newly refused values names `--{option}` in `{command}` for the class `{class}`"));
        }
    }
    for row in &rows {
        if row.len() < 5 || row[3].is_empty() || row[4].is_empty() {
            problems.push(format!("a row has no 0.37 answer or no way to move: {row:?}"));
        }
    }
    let unreleased = changelog.split("\n## ").find(|part| part.starts_with("0.37.0")).unwrap_or_default();
    if !unreleased.contains("make the signature again over the payload of the new date-time") {
        problems.push("no row tells a caller that signed a date alone to write the date-time and make the signature again".to_string());
    }
    if !unreleased.contains("take RFC 3339 and nothing else") {
        problems.push("no entry says that the time options take RFC 3339 and nothing else".to_string());
    }
    problems
}

#[test]
fn atip_optionforms_13_the_changelog_has_a_row_for_every_spelling_0_36_took_and_this_build_refuses() {
    let accepted = entries("accepted");
    let refused = refused_now(&accepted);
    assert!(refused.len() > 100, "the golden file reaches many cells ({})", refused.len());
    // The cells: every option is reached, and the spellings 0.36.0 took that 0.37 still takes need no row.
    for option in ["approval-at", "expiry", "as-of", "before", "after", "since", "providers"] {
        assert!(refused.iter().any(|(name, _, _)| name == option), "`--{option}` has a value that was taken and is refused");
    }
    for entry in accepted.iter().filter(|entry| KEPT.contains(&entry.class.as_str())) {
        for command in &entry.commands {
            assert!(admits(command, &entry.option, &entry.value), "`{command} --{}` still takes {:?} ({})", entry.option, entry.value, entry.class);
        }
    }
    assert!(!refused.iter().any(|(_, _, class)| KEPT.contains(&class.as_str())));
    // The date alone is refused by `--approval-at` only, and the lowercase `t` and `z` by none.
    assert!(refused.contains(&("approval-at".to_string(), "evidence add".to_string(), "date-only".to_string())));
    assert!(!refused.iter().any(|(option, _, class)| option != "approval-at" && class == "date-only"));
    // What 0.36.0 refused already stays refused, with no row.
    let before = entries("refused");
    assert!(before.len() > 20);
    for entry in &before {
        for command in &entry.commands {
            assert!(!admits(command, &entry.option, &entry.value), "`{command} --{}` took {:?} ({}), which 0.36.0 refused", entry.option, entry.value, entry.class);
        }
    }
    let changelog = read("CHANGELOG.md");
    let problems = changelog_problems(&changelog, &refused);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    // The table has no row that no refusal needs, and one row for each class of each group of options.
    let rows = breaking_rows(&changelog);
    assert_eq!(rows.len(), 9 + 8 + 5, "one row per class: nine for --approval-at, eight for the date options, five for --providers");
    for row in &rows {
        let (options, commands, classes) = (backticked(&row[0]), backticked(&row[1]), backticked(&row[2]));
        let needed = refused.iter().any(|(option, command, class)| options.contains(&format!("--{option}")) && commands.contains(command) && classes.contains(class));
        assert!(needed, "the row {row:?} names a value that is not refused now");
        assert_eq!(classes.len(), 1, "a row is one class: {row:?}");
    }
    // The classes of the third column are listed once each.
    for class in rows.iter().flat_map(|row| backticked(&row[2])).collect::<BTreeSet<_>>() {
        assert!(changelog.contains(&format!("- `{class}`: ")), "the class `{class}` is described");
    }
}

#[test]
fn atip_optionforms_14_the_check_of_the_changelog_fails_for_a_missing_row_a_missing_answer_and_a_missing_note() {
    let refused = refused_now(&entries("accepted"));
    let changelog = read("CHANGELOG.md");
    assert!(changelog_problems(&changelog, &refused).is_empty());
    let lines: Vec<&str> = changelog.lines().collect();
    // A row taken out: the first that names `--approval-at` and `one-digit-month`, and the first that names `--providers` and `uppercase`.
    for (option, class) in [("`--approval-at`", "`one-digit-month`"), ("`--providers`", "`uppercase`"), ("`--expiry`, `--as-of`, `--before`, `--after`, `--since`", "`unicode-minus`")] {
        let at = lines.iter().position(|line| line.starts_with(&format!("| {option}")) && line.contains(class)).unwrap_or_else(|| panic!("a row names {option} and {class}"));
        let without: String = lines.iter().enumerate().filter(|(index, _)| *index != at).map(|(_, line)| *line).collect::<Vec<_>>().join("\n");
        let problems = changelog_problems(&without, &refused);
        assert!(problems.iter().any(|problem| problem.contains("no row") && problem.contains(&class.replace('`', ""))), "{problems:?}");
    }
    // A row with no way to move.
    let at = lines.iter().position(|line| line.starts_with("| `--approval-at`") && line.contains("`date-only`")).expect("a row names --approval-at and date-only");
    let blank_move: String = lines.iter().enumerate().map(|(index, line)| if index == at { "| `--approval-at` | `evidence add` | `date-only` | refused |  |".to_string() } else { (*line).to_string() }).collect::<Vec<_>>().join("\n");
    assert!(changelog_problems(&blank_move, &refused).iter().any(|problem| problem.contains("no way to move")));
    // The note that a signature is made again, and the entry, taken out.
    let without_note = changelog.replace("make the signature again over the payload of the new date-time", "sign");
    assert!(changelog_problems(&without_note, &refused).iter().any(|problem| problem.contains("make the signature again")));
    let without_entry = changelog.replace("take RFC 3339 and nothing else", "take dates");
    assert!(changelog_problems(&without_entry, &refused).iter().any(|problem| problem.contains("RFC 3339 and nothing else")));
    // A refused value of a class no row names needs its row.
    let mut seeded = refused.clone();
    seeded.insert(("providers".to_string(), "init".to_string(), "no-such-class".to_string()));
    assert!(changelog_problems(&changelog, &seeded).iter().any(|problem| problem.contains("no-such-class")));
}

// ===========================================================================
// atip.optionforms.15 - 16: the checker's fixture and the golden file.

#[test]
fn atip_optionforms_15_the_checker_fixture_lists_values_the_new_domains_refuse_and_the_recipes_run_at_1_5() {
    let fixture: Value = serde_json::from_str(&read("crates/canon-cli/tests/atip/canon.fixture.json")).expect("the fixture is JSON");
    assert_eq!(fixture["contractVersion"], "1.5");
    let commands = fixture["commands"].as_object().expect("commands");
    let mut listed = BTreeSet::new();
    for (command, entry) in commands {
        for (argument, values) in entry["domainRejects"].as_object().into_iter().flatten() {
            for value in values.as_array().expect("values") {
                if option_of(doc_of(command), argument).is_some() && (is_of_this_change(argument)) {
                    assert!(!admits(command, argument, value), "`{command} --{argument}` refuses {value}");
                    listed.insert((command.clone(), argument.clone()));
                }
            }
        }
    }
    for (option, command) in places() {
        assert!(listed.contains(&(command.to_string(), option.to_string())), "the fixture lists values `{command} --{option}` refuses");
    }
    let justfile = read("justfile");
    assert!(justfile.contains("--contract 1.5"), "the recipes run the checker at 1.5");
}

#[test]
fn atip_optionforms_16_the_golden_file_is_the_measure_of_0_36_and_its_classes_are_each_described() {
    let file: Value = serde_json::from_str(&read(GOLDEN)).expect("the golden file is JSON");
    let classes = file["classes"].as_object().expect("classes");
    for key in ["accepted", "refused"] {
        for entry in file[key].as_array().expect("entries") {
            let class = entry["class"].as_str().expect("a class");
            assert!(classes.contains_key(class), "the class `{class}` of `{key}` is described");
            assert!(!entry["commands"].as_array().expect("commands").is_empty());
            let option = entry["option"].as_str().expect("an option");
            assert!(is_of_this_change(option), "`{option}` is an option of this change");
        }
    }
    // Every place of this change was measured: each (option, command) has an accepted entry.
    let measured: BTreeSet<(String, String)> = entries("accepted").into_iter().flat_map(|entry| entry.commands.into_iter().map(move |command| (entry.option.clone(), command))).collect();
    for (option, command) in places() {
        assert!(measured.contains(&(option.to_string(), command.to_string())), "0.36.0 was measured for `{command} --{option}`");
    }
    // The measure says what the brief says 0.36.0 took.
    let took = |option: &str, value: &str| entries("accepted").iter().any(|entry| entry.option == option && entry.value == json!(value));
    assert!(took("approval-at", "2030-01-01") && took("approval-at", " 2030-01-01") && took("approval-at", "2030-1-01") && took("approval-at", "2030-01-01 10:00:00Z") && took("approval-at", "2030-01-01T10:00:00\u{2212}02:00"));
    assert!(took("providers", "Claude") && took("providers", "claude,,pi") && took("providers", "") && took("providers", "claude, pi") && took("providers", "NONE"));
    assert!(!took("providers", "none,claude") && !took("providers", "gemini"));
}
