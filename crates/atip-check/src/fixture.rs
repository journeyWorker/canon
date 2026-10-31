//! The tool's fixture file (approaches/checker.md 5), the L0 keys of it.
//!
//! A fixture is a JSON object. At L0 the checker reads `setup`, `env`,
//! `timeoutSeconds`, `probe` and `registry`; `commands`, `search` and
//! `domainRejects` belong to L1 and L2 and are ignored when present, as is
//! any other member.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::schema::schemas;

/// The default time limit of one invocation.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest time limit a fixture may ask for.
const MAX_TIMEOUT_SECONDS: f64 = 3600.0;

/// One row of the tool's error registry (spec 7.3), as the fixture lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryRow {
    pub code: String,
    pub exit: i64,
    pub retryable: bool,
}

/// What the checker reads from a fixture.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Argv arrays run once, in order, to build the template directory.
    pub setup: Vec<Vec<String>>,
    /// The fixture's own environment variables.
    pub env: BTreeMap<String, String>,
    pub timeout: Duration,
    /// The id of the command that runs without state, split into words.
    pub probe: Option<Vec<String>>,
    /// The tool's error registry; `None` when the fixture gives none.
    pub registry: Option<Vec<RegistryRow>>,
}

impl Fixture {
    /// The probe's id: its words joined by one space, as `meta.command` spells it.
    pub fn probe_id(&self) -> Option<String> {
        self.probe.as_ref().map(|words| words.join(" "))
    }

    /// The registry row of `code`, when the fixture has a registry and the code is in it.
    pub fn row(&self, code: &str) -> Option<&RegistryRow> {
        self.registry.as_ref()?.iter().find(|row| row.code == code)
    }
}

/// Reads and validates the fixture at `path`; the reason is one sentence naming the member at fault.
pub fn load(path: &Path) -> Result<Fixture, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let document: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
    parse(&document)
}

/// Validates a parsed fixture document.
pub fn parse(document: &Value) -> Result<Fixture, String> {
    let Some(object) = document.as_object() else {
        return Err("the fixture must be a JSON object".to_string());
    };
    Ok(Fixture {
        setup: object
            .get("setup")
            .map(setup)
            .transpose()?
            .unwrap_or_default(),
        env: object.get("env").map(env).transpose()?.unwrap_or_default(),
        timeout: object
            .get("timeoutSeconds")
            .map(timeout)
            .transpose()?
            .unwrap_or(DEFAULT_TIMEOUT),
        probe: object.get("probe").map(probe).transpose()?,
        registry: object.get("registry").map(registry).transpose()?,
    })
}

fn setup(value: &Value) -> Result<Vec<Vec<String>>, String> {
    let commands = value
        .as_array()
        .ok_or("`setup` must be an array of argv arrays")?;
    commands
        .iter()
        .enumerate()
        .map(|(index, command)| {
            let words = command
                .as_array()
                .ok_or_else(|| format!("`setup/{index}` must be an array of strings"))?;
            if words.is_empty() {
                return Err(format!("`setup/{index}` must not be empty"));
            }
            words
                .iter()
                .enumerate()
                .map(|(at, word)| {
                    word.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("`setup/{index}/{at}` must be a string"))
                })
                .collect()
        })
        .collect()
}

fn env(value: &Value) -> Result<BTreeMap<String, String>, String> {
    let object = value
        .as_object()
        .ok_or("`env` must be an object of strings")?;
    object
        .iter()
        .map(|(name, value)| {
            if name.is_empty() || name.contains(['=', '\0']) {
                return Err(format!("`env` has the invalid variable name {name:?}"));
            }
            if name == "HOME" {
                return Err(
                    "`env` must not set HOME: the checker points it into the copy of the template"
                        .to_string(),
                );
            }
            let value = value
                .as_str()
                .filter(|text| !text.contains('\0'))
                .ok_or_else(|| format!("`env/{name}` must be a string"))?;
            Ok((name.clone(), value.to_string()))
        })
        .collect()
}

fn timeout(value: &Value) -> Result<Duration, String> {
    let seconds = value
        .as_f64()
        .filter(|seconds| *seconds > 0.0 && *seconds <= MAX_TIMEOUT_SECONDS);
    seconds.map(Duration::from_secs_f64).ok_or_else(|| {
        format!("`timeoutSeconds` must be a number above 0 and at most {MAX_TIMEOUT_SECONDS}")
    })
}

/// The command id pattern of the envelope schema's `meta.command`.
fn is_command_id(words: &[String]) -> bool {
    words.iter().all(|word| {
        let mut parts = word.split('-');
        let first_ok = parts.next().is_some_and(|part| {
            part.starts_with(|c: char| c.is_ascii_lowercase())
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
        first_ok
            && parts.all(|part| {
                !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            })
    })
}

fn probe(value: &Value) -> Result<Vec<String>, String> {
    let text = value
        .as_str()
        .ok_or("`probe` must be a string: the id of a command that runs without state")?;
    let words: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    if words.is_empty() || !is_command_id(&words) {
        return Err(format!(
            "`probe` {text:?} is not a command id (lower kebab-case words separated by spaces)"
        ));
    }
    Ok(words)
}

fn registry(value: &Value) -> Result<Vec<RegistryRow>, String> {
    let rows = value
        .as_array()
        .ok_or("`registry` must be an array of registry rows")?;
    let mut parsed: Vec<RegistryRow> = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        if let Some(problem) = schemas().registry_entry_problems(row).first() {
            return Err(format!(
                "`registry/{index}` is not a registry row: {problem}"
            ));
        }
        let code = row
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if parsed.iter().any(|earlier| earlier.code == code) {
            return Err(format!("`registry/{index}` repeats the code {code}"));
        }
        parsed.push(RegistryRow {
            code,
            exit: row.get("exit").and_then(Value::as_i64).unwrap_or_default(),
            retryable: row
                .get("retryable")
                .and_then(Value::as_bool)
                .unwrap_or_default(),
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse, DEFAULT_TIMEOUT};

    #[test]
    fn atip_check_29_the_l0_keys_are_read_and_the_others_are_ignored() {
        let fixture = parse(&json!({
            "setup": [["init", "--json"]],
            "env": {"X": "1"},
            "timeoutSeconds": 5,
            "probe": "member list",
            "registry": [{"code": "USAGE", "exit": 2, "retryable": false, "meaning": "m", "fix": "f"}],
            "commands": {"anything": 1}, "search": 3, "domainRejects": null, "other": true
        }))
        .expect("valid");
        assert_eq!(fixture.setup, [["init", "--json"]]);
        assert_eq!(fixture.env["X"], "1");
        assert_eq!(fixture.timeout.as_secs(), 5);
        assert_eq!(fixture.probe_id().as_deref(), Some("member list"));
        assert_eq!(fixture.row("USAGE").map(|row| row.exit), Some(2));
        let empty = parse(&json!({})).expect("an empty fixture is valid");
        assert_eq!(
            (
                empty.timeout,
                empty.probe.is_none(),
                empty.registry.is_none()
            ),
            (DEFAULT_TIMEOUT, true, true)
        );
    }

    #[test]
    fn atip_check_27_a_fixture_of_the_wrong_shape_names_the_member() {
        for (document, needle) in [
            (json!([]), "object"),
            (json!({"setup": "init"}), "setup"),
            (json!({"setup": [[]]}), "setup/0"),
            (json!({"setup": [["init", 1]]}), "setup/0/1"),
            (json!({"env": {"A": 1}}), "env/A"),
            (json!({"env": {"HOME": "/x"}}), "HOME"),
            (json!({"timeoutSeconds": 0}), "timeoutSeconds"),
            (json!({"timeoutSeconds": "5"}), "timeoutSeconds"),
            (json!({"probe": 3}), "probe"),
            (json!({"probe": "Status"}), "probe"),
            (json!({"probe": "  "}), "probe"),
            (json!({"registry": {}}), "registry"),
            (
                json!({"registry": [{"code": "usage", "exit": 2, "retryable": false, "meaning": "m", "fix": "f"}]}),
                "registry/0",
            ),
            (
                json!({"registry": [{"code": "USAGE", "exit": 3, "retryable": false, "meaning": "m", "fix": "f"}]}),
                "registry/0",
            ),
            (
                json!({"registry": [{"code": "USAGE", "exit": 2, "retryable": false, "meaning": "m", "fix": "f"}, {"code": "USAGE", "exit": 2, "retryable": false, "meaning": "m", "fix": "f"}]}),
                "repeats",
            ),
        ] {
            let reason = parse(&document).expect_err(&format!("{document} must be refused"));
            assert!(reason.contains(needle), "{reason:?} must name {needle:?}");
        }
    }
}
