//! The two regular-expression engines of spec 10.7.4, and the comparison of their answers.
//!
//! A domain `pattern` is `^`, an I-Regexp (RFC 9485) body with the restrictions of 10.7.4, and `$`. The claim of the
//! contract is that, for a pattern of this form that both engines compile, the ECMAScript reading with the `u` flag
//! and the Rust `regex` crate's reading accept the same values, up to the Unicode version of each engine's `\p{…}`
//! tables. The checker is a Rust program, so both engines run in-process: [`regress`] is the ECMAScript engine and
//! [`regex`] (default limits: a compiled size of 10 MiB and a nesting depth of 250) is the Rust one. The contract
//! repository's own record of cases (`scripts/regex-cases.json`, pinned at the tag) is the evidence that `regress`
//! reads patterns of the form as Bun's `RegExp(pattern, "u")` does; `tests/regex_parity.rs` runs every case.

/// The Unicode version of the ECMAScript engine's tables (`regress` 0.12): U+11DB0 is a letter in it.
pub const ECMASCRIPT_UNICODE: &str = "17.0";
/// The Unicode version of the Rust engine's tables (`regex-syntax` 0.8.11): U+11DB0 is unassigned in it.
pub const RUST_UNICODE: &str = "16.0.0";

/// The two-letter general categories, to name the one a code point has in an engine's tables.
const CATEGORIES: [&str; 30] = [
    "Lu", "Ll", "Lt", "Lm", "Lo", "Mn", "Mc", "Me", "Nd", "Nl", "No", "Pc", "Pd", "Ps", "Pe", "Pi",
    "Pf", "Po", "Sm", "Sc", "Sk", "So", "Zs", "Zl", "Zp", "Cc", "Cf", "Cs", "Co", "Cn",
];

/// The escapes I-Regexp allows besides `\p{…}` and `\P{…}`.
const SINGLE_ESCAPES: &str = "()*+-.?[\\]^nrt{|}";

type Memo = std::sync::Mutex<std::collections::HashMap<String, Option<String>>>;
static FORM: std::sync::LazyLock<Memo> = std::sync::LazyLock::new(Memo::default);
static RUST: std::sync::LazyLock<Memo> = std::sync::LazyLock::new(Memo::default);

/// `compute(pattern)`, remembered: a pattern is compiled by the engines at every value a probe judges, and a large one (`\p{L}{1,500}`) takes seconds to compile.
fn memoised(memo: &Memo, pattern: &str, compute: fn(&str) -> Option<String>) -> Option<String> {
    if let Ok(known) = memo.lock() {
        if let Some(answer) = known.get(pattern) {
            return answer.clone();
        }
    }
    let answer = compute(pattern);
    if let Ok(mut known) = memo.lock() {
        known.insert(pattern.to_string(), answer.clone());
    }
    answer
}

/// The ECMAScript reading of `pattern` with the `u` flag.
pub fn ecmascript(pattern: &str) -> Result<regress::Regex, String> {
    regress::Regex::with_flags(pattern, "u").map_err(|error| error.to_string())
}

/// The Rust `regex` crate's reading of `pattern`, at its default limits.
pub fn rust(pattern: &str) -> Result<regex::Regex, String> {
    regex::Regex::new(pattern).map_err(|error| error.to_string())
}

/// What one engine says of one value: it accepts it, rejects it, or could not compile the pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Accepts(bool),
    Refuses(String),
}

impl Answer {
    /// The answer in the vocabulary of `regex-cases.json`: `true`, `false` or `"error"`.
    pub fn recorded(&self) -> serde_json::Value {
        match self {
            Self::Accepts(accepted) => serde_json::Value::Bool(*accepted),
            Self::Refuses(_) => serde_json::Value::String("error".to_string()),
        }
    }
}

/// What the ECMAScript engine says of `value`.
pub fn ecmascript_answer(pattern: &str, value: &str) -> Answer {
    match ecmascript(pattern) {
        Ok(engine) => Answer::Accepts(engine.find(value).is_some()),
        Err(reason) => Answer::Refuses(reason),
    }
}

/// What the Rust engine says of `value`.
pub fn rust_answer(pattern: &str, value: &str) -> Answer {
    match rust(pattern) {
        Ok(engine) => Answer::Accepts(engine.is_match(value)),
        Err(reason) => Answer::Refuses(reason),
    }
}

/// The general category of `code_point` in the ECMAScript engine's tables.
pub fn ecmascript_category(code_point: char) -> Option<&'static str> {
    let text = code_point.to_string();
    CATEGORIES.iter().copied().find(|category| {
        ecmascript(&format!("^\\p{{{category}}}$")).is_ok_and(|engine| engine.find(&text).is_some())
    })
}

/// The general category of `code_point` in the Rust engine's tables.
pub fn rust_category(code_point: char) -> Option<&'static str> {
    let text = code_point.to_string();
    CATEGORIES.iter().copied().find(|category| {
        rust(&format!("^\\p{{{category}}}$")).is_ok_and(|engine| engine.is_match(&text))
    })
}

/// A verified difference between the Unicode versions of the two engines (spec 10.7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnicodeDifference {
    /// `U+XXXX`.
    pub code_point: String,
    pub ecmascript_category: &'static str,
    pub rust_category: &'static str,
}

impl UnicodeDifference {
    /// The sentence a report carries: both Unicode versions, the code point and the category each engine gives it.
    pub fn describe(&self) -> String {
        format!(
            "{} is {} in the ECMAScript engine's Unicode {ECMASCRIPT_UNICODE} tables and {} in the Rust engine's Unicode {RUST_UNICODE} tables",
            self.code_point, self.ecmascript_category, self.rust_category
        )
    }
}

/// How the two engines compare on one pattern and one value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comparison {
    /// Both engines compile the pattern and give the same answer (`true`: both accept the value).
    Agree(bool),
    /// The answers differ and the difference is a verified Unicode-version difference: not a failure.
    UnicodeVersion {
        ecmascript: bool,
        rust: bool,
        difference: UnicodeDifference,
    },
    /// The answers differ and nothing explains it: a failure.
    Differs { ecmascript: bool, rust: bool },
    /// One or both engines refuse the pattern; each refusal is its reason, `None` when that engine compiles it.
    Uncompilable {
        ecmascript: Option<String>,
        rust: Option<String>,
    },
}

/// The code point `value` is, when it is exactly one.
fn single_code_point(value: &str) -> Option<char> {
    let mut chars = value.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}

/// Whether `pattern` uses a Unicode category: `\p{…}` or `\P{…}`.
fn uses_category(pattern: &str) -> bool {
    pattern.contains("\\p{") || pattern.contains("\\P{")
}

/// Runs `value` through both engines. A different answer is a verified Unicode-version difference only when the value
/// is one code point, the pattern uses a category, and each engine's own tables give the code point a different general
/// category (spec 10.7.4).
pub fn compare(pattern: &str, value: &str) -> Comparison {
    let (ecmascript, rust) = (ecmascript_answer(pattern, value), rust_answer(pattern, value));
    match (ecmascript, rust) {
        (Answer::Accepts(es), Answer::Accepts(rs)) if es == rs => Comparison::Agree(es),
        (Answer::Accepts(es), Answer::Accepts(rs)) => {
            let difference = single_code_point(value)
                .filter(|_| uses_category(pattern))
                .and_then(|code_point| {
                    let (in_es, in_rust) = (ecmascript_category(code_point)?, rust_category(code_point)?);
                    (in_es != in_rust).then(|| UnicodeDifference {
                        code_point: format!("U+{:04X}", u32::from(code_point)),
                        ecmascript_category: in_es,
                        rust_category: in_rust,
                    })
                });
            match difference {
                Some(difference) => Comparison::UnicodeVersion {
                    ecmascript: es,
                    rust: rs,
                    difference,
                },
                None => Comparison::Differs {
                    ecmascript: es,
                    rust: rs,
                },
            }
        }
        (es, rs) => Comparison::Uncompilable {
            ecmascript: match es {
                Answer::Refuses(reason) => Some(reason),
                Answer::Accepts(_) => None,
            },
            rust: match rs {
                Answer::Refuses(reason) => Some(reason),
                Answer::Accepts(_) => None,
            },
        },
    }
}

/// Why `pattern` is not of the form of spec 10.7.4: `^`, an I-Regexp body with its restrictions, `$`, and it compiles in the
/// ECMAScript engine. `None` when it is. (Compiling in the Rust engine is [`rust_problem`].)
pub fn form_problem(pattern: &str) -> Option<String> {
    memoised(&FORM, pattern, uncached_form_problem)
}

fn uncached_form_problem(pattern: &str) -> Option<String> {
    let body = pattern
        .strip_prefix('^')
        .and_then(|rest| rest.strip_suffix('$'))
        .filter(|_| pattern.len() >= 2);
    let Some(body) = body else {
        return Some("is not ^…$".to_string());
    };
    if let Some(problem) = body_problem(body) {
        return Some(problem);
    }
    ecmascript(pattern)
        .err()
        .map(|reason| format!("does not compile as an ECMAScript pattern with the u flag: {reason}"))
}

/// Why the Rust `regex` crate refuses `pattern` at its default limits; `None` when it compiles.
pub fn rust_problem(pattern: &str) -> Option<String> {
    memoised(&RUST, pattern, uncached_rust_problem)
}

fn uncached_rust_problem(pattern: &str) -> Option<String> {
    rust(pattern)
        .err()
        .map(|reason| format!("the Rust regex crate refuses it with its default limits: {}", reason.lines().last().unwrap_or_default().trim()))
}

/// The I-Regexp (RFC 9485) syntax of the body of a domain pattern with the restrictions of spec 10.7.4 that make the two
/// readings of `^body$` agree: no `|` outside parentheses, no `.`, `^` or `$` outside a character class, no `\-` outside
/// one, and no `&&` or `~~` inside one. The first problem, or `None`.
pub fn body_problem(body: &str) -> Option<String> {
    let mut parser = Body {
        cps: body.chars().collect(),
        i: 0,
    };
    let outcome = parser.alternatives(0).and_then(|()| match parser.at(0) {
        Some(c) => parser.fail(&format!("unmatched {c}")),
        None => Ok(()),
    });
    outcome.err()
}

struct Body {
    cps: Vec<char>,
    i: usize,
}

impl Body {
    fn at(&self, offset: usize) -> Option<char> {
        self.cps.get(self.i + offset).copied()
    }

    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at offset {}", self.i))
    }

    fn is_category(&self) -> bool {
        self.at(0) == Some('\\') && matches!(self.at(1), Some('p' | 'P'))
    }

    fn escape(&mut self, in_class: bool) -> Result<(), String> {
        if self.is_category() {
            if self.at(2) != Some('{') {
                return self.fail("\\p without {");
            }
            let Some(end) = self.cps[self.i + 3..].iter().position(|c| *c == '}') else {
                return self.fail("unclosed \\p{");
            };
            let name: String = self.cps[self.i + 3..self.i + 3 + end].iter().collect();
            if !is_category_name(&name) {
                return self.fail(&format!("unknown category {name}"));
            }
            self.i += 3 + end + 1;
            return Ok(());
        }
        let c = self.at(1);
        match c {
            Some(c) if SINGLE_ESCAPES.contains(c) => {
                if c == '-' && !in_class {
                    return self.fail("\\- outside a character class");
                }
                self.i += 2;
                Ok(())
            }
            other => self.fail(&format!(
                "escape \\{} is not I-Regexp",
                other.map(String::from).unwrap_or_default()
            )),
        }
    }

    fn class_char(&mut self) -> Result<bool, String> {
        let Some(c) = self.at(0) else {
            return Ok(false);
        };
        if c == '\\' {
            self.escape(true)?;
            return Ok(true);
        }
        if c == '-' || c == '[' || c == ']' {
            return Ok(false);
        }
        if (c == '&' || c == '~') && self.at(1) == Some(c) {
            return self.fail(&format!("{c}{c} in a character class (a set operation in Rust)"));
        }
        self.i += 1;
        Ok(true)
    }

    fn char_class(&mut self) -> Result<(), String> {
        self.i += 1;
        if self.at(0) == Some('^') {
            self.i += 1;
        }
        let mut items = 0;
        if self.at(0) == Some('-') {
            self.i += 1;
            items += 1;
        }
        while self.at(0) != Some(']') {
            if self.at(0).is_none() {
                return self.fail("unclosed [");
            }
            if self.is_category() {
                self.escape(true)?;
            } else if self.at(0) == Some('-') && self.at(1) == Some(']') {
                self.i += 1;
            } else {
                if !self.class_char()? {
                    let c = self.at(0).map(String::from).unwrap_or_default();
                    return self.fail(&format!("{c} in a character class"));
                }
                if self.at(0) == Some('-') && self.at(1) != Some(']') {
                    self.i += 1;
                    if self.is_category() || !self.class_char()? {
                        return self.fail("bad range");
                    }
                }
            }
            items += 1;
        }
        if items == 0 {
            return self.fail("empty character class");
        }
        self.i += 1;
        Ok(())
    }

    fn digits(&mut self) -> Result<(), String> {
        if !self.at(0).is_some_and(|c| c.is_ascii_digit()) {
            return self.fail("quantifier without digits");
        }
        while self.at(0).is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        Ok(())
    }

    fn quantifier(&mut self) -> Result<(), String> {
        match self.at(0) {
            Some('*' | '+' | '?') => self.i += 1,
            Some('{') => {
                self.i += 1;
                self.digits()?;
                if self.at(0) == Some(',') {
                    self.i += 1;
                    if self.at(0).is_some_and(|c| c.is_ascii_digit()) {
                        self.digits()?;
                    }
                }
                if self.at(0) != Some('}') {
                    return self.fail("unclosed {");
                }
                self.i += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn alternatives(&mut self, depth: usize) -> Result<(), String> {
        self.branch(depth)?;
        while self.at(0) == Some('|') {
            if depth == 0 {
                return self.fail("| outside parentheses");
            }
            self.i += 1;
            self.branch(depth)?;
        }
        Ok(())
    }

    fn branch(&mut self, depth: usize) -> Result<(), String> {
        while let Some(c) = self.at(0) {
            if c == '|' || c == ')' {
                break;
            }
            match c {
                '(' => {
                    self.i += 1;
                    self.alternatives(depth + 1)?;
                    if self.at(0) != Some(')') {
                        return self.fail("unclosed (");
                    }
                    self.i += 1;
                }
                '[' => self.char_class()?,
                '\\' => self.escape(false)?,
                '.' => return self.fail(". outside a character class"),
                '^' | '$' => return self.fail(&format!("{c} outside a character class")),
                '*' | '+' | '?' | '{' | '}' | ']' => {
                    return self.fail(&format!("{c} without an atom"))
                }
                _ => self.i += 1,
            }
            self.quantifier()?;
        }
        Ok(())
    }
}

/// `L`, `Lu`, ... : the general categories I-Regexp allows.
fn is_category_name(name: &str) -> bool {
    let mut chars = name.chars();
    let (Some(first), second, None) = (chars.next(), chars.next(), chars.next().map(|_| ())) else {
        return false;
    };
    let rest = match first {
        'L' => "lmotu",
        'M' => "cen",
        'N' => "dlo",
        'P' => "cdefios",
        'Z' => "lps",
        'S' => "ckmo",
        'C' => "cfno",
        _ => return false,
    };
    second.is_none_or(|c| rest.contains(c))
}

#[cfg(test)]
mod tests {
    use super::{body_problem, compare, ecmascript_category, form_problem, is_category_name, rust_category, rust_problem, Comparison};

    #[test]
    fn atip_checkl1_regex_the_form_of_a_pattern_is_the_restricted_i_regexp() {
        for good in [
            "^[a-z][a-z0-9-]*$",
            "^(core|lead)$",
            "^\\p{L}+$",
            "^[^a]$",
            "^a{1,3}b?$",
            "^[\\-a]$",
            "^[a\\-]$",
            "^x-[a&b]$",
            "^[a~b]$",
        ] {
            assert_eq!(form_problem(good), None, "{good}");
        }
        for (bad, why) in [
            ("[a-z]+", "^…$"),
            ("^a|b$", "| outside"),
            ("^a.b$", "."),
            ("^a\\-b$", "\\-"),
            ("^[a&&b]+$", "&&"),
            ("^[a~~b]+$", "~~"),
            ("^\\d$", "escape"),
            ("^(?:a)$", "without an atom"),
            ("^a^b$", "^"),
            ("^[[:alpha:]]$", "character class"),
            ("^[]$", "character class"),
            ("^\\p{Xx}$", "category"),
            ("^a{2$", "unclosed"),
            ("^(a$", "unclosed"),
            ("^a)$", "unmatched"),
        ] {
            let problem = form_problem(bad).unwrap_or_else(|| panic!("{bad} must be refused"));
            assert!(problem.contains(why), "{bad}: {problem}");
        }
        assert_eq!(body_problem("a"), None);
    }

    #[test]
    fn atip_checkl1_regex_category_names_are_the_general_categories_of_i_regexp() {
        for name in ["L", "Lu", "Ll", "Nd", "Pc", "Zs", "Cf", "Sm", "Mn"] {
            assert!(is_category_name(name), "{name}");
        }
        for name in ["", "X", "Lx", "Luu", "Cs", "Zx"] {
            assert!(!is_category_name(name), "{name}");
        }
    }

    #[test]
    fn atip_checkl1_regex_the_rust_engine_refuses_a_pattern_over_its_size_limit() {
        assert_eq!(rust_problem("^\\p{L}{1,10}$"), None);
        let problem = rust_problem("^\\p{L}{1,500}$").expect("over the compiled size limit");
        assert!(problem.contains("Rust regex"), "{problem}");
    }

    #[test]
    fn atip_checkl1_regex_a_letter_assigned_in_unicode_17_is_a_version_difference_and_nothing_else_is() {
        assert_eq!(ecmascript_category('\u{11DB0}'), Some("Lo"));
        assert_eq!(rust_category('\u{11DB0}'), Some("Cn"));
        // Unicode 16 added the Garay letters: both engines have them.
        assert_eq!(rust_category('\u{10D50}'), Some("Lu"));
        assert_eq!(ecmascript_category('\u{10D50}'), Some("Lu"));
        match compare("^\\p{L}$", "\u{11DB0}") {
            Comparison::UnicodeVersion { ecmascript, rust, difference } => {
                assert!(ecmascript && !rust);
                assert_eq!(difference.code_point, "U+11DB0");
                assert!(difference.describe().contains("Unicode 17.0") && difference.describe().contains("Unicode 16.0.0"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(compare("^\\p{L}$", "a"), Comparison::Agree(true));
        assert_eq!(compare("^[a&&b]+$", "a"), Comparison::Differs { ecmascript: true, rust: false });
        // The same value with no category in the pattern is not explained by a Unicode version.
        assert_eq!(
            compare("^[\u{11DB0}]$", "\u{11DB0}"),
            Comparison::Agree(true)
        );
        assert!(matches!(compare("^\\p{L}{1,500}$", "a"), Comparison::Uncompilable { ecmascript: None, rust: Some(_) }));
    }
}
