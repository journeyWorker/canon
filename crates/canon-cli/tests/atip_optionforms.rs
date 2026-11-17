//! `canon-0-37-option-forms` (scenarios `atip.optionforms.01-16`): the time options take RFC 3339 and nothing else, and the provider options take the exact names, against the
//! built binary and the documents it declares.
//!
//! `--approval-at` takes a date-time (`YYYY-MM-DDTHH:MM:SS`, a fraction, `Z` or `+HH:MM`); `--expiry`, `--as-of`, `--before`, `--after` and `--since` take a full-date or the
//! same date-time; `T` and `Z` are read in either case. The provider list was `--providers` in 0.37 and is the repeatable `--harness` and the switch `--no-harness` since `canon-0-38-provider-option`
//! (scenarios `.09` and `.10` hold the names those take). `atip/option_forms_0_36.json` holds what canon 0.36.0 accepted, per option and class of value.
//!
//! The check that seeded faults must trip is a small pure function in this file (`forms_problems`): the real input passes it, each seeded fault is
//! found, and no sweep is vacuous. Tests are named by scenario id (`atip_optionforms_05_…` proves `atip.optionforms.05`). The rollback of a strategy is in
//! `spec030_reads.rs`, which holds the strategy fixture (`atip_optionforms_05_…`, `atip_optionforms_06_…`, `atip_optionforms_01_…`).

#[path = "support/hermetic.rs"]
mod hermetic;
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

/// The pattern of each option, as `arg_rules` writes it.
const TIMESTAMP: &str = canon_cli::arg_rules::TIMESTAMP;
const DATE_TIME: &str = canon_cli::arg_rules::DATE_TIME;

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

/// The values `--harness` refuses: what 0.37 took as part of a list and what it refused, one name each (a capital, `none`, white space around, an empty value, an unknown name).
const NOT_A_NAME: [&str; 10] = ["Claude", "NONE", "None", "none", "claude ", " claude", "claude,omp", "", "  ", "gemini"];

/// The provider selections that are taken, as the names given (a name written twice among them) and `none` for `--no-harness`: the lists 0.37 took, written the 0.38 way.
const A_SELECTION: [&[&str]; 6] = [&["claude"], &["claude", "omp"], &[], &["omp", "omp"], &["pi", "codex", "claude"], &["claude", "codex", "omp", "pi"]];

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
        _ => TIMESTAMP,
    }
}

/// The places of the options of this change, `(option, command)`.
fn places() -> Vec<(&'static str, &'static str)> {
    let mut found: Vec<(&str, &str)> = APPROVAL_AT_COMMANDS.iter().map(|command| ("approval-at", *command)).collect();
    for (option, commands) in DATE_OPTIONS {
        found.extend(commands.iter().map(|command| (option, *command)));
    }
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
// atip.optionforms.09 - 10 (F4): the provider names (`--harness` and `--no-harness` since 0.38).

/// A new git directory, which `init` sets up.
fn fresh() -> TempDir {
    let dir = TempDir::new().expect("a directory");
    harness::git(dir.path(), &["init", "-q", "."]);
    dir
}

fn tree_of(dir: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    harness::tree(dir)
}

/// The arguments that give `names` (`--harness` for each, or `--no-harness` for none).
fn selection_args(names: &[&str]) -> Vec<String> {
    if names.is_empty() {
        return vec!["--no-harness".to_string()];
    }
    names.iter().flat_map(|name| ["--harness".to_string(), (*name).to_string()]).collect()
}

#[test]
fn atip_optionforms_09_init_and_the_three_skills_commands_take_no_provider_and_each_name_as_the_list_of_0_37_was_taken() {
    for names in A_SELECTION {
        let given = selection_args(names);
        // `init` records the list it is given.
        let dir = fresh();
        let mut argv = vec!["init".to_string()];
        argv.extend(given.clone());
        argv.push("--json".to_string());
        let output = harness::canon_in(dir.path(), home(), &argv, &[]);
        assert!(output.status.success(), "init {names:?}: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let yaml = std::fs::read_to_string(dir.path().join("canon.yaml")).expect("canon.yaml");
        let mut chosen: Vec<&str> = names.to_vec();
        chosen.sort_unstable();
        chosen.dedup();
        assert!(yaml.contains(&format!("providers: [{}]", chosen.join(", "))), "init {names:?}: {yaml}");
        // The skills commands take it.
        for command in ["install", "check", "doctor"] {
            let mut args: Vec<String> = vec!["skills".to_string(), command.to_string()];
            args.extend(given.clone());
            if command == "install" {
                args.push("--dry-run".to_string());
            }
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let got = assert_taken(&refs, "--harness");
            assert_ne!(got.code(), "USAGE", "skills {command} {names:?}: {}", got.envelope);
        }
    }
    // `skills install --no-harness` records the empty list.
    let (_guard, repo) = harness::copy_of(base());
    let output = harness::canon_in(&repo, home(), &harness::strs(&["skills", "install", "--no-harness", "--json"]), &[]);
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(std::fs::read_to_string(repo.join("canon.yaml")).expect("canon.yaml").contains("providers: []"));
}

#[test]
fn atip_optionforms_10_a_provider_name_that_is_not_exact_is_refused_by_the_four_commands_and_nothing_is_written() {
    for value in NOT_A_NAME {
        let given = format!("--harness={value}");
        // `init` in a new directory writes nothing.
        let dir = fresh();
        let before = tree_of(dir.path());
        let output = harness::canon_in(dir.path(), home(), &harness::strs(&["init", &given, "--json"]), &[]);
        let document = envelope(&output);
        assert_eq!((output.status.code(), document["error"]["code"].as_str(), document["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--harness")), "init `{value}`: {document}");
        assert_eq!(document["error"]["expected"], json!(["claude", "codex", "omp", "pi"]), "init `{value}`: {document}");
        assert_eq!(harness::changed(&before, &tree_of(dir.path())), Vec::<PathBuf>::new(), "init `{value}` wrote");
        assert!(!dir.path().join("canon.yaml").exists());
        for command in ["install", "check", "doctor"] {
            assert_refused(&["skills", command, &given], "USAGE", "--harness");
        }
    }
    // A refused name leaves canon.yaml's own setting alone.
    let (_guard, repo) = harness::copy_of(base());
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).expect("canon.yaml");
    let output = harness::canon_in(&repo, home(), &harness::strs(&["skills", "install", "--harness=Claude", "--json"]), &[]);
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
    assert!(!places().is_empty());
    // The five date options say one declaration, and `--approval-at` another; each is a pattern of digits and separators.
    let time: BTreeSet<String> = DATE_OPTIONS.iter().flat_map(|(option, commands)| commands.iter().map(move |command| option_of(doc_of(command), option).expect("a date option")["domain"].to_string())).collect();
    assert_eq!(time.len(), 1, "{time:?}");
    let approval: BTreeSet<String> = APPROVAL_AT_COMMANDS.iter().map(|command| option_of(doc_of(command), "approval-at").expect("--approval-at")["domain"].to_string()).collect();
    assert_eq!(approval.len(), 1, "{approval:?}");
    assert_ne!(time, approval);
    for pattern in [TIMESTAMP, DATE_TIME] {
        assert_eq!(atip::domain::pattern_problem(pattern, atip::Minor::parse("1.5").expect("1.5")), None, "{pattern}");
        assert!(!pattern.contains("White_Space"), "a pattern of characters alone, no white-space class: {pattern}");
    }
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
    table
}

#[test]
fn atip_optionforms_12_the_table_of_values_inside_and_outside_each_domain_agrees_with_the_real_parser() {
    let table = value_table();
    assert!(!table.is_empty());
    for (command, option, value, taken) in &table {
        // The declared domain.
        assert_eq!(admits(command, option, &json!(value)), *taken, "`{command} --{option}` and {value:?}");
        // The parser of the value.
        let parsed = match *option {
            "approval-at" => canon_cli::timestamp::parse_date_time(value).is_ok(),
            _ => canon_cli::timestamp::parse(value).is_ok(),
        };
        assert_eq!(parsed, *taken, "the parser of `--{option}` and {value:?}");
    }
    // A date or a time that does not exist matches the pattern (it states the spelling) and is refused by the parser: the validator's.
    for value in ["2026-02-30T10:00:00Z", "2030-01-01T25:00:00Z", "2030-13-01T00:00:00Z", "2030-01-01T10:60:00Z", "2030-01-01T10:00:00+24:00", "2030-01-01T23:59:60Z", "2030-01-01T10:00:60.5+01:00"] {
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
    assert!(!found.is_empty(), "{found:?}");
    let found = seed("export", "after", Some(json!({ "pattern": DATE_TIME })));
    assert!(!found.is_empty(), "{found:?}");
    let found = seed("divergence add", "expiry", Some(json!({ "pattern": DATE_TIME })));
    assert!(!found.is_empty(), "{found:?}");
    let refs: Vec<&Value> = real.iter().collect();
    assert!(forms_problems(&refs).is_empty());
}

// ===========================================================================
// The golden file of 0.36.0.

/// One class of value of the golden file, as an entry says it: the option, a value of it, and the commands that took it in 0.36.0.
struct Entry {
    option: String,
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
            value: json!(entry["value"].as_str().expect("a value")),
            commands: entry["commands"].as_array().expect("commands").iter().map(|command| command.as_str().expect("a command").to_string()).collect(),
        })
        .collect()
}

fn is_of_this_change(option: &str) -> bool {
    option == "approval-at" || DATE_OPTIONS.iter().any(|(name, _)| *name == option)
}

// ===========================================================================
// atip.optionforms.15 - 16: the checker's fixture and the golden file.

#[test]
fn atip_optionforms_15_the_checker_fixture_lists_values_the_new_domains_refuse() {
    let fixture: Value = serde_json::from_str(&read("crates/canon-cli/tests/atip/canon.fixture.json")).expect("the fixture is JSON");
    assert_eq!(fixture["contractVersion"], canon_cli::json_out::DECLARED_MINOR);
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
    // `--providers` is not read here: its entries are under `supersededBy038`, which holds what 0.36.0 took of it as the record.
    assert!(entries("accepted").iter().all(|entry| entry.option != "providers") && entries("refused").iter().all(|entry| entry.option != "providers"));
    let superseded: Value = serde_json::from_str(&read(GOLDEN)).expect("the golden file is JSON");
    let kept = &superseded["supersededBy038"];
    assert_eq!((kept["change"].as_str(), kept["option"].as_str()), (Some("canon-0-38-provider-option"), Some("providers")));
    let held = |key: &str, value: &str| kept[key].as_array().expect("entries").iter().any(|entry| entry["value"] == value);
    assert!(held("accepted", "Claude") && held("accepted", "claude,,pi") && held("accepted", "") && held("accepted", "claude, pi") && held("accepted", "NONE"));
    assert!(!held("accepted", "none,claude") && !held("accepted", "gemini") && held("refused", "gemini"));}

// ===========================================================================
// `canon-0-38-date-refusals`: a leap second is refused in every place that takes a date, and a date, a time or an offset that does not exist says so.

/// The call that gives `--option` the value `value` in `command`, any place that takes a date.
fn place_call(option: &str, command: &str, value: &str) -> Vec<String> {
    let given = format!("--{option}={value}");
    let ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let mut argv: Vec<&str> = match command {
        "evidence approval-payload" => vec!["evidence", "approval-payload", "--scenario", "auth.login.01", "--project", "root", "--approver", "alice"],
        "evidence add" => vec!["evidence", "add", "--project", "root", "--scenario", "auth.login.02", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--approver", "alice", "--approval-role", "human", "--signature-file", "s.sig"],
        "strategy approval-payload" => vec!["strategy", "approval-payload", ulid, "--evaluation", "evaluation.json", "--approver", "alice"],
        "strategy rollback" => vec!["strategy", "rollback", ulid, "--reason", "r", "--actor", "alice", "--signature-file", "s.sig"],
        _ => return date_call(option, command, value),
    };
    argv.push(&given);
    harness::strs(&argv)
}

/// Every `(option, command)` place that takes a date, with the call of a value there refused with `USAGE` for the option and nothing written; its message.
fn refused_in_every_place(value: &str) -> Vec<(String, String)> {
    refused_places(value).into_iter().map(|(_, command, message)| (command, message)).collect()
}

/// [`refused_in_every_place`] with the option of each place: `(option, command, message)`.
fn refused_places(value: &str) -> Vec<(String, String, String)> {
    let places = places();
    assert!(!places.is_empty());
    places
        .into_iter()
        .map(|(option, command)| {
            let argv = place_call(option, command, value);
            let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
            let got = call(&argv);
            assert_eq!((got.exit, got.code(), got.field()), (2, "USAGE", format!("--{option}").as_str()), "`canon {command} --{option} {value}`: {}", got.envelope);
            assert_eq!(got.envelope["error"]["received"], value, "`canon {command} --{option} {value}`: {}", got.envelope);
            assert!(got.changed.is_empty(), "`canon {command} --{option} {value}` wrote {:?}", got.changed);
            (option.to_string(), command.to_string(), got.envelope["error"]["message"].as_str().unwrap_or_default().to_string())
        })
        .collect()
}

#[test]
fn atip_daterefusals_01_a_leap_second_is_refused_in_every_place_and_the_second_before_and_the_next_day_are_taken() {
    for value in ["2030-01-01T23:59:60Z", "1998-12-31T23:59:60Z", "2030-01-01T10:00:60Z", "2030-01-01T23:59:60.5Z", "2030-01-01T23:59:60+01:00"] {
        refused_in_every_place(value);
    }
    // The refusal names the second after the preceding second, in UTC, in every place: the next midnight only after a `23:59:59Z` second.
    for (value, named) in [
        ("2030-01-01T23:59:60Z", "2030-01-02T00:00:00Z"),
        ("1998-12-31T23:59:60Z", "1999-01-01T00:00:00Z"),
        ("2030-01-01T10:00:60Z", "2030-01-01T10:01:00Z"),
        ("2030-01-01T23:59:60.5Z", "2030-01-02T00:00:00.500Z"),
        ("2030-01-01T23:59:60+01:00", "2030-01-01T23:00:00Z"),
    ] {
        for (command, message) in refused_in_every_place(value) {
            assert!(message.contains(named), "`{command}` {value}: {message}");
        }
    }
    // Past 9999-12-31 the following second has no spelling, and none is offered.
    for value in ["9999-12-31T23:59:60Z", "9999-12-31T23:59:60-01:00"] {
        refused_in_every_place(value);
    }
    for (option, commands) in DATE_OPTIONS {
        for command in commands {
            for value in ["2030-01-01T23:59:59Z", "2030-01-02T00:00:00Z"] {
                let argv = date_call(option, command, value);
                let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
                assert_taken(&argv, &format!("--{option}"));
            }
        }
    }
    for at in ["2030-01-01T23:59:59Z", "2030-01-02T00:00:00Z"] {
        assert_taken(&payload_args(at), "--approval-at");
    }
    // A date alone has no second.
    canon_cli::timestamp::parse("2030-01-01").expect("a date alone has no second");
}

#[test]
fn atip_daterefusals_02_a_date_a_time_or_an_offset_that_does_not_exist_is_refused_in_every_place_and_says_what_does_not_exist() {
    for value in ["2026-02-30T10:00:00Z", "2026-02-29T10:00:00Z", "2026-13-01T10:00:00Z", "2026-01-01T25:00:00Z", "2026-01-01T10:61:00Z", "2026-01-01T10:00:00+24:00"] {
        refused_in_every_place(value);
    }
    // The date options also take the date alone.
    for value in ["2026-02-30", "2026-02-29", "2026-04-31", "2026-13-01"] {
        for (option, commands) in DATE_OPTIONS {
            for command in commands {
                let argv = date_call(option, command, value);
                let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
                let got = call(&argv);
                assert_eq!((got.exit, got.code(), got.field()), (2, "USAGE", format!("--{option}").as_str()), "{}", got.envelope);
            }
        }
    }
    canon_cli::timestamp::parse("2028-02-29").expect("a day of the calendar");
}

#[test]
fn atip_daterefusals_03_a_misspelled_value_keeps_the_spelling_refusal_of_0_37() {
    for value in ["2026-2-30", "2026-02-30 10:00:00Z", "2030-1-1T10:00:00Z"] {
        refused_in_every_place(value);
    }
}
