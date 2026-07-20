//! Hook-seam wiring (design decision 8, D8; spec `gated-task-completion`
//! "Hook-seam wiring generation"). [`install_hooks`] is the idempotent,
//! diff-only JSON-merge LOGIC `canon gate install-hooks` (S5 wave-2 task
//! 4.1, a canon-cli concern outside this crate's territory) will call
//! against a real `.claude/settings.json` / `.codex/hooks.json`;
//! [`PRE_COMMIT_SCRIPT`] is the static, portable pre-commit hook for
//! repos with no donor-CLI `hook run <kind>` wiring at all (task 4.2). This
//! module never reads or writes a real file — every function here is a
//! pure [`serde_json::Value`] transform, tested against constructed
//! fixtures — matching the constraint that THIS change ships only the
//! seam and the script; wiring the internal monorepo's own `.claude/settings.json`/
//! `.codex/hooks.json` (task 4.3) is a documented, separate follow-up
//! (design.md decision 8 / Migration Plan step 2).
//!
//! # The reused wiring shape (design decision 8)
//! `{matcher?, hooks: [{type: "command", command, timeout}]}` — the
//! IDENTICAL top-level `{"hooks": {"<Event>": [<group>, ...]}}` layout
//! already used by both a `.claude/settings.json` and a
//! `.codex/hooks.json` (verified against both files, 2026-07-11;
//! e.g. `.claude/settings.json`'s `PreToolUse` → `[{matcher: "Bash",
//! hooks: [{type: "command", command: "other-cli hook run
//! pre-bash-guard", timeout: 5}]}]`) — [`install_hooks`]
//! reuses this SHAPE, not any donor-CLI-specific wiring; it never removes or
//! reorders an existing entry, only appends (module doc's "additive;
//! does not remove existing third-party entries", design.md Migration Plan
//! step 2).
//!
//! # donor-CLI migration-target boundary (design decision 7, task 4.4)
//! the donor CLI's task-flip and gate-markers modules are NOT touched by this
//! change and are not even reachable from this worktree (canon is a
//! standalone repo; those files live in the internal monorepo's own tree). A SEPARATE,
//! donor-CLI-side change swaps their callers to shell out to `canon gate
//! task` — this crate ships the target capability, not the migration
//! (design.md decision 7's own text).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The generic pre-commit script (module doc) — embedded verbatim from
/// `scripts/canon-gate-pre-commit.sh` so the shipped file and this
/// constant can never drift out of sync.
pub const PRE_COMMIT_SCRIPT: &str = include_str!("../scripts/canon-gate-pre-commit.sh");

/// One hook-seam entry to install — the `{matcher?, hooks: [{type,
/// command, timeout}]}` shape's per-command half (module doc).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HookEntry {
    /// The Claude-Code/Codex hook event name (`"PreToolUse"`,
    /// `"Stop"`, ...) — this module does not constrain it to a closed
    /// set; the two consumer schemas already share an open event
    /// vocabulary (module doc citation).
    pub event: String,
    /// `None` for matcher-less events (`Stop`/`UserPromptSubmit`/
    /// `SessionStart` in the cited real files carry no `matcher` key
    /// at all — `None` here must round-trip to an ABSENT key, never a
    /// JSON `null`, matching that convention exactly).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    /// The shell command this hook invokes (e.g. `"canon gate task"`).
    pub command: String,
    pub timeout: u32,
}

impl HookEntry {
    pub fn new(event: impl Into<String>, matcher: Option<String>, command: impl Into<String>, timeout: u32) -> Self {
        Self { event: event.into(), matcher, command: command.into(), timeout }
    }
}

/// Whether [`install_hooks`] changed `settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    /// The entry was already present — `settings` is untouched
    /// (spec.md "Installing hooks is idempotent": "the second run
    /// reports no diff and writes nothing").
    Unchanged,
    /// The entry was appended.
    Installed,
}

/// Idempotent, diff-only merge of `entry` into `settings` (design
/// decision 8; spec.md "Hook-seam wiring generation" + "Installing
/// hooks is idempotent"). Mutates `settings` in place, normalizing a
/// non-object root / missing `hooks` key / missing event array to an
/// empty shape rather than erroring — a fresh repo with no
/// `.claude/settings.json` content at all installs cleanly on the
/// first call.
///
/// Merge rule, in order:
/// 1. Find an existing group under `settings.hooks[entry.event]` whose
///    `matcher` matches `entry.matcher` EXACTLY (`None` only matches an
///    absent/`null` `matcher` key — never treated as a wildcard).
/// 2. If found and it already carries a `hooks[].command ==
///    entry.command` entry → [`InstallOutcome::Unchanged`], nothing
///    written.
/// 3. If found but missing that command → APPEND `{type: "command",
///    command, timeout}` to that group's own `hooks` array (every
///    other command in the group, third-party entries included, is
///    left byte-for-byte in place).
/// 4. If no group with that matcher exists for the event → APPEND a
///    brand-new group to the event's array (never replaces or reorders
///    an existing group with a DIFFERENT matcher).
pub fn install_hooks(settings: &mut Value, entry: &HookEntry) -> InstallOutcome {
    if !settings.is_object() {
        *settings = json!({});
    }
    let root = settings.as_object_mut().expect("normalized to an object above");

    let hooks = root.entry("hooks".to_string()).or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks_obj = hooks.as_object_mut().expect("normalized to an object above");

    let event_array = hooks_obj.entry(entry.event.clone()).or_insert_with(|| json!([]));
    if !event_array.is_array() {
        *event_array = json!([]);
    }
    let event_array = event_array.as_array_mut().expect("normalized to an array above");

    for group in event_array.iter_mut() {
        let group_matcher = group.get("matcher").and_then(Value::as_str);
        if group_matcher != entry.matcher.as_deref() {
            continue;
        }
        let Some(group_obj) = group.as_object_mut() else { continue };
        let hooks_list = group_obj.entry("hooks".to_string()).or_insert_with(|| json!([]));
        if !hooks_list.is_array() {
            *hooks_list = json!([]);
        }
        let hooks_list = hooks_list.as_array_mut().expect("normalized to an array above");

        let already_present = hooks_list.iter().any(|h| h.get("command").and_then(Value::as_str) == Some(entry.command.as_str()));
        if already_present {
            return InstallOutcome::Unchanged;
        }

        hooks_list.push(json!({ "type": "command", "command": entry.command, "timeout": entry.timeout }));
        return InstallOutcome::Installed;
    }

    let mut new_group = serde_json::Map::new();
    if let Some(matcher) = &entry.matcher {
        new_group.insert("matcher".to_string(), json!(matcher));
    }
    new_group.insert("hooks".to_string(), json!([{ "type": "command", "command": entry.command, "timeout": entry.timeout }]));
    event_array.push(Value::Object(new_group));

    InstallOutcome::Installed
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::process::{Command, Stdio};

    use super::*;

    fn bash_entry() -> HookEntry {
        HookEntry::new("PreToolUse", Some("Bash".to_string()), "canon gate task", 5)
    }

    // ── install_hooks: idempotent, diff-only (spec.md "Installing hooks is idempotent") ──

    #[test]
    fn installs_into_an_empty_settings_object() {
        let mut settings = json!({});
        let outcome = install_hooks(&mut settings, &bash_entry());
        assert_eq!(outcome, InstallOutcome::Installed);
        assert_eq!(
            settings,
            json!({
                "hooks": {
                    "PreToolUse": [
                        { "matcher": "Bash", "hooks": [{ "type": "command", "command": "canon gate task", "timeout": 5 }] }
                    ]
                }
            })
        );
    }

    #[test]
    fn a_second_install_of_the_same_entry_is_a_no_op() {
        let mut settings = json!({});
        install_hooks(&mut settings, &bash_entry());
        let after_first = settings.clone();

        let outcome = install_hooks(&mut settings, &bash_entry());

        assert_eq!(outcome, InstallOutcome::Unchanged);
        assert_eq!(settings, after_first, "a second install must write nothing");
    }

    #[test]
    fn installs_additively_alongside_an_existing_third_party_entry_same_matcher() {
        let mut settings = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "other-cli hook run pre-bash-guard", "timeout": 5 }] }
                ]
            }
        });

        let outcome = install_hooks(&mut settings, &bash_entry());

        assert_eq!(outcome, InstallOutcome::Installed);
        let group = &settings["hooks"]["PreToolUse"][0];
        let commands: Vec<&str> = group["hooks"].as_array().unwrap().iter().map(|h| h["command"].as_str().unwrap()).collect();
        assert_eq!(commands, vec!["other-cli hook run pre-bash-guard", "canon gate task"], "the existing third-party entry stays, ours is appended");
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1, "same matcher merges into the SAME group, not a second one");
    }

    #[test]
    fn a_different_matcher_gets_its_own_new_group_not_merged() {
        let mut settings = json!({
            "hooks": {
                "PostToolUse": [
                    { "matcher": "Edit|Write|MultiEdit|NotebookEdit", "hooks": [{ "type": "command", "command": "other-cli hook run post-edit-guard", "timeout": 5 }] }
                ]
            }
        });
        let entry = HookEntry::new("PostToolUse", Some("Bash".to_string()), "canon gate task", 5);

        install_hooks(&mut settings, &entry);

        let groups = settings["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(groups.len(), 2, "a different matcher is a SEPARATE group, never merged into an unrelated one");
        assert_eq!(groups[0]["matcher"], "Edit|Write|MultiEdit|NotebookEdit");
        assert_eq!(groups[1]["matcher"], "Bash");
    }

    #[test]
    fn a_matcher_less_event_round_trips_with_no_matcher_key_at_all() {
        let mut settings = json!({});
        let entry = HookEntry::new("Stop", None, "canon gate task", 5);

        install_hooks(&mut settings, &entry);

        let group = &settings["hooks"]["Stop"][0];
        assert!(group.get("matcher").is_none(), "matcher-less entries must never gain a null/absent-but-present matcher key");
    }

    #[test]
    fn other_events_are_left_completely_untouched() {
        let mut settings = json!({
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "other-cli hook run gate-stop", "timeout": 5 }] }] },
            "env": { "SOME_VAR": "1" }
        });
        let before_stop = settings["hooks"]["Stop"].clone();
        let before_env = settings["env"].clone();

        install_hooks(&mut settings, &bash_entry());

        assert_eq!(settings["hooks"]["Stop"], before_stop);
        assert_eq!(settings["env"], before_env);
    }

    // ── PRE_COMMIT_SCRIPT: actually exercised via `sh`, not just string assertions ──

    fn write_stub_canon(dir: &std::path::Path, exit_code: i32) {
        let path = dir.join("canon");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, "#!/bin/sh\nexit {exit_code}").unwrap();
        file.flush().unwrap();
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn run_script(path_dir: Option<&std::path::Path>, advisory: Option<&str>) -> std::process::ExitStatus {
        let script_dir = std::env::temp_dir().join(format!("canon-gate-pre-commit-test-{}", std::process::id()));
        fs::create_dir_all(&script_dir).unwrap();
        let script_path = script_dir.join("canon-gate-pre-commit.sh");
        fs::write(&script_path, PRE_COMMIT_SCRIPT).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut cmd = Command::new("/bin/sh");
        cmd.arg(&script_path);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

        let path_var = match path_dir {
            Some(dir) => format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default()),
            None => "/nonexistent-canon-gate-test-path".to_string(),
        };
        cmd.env("PATH", path_var);

        match advisory {
            Some(v) => {
                cmd.env("CANON_GATE_ADVISORY", v);
            }
            None => {
                cmd.env_remove("CANON_GATE_ADVISORY");
            }
        }

        cmd.status().expect("sh must be available to run the pre-commit script")
    }

    #[test]
    fn exits_zero_when_canon_is_not_installed() {
        let status = run_script(None, None);
        assert!(status.success(), "a missing canon binary must never block a commit (fail-soft, §7)");
    }

    #[test]
    fn advisory_mode_never_blocks_even_when_the_gate_fails() {
        let dir = tempfile::tempdir().unwrap();
        write_stub_canon(dir.path(), 1);
        let status = run_script(Some(dir.path()), None); // default is advisory (1)
        assert!(status.success(), "CANON_GATE_ADVISORY defaults to 1 — a gate failure must not block the commit");
    }

    #[test]
    fn blocking_mode_fails_the_commit_when_the_gate_fails() {
        let dir = tempfile::tempdir().unwrap();
        write_stub_canon(dir.path(), 1);
        let status = run_script(Some(dir.path()), Some("0"));
        assert!(!status.success(), "CANON_GATE_ADVISORY=0 must propagate a gate failure");
    }

    #[test]
    fn a_passing_gate_always_exits_zero() {
        let dir = tempfile::tempdir().unwrap();
        write_stub_canon(dir.path(), 0);
        let status = run_script(Some(dir.path()), Some("0"));
        assert!(status.success());
    }
}
