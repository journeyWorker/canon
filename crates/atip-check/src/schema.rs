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
    registry_entry: Validator,
}

/// The validators, compiled once.
///
/// # Panics
/// When an embedded schema is not JSON Schema 2020-12 that compiles: the
/// schemas are fixed at build time, so that is a defect of this build.
pub fn schemas() -> &'static Schemas {
    static SCHEMAS: OnceLock<Schemas> = OnceLock::new();
    SCHEMAS.get_or_init(|| Schemas::compile().unwrap_or_else(|reason| panic!("the embedded contract schemas do not compile: {reason}")))
}

impl Schemas {
    fn compile() -> Result<Self, String> {
        let parse = |name: &str, text: &str| serde_json::from_str::<Value>(text).map_err(|error| format!("{name}: {error}"));
        let error = parse("error.schema.json", atip::schema::ERROR_SCHEMA)?;
        let envelope = parse("envelope.schema.json", atip::schema::ENVELOPE_SCHEMA)?;
        let registry_entry = parse("registry-entry.schema.json", atip::schema::REGISTRY_ENTRY_SCHEMA)?;
        let registry = Registry::new()
            .add(ERROR_ID, error)
            .and_then(|builder| builder.add(ENVELOPE_ID, envelope.clone()))
            .and_then(|builder| builder.add(REGISTRY_ENTRY_ID, registry_entry.clone()))
            .and_then(jsonschema::RegistryBuilder::prepare)
            .map_err(|error| error.to_string())?;
        let build = |schema: &Value| options(&registry).build(schema).map_err(|error| error.to_string());
        Ok(Self { envelope: build(&envelope)?, registry_entry: build(&registry_entry)?, registry })
    }

    /// A validator for `schema` under the checker's rules: JSON Schema 2020-12, `format` an annotation, the contract schemas resolvable by their `$id`.
    pub fn validator(&self, schema: &Value) -> Result<Validator, String> {
        options(&self.registry).build(schema).map_err(|error| error.to_string())
    }

    /// The problems of `document` against the envelope schema, empty when it validates.
    pub fn envelope_problems(&self, document: &Value) -> Vec<String> {
        problems(&self.envelope, document)
    }

    /// The problems of `row` against the registry-entry schema, empty when it validates.
    pub fn registry_entry_problems(&self, row: &Value) -> Vec<String> {
        problems(&self.registry_entry, row)
    }
}

fn options<'r>(registry: &'r Registry<'r>) -> ValidationOptions<'r> {
    jsonschema::options().with_draft(Draft::Draft202012).should_validate_formats(false).with_registry(registry)
}

fn problems(validator: &Validator, document: &Value) -> Vec<String> {
    validator.iter_errors(document).map(|error| format!("{}: {error}", pointer(&error.instance_path().to_string()))).collect()
}

fn pointer(path: &str) -> &str {
    if path.is_empty() {
        "/"
    } else {
        path
    }
}
