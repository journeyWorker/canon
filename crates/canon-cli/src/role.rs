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

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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
