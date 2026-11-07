//! The path glob of canon (0.39, `canon-0-39-staleness-scope`): `code.exclude`, the policy's `code_scopes` and the
//! `--surface-ref` of evidence all read a pattern with this one matcher.
//!
//! A pattern is matched against a repository-relative path with `/` separators:
//!
//! - `*` matches any run of bytes except `/`; `?` matches one byte except `/`; `**` matches any run of bytes
//!   including `/`; every other byte is literal. A `?` counts one byte, not one character.
//! - A pattern that names a directory also matches everything below it (`src` matches `src/a.rs`).
//! - A leading `/` is dropped, and so is a trailing one (`src/` is `src`).
//! - [`Anchor`] says where a pattern without a `/` starts. `code.exclude` and `code_scopes` match it at any depth
//!   ([`Anchor::Anywhere`]: `*.md` matches `README.md` and `a/b/c.md`); a surface ref matches it at the root
//!   ([`Anchor::Root`]: `Cargo.toml` is the file at the root, not every manifest). A pattern with a `/` in it is
//!   anchored at the root either way.
//!
//! The matcher runs the pattern as an automaton over the path: one pass, time proportional to the length of the
//! pattern times the length of the path, whatever the stars are. A pattern is compiled once ([`Glob::new`]) and
//! matched against many paths.

/// Where a pattern without a `/` starts matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// At any depth: it matches a file name, or the name of a directory above the file.
    Anywhere,
    /// At the repository root only.
    Root,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Byte(u8),
    /// `?`.
    One,
    /// `*`.
    Run,
    /// `**`.
    Deep,
}

impl Token {
    fn is_star(self) -> bool {
        matches!(self, Self::Run | Self::Deep)
    }
}

/// A compiled pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glob {
    tokens: Vec<Token>,
    /// A pattern without a `/` under [`Anchor::Anywhere`]: it is matched against each name of the path.
    by_name: bool,
}

/// The bit sets of the automaton fit on the stack up to this many `u64` words (three sets of 4 words: 255 tokens).
const STACK_WORDS: usize = 12;

impl Glob {
    /// Compile `pattern`. A pattern that is empty once its leading and trailing `/` are dropped matches nothing.
    pub fn new(pattern: &str, anchor: Anchor) -> Self {
        let slashed = pattern.contains('/');
        let bytes = pattern.trim_matches('/').as_bytes();
        let mut tokens: Vec<Token> = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'*' => {
                    let run = bytes[i..].iter().take_while(|&&b| b == b'*').count();
                    tokens.push(if run > 1 { Token::Deep } else { Token::Run });
                    i += run;
                    continue;
                }
                b'?' => tokens.push(Token::One),
                byte => tokens.push(Token::Byte(byte)),
            }
            i += 1;
        }
        Self { tokens, by_name: !slashed && anchor == Anchor::Anywhere }
    }

    /// Whether the pattern matches `path`, or a directory above it.
    pub fn matches(&self, path: &str) -> bool {
        if self.tokens.is_empty() {
            return false;
        }
        let words = (self.tokens.len() + 1).div_ceil(64);
        let mut stack = [0_u64; STACK_WORDS];
        let mut heap;
        let sets: &mut [u64] = if words * 3 <= STACK_WORDS {
            &mut stack[..words * 3]
        } else {
            heap = vec![0_u64; words * 3];
            &mut heap
        };
        let (begin, rest) = sets.split_at_mut(words);
        let (live, next) = rest.split_at_mut(words);
        self.scan(path.as_bytes(), begin, live, next)
    }

    /// Run the automaton over `path`. The three sets are zeroed and hold `tokens + 1` bits each: bit `j` of a set
    /// says that the first `j` tokens have matched what was read. `begin` is where a name starts, `live` where the
    /// run is, `next` the room for the step.
    fn scan(&self, path: &[u8], begin: &mut [u64], live: &mut [u64], next: &mut [u64]) -> bool {
        let done = self.tokens.len();
        set(begin, 0);
        self.close(begin);
        live.copy_from_slice(begin);
        for at in 0..=path.len() {
            // The end of a name, of a directory on the way, or of the path: the pattern may be done.
            if (at == path.len() || path[at] == b'/') && has(live, done) {
                return true;
            }
            let Some(&byte) = path.get(at) else { break };
            if byte == b'/' && self.by_name {
                live.copy_from_slice(begin);
                continue;
            }
            next.fill(0);
            let mut any = false;
            for (word, bits) in live.iter().copied().enumerate() {
                let mut bits = bits;
                while bits != 0 {
                    let j = word * 64 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let Some(&token) = self.tokens.get(j) else { continue };
                    let to = match token {
                        Token::Byte(expected) => (byte == expected).then_some(j + 1),
                        Token::One => (byte != b'/').then_some(j + 1),
                        Token::Run => (byte != b'/').then_some(j),
                        Token::Deep => Some(j),
                    };
                    if let Some(to) = to {
                        set(next, to);
                        any = true;
                    }
                }
            }
            if !any && !self.by_name {
                return false;
            }
            self.close(next);
            live.copy_from_slice(next);
        }
        false
    }

    /// Add the positions reached by skipping a star that matches nothing.
    fn close(&self, set_of: &mut [u64]) {
        for (j, token) in self.tokens.iter().enumerate() {
            if token.is_star() && has(set_of, j) {
                set(set_of, j + 1);
            }
        }
    }
}

fn has(set_of: &[u64], j: usize) -> bool {
    set_of[j / 64] & (1 << (j % 64)) != 0
}

fn set(set_of: &mut [u64], j: usize) {
    set_of[j / 64] |= 1 << (j % 64);
}

/// Whether `pattern` matches `path` with [`Anchor::Anywhere`] (`code.exclude`, `code_scopes`).
pub fn matches(pattern: &str, path: &str) -> bool {
    Glob::new(pattern, Anchor::Anywhere).matches(path)
}

/// Whether `pattern` matches `path` with [`Anchor::Root`] (a surface ref).
pub fn matches_rooted(pattern: &str, path: &str) -> bool {
    Glob::new(pattern, Anchor::Root).matches(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The matcher before 0.39: a recursion per `*`. It stands here as the oracle of the small inputs the new one
    /// must answer the same, with a trailing `/` of the pattern left out (the new one drops it).
    fn recursive(glob: &str, path: &str, anchored: bool) -> bool {
        fn wild(glob: &[u8], text: &[u8]) -> bool {
            match glob.split_first() {
                None => text.is_empty(),
                Some((b'*', rest)) if rest.first() == Some(&b'*') => {
                    let rest = &rest[rest.iter().take_while(|&&c| c == b'*').count()..];
                    (0..=text.len()).any(|i| wild(rest, &text[i..]))
                }
                Some((b'*', rest)) => (0..=text.iter().position(|&c| c == b'/').unwrap_or(text.len())).any(|i| wild(rest, &text[i..])),
                Some((b'?', rest)) => text.first().is_some_and(|&c| c != b'/') && wild(rest, &text[1..]),
                Some((c, rest)) => text.first() == Some(c) && wild(rest, &text[1..]),
            }
        }
        let glob = glob.trim_start_matches('/');
        if anchored {
            path.match_indices('/').any(|(i, _)| wild(glob.as_bytes(), &path.as_bytes()[..i])) || wild(glob.as_bytes(), path.as_bytes())
        } else {
            path.split('/').any(|name| wild(glob.as_bytes(), name.as_bytes()))
        }
    }

    fn strings(alphabet: &[char], longest: usize) -> Vec<String> {
        let mut all = vec![String::new()];
        let mut level = vec![String::new()];
        for _ in 0..longest {
            level = level.iter().flat_map(|s| alphabet.iter().map(move |c| format!("{s}{c}"))).collect();
            all.extend(level.iter().cloned());
        }
        all
    }

    #[test]
    fn the_automaton_answers_what_the_recursion_answered_on_every_small_pattern_and_path() {
        let patterns: Vec<String> = strings(&['a', 'b', '/', '*', '?'], 4).into_iter().filter(|p| !p.trim_matches('/').is_empty() && !p.ends_with('/')).collect();
        let paths: Vec<String> = strings(&['a', 'b', '/'], 5).into_iter().filter(|p| !p.is_empty()).collect();
        for pattern in &patterns {
            let anywhere = Glob::new(pattern, Anchor::Anywhere);
            let root = Glob::new(pattern, Anchor::Root);
            for path in &paths {
                let anchored = pattern.contains('/');
                assert_eq!(anywhere.matches(path), recursive(pattern, path, anchored), "{pattern:?} against {path:?}, anywhere");
                assert_eq!(root.matches(path), recursive(pattern, path, true), "{pattern:?} against {path:?}, rooted");
            }
        }
    }

    #[test]
    fn star_and_question_mark_stop_at_a_slash_and_double_star_does_not() {
        for (pattern, path, expected) in [
            ("src/*.rs", "src/a.rs", true),
            ("src/*.rs", "src/x/b.rs", false),
            ("src/**", "src/x/b.rs", true),
            ("src/**/b.rs", "src/b.rs", false),
            ("src/**/b.rs", "src/x/y/b.rs", true),
            ("a?c", "abc", true),
            ("a?c", "a/c", false),
            ("a**c", "a/b/c", true),
            ("a*c", "a/c", false),
        ] {
            assert_eq!(matches_rooted(pattern, path), expected, "{pattern} against {path}");
        }
    }

    #[test]
    fn a_name_that_is_a_directory_matches_everything_below_it() {
        for pattern in ["src", "src/", "/src", "/src/", "src/**", "s*c"] {
            assert!(matches_rooted(pattern, "src/a.rs"), "{pattern}");
            assert!(matches_rooted(pattern, "src/x/b.rs"), "{pattern}");
        }
        assert!(!matches_rooted("src", "srcs/a.rs"));
        assert!(!matches_rooted("src", "lib/src/a.rs"));
        assert!(!matches_rooted("src/*", "src"), "the slash of the pattern needs one in the path");
    }

    #[test]
    fn a_pattern_without_a_slash_is_anchored_at_the_root_by_the_root_anchor_and_at_any_depth_by_the_other() {
        assert!(matches_rooted("Cargo.toml", "Cargo.toml"));
        assert!(!matches_rooted("Cargo.toml", "crates/x/Cargo.toml"));
        assert!(!matches_rooted("*.md", "docs/a.md"));
        assert!(matches_rooted("*.md", "a.md"));
        assert!(matches("Cargo.toml", "crates/x/Cargo.toml"));
        assert!(matches("*.md", "docs/a.md"));
        assert!(matches("docs", "docs/a.md"));
        assert!(matches("a", "x/a/b.rs"), "a directory of that name above the file");
    }

    #[test]
    fn a_pattern_with_a_slash_is_anchored_at_the_root_whichever_the_anchor() {
        assert!(matches("docs/**", "docs/a.md"));
        assert!(!matches("docs/**", "x/docs/a.md"));
        assert!(matches("docs/", "docs/a.md"), "a trailing slash is dropped");
        assert!(!matches("docs/", "x/docs/a.md"), "and the pattern stays anchored");
    }

    #[test]
    fn an_empty_pattern_matches_nothing() {
        for pattern in ["", "/", "//"] {
            assert!(!matches(pattern, "a"), "{pattern:?}");
            assert!(!matches_rooted(pattern, "a"), "{pattern:?}");
        }
    }

    #[test]
    fn a_question_mark_counts_one_byte() {
        assert!(!matches_rooted("a?b", "a\u{e9}b"), "two bytes are not one");
        assert!(matches_rooted("a??b", "a\u{e9}b"));
    }

    #[test]
    fn a_pattern_of_twenty_stars_over_a_path_of_two_hundred_bytes_ends_at_once() {
        let pattern = "*a".repeat(20) + "z";
        let path = "a".repeat(200);
        let started = std::time::Instant::now();
        assert!(!matches(&pattern, &path));
        assert!(!matches_rooted(&pattern, &path));
        let deep = "**a".repeat(20) + "z";
        assert!(!matches(&deep, &format!("{}/{}", "a".repeat(100), "a".repeat(99))));
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "took {:?}", started.elapsed());
    }

    #[test]
    fn a_pattern_longer_than_the_stack_sets_is_matched_the_same() {
        let long = "a".repeat(300);
        assert!(matches_rooted(&long, &format!("{long}/b")));
        assert!(!matches_rooted(&long, &"a".repeat(299)));
        assert!(matches_rooted(&format!("{}**", "a".repeat(300)), &format!("{long}x/y")));
    }
}
