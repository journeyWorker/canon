//! The "Removed in 0.28" table: every 0.27 spelling (a command, a bare group
//! that used to run, an option) with the 0.28 spelling that replaced it.
//!
//! One data table, three consumers: [`crate::removed`] reads it to turn an old
//! spelling into a usage error (exit 2, `error.hint` names the replacement,
//! `error.expected` lists it), [`crate::spec`] emits it as the spec's `removed`
//! section, and the README's Upgrading list is generated from that section.
//! It mirrors the 0.27 → 0.28 sections of `docs/design/cli-commands-0.28.md`;
//! the 0.24 option renames the 0.25 hints already covered stay here too
//! (`since`), so one lookup answers every removed spelling.

/// What kind of spelling a row removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A command (a leaf, or a group word): `old` is the command path.
    Command,
    /// A group that ran bare in 0.27 and is now a usage error without a subcommand.
    Bare,
    /// An option (`--name` or `-n`) typed on one of `scope`'s commands.
    Option,
}

/// One removed spelling and its replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub kind: Kind,
    /// The removed spelling: a command path (`gate task`), a group (`brief`), or an option (`--agent-id`, `-k`).
    pub old: &'static str,
    /// Command paths (without `canon`) the removed option was typed on; empty for commands.
    pub scope: &'static [&'static str],
    /// The 0.28 spelling that replaces it. A command is written without `canon`; it is what the hint tells the caller to type.
    pub new: &'static str,
    /// What else changed, one clause (empty when `new` says it all).
    pub note: &'static str,
    /// The decision that made the change (the table's Rule column).
    pub rule: &'static str,
    /// The release that removed the spelling.
    pub since: &'static str,
    /// An option row only: the option's value becomes the operand of the replacement command, so the hint can name
    /// the command the caller typed with the value filled in (see [`crate::removed`]).
    pub operand: bool,
}

const V28: &str = "0.28.0";
const V24: &str = "0.24.0";

const fn command(old: &'static str, new: &'static str, note: &'static str, rule: &'static str) -> Row {
    Row { kind: Kind::Command, old, scope: &[], new, note, rule, since: V28, operand: false }
}

const fn bare(old: &'static str, new: &'static str, note: &'static str, rule: &'static str) -> Row {
    Row { kind: Kind::Bare, old, scope: &[], new, note, rule, since: V28, operand: false }
}

const fn option(old: &'static str, scope: &'static [&'static str], new: &'static str, note: &'static str, rule: &'static str) -> Row {
    Row { kind: Kind::Option, old, scope, new, note, rule, since: V28, operand: false }
}

/// An option whose value moved into the operand of `new` (or of the leaf it was typed on, when `scope` holds `new`).
const fn operand(old: &'static str, scope: &'static [&'static str], new: &'static str, note: &'static str, rule: &'static str) -> Row {
    Row { kind: Kind::Option, old, scope, new, note, rule, since: V28, operand: true }
}

/// An option 0.24 renamed; 0.25 stopped parsing the old name.
const fn renamed_024(old: &'static str, scope: &'static [&'static str], new: &'static str) -> Row {
    Row { kind: Kind::Option, old, scope, new, note: "", rule: "one name per concept", since: V24, operand: false }
}

const CHANGE_OPERAND: &[&str] = &["brief show", "brief check", "brief approve"];
const SCENARIO_OPERAND: &[&str] = &["divergence add", "divergence defer", "divergence resolve", "divergence accept"];
const REVIEWER: &[&str] = &["divergence add", "divergence defer", "divergence resolve", "divergence accept", "finding add", "review scenario", "review change"];
const SKILLS: &[&str] = &["skills install", "skills check", "skills doctor"];
const TASK: &[&str] = &["dispatch begin", "evidence add", "evidence approval-payload"];
const DESTRUCTIVE: &[&str] = &["artifacts move", "ledger compact", "migrate", "purge", "tier age"];

/// Every removed spelling, in the order of the table's 0.27 → 0.28 sections.
/// `new` is a spelling (a command path with its option names), never a command
/// line with placeholders: hints stay free of `<PLACEHOLDER>` and ellipses (T14),
/// and `note` says in words what an operand is.
pub const ROWS: &[Row] = &[
    // ── Commands ──
    command("artifacts migrate", "artifacts move --tier", "the bare run previews; `--commit --plan-digest` acts", "D6, D1 Renames"),
    bare("brief", "brief show", "a group is never runnable; the change is an optional operand", "D2"),
    command("context-pack new", "context-pack add --manifest", "", "D1 Renames"),
    command("demo add", "demo review", "", "D1 Renames"),
    command("divergence promote", "gate promote --kind divergence", "", "D1 Renames"),
    command("divergence status", "divergence list", "", "D1 Renames"),
    command("evidence vault", "artifacts store", "", "D1 Renames"),
    command("fmt", "format", "the `fmt` alias is gone; the directory is an optional operand", "D8, D12"),
    command("gate selftest", "selftest --suite trust-spine-gate", "", "D1 Renames"),
    command("gate task", "task done", "the task id is the operand; installed hooks are rewritten by `canon migrate`", "D1 Renames"),
    command("ingest artifacts", "ingest records", "\"artifact\" now means the blob store only", "D1 Renames"),
    command("learn promote", "strategy promote", "the strategy id is the operand", "D1 Renames"),
    command("learn approve", "strategy approval-payload --evaluation --approver --approval-at", "the strategy id is the operand; `approve` is reserved for commands that record a human approval", "D1 Renames"),
    command("learn rollback", "strategy rollback", "the strategy id is the operand", "D1 Renames"),
    command("regime-key", "regime show --role-segment --repo-name --area --hash", "", "D1 Renames, D5"),
    bare("report", "report build", "`report check` and `report snapshot` are the other two modes", "D1 Renames"),
    command("review add", "review scenario", "split by what it reviews: `review change` reviews a chore change", "D1 Renames"),
    command("subject status", "subject show", "`subject move --to` changes the state", "D1 Renames"),
    // ── Modes that became leaves ──
    option("--check", &["report"], "report check", "", "D1 Renames"),
    operand("--snapshot", &["report"], "report snapshot", "the output directory is the operand", "D1 Renames"),
    option("--check", &["format"], "format", "`format` always validates; drop the flag", "D12"),
    option("--check-config", &["init"], "--check", "", "D1 Renames"),
    // ── Primary object moved into the operand ──
    operand("--change-id", CHANGE_OPERAND, "brief show", "the change is the operand", "D1 r2"),
    operand("--change", CHANGE_OPERAND, "brief show", "the change is the operand", "D1 r2"),
    operand("--change-id", &["finding close"], "finding close", "the change is the operand", "D1 r2"),
    operand("--run-id", &["dispatch end"], "dispatch end", "the run is the operand", "D1 r1"),
    operand("--subject-id", &["subject adopt", "subject unadopt"], "subject adopt", "the subject is the operand and the change is `--change-id`", "D1 r1"),
    operand("--name", &["prompt add", "prompt show"], "prompt add", "name and version are one operand, name@version", "D1 r1, D12"),
    operand("--version", &["prompt add", "prompt show"], "prompt add", "name and version are one operand, name@version", "D1 r1, D12"),
    operand("--scenario-id", SCENARIO_OPERAND, "divergence add", "the scenario is the operand", "D1 r2"),
    operand("--scenario-id", &["review scenario"], "review scenario", "the scenarios are operands", "D1 r2"),
    option("--scenario-ids-from", &["review scenario"], "--input", "a batch is an `--input` array", "D7"),
    operand("--change-id", &["review scenario"], "review change", "a chore change is reviewed with `review change`", "D1 Renames"),
    operand("--scenario-id", &["review change"], "review scenario", "scenarios are reviewed with `review scenario`", "D1 Renames"),
    option("--scenario-ids-from", &["review change"], "review scenario", "scenarios are reviewed with `review scenario`", "D1 Renames"),
    operand("--change-id", &["review change"], "review change", "the change is the operand", "D1 r2"),
    option("--from", &["scenario new"], "--input", "a batch is an `--input` array with `operands` per element", "D7"),
    option("--all", &["schema"], "schema", "the listing is unfiltered; `--as` filters it by role", "D3"),
    // ── Attribution and approval options ──
    option("--agent-id", &["dispatch begin"], "--actor-id", "", "D1 Renames"),
    option("--reviewer", REVIEWER, "--actor-id", "it is the reviewer, and required where `--reviewer` was", "D1 Renames, O9"),
    option("--by", &["brief approve", "policy approve"], "--approver", "", "D1 Renames"),
    option("--principal", &["strategy approval-payload"], "--approver", "", "D1 Renames"),
    option("--approval-by", &["evidence add", "evidence approval-payload"], "--approver", "", "D1 Renames"),
    option("--approved-at", &["strategy rollback"], "--approval-at", "", "D1 Renames"),
    option("--signature", &["strategy promote"], "--signature-file", "", "D1 Renames"),
    option("--approval-signature-file", &["evidence add"], "--signature-file", "", "D1 Renames"),
    option("--context-manifest", &["dispatch begin"], "--manifest", "", "D1 Renames"),
    // ── Commit options (0.28 D7b) ──
    option("--sha", &["divergence add", "divergence defer", "divergence resolve"], "--rev", "the commit the record is about; default HEAD", "D1 Renames, D7b"),
    option("--pin", &["review scenario", "review change"], "--rev", "default HEAD", "D1 Renames, D7b"),
    option("--artifact-sha", &["evidence add", "evidence audit", "evidence approval-payload"], "--rev", "default HEAD", "D1 Renames, D7b"),
    option("--reviewed-sha", &["finding add"], "--rev", "default HEAD", "D7b"),
    option("--resolution-sha", &["finding add", "finding close"], "--resolution-rev", "default HEAD when fixed", "D7b"),
    option("--introduced-by", &["finding add"], "--introduced-rev", "no default", "D7b"),
    // ── Value-named and shared options ──
    option("--out", &["export"], "--output", "`-o` is the short form", "D1 Renames"),
    option("-k", &["retrieve"], "--limit", "", "D1 Renames"),
    option("--k", &["retrieve"], "--limit", "", "D1 Renames"),
    option("--role", &["retrieve"], "--for-role", "`--role` means only attribution", "D12"),
    option("--kind", &["evidence add"], "--class", "an external re-audit is `evidence audit`, not `--class audit`", "D1 Renames, Q3"),
    option("--kind", &["change new"], "--change-kind", "`--kind` is reserved for record kinds", "D12"),
    option("--status", &["dispatch end"], "--run-outcome", "", "D1 Renames, D12"),
    option("--status", &["divergence add"], "--state", "", "D1 Renames, D12"),
    option("--outcome", &["review scenario", "review change"], "--review-outcome", "", "D12"),
    option("--task", TASK, "--task-id", "", "D1 Renames"),
    option("--plugin", &["query"], "--plugin-id", "", "D1 Renames"),
    option("--parent-run", &["dispatch begin"], "--parent-run-id", "", "D1 Renames"),
    option("--source", SKILLS, "--skills-source", "", "D12"),
    option("--source", &["ingest plans"], "--plan-source", "", "D1 Renames, D12"),
    option("--role", &["regime show"], "--role-segment", "", "D1 Renames, D5"),
    option("--repo", &["regime show"], "--repo-name", "the repo segment of the key; `--repo` names a directory on other commands", "D1 Renames, D5"),
    option("--to", &["artifacts move"], "--tier", "", "D1 Renames"),
    option("--target", SKILLS, "--repo", "the repo is `--repo` on every command; omit it to use the nearest directory holding canon.yaml", "D5"),
    option("--canon-yaml", &["ingest sessions", "query", "tier age"], "--repo", "the repo's canon.yaml is found from `--repo`, or from the nearest directory holding one when it is omitted", "D5"),
    // ── Destructive leaves preview by default ──
    option("--dry-run", DESTRUCTIVE, "--commit --plan-digest", "the bare run previews; `--commit --plan-digest` acts", "D6"),
    option("--plan", &["purge"], "purge", "the bare run previews; `--commit --plan-digest` acts", "D6"),
    option("--confirm", &["purge"], "--commit --plan-digest", "the bare run previews; `--commit --plan-digest` acts", "D6"),
    // ── Evidence re-audit moved to `evidence audit` ──
    option("--evaluator-id", &["evidence add"], "evidence audit", "an external evaluator's re-audit is its own command", "Q3"),
    option("--evaluator-version", &["evidence add"], "evidence audit", "an external evaluator's re-audit is its own command", "Q3"),
    option("--evaluator-digest", &["evidence add"], "evidence audit", "an external evaluator's re-audit is its own command", "Q3"),
    option("--audit-result", &["evidence add"], "evidence audit", "an external evaluator's re-audit is its own command", "Q3"),
    // ── 0.24 renames (removed in 0.25) ──
    renamed_024("--change", &["scenario new"], "--change-id"),
    renamed_024("--project", &["feature new", "scenario new"], "--project-id"),
    renamed_024("--subject", &["change new", "retrieve", "scenario new"], "--subject-id"),
    renamed_024("--actor", &["feature new", "strategy rollback", "scenario new"], "--actor-id"),
    renamed_024("--run", &["ingest records"], "--run-id"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_names_what_replaces_it() {
        for row in ROWS {
            assert!(!row.old.is_empty() && !row.new.is_empty(), "{row:?}");
            assert_eq!(row.kind == Kind::Option, !row.scope.is_empty(), "an option row names its commands, a command row none: {row:?}");
            assert!(row.old != row.new || row.kind == Kind::Option, "{row:?}");
            assert!(![row.new, row.note].iter().any(|text| text.contains("...") || text.contains('\u{2026}') || text.contains('<')), "a hint carries no placeholder or ellipsis: {row:?}");
        }
    }

    #[test]
    fn no_row_is_listed_twice() {
        for (i, a) in ROWS.iter().enumerate() {
            for b in &ROWS[i + 1..] {
                let overlap = a.kind == b.kind && a.old == b.old && (a.kind != Kind::Option || a.scope.iter().any(|s| b.scope.contains(s)));
                assert!(!overlap, "{a:?} and {b:?} remove the same spelling");
            }
        }
    }
}
