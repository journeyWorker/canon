//! `canon init [--repo <dir>]` + `canon init --check-config [--repo
//! <dir>]` (s19 `canon-init-scaffold` spec): scaffolds a fresh, WORKING
//! `canon.yaml` skeleton (design D8/D9) at `<repo>/canon.yaml` --
//! refuses to overwrite an existing one, mirroring
//! `crate::scaffold::run_feature_new`'s own `create_new` refusal
//! convention -- or, with `--check-config`, READ-ONLY validates an
//! EXISTING `canon.yaml` by chaining the SAME three independently
//! strict loaders `canon inventory sync`/`canon ingest plans`/`canon
//! tier age` already use: [`TierPolicy::from_yaml`],
//! [`crate::inventory::load_spec_roots`],
//! [`crate::plans::load_plan_sources_from_config`] (design D7). This
//! module reimplements NONE of their validation logic -- it only
//! chains them and formats one PASS/FAIL/"not configured" line per
//! section, never stopping at the first failure (mirroring `canon fmt
//! --check`'s own "report everything" convention).
//!
//! # `<repo>` is used literally, never an ancestor walk
//! Every other subcommand's `--repo` resolves through
//! `crate::context::resolve_repo_root`'s nearest-ancestor-`canon.yaml`
//! walk -- appropriate for a command operating INSIDE an already-
//! configured repo. `canon init`'s whole job is bootstrapping the
//! FIRST `canon.yaml`, so walking up to find some OTHER ancestor's
//! existing config would resolve to the wrong place entirely (and
//! could spuriously refuse-as-already-exists against a config this
//! invocation never intended to touch); `<repo>/canon.yaml` (spec.md's
//! own literal join) is used exactly as given, default `.` meaning cwd.

use std::fs;
use std::io::Write as _;
use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_model::paths;
use canon_store::policy::{BackendConfig, TierPolicy};

/// Kinds routed to `hot` by [`skeleton_yaml`] (s32 `sqlite-hot-backend`):
/// the same hot-class set the tiered-storage docs/this repo's own
/// `canon.yaml` already use (task/handoff/session/run/event) --
/// `canon init` can now afford to route them there by default because
/// `hot`'s sqlite backend needs no operator-supplied credential
/// (unlike postgres's `dsn_env` or s3's `bucket_env`, which `init`
/// still can't guess -- `cold`-class kinds stay on `local`). Every
/// OTHER kind (`RecordKind::ALL` minus this set) routes to `local`.
const HOT_KINDS: [RecordKind; 5] = [RecordKind::Task, RecordKind::Handoff, RecordKind::Session, RecordKind::Run, RecordKind::Event];

/// The line [`scaffold_gitignore`] ensures is present in
/// `<repo>/.gitignore`: one glob covering the sqlite hot tier's db
/// file AND its WAL/SHM sidecars (`.canon/hot.db-wal`/`.canon/hot.db-shm`
/// -- sqlite's own WAL-journal-mode naming convention), since all
/// three share the `.canon/hot.db` prefix.
const GITIGNORE_LINE: &str = paths::HOT_DB_GITIGNORE;

/// D8/D9's skeleton `canon.yaml` body: every one of `RecordKind::ALL`'s
/// thirteen wire strings (s36: `subject` is the reviewed 13th kind)
/// routed to either `local` (git-backed) or `hot`
/// (sqlite-backed, [`HOT_KINDS`]) -- the two zero-env-var rungs (s32
/// `sqlite-hot-backend`: sqlite needs no operator-supplied credential,
/// unlike postgres/s3) -- commented `tiers.hot` (postgres swap) /
/// `tiers.cold` stanzas documenting the scale-up path, one working
/// `specs.roots[]` entry (D9: a present `specs:` section requires at
/// least one root, so this ships real rather than empty), and a
/// present-but-empty `plans: { sources: [] }` (D9:
/// `load_plan_sources_from_config` treats an empty `sources: []` as a
/// legitimate, already-configured zero-source state, never a parse
/// failure).
fn skeleton_yaml() -> String {
    let mut routing = String::new();
    for kind in RecordKind::ALL {
        let rung = if HOT_KINDS.contains(&kind) { "hot" } else { "local" };
        routing.push_str(&format!("  {}: {rung}\n", kind.as_str()));
    }

    let mut out = String::new();
    out.push_str("# canon.yaml -- scaffolded by `canon init` (s19 canon-init-scaffold).\n");
    out.push_str("# `local` (git-backed) and `hot` (sqlite-backed, s32 sqlite-hot-\n");
    out.push_str("# backend) both need zero operator-supplied credentials, so every\n");
    out.push_str("# kind below is already routed -- task/handoff/session/run/event to\n");
    out.push_str("# `hot`, everything else to `local`. Flip a `routing:` line to `cold`\n");
    out.push_str("# once you have a real `bucket_env` credential `init` cannot guess\n");
    out.push_str("# (see the commented `tiers.cold` stanza below; s27\n");
    out.push_str("# tier-role-backend-split: routing/aging name a capability RUNG, the\n");
    out.push_str("# backend is tagged separately via `tiers.<rung>.backend`).\n");
    out.push_str("tiers:\n");
    out.push_str("  local:\n");
    out.push_str("    backend: git\n");
    out.push_str(&format!("    root: {}\n", paths::LEDGER_DIR));
    out.push_str("  hot:\n");
    out.push_str("    backend: sqlite\n");
    out.push_str(&format!("    path: {}\n", paths::HOT_DB_FILE));
    out.push_str("  # hot (same-class swap for team-scale multi-agent concurrency --\n");
    out.push_str("  # sqlite's WAL journal mode already covers concurrent batch\n");
    out.push_str("  # ingest from a single operator; swap to postgres once you need a\n");
    out.push_str("  # real server -- comment out the live `hot:` block above and\n");
    out.push_str("  # uncomment this one):\n");
    out.push_str("  #   backend: postgres\n");
    out.push_str("  #   dsn_env: CANON_PG_DSN\n");
    out.push_str("  #   schema: canon_v1\n");
    out.push_str("  # cold:\n");
    out.push_str("  #   backend: s3\n");
    out.push_str("  #   bucket_env: CANON_R2_BUCKET\n");
    out.push_str("  #   prefix: \"canon/\"\n");
    out.push_str("routing:\n");
    out.push_str(&routing);
    out.push_str("specs:\n");
    out.push_str("  roots:\n");
    out.push_str("    - id: root\n");
    out.push_str("      root: specs\n");
    out.push_str("plans:\n");
    out.push_str("  sources: []\n");
    out
}

/// Ensures [`GITIGNORE_LINE`] is present in `<repo>/.gitignore` --
/// appends it (creating the file if absent) UNLESS it is already
/// there, so a `.gitignore` a prior `canon init` (or the repo's own
/// `git init`) already wrote is never duplicated. `canon init` itself
/// stays a fresh-repo bootstrap (`run_init`'s `canon.yaml` refuses to
/// overwrite), but `.gitignore` commonly PRE-EXISTS a `canon init`
/// invocation (e.g. a `git init`-then-`canon init` sequence), so this
/// appends rather than mirroring `canon.yaml`'s create-fails-if-exists
/// refusal.
fn scaffold_gitignore(repo: &Path) -> std::io::Result<()> {
    let path = repo.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == GITIGNORE_LINE) {
        return Ok(());
    }
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(format!("# canon's sqlite hot tier (s32 sqlite-hot-backend) -- db file + WAL/SHM sidecars.\n{GITIGNORE_LINE}\n").as_bytes())?;
    Ok(())
}

/// `canon init [--repo <dir>]` (task 4.1). Returns the process exit
/// code: `0` on a fresh `canon.yaml` written, `2` on a refused
/// invocation (an existing `canon.yaml`) -- `create_new` (atomic
/// create-fails-if-exists), so the existing file's bytes are UNTOUCHED
/// either way.
pub fn run_init(repo: &Path) -> i32 {
    if let Err(e) = fs::create_dir_all(repo) {
        eprintln!("canon init: failed to create `{}`: {e}", repo.display());
        return 2;
    }
    let canon_yaml_path = repo.join("canon.yaml");
    let content = skeleton_yaml();
    match fs::OpenOptions::new().write(true).create_new(true).open(&canon_yaml_path) {
        Ok(mut file) => match file.write_all(content.as_bytes()) {
            Ok(()) => match scaffold_gitignore(repo) {
                Ok(()) => {
                    println!("canon init: wrote {}", canon_yaml_path.display());
                    0
                }
                Err(e) => {
                    eprintln!("canon init: wrote `{}` but failed to update `.gitignore`: {e}", canon_yaml_path.display());
                    2
                }
            },
            Err(e) => {
                eprintln!("canon init: failed to write `{}`: {e}", canon_yaml_path.display());
                2
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            eprintln!("canon init: refused — `{}` already exists; never overwriting an existing config", canon_yaml_path.display());
            2
        }
        Err(e) => {
            eprintln!("canon init: failed to create `{}`: {e}", canon_yaml_path.display());
            2
        }
    }
}

/// `canon init --check-config [--repo <dir>]` (task 4.2). Returns the
/// process exit code: `2` when `<repo>/canon.yaml` is missing (fails
/// loud, distinct from any content report), `0` when every PRESENT
/// section parses clean under its own existing loader, `1` when at
/// least one present section fails -- printing one PASS/FAIL/"not
/// configured" line per section regardless (never stopping at the
/// first failure, design D7).
pub fn run_check_config(repo: &Path) -> i32 {
    let canon_yaml_path = repo.join("canon.yaml");
    let Ok(text) = fs::read_to_string(&canon_yaml_path) else {
        eprintln!("canon init --check-config: refused — `{}` does not exist; run `canon init` first", canon_yaml_path.display());
        return 2;
    };

    let mut all_ok = true;
    let mut report = String::new();

    // s29 design D9: `TierPolicy::from_yaml_at` has no `canon-store`
    // dependency to call `validate_schema_ident` itself, so a
    // `tiers.<rung>.schema` `PgTier::connect` would reject at attach
    // time could otherwise parse clean here -- checked explicitly, so
    // `[PASS] tiers/routing/aging` can never be printed over a
    // malformed schema.
    match TierPolicy::from_yaml_at(&text, repo) {
        Ok(policy) => {
            let bad_schema = policy.tiers.values().find_map(|cfg| match cfg {
                BackendConfig::Postgres(pg) => canon_store::pg_tier::validate_schema_ident(&pg.schema).err(),
                _ => None,
            });
            match bad_schema {
                None => report.push_str("[PASS] tiers/routing/aging\n"),
                Some(e) => {
                    all_ok = false;
                    report.push_str(&format!("[FAIL] tiers/routing/aging: {e}\n"));
                }
            }
        }
        Err(e) => {
            all_ok = false;
            report.push_str(&format!("[FAIL] tiers/routing/aging: {e}\n"));
        }
    }

    // `load_spec_roots` resolves the single default root even for an
    // absent `specs:` key (a legitimate, already-successful state) --
    // this section is PASS/FAIL only, never "not configured".
    match crate::inventory::load_spec_roots(&canon_yaml_path) {
        Ok(_) => report.push_str("[PASS] specs\n"),
        Err(e) => {
            all_ok = false;
            report.push_str(&format!("[FAIL] specs: {e}\n"));
        }
    }

    // `load_plan_sources_from_config` itself can't distinguish a
    // legitimately ABSENT `plans:` key from a present-but-empty
    // `sources: []` (both resolve to `Ok(vec![])`, its own established
    // fail-soft-on-absent contract) -- that distinction is made here,
    // once, off the SAME already-parsed YAML doc, never a second
    // config parser.
    let plans_present = serde_yaml::from_str::<serde_yaml::Value>(&text).ok().and_then(|doc| doc.get("plans").cloned()).is_some();
    if !plans_present {
        report.push_str("[not configured] plans\n");
    } else {
        match crate::plans::load_plan_sources_from_config(&canon_yaml_path, repo) {
            Ok(_) => report.push_str("[PASS] plans\n"),
            Err(e) => {
                all_ok = false;
                report.push_str(&format!("[FAIL] plans: {e}\n"));
            }
        }
    }

    print!("{report}");
    if all_ok { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_yaml_routes_hot_class_kinds_to_hot_and_the_rest_to_local() {
        let yaml = skeleton_yaml();
        for kind in RecordKind::ALL {
            let expected = if HOT_KINDS.contains(&kind) { "hot" } else { "local" };
            assert!(
                yaml.contains(&format!("{}: {expected}", kind.as_str())),
                "missing `{expected}`-routing line for `{}`: {yaml}",
                kind.as_str()
            );
        }
    }

    #[test]
    fn skeleton_yaml_configures_hot_as_sqlite_with_a_resolved_path() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml_path = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml_path, skeleton_yaml()).unwrap();
        let text = std::fs::read_to_string(&canon_yaml_path).unwrap();

        let policy = TierPolicy::from_yaml_at(&text, dir.path()).unwrap();
        let hot = policy.tiers.get(&canon_store::policy::Rung::Hot).expect("scaffolded config must configure a `hot` rung");
        match hot {
            BackendConfig::Sqlite(cfg) => assert_eq!(cfg.path, dir.path().join(".canon/hot.db")),
            other => panic!("expected the scaffolded `hot` rung to be sqlite-backed, got {other:?}"),
        }
    }

    #[test]
    fn skeleton_yaml_parses_clean_through_every_existing_loader() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml_path = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml_path, skeleton_yaml()).unwrap();
        let text = std::fs::read_to_string(&canon_yaml_path).unwrap();

        assert!(TierPolicy::from_yaml_at(&text, dir.path()).is_ok());
        assert!(crate::inventory::load_spec_roots(&canon_yaml_path).is_ok());
        assert!(crate::plans::load_plan_sources_from_config(&canon_yaml_path, dir.path()).is_ok());
    }

    #[test]
    fn scaffold_gitignore_creates_a_fresh_file_with_the_hot_db_glob() {
        let dir = tempfile::tempdir().unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text.lines().any(|line| line.trim() == GITIGNORE_LINE), "{text}");
    }

    #[test]
    fn scaffold_gitignore_appends_to_an_existing_file_without_disturbing_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text.contains("target/\n"), "{text}");
        assert!(text.lines().any(|line| line.trim() == GITIGNORE_LINE), "{text}");
    }

    #[test]
    fn scaffold_gitignore_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text.matches(GITIGNORE_LINE).count(), 1, "the line must never be duplicated across repeated scaffolds: {text}");
    }
}
