//! Where the `hot` rung keeps its records, shown (0.30 `record-links`).
//!
//! The location rule is unchanged: `canon.yaml`'s `tiers.hot` block names it,
//! and a sqlite `path` is resolved against the directory of the `canon.yaml`
//! that was read. A second worktree of one repository has its own `canon.yaml`
//! and so its own file, which is right for what the hot rung holds in a
//! checkout (the scenario index is built from that worktree's `.feature`
//! files); sharing one store between worktrees is a separate change. What was
//! missing is any place that says where it is: `canon init --check` and every
//! command that writes hot records ([`HotStore::line`]) print it now.

use std::path::Path;

use canon_store::policy::{BackendConfig, Rung, TierPolicy};
use canon_model::RecordKind;
use canon_store::sqlite_tier::SqliteTier;
use canon_store::tier::{Tier, TierQuery};

/// `path` made absolute and without its `.` components, so `/repo/./.canon/hot.db` is under `.` run in `/repo`.
fn clean(path: &Path) -> std::path::PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()).components().filter(|component| !matches!(component, std::path::Component::CurDir)).collect()
}

/// The hot rung's store, as `canon.yaml` configures it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotStore {
    /// A sqlite file, as the repository shows it: relative to the repository, or absolute when it lies outside.
    Sqlite { path: String },
    /// A Postgres schema; the connection string is in the environment variable `dsn_env` and is never printed.
    Postgres { schema: String, dsn_env: String },
}

impl HotStore {
    /// The hot store `canon_yaml` configures, or `None` when it has no hot rung (or does not parse).
    pub fn of(canon_yaml: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(canon_yaml).ok()?;
        let repo = crate::tiers::project_dir(canon_yaml);
        Self::from_policy(&TierPolicy::from_yaml_at(&text, repo).ok()?, repo)
    }

    /// The hot store a write of `kinds` goes to: `Some` when `canon_yaml` configures a hot rung and routes at least one of `kinds` to it.
    pub fn written_by(canon_yaml: &Path, kinds: &[RecordKind]) -> Option<Self> {
        let text = std::fs::read_to_string(canon_yaml).ok()?;
        let repo = crate::tiers::project_dir(canon_yaml);
        let policy = TierPolicy::from_yaml_at(&text, repo).ok()?;
        kinds
            .iter()
            .any(|kind| policy.routing.get(kind) == Some(&Rung::Hot))
            .then(|| Self::from_policy(&policy, repo))
            .flatten()
    }

    /// The hot store of a parsed policy whose relative paths were resolved against `repo`.
    pub fn from_policy(policy: &TierPolicy, repo: &Path) -> Option<Self> {
        Some(match policy.tiers.get(&Rung::Hot)? {
            BackendConfig::Sqlite(config) => HotStore::Sqlite { path: clean(&config.path).strip_prefix(clean(repo)).map_or_else(|_| config.path.display().to_string(), |relative| relative.display().to_string()) },
            BackendConfig::Postgres(config) => HotStore::Postgres { schema: config.schema.clone(), dsn_env: config.dsn_env.clone() },
            BackendConfig::Git(_) | BackendConfig::S3(_) => return None,
        })
    }

    /// Where the records are: the file, or the schema.
    pub fn location(&self) -> String {
        match self {
            HotStore::Sqlite { path } => path.clone(),
            HotStore::Postgres { schema, dsn_env } => format!("postgres schema `{schema}` (connection string in ${dsn_env})"),
        }
    }

    /// What kind of store it is and who shares it.
    pub fn note(&self) -> &'static str {
        match self {
            HotStore::Sqlite { .. } => "sqlite file of this worktree; another worktree of the repository has its own",
            HotStore::Postgres { .. } => "shared by every worktree that reads the same connection string",
        }
    }

    /// What a command that wrote hot records says, in one line.
    pub fn line(&self) -> String {
        format!("hot store: {} ({})", self.location(), self.note())
    }

    /// Open a sqlite hot store the way a reader does, without creating it: `Err` names the file and why it will not open.
    pub fn check(&self, repo: &Path) -> Result<(), String> {
        let HotStore::Sqlite { path } = self else { return Ok(()) };
        // The open alone does not read the file: a query that matches nothing (`since` in the future) does.
        let tier = SqliteTier::connect_read_only(&repo.join(path)).map_err(|e| format!("cannot open {path}: {e}"))?;
        let query = TierQuery::kind(RecordKind::Session).since(chrono::Utc::now() + chrono::Duration::days(1));
        tier.read(&query).map(drop).map_err(|e| format!("cannot open {path}: {e}"))
    }
}

/// The hot store of the repo whose `canon.yaml` is `canon_yaml`, for a report: its [`HotStore::location`].
pub fn location(canon_yaml: &Path) -> Option<String> {
    HotStore::of(canon_yaml).map(|store| store.location())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(yaml: &str) -> Option<HotStore> {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), yaml).unwrap();
        HotStore::of(&dir.path().join("canon.yaml"))
    }

    #[test]
    fn a_sqlite_hot_rung_shows_its_file_relative_to_the_repo() {
        let hot = store("tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\n").unwrap();
        assert_eq!(hot, HotStore::Sqlite { path: ".canon/hot.db".to_string() });
        assert_eq!(hot.location(), ".canon/hot.db");
        assert_eq!(hot.line(), "hot store: .canon/hot.db (sqlite file of this worktree; another worktree of the repository has its own)");
    }

    #[test]
    fn a_postgres_hot_rung_names_the_schema_and_never_the_connection_string() {
        let hot = store("tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }\n").unwrap();
        assert_eq!(hot.location(), "postgres schema `canon_v1` (connection string in $CANON_PG_DSN)");
    }

    #[test]
    fn no_hot_rung_shows_nothing() {
        assert_eq!(store("tiers:\n  local: { backend: git, root: .canon/ledger }\n"), None);
    }
}
