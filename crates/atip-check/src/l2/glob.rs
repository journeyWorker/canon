//! The glob of a fixture's `allowedEffects` (approaches/checker.md 4, 5).
//!
//! The checker does not fix a glob syntax; this is the one it reads, and the report of a change names it: `*` matches
//! any run of characters except `/`, `**` any run including `/` (`**/` also matches nothing), `?` one character
//! except `/`, every other character itself. A pattern names a path of the invocation's directory as `path/to/file`
//! (a leading `./` is allowed) and a path under `HOME` as `~/path/to/file`. A path matches when the whole path does.

/// Whether the changed path `changed` (`./a/b` in the directory, `~/a/b` in `HOME`, as `Outcome::changes` spells it) matches `pattern`.
pub fn matches(pattern: &str, changed: &str) -> bool {
    let (home, path) = match changed.strip_prefix("~/") {
        Some(rest) => (true, rest),
        None => (false, changed.strip_prefix("./").unwrap_or(changed)),
    };
    let pattern = match pattern.strip_prefix("~/") {
        Some(rest) if home => rest,
        Some(_) => return false,
        None if home => return false,
        None => pattern.strip_prefix("./").unwrap_or(pattern),
    };
    glob(pattern.as_bytes(), path.as_bytes())
}

fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) if rest.first() == Some(&b'*') => {
            let rest = &rest[1..];
            // `**/` also matches the empty path before it.
            if rest.first() == Some(&b'/') && glob(&rest[1..], text) {
                return true;
            }
            (0..=text.len()).any(|at| glob(rest, &text[at..]))
        }
        Some((b'*', rest)) => {
            let end = text.iter().position(|byte| *byte == b'/').unwrap_or(text.len());
            (0..=end).any(|at| glob(rest, &text[at..]))
        }
        Some((b'?', rest)) => text.split_first().is_some_and(|(byte, tail)| *byte != b'/' && glob(rest, tail)),
        Some((byte, rest)) => text.split_first().is_some_and(|(head, tail)| head == byte && glob(rest, tail)),
    }
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn a_pattern_matches_the_whole_path() {
        assert!(matches(".canon/audit/access.jsonl", "./.canon/audit/access.jsonl"));
        assert!(!matches(".canon/audit", "./.canon/audit/access.jsonl"));
        assert!(matches(".canon/audit/*.jsonl", "./.canon/audit/access.jsonl"));
        assert!(!matches("*.jsonl", "./.canon/audit/access.jsonl"));
        assert!(matches("**/*.jsonl", "./.canon/audit/access.jsonl"));
        assert!(matches("**/access.jsonl", "./access.jsonl"));
        assert!(matches(".canon/**", "./.canon/audit/access.jsonl"));
        assert!(matches("a?c", "./abc"));
        assert!(!matches("a?c", "./a/c"));
    }

    #[test]
    fn home_paths_have_their_own_prefix() {
        assert!(matches("~/.cache/tool/*", "~/.cache/tool/index"));
        assert!(!matches(".cache/tool/index", "~/.cache/tool/index"));
        assert!(!matches("~/index", "./index"));
    }
}
