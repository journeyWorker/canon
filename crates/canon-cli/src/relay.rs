//! Relayed approvals (0.32 `relay-approval`): an agent records the answer a
//! human gave in the conversation.
//!
//! `brief approve`, `policy approve`, `divergence accept` and the re-binding
//! `migrate --commit` are the human's decisions, so a declared agent role is
//! refused (`HUMAN_ONLY`). With `--relayed-via <tool>` (the tool the human was
//! asked with) the agent records the human's answer for them, and the record
//! says so: the agent's role and actor and the tool. Canon cannot tell that
//! the human really answered; the marker and the answer on the record are the
//! whole guarantee, and they are what a reader later tells a relayed approval
//! from the human's own by.
//!
//! The marker in an `Approved:` line sits before the digest and outside it:
//! `Approved: <who> <when> — "<words>" (relayed by <role> <actor> via <tool>) (brief sha256:<digest>)`.
//! A line without it is the human's own and reads as before.
//!
//! `canon.yaml` turns relaying off with `approval: { relay: deny }`; absent or
//! `allow` permits it.

use std::path::Path;

use canon_model::ApprovalRelay;

/// The option that declares a relayed approval.
pub const FLAG: &str = "--relayed-via";
/// The `canon.yaml` key that allows or denies relaying.
pub const CONFIG_KEY: &str = "approval.relay";
/// The environment variable that names the relaying agent's actor.
pub const ACTOR_ENV: &str = "CANON_ACTOR";

/// `canon.yaml`'s `approval:` section, strict once present.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApprovalSection {
    #[serde(default)]
    relay: Option<RelayMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum RelayMode {
    Allow,
    Deny,
}

/// Whether `canon.yaml` at `repo` allows relayed approvals: `approval.relay`
/// is `allow` (the default when the file, the section or the key is absent)
/// or `deny`. `Err` names a malformed section; a typo meant to turn relaying
/// off must not leave it on.
pub fn relay_allowed(repo: &Path) -> Result<bool, String> {
    let Ok(text) = std::fs::read_to_string(repo.join("canon.yaml")) else { return Ok(true) };
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("canon.yaml is not valid YAML: {e}"))?;
    let Some(section) = doc.get("approval") else { return Ok(true) };
    let section: RawApprovalSection = serde_yaml::from_value(section.clone()).map_err(|e| format!("canon.yaml `approval:` section is malformed (expected `approval: {{ relay: allow | deny }}`): {e}"))?;
    Ok(section.relay != Some(RelayMode::Deny))
}

/// Whether `value` can sit in an `Approved:` line's marker: one token with no
/// whitespace, parentheses or quotes.
pub fn is_token(value: &str) -> bool {
    !value.is_empty() && !value.contains(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\''))
}

/// The marker of an `Approved:` line: `(relayed by <role> <actor> via <tool>)`.
pub fn marker(relay: &ApprovalRelay) -> String {
    format!("(relayed by {} {} via {})", relay.role, relay.actor, relay.tool)
}

/// The relay an `Approved:` line's value records, `None` for the human's own
/// line (and for every line written before 0.32).
pub fn parse_marker(line: &str) -> Option<ApprovalRelay> {
    let line = line.trim_end();
    let without_digest = match line.rsplit_once("(brief ") {
        Some((head, digest)) if digest.ends_with(')') => head.trim_end(),
        _ => line,
    };
    let inner = without_digest.strip_suffix(')')?;
    let (_, relayed) = inner.rsplit_once("(relayed by ")?;
    let mut words = relayed.split(' ');
    let (role, actor, via, tool) = (words.next()?, words.next()?, words.next()?, words.next()?);
    if via != "via" || words.next().is_some() || ![role, actor, tool].into_iter().all(is_token) {
        return None;
    }
    Some(ApprovalRelay { role: role.to_string(), actor: actor.to_string(), tool: tool.to_string(), approver: None, answer: None })
}

/// The text of the relay for a human reading a status line.
pub fn describe(relay: &ApprovalRelay) -> String {
    format!("relayed by {} `{}` via {}", relay.role, relay.actor, relay.tool)
}

/// What a human-only act is called with: the options a relayed run needs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Call<'a> {
    /// `--relayed-via`.
    pub via: Option<&'a str>,
    /// `--actor` (else [`ACTOR_ENV`]).
    pub actor: Option<&'a str>,
    /// The human's name, `--approver`.
    pub approver: Option<&'a str>,
    /// The human's answer, `--note`.
    pub note: Option<&'a str>,
}

/// Decides how a human-only act runs (0.32): `Ok(None)` in the human's own
/// shell (no role declared, no relay option), `Ok(Some(relay))` for an agent
/// recording the human's answer, `Err` for every refusal. `human_call` is the
/// call the human runs, `relay_call` the relayed one, both with ids filled in.
pub fn authorize(command: &str, human_call: &str, relay_call: &str, repo: &Path, call: Call<'_>) -> Result<Option<ApprovalRelay>, crate::cli_error::CliError> {
    use crate::cli_error::{CliError, CliErrorExt as _};
    use crate::role::Role;
    let role = Role::current(None);
    let (role, tool) = match (role, call.via) {
        (None, None) => return Ok(None),
        (None, Some(_)) => {
            return Err(CliError::refused("USAGE", format!("{FLAG} records a human's answer on behalf of an agent, and no agent role is declared for `{command}`"))
                .field(FLAG)
                .hint("declare the role you act as (`--as <role>` or CANON_ROLE) to relay a human's answer; in the human's own shell, run the call without the relay option"));
        }
        (Some(role), None) => {
            return Err(CliError::refused("HUMAN_ONLY", format!("`{command}` records a human's decision and is not available to the {role} role without the relay option"))
                .field("--as")
                .hint(format!(
                    "to record an answer the human gave you in this conversation: show them what they approve (`canon brief show <change> --body` for a brief), ask with the harness question tool, and only on an explicit approval run `{relay_call}`; otherwise the human runs `{human_call}` in their own shell; never remove your role (`env -u CANON_ROLE`, dropping --as) to run it yourself"
                )));
        }
        (Some(role), Some(tool)) => (role, tool),
    };
    if !is_token(tool) {
        return Err(CliError::refused("USAGE", format!("{FLAG} names the tool the human was asked with as one word, got `{tool}`"))
            .field(FLAG)
            .hint("pass the question tool's name without spaces, parentheses or quotes: `ask` (omp), `AskUserQuestion` (Claude Code) or `chat`"));
    }
    match relay_allowed(repo) {
        Ok(true) => {}
        Ok(false) => {
            return Err(CliError::refused("HUMAN_ONLY", format!("`{command}` was asked to record a relayed approval, and canon.yaml turns relaying off ({CONFIG_KEY}: deny)"))
                .field(FLAG)
                .hint(format!("the human runs `{human_call}` in their own shell, or sets `{CONFIG_KEY}: allow` in canon.yaml; never remove your role to run it yourself")));
        }
        Err(e) => {
            return Err(CliError::refused("CONFIG_UNREADABLE", e).hint("fix the `approval:` section of canon.yaml (`canon init --check` names the problem), then run the call again"));
        }
    }
    let env_actor = std::env::var(ACTOR_ENV).ok();
    let actor = call.actor.map(str::to_string).or(env_actor).unwrap_or_default();
    let actor = actor.trim();
    if actor.is_empty() {
        return Err(CliError::refused("USAGE", format!("a relayed approval names the agent that recorded it: pass --actor <your actor id> (or set {ACTOR_ENV})"))
            .field("--actor")
            .hint("give the actor id of your unit, as on every record you write"));
    }
    if !is_token(actor) {
        return Err(CliError::refused("USAGE", format!("--actor must be one word for a relayed approval, got `{actor}`")).field("--actor").hint("use your unit's actor id, without spaces, parentheses or quotes"));
    }
    for (flag, value) in [("--approver", call.approver), ("--note", call.note)] {
        if value.is_none_or(|v| v.trim().is_empty()) {
            return Err(CliError::refused("USAGE", format!("a relayed approval carries the human's own {}: pass {flag}", if flag == "--approver" { "name" } else { "answer, word for word" }))
                .field(flag)
                .hint("the name and the answer the human gave in the conversation"));
        }
    }
    Ok(Some(ApprovalRelay { role: role.as_str().to_string(), actor: actor.to_string(), tool: tool.to_string(), approver: None, answer: None }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relay() -> ApprovalRelay {
        ApprovalRelay { role: "planner".into(), actor: "impl-1".into(), tool: "ask".into(), approver: None, answer: None }
    }

    #[test]
    fn the_marker_reads_back_from_a_line_with_or_without_a_digest() {
        let line = format!("Ana 2026-10-30T00:00:00Z — \"go\" {} (brief sha256:{})", marker(&relay()), "a".repeat(64));
        assert_eq!(parse_marker(&line), Some(relay()));
        assert_eq!(parse_marker(&format!("Ana 2026-10-30 — \"go\" {}", marker(&relay()))), Some(relay()));
    }

    #[test]
    fn a_line_without_a_marker_is_the_humans_own() {
        assert_eq!(parse_marker(&format!("Ana 2026-10-30T00:00:00Z — \"go\" (brief sha256:{})", "a".repeat(64))), None);
        assert_eq!(parse_marker("Ana 2026-10-12 — \"ship (relayed by x y via z)\" (brief sha256:ab)"), None);
    }

    #[test]
    fn tokens_refuse_whitespace_parentheses_and_quotes() {
        assert!(is_token("AskUserQuestion") && is_token("impl-relay"));
        assert!(!is_token("") && !is_token("a b") && !is_token("a)") && !is_token("a\"b"));
    }
}
