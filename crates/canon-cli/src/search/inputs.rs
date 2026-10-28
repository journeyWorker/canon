//! What the search index is built from (0.31 `canon-0-31-search`).
//!
//! Four inputs, read fresh on every run, in the order the index is judged stale by:
//!
//! - the promoted, current versions of the ledger records of every kind `canon query` reads, and this
//!   worktree's sqlite hot store (rungs backed by `git` or `sqlite`; a rung backed by postgres or an
//!   object store is never read, and its backend is named in [`Corpus::unread`]);
//! - the superseded versions of finding, review and evidence records in the ledger's archive, which only
//!   `canon trace` uses;
//! - the briefs of `.canon/changes/<change>/brief.md`, section by section as `canon brief check` reads them;
//! - the `.feature` files of the spec roots, with the text of each scenario as its digest covers it.
//!
//! Records staged and not yet promoted are not read: they are not under a ledger kind directory.
//!
//! [`Corpus::fingerprint`] hashes all of it (contents, never modification times), so the index is stale
//! as soon as any input differs, and two checkouts of one commit with one hot store agree on it.

use std::collections::BTreeSet;
use std::path::Path;

use canon_model::{paths, RecordKind};
use canon_store::git_tier::GitTier;
use canon_store::policy::{BackendConfig, TierPolicy};
use canon_store::sqlite_tier::SqliteTier;
use canon_store::tier::{Tier, TierQuery};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// What differs between the index's layouts: part of the fingerprint, so a canon that indexes more (or
/// differently) never reuses an older file.
const INDEX_FORMAT: &str = "canon-search-index/1";

/// The kinds whose superseded versions `canon trace` reads from the archive.
const TRACED_KINDS: [RecordKind; 3] = [RecordKind::Finding, RecordKind::Review, RecordKind::Evidence];

/// The repo-relative path and content hash of each file read.
type Hashed = Vec<(String, String)>;

/// Why the inputs could not be read.
#[derive(Debug)]
pub enum InputError {
    /// canon.yaml, or a spec root it names, cannot be used: exit 2.
    Config(String),
    /// A record store cannot be read: exit 1.
    Unreadable(String),
}

/// One version of one record.
#[derive(Debug, Clone)]
pub struct Version {
    pub kind: RecordKind,
    /// The record's natural key, as `canon query` shows it.
    pub key: String,
    /// The first eight hex digits of the content digest, the last part of the record's name.
    pub digest: String,
    /// The record's `at`, as fixed-width UTC text (sorts as time).
    pub at: String,
    /// Whether this is a version `canon query` reads (and so a node of the graph), not an archived one.
    pub current: bool,
    pub body: Value,
}

/// One section, or one flow step, of a change's brief.
#[derive(Debug, Clone)]
pub struct BriefNode {
    pub change: String,
    /// The section key (`intent`), or the flow step id (`F1`).
    pub name: String,
    pub title: String,
    pub text: String,
}

/// One scenario of a spec root's `.feature` files.
#[derive(Debug, Clone)]
pub struct FeatureScenario {
    pub project: String,
    pub scenario_id: String,
    pub title: String,
    pub text: String,
    /// The `@flow:<change>.<F-id>` tag values.
    pub flows: Vec<String>,
}

/// A reference no committed source holds ([`crate::references`]), from one current record.
#[derive(Debug, Clone)]
pub struct Dangling {
    pub kind: RecordKind,
    pub key: String,
    pub digest: String,
    pub field: &'static str,
    /// The kind of thing the field names: `change`, `review`, `task` or `scenario`.
    pub target: &'static str,
    pub id: String,
}

/// Everything the index is built from.
#[derive(Debug)]
pub struct Corpus {
    pub versions: Vec<Version>,
    pub briefs: Vec<BriefNode>,
    pub scenarios: Vec<FeatureScenario>,
    pub dangling: Vec<Dangling>,
    /// The backends of the stores nothing was read from, sorted (`postgres`, `s3`).
    pub unread: Vec<String>,
    /// Why the references were not checked, when the sources could not be read.
    pub references_unchecked: Option<String>,
    pub fingerprint: String,
}

fn unreadable(what: &str, error: impl std::fmt::Display) -> InputError {
    InputError::Unreadable(format!("{what}: {error}"))
}

/// Read every input of the repo rooted at `repo`.
pub fn load(repo: &Path) -> Result<Corpus, InputError> {
    let canon_yaml = repo.join("canon.yaml");
    let yaml_text = std::fs::read_to_string(&canon_yaml).map_err(|e| InputError::Config(format!("reading `{}`: {e}", canon_yaml.display())))?;
    let policy = TierPolicy::from_yaml_at(&yaml_text, repo).map_err(|e| InputError::Config(format!("parsing `{}`: {e}", canon_yaml.display())))?;

    let mut unread: BTreeSet<String> = BTreeSet::new();
    let mut versions = read_records(repo, &policy, &mut unread)?;
    let (briefs, mut hashed) = read_briefs(repo)?;
    let (scenarios, features_hashed) = read_features(repo)?;
    hashed.extend(features_hashed);

    let mut references_unchecked = None;
    let mut dangling = Vec::new();
    match crate::references::Sources::load(repo) {
        Ok(sources) => {
            for version in versions.iter().filter(|v| v.current && crate::references::is_checked(v.kind)) {
                for found in sources.unresolved(version.kind, &version.body) {
                    dangling.push(Dangling { kind: version.kind, key: version.key.clone(), digest: version.digest.clone(), field: found.field, target: found.target, id: found.id });
                }
            }
        }
        Err(error) => references_unchecked = Some(error.to_string()),
    }

    versions.sort_by(|a, b| (a.kind.as_str(), &a.key, &a.digest).cmp(&(b.kind.as_str(), &b.key, &b.digest)));
    dangling.sort_by(|a, b| (a.kind.as_str(), &a.key, &a.digest, a.field, &a.id).cmp(&(b.kind.as_str(), &b.key, &b.digest, b.field, &b.id)));
    let unread: Vec<String> = unread.into_iter().collect();

    let mut hasher = Sha256::new();
    let mut feed = |line: String| {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    };
    feed(format!("{INDEX_FORMAT} canon {}", env!("CARGO_PKG_VERSION")));
    feed(format!("canon.yaml {}", hex(&Sha256::digest(yaml_text.as_bytes()))));
    for version in &versions {
        feed(format!("record {} {} {} {}", version.kind.as_str(), version.key, version.digest, version.current));
    }
    for (path, sha) in &hashed {
        feed(format!("file {path} {sha}"));
    }
    for found in &dangling {
        feed(format!("dangling {} {} {} {} {}", found.kind.as_str(), found.key, found.field, found.target, found.id));
    }
    feed(format!("unread {}", unread.join(",")));
    feed(format!("unchecked {}", references_unchecked.is_some()));
    let fingerprint = hex(&hasher.finalize());

    Ok(Corpus { versions, briefs, scenarios, dangling, unread, references_unchecked, fingerprint })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The ledger and hot-store records of every kind, and the archive's old versions of the traced kinds.
fn read_records(repo: &Path, policy: &TierPolicy, unread: &mut BTreeSet<String>) -> Result<Vec<Version>, InputError> {
    let mut out = Vec::new();
    for kind in RecordKind::ALL {
        let Ok(rungs) = crate::tiers::tiers_needed_for(policy, kind) else { continue };
        let mut rows: Vec<canon_model::RawRecord> = Vec::new();
        let mut archived: Vec<canon_model::RawRecord> = Vec::new();
        for rung in rungs {
            let query = TierQuery::kind(kind);
            match policy.tiers.get(&rung) {
                Some(BackendConfig::Git(cfg)) => {
                    let tier = GitTier::new(repo.join(&cfg.root));
                    rows.extend(tier.read(&query).map_err(|e| unreadable(&format!("the `{}` records of the ledger at `{}`", kind.as_str(), tier.root().display()), e))?.records);
                    if TRACED_KINDS.contains(&kind) {
                        if let Some(archive) = tier.archive_tier() {
                            archived.extend(archive.read(&query).map_err(|e| unreadable(&format!("the `{}` records of the archive at `{}`", kind.as_str(), archive.root().display()), e))?.records);
                        }
                    }
                }
                Some(BackendConfig::Sqlite(cfg)) => {
                    let tier = SqliteTier::connect_read_only(&cfg.path).map_err(|e| unreadable(&format!("the sqlite hot store at `{}`", cfg.path.display()), e))?;
                    rows.extend(tier.read(&query).map_err(|e| unreadable(&format!("the `{}` records of the sqlite hot store at `{}`", kind.as_str(), cfg.path.display()), e))?.records);
                }
                Some(other) => {
                    unread.insert(other.backend().as_str().to_string());
                }
                None => {}
            }
        }
        let current: BTreeSet<String> = crate::query::current_of(kind, rows.clone()).iter().map(|raw| canon_store::partition::content_digest(&raw.0)).collect();
        for (raw, is_current) in rows.into_iter().map(|raw| (raw, None)).chain(archived.into_iter().map(|raw| (raw, Some(false)))) {
            let Ok(partition) = canon_store::partition::resolve_partition(kind, &raw.0) else { continue };
            let digest = canon_store::partition::content_digest(&raw.0);
            let current = is_current.unwrap_or_else(|| current.contains(&digest));
            let at = canon_store::tier::raw_record_at(&raw).to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
            out.push(Version { kind, key: partition.natural_key, digest, at, current, body: raw.0 });
        }
    }
    Ok(out)
}

/// The briefs under `.canon/changes`, with each file's content hash.
fn read_briefs(repo: &Path) -> Result<(Vec<BriefNode>, Hashed), InputError> {
    let dir = repo.join(paths::CHANGES_DIR);
    let mut changes: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries.flatten().filter(|e| e.path().join("brief.md").is_file()).filter_map(|e| e.file_name().into_string().ok()).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(unreadable(&format!("the changes directory `{}`", dir.display()), e)),
    };
    changes.sort();
    let (mut nodes, mut hashed) = (Vec::new(), Vec::new());
    for change in changes {
        let path = dir.join(&change).join("brief.md");
        let bytes = std::fs::read(&path).map_err(|e| unreadable(&format!("the brief `{}`", path.display()), e))?;
        hashed.push((format!("{}/{change}/brief.md", paths::CHANGES_DIR), hex(&Sha256::digest(&bytes))));
        let text = String::from_utf8_lossy(&bytes);
        for section in crate::brief_check::parse_sections(&text) {
            let title = section.heading.clone().unwrap_or_else(|| section.key.clone());
            nodes.push(BriefNode { change: change.clone(), name: section.key.clone(), title, text: section.body.join("\n") });
            for step in &section.steps {
                nodes.push(BriefNode { change: change.clone(), name: step.id.clone(), title: step.text.clone(), text: step.text.clone() });
            }
        }
    }
    Ok((nodes, hashed))
}

/// The scenarios of every spec root's `.feature` files, with each file's content hash.
fn read_features(repo: &Path) -> Result<(Vec<FeatureScenario>, Hashed), InputError> {
    let sync = crate::inventory::SyncCtx::from_repo(repo).map_err(|e| InputError::Config(e.to_string()))?;
    let roots = sync.spec_roots(None).map_err(|e| InputError::Config(e.to_string()))?;
    let (mut scenarios, mut hashed) = (Vec::new(), Vec::new());
    for root in roots {
        let mut files: Vec<_> = canon_fmt::util::feature_files(&root.root).collect();
        files.sort();
        for file in files {
            let bytes = std::fs::read(&file).map_err(|e| unreadable(&format!("the spec file `{}`", file.display()), e))?;
            hashed.push((file.strip_prefix(repo).unwrap_or(&file).display().to_string(), hex(&Sha256::digest(&bytes))));
            let Ok(text) = String::from_utf8(bytes) else { continue };
            let bodies = canon_fmt::gherkin::scenario_bodies(&text);
            for scanned in canon_fmt::gherkin::scan(&text).scenarios {
                let Some(body) = bodies.get(&scanned.scenario_id) else { continue };
                scenarios.push(FeatureScenario { project: root.id.as_str().to_string(), scenario_id: scanned.scenario_id.clone(), title: scanned.title.clone(), text: body.text.clone(), flows: scanned.flow_tags.clone() });
            }
        }
    }
    Ok((scenarios, hashed))
}
