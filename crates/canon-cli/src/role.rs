//! STUB for the merge: T1 owns the real `role` module (declared role,
//! `--as`, `CANON_ROLE`). This file only matches the contract T2 codes
//! against: the four roles and `Role::current(flag)`.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Planner,
    Implementer,
    Reviewer,
    Orchestrator,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Planner => "planner",
            Role::Implementer => "implementer",
            Role::Reviewer => "reviewer",
            Role::Orchestrator => "orchestrator",
        }
    }

    /// The declared role: `flag` wins over `CANON_ROLE`; `None` when neither
    /// is set or the value is not a role.
    pub fn current(flag: Option<Role>) -> Option<Role> {
        flag.or_else(|| match std::env::var("CANON_ROLE").ok()?.as_str() {
            "planner" => Some(Role::Planner),
            "implementer" => Some(Role::Implementer),
            "reviewer" => Some(Role::Reviewer),
            "orchestrator" => Some(Role::Orchestrator),
            _ => None,
        })
    }
}
