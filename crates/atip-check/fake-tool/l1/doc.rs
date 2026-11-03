//! The documents `schema` prints, built from the model of `spec.rs`, and the generic seeded patch applied to them.

use serde_json::{json, Map, Value};

use super::patch;
use super::spec::{Arg, Command, Group, Rule, Shape, Validator};
use crate::Tool;

/// The commands and groups of the tool as the configuration shapes them.
#[derive(Debug)]
pub struct Model {
    pub commands: Vec<Command>,
    pub groups: Vec<Group>,
}

impl Model {
    pub fn of(tool: &Tool) -> Self {
        let shape = Shape {
            is_1_1: tool.is_1_1(),
            is_1_2: tool.is_1_2(),
            limit_not_flag_only: tool.flag("limitNotFlagOnly"),
            repeatable_default: tool.flag("repeatableDefault"),
            serve: tool.config.contains_key("serveMode"),
            serve_without_note: tool.flag("serveNoEffectNote"),
            deep: tool.flag("deepTree"),
            l2: tool.flag("l2"),
            pair_code: tool.flag("pairCode"),
            batch: !tool.flag("noBatch"),
            tag_flag_only_rule: tool.flag("tagFlagOnlyRule"),
        };
        let mut commands = super::spec::commands(shape, !tool.flag("noSchema"));
        if tool.flag("sharedEnv") {
            // Seeded: every option that reads the environment reads the same variable, so none is the sole declarer of its own.
            for command in &mut commands {
                for arg in command.args.iter_mut().filter(|arg| arg.env.is_some()) {
                    arg.env = Some("FAKE_SHARED");
                }
            }
        }
        if tool.flag("stampAudit") {
            // Seeded: the audit line `stamp` appends is declared by its effectNote, as the contract asks of a named side effect.
            if let Some(stamp) = commands.iter_mut().find(|command| command.id == "stamp") {
                stamp.effect_note = Some("appends one line to .stamp-audit.log");
            }
        }
        Self {
            commands,
            groups: super::spec::groups(shape),
        }
    }

    pub fn command(&self, id: &str) -> Option<&Command> {
        self.commands.iter().find(|command| command.id == id)
    }

    pub fn group(&self, id: &str) -> Option<&Group> {
        self.groups.iter().find(|group| group.id == id)
    }

    /// The ids of the direct children of `parent` (`""` for the root), groups and commands, in tree order.
    pub fn children(&self, parent: &str) -> Vec<(String, bool)> {
        let mut nodes: Vec<(String, bool)> = self
            .groups
            .iter()
            .map(|group| (group.id.to_string(), true))
            .chain(self.commands.iter().map(|command| (command.id.to_string(), false)))
            .filter(|(id, _)| {
                let words: Vec<&str> = id.split(' ').collect();
                let prefix = words[..words.len() - 1].join(" ");
                prefix == parent
            })
            .collect();
        nodes.sort();
        nodes
    }

    fn summary(&self, id: &str) -> &'static str {
        self.command(id)
            .map(|command| command.summary)
            .or_else(|| self.group(id).map(|group| group.summary))
            .unwrap_or_default()
    }

    pub fn entry(&self, id: &str, is_group: bool) -> Value {
        let mut entry = json!({
            "command": id,
            "path": id.split(' ').collect::<Vec<_>>(),
            "kind": if is_group { "group" } else { "command" },
            "summary": self.summary(id),
        });
        if let Some(command) = self.command(id) {
            entry["effect"] = json!(command.effect);
        }
        entry
    }

    /// Every group and command, a group before its commands.
    pub fn all_ids(&self) -> Vec<(String, bool)> {
        let mut nodes: Vec<(String, bool)> = self
            .groups
            .iter()
            .map(|group| (group.id.to_string(), true))
            .chain(self.commands.iter().map(|command| (command.id.to_string(), false)))
            .collect();
        nodes.sort();
        nodes
    }
}

fn operand_doc(arg: &Arg) -> Value {
    let mut doc = Map::new();
    doc.insert("name".into(), json!(arg.name));
    doc.insert("valueName".into(), json!(arg.value_name.unwrap_or(arg.name)));
    doc.insert("valueType".into(), json!(arg.value_type));
    doc.insert("required".into(), json!(arg.required));
    doc.insert("variadic".into(), json!(arg.repeatable));
    doc.insert("default".into(), arg.default.clone());
    doc.insert("help".into(), json!(arg.help));
    extras(arg, &mut doc);
    Value::Object(doc)
}

fn option_doc(arg: &Arg) -> Value {
    let mut doc = Map::new();
    doc.insert("name".into(), json!(arg.name));
    doc.insert("valueType".into(), json!(arg.value_type));
    doc.insert("required".into(), json!(arg.required));
    doc.insert("repeatable".into(), json!(arg.repeatable));
    doc.insert("default".into(), arg.default.clone());
    doc.insert("help".into(), json!(arg.help));
    if arg.takes_value() {
        doc.insert("valueName".into(), json!(arg.value_name.unwrap_or(arg.name)));
    }
    if let Some(short) = arg.short {
        doc.insert("short".into(), json!(short.to_string()));
    }
    if arg.flag_only {
        doc.insert("flagOnly".into(), json!(true));
    }
    if let Some(env) = arg.env {
        doc.insert("env".into(), json!(env));
    }
    extras(arg, &mut doc);
    Value::Object(doc)
}

fn extras(arg: &Arg, doc: &mut Map<String, Value>) {
    if !arg.possible.is_empty() {
        doc.insert("possibleValues".into(), json!(arg.possible));
    }
    if let Some(note) = arg.default_note {
        doc.insert("defaultNote".into(), json!(note));
    }
    if let Some(domain) = &arg.domain {
        doc.insert("domain".into(), domain.clone());
    }
    if let Some(env) = arg.env {
        doc.insert("env".into(), json!(env));
    }
}

fn rule_doc(rule: &Rule) -> Value {
    let mut doc = Map::new();
    doc.insert("rule".into(), json!(rule.rule));
    if let Some((name, values)) = &rule.when {
        let mut condition = Map::new();
        condition.insert("name".into(), json!(name));
        if !values.is_empty() {
            condition.insert("in".into(), json!(values));
        }
        doc.insert("if".into(), Value::Object(condition));
        doc.insert("then".into(), json!(rule.then));
    } else {
        doc.insert("names".into(), json!(rule.names));
    }
    if let Some(code) = rule.code {
        doc.insert("code".into(), json!(code));
    }
    Value::Object(doc)
}

fn validator_doc(tool: &str, validator: &Validator) -> Value {
    json!({
        "arguments": validator.arguments,
        "code": validator.code,
        "description": validator.description,
        "rejects": validator.rejects.iter().map(|line| line.replace("{tool}", tool)).collect::<Vec<_>>(),
    })
}

/// The command document of `command`, before any seeded patch.
pub fn command_doc(tool_name: &str, command: &Command) -> Value {
    let mut doc = Map::new();
    doc.insert("kind".into(), json!("command"));
    doc.insert("command".into(), json!(command.id));
    doc.insert("path".into(), json!(command.path()));
    doc.insert("summary".into(), json!(command.summary));
    doc.insert("effect".into(), json!(command.effect));
    if let Some(idempotency) = command.idempotency {
        doc.insert("idempotency".into(), json!(idempotency));
    }
    if command.is_async {
        doc.insert("async".into(), json!(true));
    }
    if let Some(note) = command.effect_note {
        doc.insert("effectNote".into(), json!(note));
    }
    doc.insert(
        "operands".into(),
        Value::Array(command.operands().map(operand_doc).collect()),
    );
    doc.insert(
        "options".into(),
        Value::Array(command.flags().map(option_doc).collect()),
    );
    if !command.rules.is_empty() {
        doc.insert(
            "constraints".into(),
            Value::Array(command.rules.iter().map(rule_doc).collect()),
        );
    }
    if !command.validators.is_empty() {
        doc.insert(
            "validators".into(),
            Value::Array(
                command
                    .validators
                    .iter()
                    .map(|validator| validator_doc(tool_name, validator))
                    .collect(),
            ),
        );
    }
    let mut errors = command.errors.clone();
    errors.sort_unstable();
    errors.dedup();
    doc.insert("errors".into(), json!(errors));
    doc.insert(
        "outputs".into(),
        Value::Array(
            command
                .outputs
                .iter()
                .map(|output| {
                    let mut entry = json!({
                        "kind": if output.event.is_some() { "stream" } else { "report" },
                        "schemaRef": format!("#/schemas/{}", output.schema),
                        "schemaVersion": output.version,
                    });
                    if let Some(event) = output.event {
                        entry["mediaType"] = json!("application/x-ndjson");
                        entry["eventRef"] = json!(format!("#/schemas/{event}"));
                    }
                    if let Some(when) = output.when {
                        entry["when"] = json!(when);
                    }
                    if output.batch {
                        entry["batch"] = json!(true);
                    }
                    if let Some(fields) = &output.fields {
                        entry["fields"] = fields.clone();
                    }
                    entry
                })
                .collect(),
        ),
    );
    if let Some(input) = &command.input {
        doc.insert("input".into(), input.clone());
    }
    doc.insert(
        "examples".into(),
        json!(command
            .examples
            .iter()
            .map(|line| line.replace("{tool}", tool_name))
            .collect::<Vec<_>>()),
    );
    let mut schemas = Map::new();
    for (name, schema) in &command.schemas {
        schemas.insert((*name).to_string(), schema.clone());
    }
    doc.insert("schemas".into(), Value::Object(schemas));
    Value::Object(doc)
}

/// The group document of `id`, before any seeded patch.
pub fn group_doc(model: &Model, id: &str) -> Value {
    let children: Vec<Value> = model
        .children(id)
        .iter()
        .map(|(child, is_group)| model.entry(child, *is_group))
        .collect();
    json!({
        "kind": "group",
        "command": id,
        "path": id.split(' ').collect::<Vec<_>>(),
        "summary": model.summary(id),
        "commands": children,
    })
}

/// The tree document, before any seeded patch.
pub fn tree_doc(model: &Model, groups_last: bool, level: &str) -> Value {
    let mut ids = model.all_ids();
    if groups_last {
        ids.sort_by_key(|(_, is_group)| *is_group);
    }
    json!({
        "kind": "tree",
        "level": level,
        "commands": ids.iter().map(|(id, is_group)| model.entry(id, *is_group)).collect::<Vec<_>>(),
    })
}

/// `document` after the seeded patch of the configuration member `docPatch` under `key`, and the nondeterminism
/// the configuration asks for.
pub fn patched(tool: &Tool, key: &str, mut document: Value) -> Value {
    if let Some(ops) = tool
        .config
        .get("docPatch")
        .and_then(|patches| patches.get(key))
        .and_then(Value::as_array)
    {
        if let Err(reason) = patch::apply(&mut document, ops) {
            eprintln!("fake tool: docPatch {key}: {reason}");
            std::process::exit(70);
        }
    }
    let unstable = if key == "<tree>" {
        tool.flag("treeNondeterministic")
    } else {
        tool.lists("docNondeterministic", key)
    };
    if unstable {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        document["x-fake"] = json!({"run": nanos.to_string()});
    }
    if tool.flag("usageRenamed") {
        rename_usage(&mut document, &tool.usage_name());
    }
    document
}

/// Seeded (`usageRenamed`): the tool has no `USAGE`; every `errors` list of a document names `replacement` in its place.
fn rename_usage(document: &mut Value, replacement: &str) {
    match document {
        Value::Object(members) => {
            for (key, value) in members.iter_mut() {
                if key == "errors" {
                    if let Value::Array(items) = value {
                        for item in items.iter_mut().filter(|item| item.as_str() == Some("USAGE")) {
                            *item = json!(replacement);
                        }
                        items.sort_by_key(|item| item.as_str().map(str::to_string));
                    }
                }
                rename_usage(value, replacement);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                rename_usage(item, replacement);
            }
        }
        _ => {}
    }
}
