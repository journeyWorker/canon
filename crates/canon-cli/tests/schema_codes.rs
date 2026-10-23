//! `canon schema <command>` lists the error codes the command can emit
//! (0.20 I1). The per-command table in `src/schema_codes.rs` is DERIVED from
//! the sources here, never written by hand:
//!
//! - every registered code literal in a source file is attributed to the
//!   command named by the `emit("canon <command>", …)` call that follows it
//!   (within [`EMIT_WINDOW`] lines);
//! - a code with no such call (a helper that builds the error for its caller)
//!   goes to the commands in [`FILE_DEFAULTS`] for its file;
//! - every command can emit the [`GLOBAL`] codes (`USAGE`, `UNKNOWN_ROLE`).
//!
//! The test fails on any difference between the derived table and the checked
//! in one, and on a source file that holds codes but is not accounted for, so
//! a new error site cannot be missing from `canon schema`.
//! `CANON_BLESS_SCHEMA_CODES=1` rewrites the table.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use canon_cli::cli_error::all_codes;
use canon_cli::schema::{error_codes, META};

/// How many lines after a code literal its `emit("canon …")` may sit.
const EMIT_WINDOW: usize = 6;

/// Files that are not error sites: the registry and the generated table, and
/// `context.rs`, whose one code (`CONFIG_MISSING`, built by `require_config`)
/// is emitted by `run::entry` for every command that needs canon state (see
/// [`needs_config_leaves`]).
const IGNORED: &[&str] = &["cli_error.rs", "schema_codes.rs", "context.rs"];

/// For each source file, the command prefixes that get a code the file builds
/// without an `emit("canon …")` next to it: helpers, `to_cli_error`
/// conversions and constants emitted by the caller. A prefix names a command
/// or a group (`brief` is `brief approve` and `brief check`).
const FILE_DEFAULTS: &[(&str, &[&str])] = &[
    // `store_error`: the refusal of a store that cannot be opened or attached, emitted by both commands.
    ("artifacts.rs", &["artifacts verify", "artifacts migrate"]),
    // `CHANGED_CODE`, picked by `evidence add`'s brief refusal.
    ("brief.rs", &["evidence add", "evidence audit"]),
    ("brief_approve.rs", &["brief approve"]),
    // `check` is shared by `brief check` and `brief approve`.
    ("brief_check.rs", &["brief check", "brief approve", "brief show"]),
    ("change.rs", &["change new"]),
    ("context_pack.rs", &["context-pack", "prompt"]),
    // The server's refusals (`to_cli_error`, the request guards) and the briefs page's.
    ("dashboard.rs", &["dashboard"]),
    ("dashboard_briefs.rs", &["dashboard"]),
    ("demo.rs", &["demo"]),
    ("dispatch.rs", &["dispatch begin", "dispatch end"]),
    ("divergence.rs", &["divergence"]),
    ("evidence.rs", &["evidence add", "evidence audit"]),
    ("finding.rs", &["finding add", "finding close"]),
    // The typed-atom errors `canon evidence add` reports with `gate task`'s grading;
    // `promote_after_stage`'s command comes from its caller.
    ("gate.rs", &["gate task", "evidence add", "evidence audit"]),
    ("init.rs", &["init"]),
    // `require_spec_root`: the refusal of a `--project-id` no configured spec root carries, made by every write command that takes one.
    ("inventory.rs", &["evidence add", "evidence audit", "evidence approval-payload", "review scenario", "review change", "divergence add", "divergence resolve", "divergence defer", "divergence accept", "scenario new"]),
    ("learn.rs", &["learn"]),
    ("ledger.rs", &["ledger compact"]),
    // Every code is next to its emit.
    ("main.rs", &[]),
    ("run/mod.rs", &[]),
    ("run/entry.rs", &[]),
    // `project_for`: `--project-id` is required when several spec roots exist, for each of these.
    ("run/divergence.rs", &["divergence add", "divergence resolve", "divergence defer", "divergence accept"]),
    ("run/evidence.rs", &["evidence add", "evidence audit", "evidence approval-payload"]),
    ("run/review.rs", &["review scenario", "review change"]),
    ("run/top.rs", &[]),
    ("run/adapter.rs", &[]),
    ("run/ingest.rs", &[]),
    ("run/inventory.rs", &[]),
    ("run/plugin.rs", &[]),
    ("run/skills.rs", &[]),
    ("run/tier.rs", &[]),
    ("migrate.rs", &["migrate"]),
    ("policy_approval.rs", &["brief approve", "policy approve"]),
    ("paging.rs", &["query", "finding list", "divergence status", "export"]),
    // `ScopeError::to_cli_error`, emitted by `query`.
    ("query.rs", &["query"]),
    ("retention.rs", &["export", "purge"]),
    ("review.rs", &["review scenario", "review change"]),
    // Every code is next to its `human("canon selftest")`.
    ("selftest.rs", &[]),
    // The shared scenario validation (`Planner`) builds its errors for its caller, `scenario new`.
    ("scaffold.rs", &["scenario new"]),
    // `UNKNOWN_COMMAND` is next to its `emit("canon schema", …)`.
    ("schema.rs", &[]),
    ("status.rs", &["status"]),
    ("subject.rs", &["subject"]),
];

/// An `emit(&format!("canon <prefix> {verb}"), …)` shared by several commands:
/// the commands that reach it.
const TEMPLATED: &[(&str, &[&str])] = &[("divergence", &["divergence resolve", "divergence defer"])];

/// A code spliced into a command's own JSON document (`to_json()`), with no
/// `emit` to name its command (or a helper whose caller emits it for one
/// command only): the command that prints it.
const SPLICED: &[(&str, &str)] = &[("INGEST_SOURCE_UNUSABLE", "ingest plans"), ("FINDING_UNKNOWN_SCENARIO", "finding add")];

/// Codes every command can emit, whatever its sources say: a `--json` usage
/// error, and the refusal of an unknown `CANON_ROLE` before any command runs.
const GLOBAL: &[&str] = &["USAGE", "UNKNOWN_ROLE"];

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// A file's non-test, non-comment lines.
fn code_lines(file: &Path) -> Vec<String> {
    let text = fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
    let live = text.split("#[cfg(test)]").next().expect("split yields one piece");
    live.lines().map(|l| if l.trim_start().starts_with("//") { String::new() } else { l.to_string() }).collect()
}

/// The command a line's `emit("canon <command> …"` or
/// `emit(&format!("canon <command> {verb}"` names, as the words before the
/// first flag or placeholder; a placeholder is flagged by a trailing `{`.
/// `Some("")` is the bare `canon` (usage), and `Some("<caller>")` an emit whose
/// command the caller passes.
fn emitted_command(line: &str) -> Option<String> {
    if line.contains(".emit(command,") {
        return Some("<caller>".to_string());
    }
    let start = ["emit(\"canon", "emit(&format!(\"canon", "emit_with_data(\"canon", "human(\"canon"].iter().find_map(|p| line.find(p))?;
    let rest = &line[start..];
    let quoted = &rest[rest.find("\"canon")? + 1..];
    let text = &quoted[..quoted.find('"')?];
    let words: Vec<&str> = text.split(' ').skip(1).take_while(|w| !w.starts_with('-') && !w.starts_with('{')).collect();
    let templated = text.contains('{');
    Some(format!("{}{}", words.join(" "), if templated { " {" } else { "" }))
}

fn leaves_under(prefix: &str) -> Vec<&'static str> {
    let found: Vec<&'static str> = META.iter().map(|m| m.path).filter(|p| *p == prefix || p.starts_with(&format!("{prefix} "))).collect();
    assert!(!found.is_empty(), "`{prefix}` names no command");
    found
}

/// The leaves `run::entry` refuses with `CONFIG_MISSING` when no `canon.yaml` is
/// found: those with a `--repo` option that are not in
/// `canon_cli::context::CONFIG_EXEMPT`. A leaf's options come from its own
/// `canon schema` output, so a new `--repo` option is picked up by itself.
fn needs_config_leaves() -> Vec<&'static str> {
    META.iter()
        .map(|m| m.path)
        .filter(|path| canon_cli::context::needs_config(path))
        .filter(|path| {
            let mut args: Vec<&str> = vec!["schema"];
            args.extend(path.split(' '));
            args.push("--json");
            let out = std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(&args).output().expect("spawn canon");
            let body: serde_json::Value = serde_json::from_slice(&out.stdout).expect("schema is JSON");
            body["data"]["options"].as_array().expect("options").iter().any(|o| o["name"] == "repo")
        })
        .collect()
}

fn derive() -> BTreeMap<&'static str, BTreeSet<&'static str>> {
    let mut table: BTreeMap<&'static str, BTreeSet<&'static str>> = META.iter().map(|m| (m.path, GLOBAL.iter().copied().collect())).collect();
    let codes: Vec<&'static str> = all_codes().iter().map(|info| info.code).filter(|c| !GLOBAL.contains(c)).collect();
    let defaults: BTreeMap<&str, &[&str]> = FILE_DEFAULTS.iter().copied().collect();
    let mut problems = Vec::new();
    let mut files: Vec<PathBuf> = ["", "run"]
        .iter()
        .flat_map(|dir| fs::read_dir(src_dir().join(dir)).expect("read src").map(|e| e.expect("dir entry").path()).filter(|p| p.extension().is_some_and(|e| e == "rs")).collect::<Vec<_>>())
        .collect();
    files.sort();
    let mut promoters = BTreeSet::new();
    for file in files {
        let name = file.strip_prefix(src_dir()).expect("under src").to_str().expect("utf-8 name").to_string();
        if IGNORED.contains(&name.as_str()) {
            continue;
        }
        let lines = code_lines(&file);
        // `gate::promote_after_stage(command, …)` emits `PROMOTE_FAILED` under
        // the command its caller passes.
        for line in &lines {
            if let Some(i) = line.find("promote_after_stage(\"canon ") {
                let rest = &line[i + "promote_after_stage(\"".len()..];
                promoters.insert(rest[..rest.find('"').expect("closing quote")].trim_start_matches("canon ").to_string());
            }
        }
        let mut file_has_codes = false;
        for (i, line) in lines.iter().enumerate() {
            for code in codes.iter().filter(|c| line.contains(&format!("\"{c}\""))) {
                file_has_codes = true;
                let window = &lines[i..(i + 1 + EMIT_WINDOW).min(lines.len())];
                let spliced = SPLICED.iter().find(|(c, _)| c == code).map(|(_, command)| command.to_string());
                let targets: Vec<&'static str> = match window.iter().find_map(|l| emitted_command(l)).or(spliced) {
                    // Bare `canon` is usage, every command has it; a caller-supplied
                    // command is covered by the `promote_after_stage` rule below.
                    Some(command) if command.is_empty() || command == "<caller>" => continue,
                    Some(command) => match command.strip_suffix(" {") {
                        Some(prefix) => {
                            let verbs = TEMPLATED.iter().find(|(p, _)| *p == prefix).unwrap_or_else(|| panic!("{name}:{}: templated emit under `{prefix}` needs a TEMPLATED entry", i + 1));
                            verbs.1.iter().flat_map(|c| leaves_under(c)).collect()
                        }
                        None => leaves_under(&command),
                    },
                    None => match defaults.get(name.as_str()) {
                        Some(prefixes) => prefixes.iter().flat_map(|p| leaves_under(p)).collect(),
                        None => {
                            problems.push(format!("{name}:{}: `{code}` has no `emit(\"canon …\")` within {EMIT_WINDOW} lines and the file has no FILE_DEFAULTS entry", i + 1));
                            continue;
                        }
                    },
                };
                if targets.is_empty() {
                    problems.push(format!("{name}:{}: `{code}` is attributed to no command", i + 1));
                }
                for target in targets {
                    table.get_mut(target).expect("leaf has a row").insert(code);
                }
            }
        }
        if file_has_codes && !defaults.contains_key(name.as_str()) {
            problems.push(format!("{name} builds error codes but has no FILE_DEFAULTS entry (use an empty list when every code sits next to its emit)"));
        }
    }
    for leaf in needs_config_leaves() {
        table.get_mut(leaf).expect("leaf has a row").insert("CONFIG_MISSING");
    }
    for command in promoters {
        for leaf in leaves_under(&command) {
            table.get_mut(leaf).expect("leaf has a row").insert("PROMOTE_FAILED");
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    table
}

fn render(table: &BTreeMap<&str, BTreeSet<&str>>) -> String {
    let mut out = String::from(
        "//! GENERATED by `tests/schema_codes.rs` (`CANON_BLESS_SCHEMA_CODES=1 cargo test -p canon-cli --test schema_codes`): do not edit.\n\n#[rustfmt::skip]\npub(crate) const ERROR_CODES: &[(&str, &[&str])] = &[\n",
    );
    for (path, codes) in table {
        let list = codes.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        out.push_str(&format!("    (\"{path}\", &[{list}]),\n"));
    }
    out.push_str("];\n");
    out
}

#[test]
fn the_schema_error_codes_are_the_codes_the_sources_emit() {
    let table = derive();
    let rendered = render(&table);
    let checked_in = src_dir().join("schema_codes.rs");
    if std::env::var_os("CANON_BLESS_SCHEMA_CODES").is_some() {
        fs::write(&checked_in, &rendered).expect("write schema_codes.rs");
        return;
    }
    for (path, codes) in &table {
        let listed: BTreeSet<&str> = error_codes(path).into_iter().collect();
        let missing: Vec<&&str> = codes.difference(&listed).collect();
        let stale: Vec<&&str> = listed.difference(codes).collect();
        assert!(missing.is_empty() && stale.is_empty(), "`canon schema {path}`: error codes missing {missing:?}, no longer emitted {stale:?}; run CANON_BLESS_SCHEMA_CODES=1 cargo test -p canon-cli --test schema_codes");
    }
    assert_eq!(fs::read_to_string(&checked_in).expect("read schema_codes.rs"), rendered, "schema_codes.rs is out of date; re-bless it");
}

#[test]
fn every_registered_code_is_emitted_by_some_command() {
    let table = derive();
    let emitted: BTreeSet<&str> = table.values().flatten().copied().collect();
    let orphans: Vec<&str> = all_codes().iter().map(|info| info.code).filter(|c| !emitted.contains(c)).collect();
    assert!(orphans.is_empty(), "registered but attributed to no command: {orphans:?}");
}
