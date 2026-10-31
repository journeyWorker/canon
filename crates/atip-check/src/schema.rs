//! The contract schemas, as the checker reads them.
//!
//! The three schemas are the originals the `atip` crate embeds (`atip::schema`):
//! this module parses them and builds the validators, it never rewrites a
//! schema. They are JSON Schema 2020-12; `format` is an annotation, never an
//! assertion (the checker's authors decided it so, because a format a tool
//! cannot know must not fail a conforming document).

use std::sync::OnceLock;

use jsonschema::{Draft, Registry, ValidationOptions, Validator};
use serde_json::Value;

const ERROR_ID: &str = "https://atip.dev/1/error.schema.json";
const REGISTRY_ENTRY_ID: &str = "https://atip.dev/1/registry-entry.schema.json";
const ENVELOPE_ID: &str = "https://atip.dev/1/envelope.schema.json";

/// The validators of the three contract schemas.
#[derive(Debug)]
pub struct Schemas {
    registry: Registry<'static>,
    envelope: Validator,
    /// The success and failure branches of the envelope, so a problem is reported where it is, not as "no branch fits".
    success: Validator,
    failure: Validator,
    registry_entry: Validator,
}

/// One thing wrong with a document: where (a JSON pointer, `/` for the root) and what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

impl Problem {
    /// Whether the problem is at `prefix` or below it.
    pub fn is_under(&self, prefix: &str) -> bool {
        self.path == prefix || self.path.starts_with(&format!("{prefix}/"))
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// The validators, compiled once.
///
/// # Panics
/// When an embedded schema is not JSON Schema 2020-12 that compiles: the
/// schemas are fixed at build time, so that is a defect of this build.
pub fn schemas() -> &'static Schemas {
    static SCHEMAS: OnceLock<Schemas> = OnceLock::new();
    SCHEMAS.get_or_init(|| {
        Schemas::compile().unwrap_or_else(|reason| {
            panic!("the embedded contract schemas do not compile: {reason}")
        })
    })
}

impl Schemas {
    fn compile() -> Result<Self, String> {
        let parse = |name: &str, text: &str| {
            serde_json::from_str::<Value>(text).map_err(|error| format!("{name}: {error}"))
        };
        let error = parse("error.schema.json", atip::schema::ERROR_SCHEMA)?;
        let envelope = parse("envelope.schema.json", atip::schema::ENVELOPE_SCHEMA)?;
        let registry_entry = parse(
            "registry-entry.schema.json",
            atip::schema::REGISTRY_ENTRY_SCHEMA,
        )?;
        let registry = Registry::new()
            .add(ERROR_ID, error)
            .and_then(|builder| builder.add(ENVELOPE_ID, envelope.clone()))
            .and_then(|builder| builder.add(REGISTRY_ENTRY_ID, registry_entry.clone()))
            .and_then(jsonschema::RegistryBuilder::prepare)
            .map_err(|error| error.to_string())?;
        let build = |schema: &Value| {
            options(&registry)
                .build(schema)
                .map_err(|error| error.to_string())
        };
        let branch = |name: &str| {
            build(&serde_json::json!({"$ref": format!("{ENVELOPE_ID}#/$defs/{name}")}))
        };
        Ok(Self {
            envelope: build(&envelope)?,
            success: branch("success")?,
            failure: branch("failure")?,
            registry_entry: build(&registry_entry)?,
            registry,
        })
    }

    /// A validator for `schema` under the checker's rules: JSON Schema 2020-12, `format` an annotation, the contract schemas resolvable by their `$id`.
    pub fn validator(&self, schema: &Value) -> Result<Validator, String> {
        options(&self.registry)
            .build(schema)
            .map_err(|error| error.to_string())
    }

    /// The problems of `document` against the envelope schema, empty when it validates.
    pub fn envelope_problems(&self, document: &Value) -> Vec<String> {
        self.envelope_problem_list(document)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// The same, each with its place in the document. A document whose `ok` is a boolean is judged against its own
    /// branch (success or failure), so a problem deep in `meta` is reported there and not as "no branch fits".
    pub fn envelope_problem_list(&self, document: &Value) -> Vec<Problem> {
        let validator = match document.get("ok").and_then(Value::as_bool) {
            Some(true) => &self.success,
            Some(false) => &self.failure,
            None => &self.envelope,
        };
        problem_list(validator, document)
    }

    /// The problems of `row` against the registry-entry schema, empty when it validates.
    pub fn registry_entry_problems(&self, row: &Value) -> Vec<String> {
        problem_list(&self.registry_entry, row)
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}

fn options<'r>(registry: &'r Registry<'r>) -> ValidationOptions<'r> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(false)
        .with_registry(registry)
}

fn problem_list(validator: &Validator, document: &Value) -> Vec<Problem> {
    validator
        .iter_errors(document)
        .map(|error| Problem {
            path: pointer(&error.instance_path().to_string()).to_string(),
            message: error.to_string(),
        })
        .collect()
}

fn pointer(path: &str) -> &str {
    if path.is_empty() {
        "/"
    } else {
        path
    }
}
