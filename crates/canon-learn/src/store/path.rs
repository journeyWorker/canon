//! Hive-style regime-key -> filesystem path derivation shared by both
//! parquet stores: `<store_root>/<role>/<repo>/<area>/<hash>/`. One
//! trajectory/strategy row per file inside that leaf directory
//! (`<id>.parquet`) — mirrors `canon-store::r2_tier`'s "one object per
//! write" append-log shape, just on a plain local directory instead of
//! `ObjectStore`.

use std::path::{Path, PathBuf};

use canon_model::ids::RegimeKey;

use crate::error::LearnError;

/// Rejects a regime-key segment that would be an unsafe path
/// component. `regime_key()`'s own canonicalization already guarantees
/// every segment is non-empty, `/`-free, lowercase-alnum-and-dash — so
/// this never actually trips on a `RegimeKey` produced through the
/// canonical path; it exists as defense-in-depth against a
/// hand-constructed `RegimeKey::parse` value reaching this store
/// directly.
fn safe_segment(segment: &str) -> Result<&str, LearnError> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Err(LearnError::UnsafePathSegment(segment.to_string()));
    }
    Ok(segment)
}

/// `<store_root>/<role>/<repo>/<area>/<hash>` — the directory every row
/// for this exact `regime_key` lives under.
pub(crate) fn namespace_dir(store_root: &Path, regime_key: &RegimeKey) -> Result<PathBuf, LearnError> {
    let mut dir = store_root.to_path_buf();
    dir.push(safe_segment(regime_key.role())?);
    dir.push(safe_segment(regime_key.repo())?);
    dir.push(safe_segment(regime_key.area())?);
    dir.push(safe_segment(regime_key.hash())?);
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regime() -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key("dev", "repo", "auth", "abc123")).unwrap()
    }

    #[test]
    fn namespace_dir_nests_every_segment() {
        let dir = namespace_dir(Path::new("/root"), &regime()).unwrap();
        assert_eq!(dir, PathBuf::from("/root/dev/repo/auth/abc123"));
    }
}
