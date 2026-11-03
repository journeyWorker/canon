//! The arguments of an invocation as a tool hands them to the crate, and the argv they are written back as.
//!
//! A tool normalises what the invocation gave (an integer is a JSON integer whether it was `--round=1` or `--round 1`,
//! a `json` argument is a JSON value) and lists it as [`Given`]s: the explicit arguments, and the values an option's
//! `env` variable supplied. A value filled in from a `default` is not a [`Given`]. The cursor of [`crate::cursor`] binds
//! to them, the preview of a plan digests them (spec 12.6), and both write the next call from them with
//! [`write_argv`]: the forms of spec 9.12.3 (`--name=value` for every valued option, a switch as `--name`, a count's name repeated, the operands after one `--`), so an invocation that read `--input -` or a file is written as the
//! options it stands for (spec 12.5), never as `--input -`.

use serde_json::Value;

use crate::jcs;

/// Where a value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The invocation gave it (on argv, or in an `--input` document, which says the same).
    Explicit,
    /// The option's `env` variable supplied it (spec 11.7 step 5).
    Env,
}

/// How an argument is written on argv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An option without a value: `--name` when the value is `true`.
    Switch,
    /// `--name` repeated as many times as the value says.
    Count,
    /// `--name=value`, the value a JSON scalar, or any JSON value written as compact JSON (a `json` option).
    Value,
    /// `--name=value` once for each item of the value, an array, in its order.
    Repeated,
    /// The operands: the items of the value, an array, as words after one `--`.
    Operands,
}

/// One argument of an invocation, normalised.
#[derive(Debug, Clone, PartialEq)]
pub struct Given {
    /// The option's long name without the dashes (`round`, `plan-digest`); `operands` for the operands.
    pub name: String,
    pub kind: Kind,
    /// The normalised value: `true` for a switch, a number for a count, an array for [`Kind::Repeated`] and [`Kind::Operands`].
    pub value: Value,
    pub source: Source,
    /// A flag-only option the tool itself defines beside the contract's (canon's `repo` and `as`).
    pub flag_only: bool,
}

impl Given {
    fn new(name: &str, kind: Kind, value: Value) -> Self {
        Given { name: name.to_string(), kind, value, source: Source::Explicit, flag_only: false }
    }

    /// An option with a value.
    pub fn option(name: &str, value: impl Into<Value>) -> Self {
        Given::new(name, Kind::Value, value.into())
    }

    /// An option given once for each of `values`.
    pub fn repeated(name: &str, values: Vec<Value>) -> Self {
        Given::new(name, Kind::Repeated, Value::Array(values))
    }

    /// A switch, which is given only when `on`.
    pub fn switch(name: &str, on: bool) -> Self {
        Given::new(name, Kind::Switch, Value::Bool(on))
    }

    /// A count, which is given only when `times` is above zero.
    pub fn count(name: &str, times: u64) -> Self {
        Given::new(name, Kind::Count, Value::from(times))
    }

    /// The operands, in order.
    pub fn operands(values: Vec<Value>) -> Self {
        Given::new("operands", Kind::Operands, Value::Array(values))
    }

    /// The same argument, supplied by the option's `env` variable.
    #[must_use]
    pub fn from_env(mut self) -> Self {
        self.source = Source::Env;
        self
    }

    /// The same argument, marked as a flag-only option of the tool's own.
    #[must_use]
    pub fn tool_flag_only(mut self) -> Self {
        self.flag_only = true;
        self
    }

    /// Whether the argument is the same as absent (spec 10.7.2): a switch that is `false`, a count of `0`, no occurrence of a
    /// repeated option or no operand, or a `null`.
    pub fn is_absent(&self) -> bool {
        match (self.kind, &self.value) {
            (_, Value::Null) => true,
            (Kind::Switch, Value::Bool(on)) => !on,
            (Kind::Count, Value::Number(times)) => times.as_u64() == Some(0),
            (Kind::Repeated | Kind::Operands, Value::Array(items)) => items.is_empty(),
            _ => false,
        }
    }
}

/// The text of a value as a word of argv: a string as it is, a number as its integer digits where it is a whole number, a
/// boolean as `true` or `false`, anything else as compact JSON. `None` for `null`.
fn word(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(jcs::to_string(value).unwrap_or_else(|_| number.to_string())),
        other => Some(other.to_string()),
    }
}

/// `--name=value`, the one form that no value can be misread in (spec 9.12.3, 11.9): not as an option, as a stdin selector or
/// as an inline-JSON or file selector, whatever the value starts with or whether it is empty.
fn option(name: &str, value: &str, out: &mut Vec<String>) {
    out.push(format!("--{name}={value}"));
}

/// The argv of the invocation `path given...`, without the executable: the command words, the options in the order of
/// `given`, then `--` and the operands. A value from an `env` variable, an absent argument and `--input` (which only carries
/// arguments that are already in `given`) are left out. Every valued option is written `--name=value`.
pub fn write_argv(path: &[&str], given: &[Given]) -> Vec<String> {
    let mut words: Vec<String> = path.iter().map(ToString::to_string).collect();
    let mut operands: Vec<String> = Vec::new();
    for argument in given {
        if argument.source == Source::Env || argument.is_absent() || argument.name == "input" {
            continue;
        }
        match (argument.kind, &argument.value) {
            (Kind::Switch, _) => words.push(format!("--{}", argument.name)),
            (Kind::Count, Value::Number(times)) => {
                for _ in 0..times.as_u64().unwrap_or(0) {
                    words.push(format!("--{}", argument.name));
                }
            }
            (Kind::Repeated, Value::Array(items)) => {
                for item in items {
                    if let Some(text) = word(item) {
                        option(&argument.name, &text, &mut words);
                    }
                }
            }
            (Kind::Operands, Value::Array(items)) => operands.extend(items.iter().filter_map(word)),
            (_, value) => {
                if let Some(text) = word(value) {
                    option(&argument.name, &text, &mut words);
                }
            }
        }
    }
    if !operands.is_empty() {
        words.push("--".to_string());
        words.extend(operands);
    }
    words
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn atip_cratecursor_09_arguments_are_written_as_argv_options_and_operands_after_one_double_dash() {
        let given = vec![
            Given::option("status", "active"),
            Given::switch("verbose", true),
            Given::count("level", 2),
            Given::repeated("label", vec![json!("a"), json!("b c")]),
            Given::option("round", 1),
            Given::option("filter", json!({ "a": [1, 2] })),
            Given::operands(vec![json!("one"), json!("-two")]),
            Given::option("after", "x"),
        ];
        assert_eq!(
            write_argv(&["acme", "member", "list"], &given),
            ["acme", "member", "list", "--status=active", "--verbose", "--level", "--level", "--label=a", "--label=b c", "--round=1", "--filter={\"a\":[1,2]}", "--after=x", "--", "one", "-two"]
        );
        // No operand: no `--`.
        assert_eq!(write_argv(&["acme", "ls"], &[Given::option("a", "b")]), ["acme", "ls", "--a=b"]);
    }

    #[test]
    fn atip_cratecursor_09_a_default_an_env_value_an_absent_argument_and_input_are_not_written() {
        let given = vec![
            Given::option("kept", "yes"),
            Given::option("from-env", "secret-free").from_env(),
            Given::switch("quiet", false),
            Given::count("level", 0),
            Given::repeated("label", vec![]),
            Given::operands(vec![]),
            Given::option("input", "-"),
            Given::option("nothing", Value::Null),
        ];
        assert_eq!(write_argv(&["acme", "ls"], &given), ["acme", "ls", "--kept=yes"]);
    }

    #[test]
    fn atip_cratecursor_10_a_value_that_is_empty_or_starts_with_a_dash_is_written_joined_like_every_other() {
        let given = vec![Given::option("name", "-x"), Given::option("empty", ""), Given::option("dashes", "--"), Given::option("stdin", "-"), Given::option("neg", -5), Given::repeated("label", vec![json!("-a"), json!("b")])];
        assert_eq!(write_argv(&["acme", "ls"], &given), ["acme", "ls", "--name=-x", "--empty=", "--dashes=--", "--stdin=-", "--neg=-5", "--label=-a", "--label=b"]);
    }

    #[test]
    fn an_absent_argument_is_the_same_as_not_given() {
        assert!(Given::switch("a", false).is_absent() && !Given::switch("a", true).is_absent());
        assert!(Given::count("a", 0).is_absent() && !Given::count("a", 1).is_absent());
        assert!(Given::repeated("a", vec![]).is_absent() && Given::operands(vec![]).is_absent());
        assert!(!Given::option("a", "").is_absent(), "an empty value is a value");
        assert!(!Given::option("a", 0).is_absent(), "an integer option given 0 is given (spec 10.8.1)");
    }
}
