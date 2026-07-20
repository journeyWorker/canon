//! The kebab-token grammar (design.md D2, tasks.md 1.4; `plugin-overlay-
//! registry` spec): `[a-z0-9]+(-[a-z0-9]+)*` -- a manifest's `namespace`
//! or any overlay `kind` failing this grammar fails resolution with a
//! diagnostic, exactly like a missing required field. So the combined
//! `<namespace>.<kind>` on-disk directory string is exactly two
//! dot-joined kebab tokens, containing no `/`, `..`, or other path
//! separator.
//!
//! Mirrors `canon_model::ids`'s private `is_kebab_slug` helper
//! (`crates/canon-model/src/ids.rs:119-125`) by INSPIRATION -- that
//! helper is not `pub`, and this crate's grammar is its own vocabulary
//! (a namespace/overlay-kind token, not a join-spine key), so there is
//! nothing to import even if it were exported.

/// `true` iff `s` matches `[a-z0-9]+(-[a-z0-9]+)*`: non-empty, lowercase
/// alphanumeric segments joined by single hyphens -- no leading/trailing
/// hyphen, no `--`, no uppercase, no underscore, no `.`/`/`.
pub fn is_kebab_token(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_single_and_multi_segment_tokens() {
        assert!(is_kebab_token("porting"));
        assert!(is_kebab_token("porting-coverage"));
        assert!(is_kebab_token("a1-b2-c3"));
        assert!(is_kebab_token("a"));
        assert!(is_kebab_token("9"));
    }

    #[test]
    fn rejects_empty_uppercase_underscore_and_path_separators() {
        assert!(!is_kebab_token(""));
        assert!(!is_kebab_token("Porting_Two"));
        assert!(!is_kebab_token("coverage/extra"));
        assert!(!is_kebab_token("coverage.extra"));
        assert!(!is_kebab_token("has space"));
    }

    #[test]
    fn rejects_leading_trailing_and_doubled_hyphens() {
        assert!(!is_kebab_token("-porting"));
        assert!(!is_kebab_token("porting-"));
        assert!(!is_kebab_token("por--ting"));
    }
}
