//! The role an agent declares for itself (0.21 D3).
//!
//! canon cannot detect a role, any more than it can detect a session id: the
//! harness knows which subagent it spawned. The role arrives as the
//! `CANON_ROLE` environment variable or the global `--as <role>` flag (the
//! flag wins). It is guidance, not enforcement: `schema` lists the role's
//! commands, `status` and `meta.next` keep the role's steps, and `--role` on
//! `evidence add` / `evidence audit` / `review scenario` / `review change` defaults to it. Unset, every command behaves
//! as before.
//!
//! [`init`] runs once at startup: it resolves the declared role, refuses an
//! unknown `CANON_ROLE` value (`UNKNOWN_ROLE`), and remembers the result so
//! [`Role::current`] can be called from anywhere without threading the flag.

use std::sync::OnceLock;

use clap::ValueEnum;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli_error::CliError;

/// The environment variable that declares the role.
pub const ENV: &str = "CANON_ROLE";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, ValueEnum, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Role {
    Planner,
    Implementer,
    Reviewer,
    Orchestrator,
}

/// Every role, in the order `schema` and `status` list them.
pub const ALL: &[Role] = &[Role::Planner, Role::Implementer, Role::Reviewer, Role::Orchestrator];

/// The role resolved at startup by [`init`]: `--as`, else `CANON_ROLE`.
static DECLARED: OnceLock<Option<Role>> = OnceLock::new();

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Planner => "planner",
            Role::Implementer => "implementer",
            Role::Reviewer => "reviewer",
            Role::Orchestrator => "orchestrator",
        }
    }

    /// The role named by `text` (trimmed, case-insensitive), or `None`.
    pub fn parse(text: &str) -> Option<Role> {
        let text = text.trim();
        ALL.iter().copied().find(|role| role.as_str().eq_ignore_ascii_case(text))
    }

    /// The declared role: `flag` when given, else the one [`init`] resolved
    /// from `--as` / `CANON_ROLE`. Without [`init`] (a library caller) the
    /// environment is read directly and an unknown value reads as undeclared.
    pub fn current(flag: Option<Role>) -> Option<Role> {
        flag.or_else(|| match DECLARED.get() {
            Some(role) => *role,
            None => std::env::var(ENV).ok().and_then(|value| Role::parse(&value)),
        })
    }
}

/// The attribution of a record a leaf writes without a `--role` option: the
/// declared role when there is one (D12), else `fallback` (what the leaf
/// recorded before roles were declared; `None` records no role), in `session` when `--session-id` named one.
pub fn attributed(agent_id: &str, fallback: Option<&canon_model::RoleId>, session: Option<&canon_model::SessionId>) -> canon_model::Actor {
    let declared = Role::current(None).map(|role| canon_model::RoleId::parse(role.as_str()).expect("a role name is a valid role id"));
    let actor = match declared.as_ref().or(fallback) {
        Some(role) => canon_model::Actor::new(agent_id, role.clone()),
        None => canon_model::Actor::new_unattributed(agent_id),
    };
    match session {
        Some(session) => actor.with_session(session.clone()),
        None => actor,
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Roles a repo's policy names: the keys of `risk_routing` and every value
/// under a `role`, `roles`, `required_roles` or `require_roles` key, wherever
/// it sits in `.canon/policy.yaml`. An unreadable or absent policy names none.
fn policy_roles(repo: &std::path::Path) -> Vec<String> {
    fn walk(value: &serde_yaml::Value, out: &mut Vec<String>) {
        use serde_yaml::Value;
        match value {
            Value::Mapping(map) => {
                for (key, child) in map {
                    match key.as_str() {
                        Some("risk_routing") => {
                            if let Value::Mapping(routes) = child {
                                out.extend(routes.keys().filter_map(|k| k.as_str().map(str::to_string)));
                            }
                        }
                        Some("role" | "roles" | "required_roles" | "require_roles") => collect_strings(child, out),
                        _ => {}
                    }
                    walk(child, out);
                }
            }
            Value::Sequence(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }
    fn collect_strings(value: &serde_yaml::Value, out: &mut Vec<String>) {
        match value {
            serde_yaml::Value::String(text) => out.push(text.clone()),
            serde_yaml::Value::Sequence(items) => {
                for item in items {
                    collect_strings(item, out);
                }
            }
            _ => {}
        }
    }
    let Ok(text) = std::fs::read_to_string(repo.join(canon_model::paths::POLICY_FILE)) else { return Vec::new() };
    let Ok(document) = serde_yaml::from_str::<serde_yaml::Value>(&text) else { return Vec::new() };
    let mut out = Vec::new();
    walk(&document, &mut out);
    out
}

/// `--role` is an open slug: a policy may require a role canon does not know.
/// A value that is none of the four roles and no role the policy names still
/// runs, with a warning (`meta.warnings`) that names the closest known role.
pub fn warn_if_unknown(role: &canon_model::RoleId, repo: &std::path::Path) {
    let value = role.as_str();
    if Role::parse(value).is_some() {
        return;
    }
    let named = policy_roles(repo);
    if named.iter().any(|known| known == value) {
        return;
    }
    let known: Vec<&str> = ALL.iter().map(|r| r.as_str()).chain(named.iter().map(String::as_str)).collect();
    let nearest = crate::suggest::nearest(value, known.iter().copied());
    let message = match nearest.first() {
        Some(closest) => format!("role `{value}` is not a known role (planner, implementer, reviewer, orchestrator, or one the policy names); did you mean `{closest}`?"),
        None => format!("role `{value}` is not a known role (planner, implementer, reviewer, orchestrator, or one the policy names); it is recorded as given"),
    };
    crate::json_out::warn(message);
}

/// Resolve the declared role once, at startup. The `--as` flag wins over
/// `CANON_ROLE`; an unknown `CANON_ROLE` value is refused unless the flag
/// overrides it.
pub fn init(flag: Option<Role>) -> Result<Option<Role>, CliError> {
    let role = match flag {
        Some(role) => Some(role),
        None => match std::env::var(ENV) {
            Ok(value) if !value.trim().is_empty() => Some(Role::parse(&value).ok_or_else(|| {
                CliError::refused("UNKNOWN_ROLE", format!("{ENV}=`{}` is not a role", value.trim()))
                    .field(ENV)
                    .expected(["planner", "implementer", "reviewer", "orchestrator"])
                    .hint(format!("use one of: {}; or pass --as <role>", ALL.iter().map(|r| r.as_str()).collect::<Vec<_>>().join(", ")))
            })?),
            _ => None,
        },
    };
    Ok(*DECLARED.get_or_init(|| role))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_is_trimmed_and_case_insensitive() {
        assert_eq!(Role::parse(" Reviewer "), Some(Role::Reviewer));
        assert_eq!(Role::parse("human"), None);
        assert_eq!(Role::parse(""), None);
    }

    #[test]
    fn flag_wins_over_everything() {
        assert_eq!(Role::current(Some(Role::Planner)), Some(Role::Planner));
    }
}
