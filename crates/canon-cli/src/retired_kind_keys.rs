//! A `canon.yaml` that still routes or ages the record kinds canon 0.39 retired (0.39 `session-kinds-removal`, F1).
//!
//! `routing.session`, `routing.event`, `aging.session` and `aging.event` are read as keys that route nothing: their
//! values are neither read nor checked, and any other unknown kind name is still a config error. `canon migrate`
//! deletes the lines. Until then a read command (`status`, `gate check`, `context`, `init --check`) warns
//! with `RETIRED_KIND_KEYS` and puts `canon migrate --json` first in `meta.next`.
//!
//! Unlike the 0.29 layout ([`crate::old_layout`]) this never refuses a command that writes: the keys hold no
//! files, so nothing a write makes can sit beside an old one.

use std::path::Path;

use crate::status::{NextStep, StepActor};

/// The retired keys `repo`'s `canon.yaml` still names, sorted (`routing.session`); empty when it has none or cannot be read
/// (the command that needs the config reports that).
pub fn found(repo: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(repo.join("canon.yaml")) else { return Vec::new() };
    canon_store::policy::TierPolicy::from_yaml_at(&text, repo).map(|policy| policy.retired_kind_keys).unwrap_or_default()
}

/// The warning's message for `keys`.
pub fn message(keys: &[String]) -> String {
    let named = keys.iter().map(|key| format!("`{key}`")).collect::<Vec<_>>().join(", ");
    format!(
        "canon.yaml names {named}: canon 0.39 has no `session` or `event` record kind, so these keys route and age nothing. Hint: run `canon migrate --json` to preview deleting them (a bare run only previews; `--commit --plan-digest <digest>` applies it)"
    )
}

/// The `canon migrate --json` step that comes first in `meta.next`.
fn migrate_step(keys: &[String]) -> NextStep {
    NextStep {
        command: "canon migrate --json".into(),
        why: format!("canon.yaml still names the retired record kind keys {}; the preview lists the lines it deletes and the exact command that applies it", keys.join(", ")),
        actor: StepActor::Agent,
        role: Some(crate::role::Role::Orchestrator),
    }
}

/// For `status`, `gate check`, `context` and `init --check`: warns when `repo`'s `canon.yaml` names a retired kind, and with
/// `lead` puts `canon migrate --json` first in `meta.next`.
pub fn announce(repo: &Path, lead: bool) {
    let keys = found(repo);
    if keys.is_empty() {
        return;
    }
    crate::json_out::warn("RETIRED_KIND_KEYS", message(&keys));
    if lead {
        crate::json_out::prepend_next(migrate_step(&keys));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_retired_01_the_retired_keys_are_listed_in_one_sorted_message() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "routing:\n  session: hot\n  event: hot\n  task: hot\naging:\n  event: { after: 7d, to: cold }\n").unwrap();
        let keys = found(dir.path());
        assert_eq!(keys, ["aging.event", "routing.event", "routing.session"]);
        let text = message(&keys);
        assert!(text.contains("`routing.session`") && text.contains("`aging.event`") && text.contains("canon migrate --json"));
    }

    #[test]
    fn tier_retired_02_a_config_without_the_keys_or_one_that_does_not_parse_has_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(found(dir.path()).is_empty(), "no canon.yaml");
        std::fs::write(dir.path().join("canon.yaml"), "routing:\n  task: hot\n").unwrap();
        assert!(found(dir.path()).is_empty());
        std::fs::write(dir.path().join("canon.yaml"), "routing:\n  sesion: hot\n").unwrap();
        assert!(found(dir.path()).is_empty(), "a misspelled kind is a config error, reported by the command that reads it");
    }
}
