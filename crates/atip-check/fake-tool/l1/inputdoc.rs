//! The `input` member of a command document and the schemas it names, generated from the arguments of the model (spec 10.10,
//! 11.5): the fragment schema describes the document as it is written (a numeric string is a number there), the arguments
//! schema the argument object of 11.7 step 6.

use serde_json::{json, Map, Value};

use super::spec::{Arg, Command};

/// The most bytes a document may have.
pub const MAX_BYTES: usize = 4096;
/// The most documents of a batch.
pub const MAX_ITEMS: usize = 8;

const INTEGER_PATTERN: &str = "^-?(0|[1-9][0-9]*)$";
const NUMBER_PATTERN: &str = "^-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE][+-]?[0-9]+)?$";

/// The key a document gives the option `name`: its camelCase.
pub fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for character in name.chars() {
        if character == '-' {
            upper = true;
        } else if upper {
            out.extend(character.to_uppercase());
            upper = false;
        } else {
            out.push(character);
        }
    }
    out
}

/// The names of the fragment and of the arguments schema of a command that takes `--input`.
fn schema_names(id: &str) -> (&'static str, &'static str) {
    match id {
        "init" => ("InitInput", "InitArguments"),
        "member tag" => ("MemberTagInput", "MemberTagArguments"),
        "record put" => ("RecordPutInput", "RecordPutArguments"),
        "reject" => ("RejectInput", "RejectArguments"),
        "team add" => ("TeamAddInput", "TeamAddArguments"),
        _ => ("CommandInput", "CommandArguments"),
    }
}

/// The schema of one value of `arg` as a document writes it.
fn written(arg: &Arg) -> Value {
    match arg.value_type {
        "integer" => json!({"type": ["integer", "string"], "pattern": INTEGER_PATTERN}),
        "number" => json!({"type": ["number", "string"], "pattern": NUMBER_PATTERN}),
        "enum" => json!({"enum": arg.possible}),
        "switch" => json!({"type": "boolean"}),
        "count" => json!({"type": "integer", "minimum": 0}),
        "json" => json!({"not": {"type": "null"}}),
        _ => json!({"type": "string"}),
    }
}

/// The schema of one value of `arg` in the argument object: numbers are numbers, and the domain applies.
fn normalised(arg: &Arg) -> Value {
    let mut schema = match arg.value_type {
        "integer" => json!({"type": "integer"}),
        "number" => json!({"type": "number"}),
        "enum" => json!({"enum": arg.possible}),
        "switch" => json!({"type": "boolean"}),
        "count" => json!({"type": "integer", "minimum": 0}),
        "json" => json!({"not": {"type": "null"}}),
        _ => json!({"type": "string"}),
    };
    if let (Some(domain), Some(target)) = (arg.domain.as_ref().and_then(Value::as_object), schema.as_object_mut()) {
        for (key, value) in domain {
            target.insert(key.clone(), value.clone());
        }
    }
    schema
}

fn many(arg: &Arg, element: Value) -> Value {
    if arg.repeatable && arg.value_type != "count" {
        json!({"type": "array", "items": element, "minItems": 1})
    } else {
        element
    }
}

/// The operands as one array.
fn operands(command: &Command, element: impl Fn(&Arg) -> Value) -> Option<Value> {
    let operands: Vec<&Arg> = command.operands().collect();
    let first = operands.first()?;
    let variadic = operands.last().is_some_and(|operand| operand.repeatable);
    let mut schema = json!({"type": "array", "items": element(first), "minItems": operands.iter().filter(|operand| operand.required).count().max(1)});
    if !variadic {
        schema["maxItems"] = json!(operands.len());
    }
    Some(schema)
}

/// Adds `input` to `command`: the option, the member of the document, its two schemas and the codes of the framing.
pub fn add_input(command: &mut Command, batch: bool, example: Value) {
    let (fragment_name, arguments_name) = schema_names(command.id);
    let documented = |arg: &&Arg| !arg.flag_only;
    let mut fragment = Map::new();
    let mut arguments = Map::new();
    if let Some(schema) = operands(command, written) {
        fragment.insert("operands".to_string(), schema);
    }
    if let Some(schema) = operands(command, normalised) {
        arguments.insert("operands".to_string(), schema);
    }
    for arg in command.flags().filter(documented) {
        fragment.insert(camel(arg.name), many(arg, written(arg)));
        arguments.insert(camel(arg.name), many(arg, normalised(arg)));
    }
    let mut required: Vec<String> = Vec::new();
    if command.operands().any(|operand| operand.required) {
        required.push("operands".to_string());
    }
    required.extend(command.flags().filter(documented).filter(|arg| arg.required).map(|arg| camel(arg.name)));
    command.schemas.push((fragment_name, json!({"type": "object", "additionalProperties": false, "properties": fragment})));
    command.schemas.push((
        arguments_name,
        json!({"type": "object", "required": required, "additionalProperties": false, "properties": arguments}),
    ));
    let mut input = json!({
        "option": "--input",
        "schemaRef": format!("#/schemas/{fragment_name}"),
        "argumentsRef": format!("#/schemas/{arguments_name}"),
        "batch": batch,
        "maxBytes": MAX_BYTES,
        "example": example,
    });
    if batch {
        input["maxItems"] = json!(MAX_ITEMS);
    }
    command.input = Some(input);
    if command.arg("input").is_none() {
        let position = command.args.iter().position(|arg| arg.name == "json").unwrap_or(command.args.len());
        command.args.insert(
            position,
            Arg::option("input", "string", "The arguments as JSON: one object, or an array for a batch").value("JSON").flag_only(),
        );
    }
    command.errors.extend(["INPUT_CONFLICT", "INPUT_INVALID", "INPUT_TOO_LARGE"]);
    if batch {
        command.errors.push("BATCH_INCOMPLETE");
    }
}
