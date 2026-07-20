//! `canon learn promote <strategy_id>` (S6 `role-strategy-memory` task
//! group 4): promote a distilled [`canon_learn::StrategyItem`] from the
//! operator-local parquet warm tier up into the git-tracked, PR-reviewed
//! tier (`<repo>/canon/strategies/<role>/<id>.md`, `LearnConfig::
//! strategies_root`). The write path itself is `canon-learn`'s
//! ([`canon_learn::promote_strategy`]); this module only resolves the
//! repo's `canon.yaml`-configured store roots (mirroring
//! `canon_cli::artifact_ingest`'s own `learn.root`/`strategies_root`
//! resolution — never a second config-reading convention) and handles
//! the `--dry-run` preview + advisory-lint surfacing.
//!
//! Exit codes mirror the rest of `canon-cli`: `0` on a written (or
//! previewed) promotion, `2` on a usage error (an unparseable
//! `strategy_id` never reaches here — clap rejects it first), `1` on a
//! real failure (a malformed `learn:` config, an unknown `strategy_id`,
//! or a filesystem write error). The advisory lint (content length, a
//! literal machine-specific absolute path) NEVER changes the exit code:
//! its warnings go to stderr and the promotion proceeds.

use std::path::Path;
use std::process::ExitCode;

use canon_learn::{plan_promotion, promote_strategy, LearnConfig, ParquetStrategyStore, Promotion, StrategyId};

use crate::context::resolve_repo_root;

/// clap `value_parser` for the positional `<strategy_id>` — a ULID
/// ([`canon_learn::StrategyId::parse`]); a malformed id is a clap usage
/// error (exit `2`), never reaching [`run_promote`].
pub fn parse_strategy_id(s: &str) -> Result<StrategyId, String> {
    StrategyId::parse(s).map_err(|e| e.to_string())
}

/// `canon learn promote <strategy_id> [--repo <dir>] [--dry-run]`.
pub fn run_promote(repo: &Path, strategy_id: &StrategyId, dry_run: bool) -> ExitCode {
    let repo = resolve_repo_root(repo);
    let canon_yaml_text = std::fs::read_to_string(repo.join("canon.yaml")).unwrap_or_default();
    // A genuinely absent `learn:` section resolves to `LearnConfig::default()`
    // inside `from_manifest`; only a MALFORMED section reaches `Err`, and that
    // fails loud rather than silently promoting into the wrong store root
    // (same discipline `canon ingest artifacts` holds).
    let learn_config = match LearnConfig::from_manifest(&canon_yaml_text) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("canon learn promote: {err}");
            return ExitCode::from(1);
        }
    };

    let strategy_store = ParquetStrategyStore::open(repo.join(&learn_config.root).join("strategies"));
    let git_tier_root = repo.join(&learn_config.strategies_root);

    let outcome = if dry_run {
        plan_promotion(&strategy_store, strategy_id, &git_tier_root)
    } else {
        promote_strategy(&strategy_store, strategy_id, &git_tier_root)
    };

    match outcome {
        Ok(Promotion { path, warnings, .. }) => {
            for warning in &warnings {
                eprintln!("canon learn promote: advisory: {warning}");
            }
            if dry_run {
                println!("[dry-run] would promote {strategy_id} -> {}", path.display());
            } else {
                println!("promoted {strategy_id} -> {}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon learn promote: {err}");
            ExitCode::from(1)
        }
    }
}
