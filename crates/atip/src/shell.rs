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

/// `command` split into words the way a POSIX shell would: whitespace
/// separates, single quotes keep everything, double quotes keep everything but
/// `\"` and `\\`.
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
}
