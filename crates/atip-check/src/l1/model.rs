//! The documents of a `schema --json` answer as the L1 checks read them: a typed, lenient view over the JSON.
//!
//! The documents are validated against `schema-doc.schema.json` before a rule reads them (`L1.tree`, `L1.walk`),
//! but a document that fails the schema still has to be readable: a parse here never panics and a member that is
//! missing or of the wrong type reads as its empty value. The schema validation, not this view, reports the
//! shape.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::schema::schemas;

/// The `#/schemas/<Name>` form of a reference (spec 10.9.2): the name, when it has that form.
pub fn ref_name(reference: &str) -> Option<&str> {
    reference.strip_prefix("#/schemas/")
}

/// `kebab-case` to `camelCase`, the document key of an option (spec 11.5).
pub fn camel(kebab: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in kebab.chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn words(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// One operand or option of a command (spec 10.6, 10.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    /// The `name`: an option's long name without `--`, an operand's kebab-case name.
    pub name: String,
    pub value_name: Option<String>,
    /// `string`, `integer`, `number`, `path`, `enum`, `json`, and for options `switch` and `count`.
    pub value_type: String,
    pub required: bool,
    /// An option given once per value; `count` is repeatable too, with a single integer value.
    pub repeatable: bool,
    /// An operand that takes the rest (spec 10.6); always `false` on an option.
    pub variadic: bool,
    /// The typed default, `Value::Null` when there is none.
    pub default: Value,
    pub possible_values: Vec<String>,
    pub flag_only: bool,
    pub short: Option<char>,
    pub domain: Option<Value>,
    pub env: Option<String>,
    /// `true` for an option, `false` for an operand.
    pub is_option: bool,
}

impl Arg {
    pub fn parse(value: &Value, is_option: bool) -> Self {
        Self {
            name: text(value, "name"),
            value_name: value
                .get("valueName")
                .and_then(Value::as_str)
                .map(str::to_string),
            value_type: text(value, "valueType"),
            required: flag(value, "required"),
            repeatable: is_option && flag(value, "repeatable"),
            variadic: !is_option && flag(value, "variadic"),
            default: value.get("default").cloned().unwrap_or(Value::Null),
            possible_values: words(value, "possibleValues"),
            flag_only: flag(value, "flagOnly"),
            short: value
                .get("short")
                .and_then(Value::as_str)
                .and_then(|short| short.chars().next()),
            domain: value.get("domain").cloned(),
            env: value
                .get("env")
                .and_then(Value::as_str)
                .map(str::to_string),
            is_option,
        }
    }

    /// Whether the argument takes no value on the command line: a `switch` or a `count`.
    pub fn is_flag(&self) -> bool {
        self.value_type == "switch" || self.value_type == "count"
    }

    /// The command-line spelling: `--name` for an option, the `name` for an operand.
    pub fn spelling(&self) -> String {
        if self.is_option {
            format!("--{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

/// One rule of `constraints` (spec 10.8.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    /// `atLeastOne`, `exactlyOne`, `atMostOne`, `requires`, `forbids`.
    pub rule: String,
    pub names: Vec<String>,
    /// `if.name`: the argument that conditions a `requires` or `forbids`.
    pub if_name: Option<String>,
    /// `if.in`: the values of `if.name` that make the condition hold; `None` when the condition is "given".
    pub if_in: Option<Vec<Value>>,
    pub then: Vec<String>,
    pub code: Option<String>,
}

impl Constraint {
    fn parse(value: &Value) -> Self {
        let condition = value.get("if");
        Self {
            rule: text(value, "rule"),
            names: words(value, "names"),
            if_name: condition
                .and_then(|condition| condition.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string),
            if_in: condition
                .and_then(|condition| condition.get("in"))
                .and_then(Value::as_array)
                .cloned(),
            then: words(value, "then"),
            code: value
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }

    /// A short name of the rule for a report: `atMostOne [a, b]`, `requires a -> [b]`.
    pub fn label(&self) -> String {
        match (self.if_name.as_deref(), self.rule.as_str()) {
            (Some(name), rule) => {
                let when = self
                    .if_in
                    .as_ref()
                    .map(|values| {
                        format!(
                            " in {}",
                            Value::Array(values.clone())
                        )
                    })
                    .unwrap_or_default();
                format!("{rule} {name}{when} -> [{}]", self.then.join(", "))
            }
            (None, rule) => format!("{rule} [{}]", self.names.join(", ")),
        }
    }
}

/// One stateless validator (spec 10.8.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Validator {
    pub arguments: Vec<String>,
    pub code: String,
    pub rejects: Vec<String>,
}

/// The `fields` of an output (spec 10.9.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Fields {
    pub records: String,
    pub paths: Vec<String>,
    pub keep: Vec<String>,
    pub default: Vec<String>,
}

/// One output of a command (spec 10.9).
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub kind: String,
    pub schema_ref: String,
    pub schema_version: String,
    pub event_ref: Option<String>,
    pub batch: bool,
    pub when: Option<String>,
    pub fields: Option<Fields>,
}

impl Output {
    fn parse(value: &Value) -> Self {
        Self {
            kind: text(value, "kind"),
            schema_ref: text(value, "schemaRef"),
            schema_version: text(value, "schemaVersion"),
            event_ref: value
                .get("eventRef")
                .and_then(Value::as_str)
                .map(str::to_string),
            batch: flag(value, "batch"),
            when: value
                .get("when")
                .and_then(Value::as_str)
                .map(str::to_string),
            fields: value.get("fields").map(|fields| Fields {
                records: text(fields, "records"),
                paths: words(fields, "paths"),
                keep: words(fields, "keep"),
                default: words(fields, "default"),
            }),
        }
    }

    /// The name `meta.output` carries for this output: the name in its `schemaRef`.
    pub fn name(&self) -> &str {
        ref_name(&self.schema_ref).unwrap_or(&self.schema_ref)
    }
}

/// The `input` of a command (spec 10.10).
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub schema_ref: String,
    pub arguments_ref: Option<String>,
    pub batch: bool,
    pub max_items: Option<u64>,
    pub max_bytes: Option<u64>,
    pub example: Value,
}

/// A `command` document (spec 10.5).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandDoc {
    pub command: String,
    pub path: Vec<String>,
    pub effect: String,
    pub operands: Vec<Arg>,
    pub options: Vec<Arg>,
    pub constraints: Vec<Constraint>,
    pub validators: Vec<Validator>,
    pub errors: Vec<String>,
    pub outputs: Vec<Output>,
    pub input: Option<Input>,
    pub examples: Vec<String>,
    /// `schemas`: every JSON Schema the command's refs name, by name.
    pub schemas: Map<String, Value>,
    /// The whole `data` the document was read from.
    pub raw: Value,
}

impl CommandDoc {
    /// Reads the `data` of a `kind: "command"` answer; `None` when `data` is not an object with that kind.
    pub fn parse(data: &Value) -> Option<Self> {
        if data.get("kind").and_then(Value::as_str) != Some("command") {
            return None;
        }
        let list = |key: &str| {
            data.get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        Some(Self {
            command: text(data, "command"),
            path: words(data, "path"),
            effect: text(data, "effect"),
            operands: list("operands")
                .iter()
                .map(|operand| Arg::parse(operand, false))
                .collect(),
            options: list("options")
                .iter()
                .map(|option| Arg::parse(option, true))
                .collect(),
            constraints: list("constraints")
                .iter()
                .map(Constraint::parse)
                .collect(),
            validators: list("validators")
                .iter()
                .map(|validator| Validator {
                    arguments: words(validator, "arguments"),
                    code: text(validator, "code"),
                    rejects: words(validator, "rejects"),
                })
                .collect(),
            errors: words(data, "errors"),
            outputs: list("outputs").iter().map(Output::parse).collect(),
            input: data.get("input").map(|input| Input {
                schema_ref: text(input, "schemaRef"),
                arguments_ref: input
                    .get("argumentsRef")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                batch: flag(input, "batch"),
                max_items: input.get("maxItems").and_then(Value::as_u64),
                max_bytes: input.get("maxBytes").and_then(Value::as_u64),
                example: input.get("example").cloned().unwrap_or(Value::Null),
            }),
            examples: words(data, "examples"),
            schemas: data
                .get("schemas")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
            raw: data.clone(),
        })
    }

    /// Every operand and option, operands first.
    pub fn args(&self) -> impl Iterator<Item = &Arg> {
        self.operands.iter().chain(self.options.iter())
    }

    /// The operand or option named `name`.
    pub fn arg(&self, name: &str) -> Option<&Arg> {
        self.args().find(|arg| arg.name == name)
    }

    /// The option named `name`.
    pub fn option(&self, name: &str) -> Option<&Arg> {
        self.options.iter().find(|option| option.name == name)
    }

    /// Whether the command has the option `--<name>`.
    pub fn has_option(&self, name: &str) -> bool {
        self.option(name).is_some()
    }

    /// The output `name` (the name in its `schemaRef`), or the only output when `name` is `None`.
    pub fn output(&self, name: Option<&str>) -> Option<&Output> {
        match name {
            Some(name) => self.outputs.iter().find(|output| output.name() == name),
            None if self.outputs.len() == 1 => self.outputs.first(),
            None => None,
        }
    }

    /// A validator of the JSON Schema `reference` (`#/schemas/<Name>`) against the command's `schemas`, with the
    /// contract's schemas resolvable by `$id`.
    pub fn compile_ref(&self, reference: &str) -> Result<jsonschema::Validator, String> {
        self.compile_ref_in(reference, &self.schemas)
    }

    /// The same against `schemas` in place of the command's own (a caller that has changed a few of them).
    pub fn compile_ref_in(
        &self,
        reference: &str,
        schemas_in: &Map<String, Value>,
    ) -> Result<jsonschema::Validator, String> {
        let name = ref_name(reference)
            .filter(|name| schemas_in.contains_key(*name))
            .ok_or_else(|| format!("{reference} does not name a member of schemas"))?;
        let mut root = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": format!("#/schemas/{}", escape_pointer(name)),
        });
        root["schemas"] = Value::Object(schemas_in.clone());
        schemas().validator(&root)
    }
}

/// An RFC 6901 reference token: `~` is `~0` and `/` is `~1`.
pub fn escape_pointer(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// One entry of a `tree` or `group` answer (spec 10.3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub command: String,
    pub path: Vec<String>,
    /// `group` or `command`.
    pub kind: String,
    pub effect: Option<String>,
}

impl TreeEntry {
    pub fn is_group(&self) -> bool {
        self.kind == "group"
    }

    fn parse(value: &Value) -> Self {
        Self {
            command: text(value, "command"),
            path: words(value, "path"),
            kind: text(value, "kind"),
            effect: value
                .get("effect")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
}

/// The `data` of a `tree` answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub level: String,
    pub commands: Vec<TreeEntry>,
}

impl Tree {
    pub fn parse(data: &Value) -> Option<Self> {
        (data.get("kind").and_then(Value::as_str) == Some("tree")).then(|| Self {
            level: text(data, "level"),
            commands: data
                .get("commands")
                .and_then(Value::as_array)
                .map(|list| list.iter().map(TreeEntry::parse).collect())
                .unwrap_or_default(),
        })
    }

    /// The direct children of the group `path`.
    pub fn children(&self, path: &[String]) -> Vec<&TreeEntry> {
        self.commands
            .iter()
            .filter(|entry| entry.path.len() == path.len() + 1 && entry.path.starts_with(path))
            .collect()
    }
}

/// The `data` of a `group` answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupDoc {
    pub command: String,
    pub path: Vec<String>,
    pub commands: Vec<TreeEntry>,
}

impl GroupDoc {
    pub fn parse(data: &Value) -> Option<Self> {
        (data.get("kind").and_then(Value::as_str) == Some("group")).then(|| Self {
            command: text(data, "command"),
            path: words(data, "path"),
            commands: data
                .get("commands")
                .and_then(Value::as_array)
                .map(|list| list.iter().map(TreeEntry::parse).collect())
                .unwrap_or_default(),
        })
    }
}

/// What the run read from the tool's `schema` answers: the tree and a document for each of its entries.
#[derive(Debug, Clone, Default)]
pub struct ToolDocs {
    /// `meta.tool` of the answers.
    pub tool: String,
    pub tree: Option<Tree>,
    /// The command documents by command id.
    pub commands: BTreeMap<String, CommandDoc>,
    /// The group documents by command id.
    pub groups: BTreeMap<String, GroupDoc>,
}

impl ToolDocs {
    /// The document of the command with id `command`.
    pub fn command(&self, command: &str) -> Option<&CommandDoc> {
        self.commands.get(command)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{camel, ref_name, CommandDoc};

    #[test]
    fn atip_checkl1_model_camel_and_refs() {
        assert_eq!(camel("plan-digest"), "planDigest");
        assert_eq!(camel("json"), "json");
        assert_eq!(ref_name("#/schemas/Report"), Some("Report"));
        assert_eq!(ref_name("#/definitions/Report"), None);
    }

    #[test]
    fn atip_checkl1_model_a_command_reference_compiles_against_its_own_schemas() {
        let data = json!({
            "kind": "command", "command": "a", "path": ["a"], "effect": "read",
            "operands": [], "options": [], "errors": [], "outputs": [], "examples": [],
            "schemas": {"Report": {"type": "object", "required": ["n"], "properties": {"n": {"$ref": "#/schemas/Count"}}}, "Count": {"type": "integer", "minimum": 0}}
        });
        let doc = CommandDoc::parse(&data).expect("a command");
        let validator = doc.compile_ref("#/schemas/Report").expect("compiles");
        assert!(validator.is_valid(&json!({"n": 1})));
        assert!(!validator.is_valid(&json!({"n": -1})));
        assert!(!validator.is_valid(&json!({})));
        assert!(doc.compile_ref("#/schemas/Nope").err().is_some());
    }
}
