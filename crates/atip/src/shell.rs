//! The shell form of a next-step `command`.
//!
//! A next step names its command as one string a human can paste; a caller
//! that runs it without a shell wants the argv. [`quote`] writes a word into
//! the string, [`split_argv`] reads the words back out, and
//! [`has_placeholder`] tells a command that is runnable from one that still
//! holds a `<value>` only the caller knows.

/// `arg` as one shell word: itself when it has only safe characters, else single-quoted.
pub fn quote(arg: &str) -> String {
    if !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./:=@,+".contains(&b)) {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

/// Whether `command` still holds a `<placeholder>`: a `<` opening a word and a later `>`.
pub fn has_placeholder(command: &str) -> bool {
    command.match_indices('<').any(|(at, _)| command[at + 1..].starts_with(|c: char| c.is_ascii_alphabetic()) && command[at..].contains('>'))
}

/// `command` split into words the way a POSIX shell would, without expansion:
/// whitespace separates; outside quotes a backslash makes the next character
/// literal (a backslash and a newline join the line, a lone backslash at the
/// end stays); single quotes keep everything; double quotes keep everything
/// but `\"` and `\\`. `split_argv(join(words.map(quote)))` is `words`.
pub fn split_argv(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    word.push(q);
                }
            }
            '"' => {
                started = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some(e @ ('"' | '\\')) => word.push(e),
                            Some(other) => {
                                word.push('\\');
                                word.push(other);
                            }
                            None => word.push('\\'),
                        },
                        other => word.push(other),
                    }
                }
            }
            '\\' => match chars.next() {
                // A backslash and a newline join the line: both vanish and no word starts.
                Some('\n') => {}
                Some(escaped) => {
                    started = true;
                    word.push(escaped);
                }
                // A lone backslash at the end of the input stays a backslash.
                None => {
                    started = true;
                    word.push('\\');
                }
            },
            other => {
                started = true;
                word.push(other);
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_leaves_safe_words_and_quotes_the_rest() {
        assert_eq!(quote("2026-01-01T00:00:00+00:00"), "2026-01-01T00:00:00+00:00");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn a_quoted_word_splits_back_to_itself() {
        for word in ["plain", "a b", "a\"b", "$(x);y", "tab\there", "back\\slash"] {
            assert_eq!(split_argv(&quote(word)), [word], "{word:?}");
        }
    }

    #[test]
    fn split_argv_reads_shell_words() {
        assert_eq!(split_argv("canon gate check"), ["canon", "gate", "check"]);
        assert_eq!(split_argv("  canon   status  "), ["canon", "status"]);
        assert_eq!(split_argv("canon x 'a b' \"c d\" e'f'\"g\""), ["canon", "x", "a b", "c d", "efg"]);
        assert_eq!(split_argv(r#"x "a \" \\ \n""#), ["x", r#"a " \ \n"#]);
        assert_eq!(split_argv("x '' y"), ["x", "", "y"]);
        assert!(split_argv("   ").is_empty());
    }

    #[test]
    fn a_placeholder_is_an_angle_word_with_a_closer() {
        assert!(has_placeholder("canon evidence add --scenario <SCENARIO_ID>"));
        assert!(has_placeholder("canon x <id>"));
        assert!(!has_placeholder("canon gate check"));
        assert!(!has_placeholder("canon x < input"));
        assert!(!has_placeholder("canon x <a"));
        assert!(!has_placeholder("canon x 1<2"));
    }

    fn join(words: &[String]) -> String {
        words.iter().map(|word| quote(word)).collect::<Vec<_>>().join(" ")
    }

    /// Every string of length 0 to `max` over `alphabet`, shortest first.
    fn strings_up_to(alphabet: &[char], max: usize) -> Vec<String> {
        let mut all = vec![String::new()];
        let mut layer = vec![String::new()];
        for _ in 0..max {
            layer = layer.iter().flat_map(|prefix| alphabet.iter().map(move |c| format!("{prefix}{c}"))).collect();
            all.extend(layer.iter().cloned());
        }
        all
    }

    /// Every list of 0 to `max` words drawn from `words`.
    fn lists_up_to(words: &[String], max: usize) -> Vec<Vec<String>> {
        let mut all = vec![Vec::new()];
        let mut layer: Vec<Vec<String>> = vec![Vec::new()];
        for _ in 0..max {
            layer = layer.iter().flat_map(|prefix| words.iter().map(move |word| [prefix.clone(), vec![word.clone()]].concat())).collect();
            all.extend(layer.iter().cloned());
        }
        all
    }

    const AWKWARD: [char; 9] = ['a', '\'', '"', '\\', ' ', '$', '`', '\n', '\t'];

    #[test]
    fn atip_crateshell_01_a_word_with_a_single_quote_is_written_and_read_back() {
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(split_argv(&quote("it's")), ["it's"]);
        assert_eq!(split_argv("canon x 'it'\\''s' y"), ["canon", "x", "it's", "y"]);
        assert_eq!(split_argv(&quote("'")), ["'"]);
        assert_eq!(split_argv(&quote("''")), ["''"]);
    }

    #[test]
    fn atip_crateshell_02_every_short_word_list_over_the_awkward_characters_round_trips() {
        // Every word of 0 to 4 characters, alone and in a list of two with every word of 0 to 2 characters.
        let words = strings_up_to(&AWKWARD, 4);
        assert_eq!(words.len(), 1 + 9 + 81 + 729 + 6561);
        for word in &words {
            let list = vec![word.clone()];
            assert_eq!(split_argv(&join(&list)), list, "{word:?}");
        }
        let short = strings_up_to(&AWKWARD, 2);
        for first in &short {
            for second in &words {
                let list = vec![first.clone(), second.clone()];
                assert_eq!(split_argv(&join(&list)), list, "{list:?}");
            }
        }
        // Every list of 0 to 4 words drawn from the empty word and the nine single characters.
        let singles: Vec<String> = std::iter::once(String::new()).chain(AWKWARD.iter().map(char::to_string)).collect();
        let lists = lists_up_to(&singles, 4);
        assert_eq!(lists.len(), 1 + 10 + 100 + 1000 + 10000);
        for list in &lists {
            assert_eq!(&split_argv(&join(list)), list, "{list:?}");
        }
    }

    #[test]
    fn atip_crateshell_03_a_backslash_outside_quotes_is_the_posix_one() {
        assert_eq!(split_argv(r"a\ b"), ["a b"]);
        assert_eq!(split_argv(r"a\\b"), [r"a\b"]);
        assert_eq!(split_argv(r"a\'b"), ["a'b"]);
        assert_eq!(split_argv(r#"a\"b"#), ["a\"b"]);
        assert_eq!(split_argv(r"a\$b"), ["a$b"]);
        assert_eq!(split_argv(r"\a"), ["a"]);
        // A backslash and a newline join the line: neither is kept, and between words they start no word.
        assert_eq!(split_argv("a\\\nb"), ["ab"]);
        assert_eq!(split_argv("a \\\n b"), ["a", "b"]);
        assert_eq!(split_argv("a \\\n"), ["a"]);
        assert!(split_argv("\\\n").is_empty());
        // A backslash that ends the input is one backslash, alone or in a word.
        assert_eq!(split_argv("a\\"), ["a\\"]);
        assert_eq!(split_argv("\\"), ["\\"]);
        assert_eq!(split_argv("a \\"), ["a", "\\"]);
        // An escaped space does not split, and an escaped newline outside the join form is a character.
        assert_eq!(split_argv("x\\\ty"), ["x\ty"]);
    }

    #[test]
    fn atip_crateshell_04_a_backslash_inside_quotes_is_unchanged_and_an_unclosed_quote_does_not_panic() {
        assert_eq!(split_argv(r#""a\xb""#), [r"a\xb"]);
        assert_eq!(split_argv(r#""\x""#), [r"\x"]);
        assert_eq!(split_argv(r#""a\"b\\c""#), ["a\"b\\c"]);
        assert_eq!(split_argv(r"'a\xb'"), [r"a\xb"]);
        assert_eq!(split_argv(r"'\'"), [r"\"]);
        assert_eq!(split_argv(r"'a\' b"), [r"a\", "b"]);
        // Quotes that never close read to the end of the input.
        assert_eq!(split_argv("'abc"), ["abc"]);
        assert_eq!(split_argv("x \"abc"), ["x", "abc"]);
        assert_eq!(split_argv("\"abc\\"), ["abc\\"]);
        assert_eq!(split_argv("'"), [""]);
        assert_eq!(split_argv("\""), [""]);
        // No input panics, whatever its quotes and backslashes.
        let words = strings_up_to(&['a', '\'', '"', '\\', ' ', '\n'], 6);
        for text in &words {
            let _ = split_argv(text);
        }
    }

    #[test]
    fn atip_crateshell_05_quote_writes_the_strings_canon_already_prints() {
        for (word, written) in [
            ("", "''"),
            ("plain", "plain"),
            ("a-b_c.d/e:f=g@h,i+j", "a-b_c.d/e:f=g@h,i+j"),
            ("2026-01-01T00:00:00+00:00", "2026-01-01T00:00:00+00:00"),
            ("a b", "'a b'"),
            ("it's", "'it'\\''s'"),
            ("'", "''\\'''"),
            ("a\"b", "'a\"b'"),
            ("back\\slash", "'back\\slash'"),
            ("$(x);y", "'$(x);y'"),
            ("tab\there", "'tab\there'"),
            ("line\nbreak", "'line\nbreak'"),
            ("a`b`", "'a`b`'"),
            ("--name=va lue", "'--name=va lue'"),
        ] {
            assert_eq!(quote(word), written, "{word:?}");
        }
    }

    #[test]
    fn atip_crateshell_07_a_word_that_would_break_unquoted_is_never_written_bare() {
        // Every word of 0 to 3 characters over the awkward ones: a word with a character outside the safe set, or the empty word, is quoted.
        let safe = |word: &str| !word.is_empty() && word.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./:=@,+".contains(&b));
        let mut bare = 0;
        for word in strings_up_to(&AWKWARD, 3) {
            let written = quote(&word);
            if safe(&word) {
                assert_eq!(written, word, "{word:?} is safe and stays bare");
                bare += 1;
                continue;
            }
            assert!(written.starts_with('\'') && written.ends_with('\''), "{word:?} is written {written:?}, not as a quoted word");
            assert_eq!(split_argv(&written), std::slice::from_ref(&word), "{word:?}");
            // Written raw, a word with white space, a quote or nothing in it does not come back as itself: the quote is what keeps it whole.
            if word.is_empty() || word.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"') {
                assert_ne!(split_argv(&word), std::slice::from_ref(&word), "{word:?} would survive unquoted, so quoting it is not needed");
            }
        }
        assert!(bare > 0, "the sweep holds words that need no quote");
        // The words that carry a shell's meaning are quoted and read back whole.
        for word in ["$HOME", "`date`", "a;b", "a|b", "a&b", "<id>", "*", "~", "a#b", "(x)"] {
            assert_ne!(quote(word), word, "{word}");
            assert_eq!(split_argv(&quote(word)), [word], "{word}");
        }
    }
}
