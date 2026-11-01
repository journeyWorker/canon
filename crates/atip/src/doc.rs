//! The documents `<tool> schema --json` answers with (spec 10), as typed values.
//!
//! The members and their order are the contract's (`schema/schema-doc.schema.json`,
//! `examples/schema-command-*.json`). Every type derives `JsonSchema`, so a tool
//! can publish the schema of its own `schema` answer. A member a document does
//! not have is absent, never `null` (spec 6.5). Tool extensions (`x-<tool>`,
//! spec 10.12) ride in `extensions` and are written beside the contract's own
//! members.
//!
//! [`crate::project`] builds the parts that come from a clap tree; the tool
//! fills in what clap cannot say (effect, errors, outputs, examples).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

/// The `valueType` of an option or an operand (spec 10.7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    /// `--name`, a boolean.
    Switch,
    /// `--name`, once per occurrence.
    Count,
    /// Any text.
    String,
    /// A whole number within ±(2^53-1).
    Integer,
    /// A finite number.
    Number,
    /// A path, relative to the working directory.
    Path,
    /// One of `possibleValues`.
    Enum,
    /// Any JSON value.
    Json,
}

impl ValueType {
    pub fn as_str(self) -> &'static str {
        match self {
            ValueType::Switch => "switch",
            ValueType::Count => "count",
            ValueType::String => "string",
            ValueType::Integer => "integer",
            ValueType::Number => "number",
            ValueType::Path => "path",
            ValueType::Enum => "enum",
            ValueType::Json => "json",
        }
    }
}

/// What running a command does to the repository and the machine (spec 12.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// Writes nothing (stdout and stderr only).
    Read,
    /// Writes or stages files or records; nothing existing is destroyed.
    Write,
    /// Permanently removes data.
    Destructive,
}

impl Effect {
    pub fn as_str(self) -> &'static str {
        match self {
            Effect::Read => "read",
            Effect::Write => "write",
            Effect::Destructive => "destructive",
        }
    }
}

/// What a repeat of a writing command with the same arguments does (spec 12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Idempotency {
    /// A repeat is a no-op, reported `unchanged`.
    ReplaySafe,
    /// Identical content is a no-op; different content under the same key is refused.
    ContentKeyed,
    /// Every call adds a record.
    Appends,
    /// A repeat is refused with exit 2 and changes nothing.
    Once,
}

/// The form an output takes on stdout (spec 10.9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OutputKind {
    /// An enveloped report.
    Report,
    /// Bytes as they are in text mode; `--json` envelopes them.
    Artifact,
    /// The stream protocol of spec 13.5.
    Stream,
}

/// Whether a tree entry is a group or a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Group,
    Command,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The `default` of an option or an operand: typed by the argument's `valueType`, so its JSON type is one of these.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum DefaultValue {
    Switch(bool),
    Integer(i64),
    Number(f64),
    Text(String),
    List(Vec<DefaultValue>),
    None(()),
}

/// The `domain` of an argument: a JSON Schema in the profile of spec 10.7.4. Only the members a domain of a number or a text may
/// carry are typed here; which of them fit which `valueType`, the safe-integer bounds and the I-Regexp form of `pattern` are
/// checked by [`crate::domain::profile_problems`].
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("minProperties" = 1))]
pub struct DomainSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive_minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive_maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple_of: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,
    /// `^`, an I-Regexp body and `$`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// A value an `if.in` list holds: an integer or a text (the valid values of an `enum`, `string` or `integer` argument).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ConditionValue {
    Integer(i64),
    Text(String),
}

/// The five rules of a constraint (spec 10.8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RuleKind {
    /// At least one of `names` is given.
    AtLeastOne,
    /// Exactly one of `names` is given.
    ExactlyOne,
    /// At most one of `names` is given.
    AtMostOne,
    /// When `if` holds, every name in `then` is given.
    Requires,
    /// When `if` holds, no name in `then` is given.
    Forbids,
}

impl RuleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleKind::AtLeastOne => "atLeastOne",
            RuleKind::ExactlyOne => "exactlyOne",
            RuleKind::AtMostOne => "atMostOne",
            RuleKind::Requires => "requires",
            RuleKind::Forbids => "forbids",
        }
    }
}

/// The `if` of a `requires` or `forbids` rule: `{name}` holds when the argument is given, `{name, in}` when it is given and
/// one of its values is in `in`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ConditionDoc {
    pub name: String,
    #[serde(rename = "in", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<ConditionValue>>")]
    pub values: Option<Vec<Value>>,
}

/// One rule of a command's `constraints` (spec 10.8).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ConstraintDoc {
    pub rule: RuleKind,
    /// The operands and options of an `atLeastOne`, `exactlyOne` or `atMostOne`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Vec<String>>,
    /// When a `requires` or `forbids` applies.
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub when: Option<ConditionDoc>,
    /// What a `requires` demands and a `forbids` refuses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub then: Option<Vec<String>>,
    /// The registered code a violation produces; absent: `USAGE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// One sentence saying what the rule means in the command's own words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Tool extensions (`x-<tool>`, spec 10.12), written beside the members above.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extensions: BTreeMap<String, Value>,
}

/// One stateless check a domain and the five rules cannot say (spec 10.8.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ValidatorDoc {
    /// The names of the operands and options it reads.
    pub arguments: Vec<String>,
    /// The registered code its failure produces, with exit 2.
    pub code: String,
    /// What it checks, in one or two plain sentences.
    pub description: String,
    /// Complete invocations that satisfy every requirement, constraint and domain and that this validator refuses.
    pub rejects: Vec<String>,
    /// Tool extensions (`x-<tool>`, spec 10.12), written beside the members above.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extensions: BTreeMap<String, Value>,
}

/// One embedded JSON Schema of a command document (`schemas`): an object, or a boolean.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[schemars(schema_with = "embedded_schema")]
pub struct EmbeddedSchema(pub Value);

fn embedded_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": ["object", "boolean"] })
}

/// One complete, valid `--input` document (`input.example`): an object whose keys are the command's option names.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[schemars(schema_with = "input_example")]
pub struct InputExample(pub Value);

fn input_example(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "object" })
}

/// One option of a command (spec 10.7).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OptionDoc {
    /// The long name without `--`.
    pub name: String,
    /// The one-letter short option, when the option has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    /// The placeholder help shows; absent for a switch and a count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_name: Option<String>,
    pub value_type: ValueType,
    /// The members of an `enum`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possible_values: Option<Vec<String>>,
    pub required: bool,
    pub repeatable: bool,
    /// The typed value used when the option is absent; `null` when there is none or it is computed.
    #[schemars(with = "DefaultValue")]
    pub default: Value,
    /// How a computed default is found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_note: Option<String>,
    /// The environment variable the parser reads when the option is absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// True when the option cannot be given in an `--input` document.
    #[serde(skip_serializing_if = "is_false")]
    pub flag_only: bool,
    /// The restrictions of the option's own value beyond its `valueType` (spec 10.7.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<DomainSchema>")]
    pub domain: Option<Value>,
    pub help: String,
}

/// One operand of a command (spec 10.6).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperandDoc {
    /// A kebab-case word naming the operand.
    pub name: String,
    /// The placeholder help shows (`CHANGE_ID`).
    pub value_name: String,
    pub value_type: ValueType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possible_values: Option<Vec<String>>,
    pub required: bool,
    /// Whether the operand takes several values.
    pub variadic: bool,
    #[schemars(with = "DefaultValue")]
    pub default: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_note: Option<String>,
    /// The restrictions of each value of the operand beyond its `valueType` (spec 10.7.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<DomainSchema>")]
    pub domain: Option<Value>,
    pub help: String,
}

/// One output of a command (spec 10.9).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputDoc {
    pub kind: OutputKind,
    /// `#/schemas/<Name>`: the JSON Schema of `data` in JSON mode.
    pub schema_ref: String,
    pub schema_version: String,
    /// The media type of an artifact's bytes in text mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// A stream's event schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_ref: Option<String>,
    /// The report a batch `--input` returns.
    #[serde(skip_serializing_if = "is_false")]
    pub batch: bool,
    /// The invocation that selects this output when there are several.
    #[serde(rename = "when", skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    /// The field paths `--fields` may select from this output (spec 6.10).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldsDoc>,
}

/// The field selection an output declares (spec 10.9.1): where the records are and which paths a caller may select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FieldsDoc {
    /// RFC 6901 pointer of the record array in `data`, or `""` when `data` is the record.
    pub records: String,
    /// The selectable paths, each in its one spelling (spec 6.10).
    pub paths: Vec<String>,
    /// The paths always kept; they are not selectable.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keep: Vec<String>,
    /// The selection without `--fields`, a subset of `paths`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Vec<String>>,
}

/// The `--input` declaration of a command (spec 10.10).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InputDoc {
    /// Always `--input`.
    pub option: &'static str,
    /// The fragment schema of one document: no `required`, flag-only options absent.
    pub schema_ref: String,
    /// The informative schema of the merged argument object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments_ref: Option<String>,
    pub batch: bool,
    pub max_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,
    /// One complete, valid document.
    #[schemars(with = "InputExample")]
    pub example: Value,
}

/// One row of the tree, and one child of a group (spec 10.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct TreeEntry {
    pub command: String,
    pub path: Vec<String>,
    pub kind: EntryKind,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<Effect>,
    /// Tool extensions (`x-<tool>`, spec 10.12), written beside the members above.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extensions: BTreeMap<String, Value>,
}

/// `schema --json`: every group and command of the tool (spec 10.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct TreeDoc {
    #[schemars(extend("const" = "tree"))]
    pub kind: &'static str,
    /// The conformance level the tool claims: `L1` or `L2`.
    pub level: String,
    pub commands: Vec<TreeEntry>,
    /// Tool extensions (`x-<tool>`, spec 10.12): a tool that filters its tree (10.2.2) says so here.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extensions: BTreeMap<String, Value>,
}

/// `schema <GROUP> --json` (spec 10.4).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct GroupDoc {
    #[schemars(extend("const" = "group"))]
    pub kind: &'static str,
    pub command: String,
    pub path: Vec<String>,
    pub summary: String,
    /// The group's direct children.
    pub commands: Vec<TreeEntry>,
    /// Tool extensions (`x-<tool>`, spec 10.12), written beside the members above.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extensions: BTreeMap<String, Value>,
}

/// `schema <COMMAND> --json` (spec 10.5).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandDoc {
    #[schemars(extend("const" = "command"))]
    pub kind: &'static str,
    pub command: String,
    pub path: Vec<String>,
    pub summary: String,
    pub effect: Effect,
    /// Absent for a `read`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<Idempotency>,
    /// Every effect the effect class does not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_note: Option<String>,
    pub operands: Vec<OperandDoc>,
    pub options: Vec<OptionDoc>,
    /// Every relation between arguments the parser or the command enforces without reading state (spec 10.8.1); absent when there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<ConstraintDoc>,
    /// The stateless checks neither a domain nor a constraint can say (spec 10.8.3); absent when there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub validators: Vec<ValidatorDoc>,
    /// Every error code the command can emit, reserved codes included.
    pub errors: Vec<String>,
    pub outputs: Vec<OutputDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<InputDoc>,
    pub examples: Vec<String>,
    /// Every JSON Schema the command's refs name, keyed by name.
    #[schemars(with = "BTreeMap<String, EmbeddedSchema>")]
    pub schemas: BTreeMap<String, Value>,
    /// Tool extensions (`x-<tool>`, spec 10.12), written beside the members above.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extensions: BTreeMap<String, Value>,
}

impl TreeDoc {
    pub const KIND: &'static str = "tree";
}

impl GroupDoc {
    pub const KIND: &'static str = "group";
}

impl CommandDoc {
    pub const KIND: &'static str = "command";
}
