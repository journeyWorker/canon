//! The open `role` registry (design decision 1): `RoleId` itself
//! (`canon-model`) validates only kebab-slug SHAPE — it never closes
//! over a fixed variant set, so adding a role never waits on a canon
//! crate release. `RoleRegistry` is the layer that decides which
//! shape-valid slugs are actually REGISTERED for a given repo: the
//! seven built-in roles this design ships (`planning|design|dev|test|
//! review|content|sim`, generalizing the donor harness's closed
//! `dev|content|sim` `PatternNamespace` union) plus whatever a
//! consumer repo's own `canon.yaml` `learn.roles:` list adds.
//!
//! A write carrying an unregistered role is rejected HERE, at write
//! time (risk section: "fail loud, not fail soft" — distinct from
//! retrieval's advisory fail-soft contract owned by a later change).

use std::collections::BTreeSet;

use canon_model::ids::RoleId;

use crate::config::LearnConfig;
use crate::error::LearnError;

/// The seven roles this design ships built in, generalizing
/// the donor harness's closed `PatternNamespace` (`dev|content|sim`)
/// to canon's wider domain set
/// (`docs/superpowers/specs/2026-07-10-canon-design.md`, S6 design
/// decision 1).
pub const BUILTIN_ROLES: &[&str] = &["planning", "design", "dev", "test", "review", "content", "sim"];

/// The set of roles registered for one repo — always a superset of
/// [`BUILTIN_ROLES`], optionally widened by that repo's own
/// `canon.yaml` `learn.roles:` list ([`RoleRegistry::from_config`]).
#[derive(Debug, Clone)]
pub struct RoleRegistry {
    roles: BTreeSet<RoleId>,
}

impl RoleRegistry {
    /// Only the built-in seven roles registered — no consumer-repo
    /// extension. `BUILTIN_ROLES` are all valid kebab slugs by
    /// construction (asserted in this module's own tests), so this
    /// never fails.
    pub fn builtin() -> Self {
        let roles = BUILTIN_ROLES.iter().map(|s| RoleId::parse(*s).expect("BUILTIN_ROLES are valid kebab slugs")).collect();
        Self { roles }
    }

    /// The built-in set widened by `extra` (a consumer repo's own
    /// additional registered roles, already-parsed `RoleId`s).
    pub fn with_extra_roles(mut self, extra: impl IntoIterator<Item = RoleId>) -> Self {
        self.roles.extend(extra);
        self
    }

    /// Built-in roles plus a repo's `canon.yaml` `learn.roles:` list
    /// (via the already-parsed [`LearnConfig`] — this function does no
    /// YAML parsing itself, mirroring `TemplateRegistry::from_manifest`'s
    /// separation of "parse the manifest" from "build the registry").
    pub fn from_config(config: &LearnConfig) -> Self {
        Self::builtin().with_extra_roles(config.extra_roles.iter().cloned())
    }

    pub fn is_registered(&self, role: &RoleId) -> bool {
        self.roles.contains(role)
    }

    /// Fail loud (design decision 1 / risk section) — `Err` rather than
    /// a silent no-op when `role` is not registered.
    pub fn validate(&self, role: &RoleId) -> Result<(), LearnError> {
        if self.is_registered(role) { Ok(()) } else { Err(LearnError::UnregisteredRole(role.as_str().to_string())) }
    }

    pub fn registered_roles(&self) -> impl Iterator<Item = &RoleId> {
        self.roles.iter()
    }
}

impl Default for RoleRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_role_slug_is_a_valid_role_id() {
        for slug in BUILTIN_ROLES {
            RoleId::parse(*slug).unwrap_or_else(|e| panic!("BUILTIN_ROLES entry {slug:?} must parse as RoleId: {e}"));
        }
    }

    #[test]
    fn builtin_roles_are_all_registered() {
        let registry = RoleRegistry::builtin();
        for slug in BUILTIN_ROLES {
            assert!(registry.is_registered(&RoleId::parse(*slug).unwrap()));
        }
    }

    #[test]
    fn an_unregistered_role_is_rejected() {
        let registry = RoleRegistry::builtin();
        let role = RoleId::parse("triage").unwrap();
        assert!(!registry.is_registered(&role));
        assert!(matches!(registry.validate(&role), Err(LearnError::UnregisteredRole(s)) if s == "triage"));
    }

    #[test]
    fn extra_roles_widen_the_registry() {
        let registry = RoleRegistry::builtin().with_extra_roles([RoleId::parse("triage").unwrap()]);
        assert!(registry.validate(&RoleId::parse("triage").unwrap()).is_ok());
        // still registers every built-in role too.
        assert!(registry.validate(&RoleId::parse("dev").unwrap()).is_ok());
    }
}
