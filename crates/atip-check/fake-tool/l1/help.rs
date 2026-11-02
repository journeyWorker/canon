//! `--help`, derived from the (patched) documents so that the help agrees with `schema` by construction.

use std::fmt::Write as _;

use serde_json::Value;

use super::doc::Model;
use crate::Tool;

fn agents_line(tool: &Tool, path: &str) -> String {
    let name = tool.name();
    if path.is_empty() && tool.flag("l2") && !tool.flag("helpNoSkillsLine") {
        format!("Agents: \"{name} schema --json\" describes every command as JSON; \"{name} skills show\" prints the guides.\n\n")
    } else if path.is_empty() {
        format!("Agents: \"{name} schema --json\" describes every command as JSON.\n\n")
    } else {
        format!("Agents: \"{name} schema {path} --json\" describes this command as JSON.\n\n")
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The help of a command, from its command document.
pub fn command_help(tool: &Tool, doc: &Value) -> String {
    let path = doc["command"].as_str().unwrap_or_default();
    let name = tool.name();
    let mut out = String::new();
    if !tool.flag("helpNoSchemaLine") && !tool.flag("noSchema") {
        out.push_str(&agents_line(tool, path));
    }
    let _ = writeln!(out, "{}\n", doc["summary"].as_str().unwrap_or_default());
    let operands = doc["operands"].as_array().cloned().unwrap_or_default();
    let mut usage = format!("Usage: {name} {path} [OPTIONS]");
    for operand in &operands {
        let value_name = operand["valueName"].as_str().unwrap_or_default();
        let required = operand["required"].as_bool().unwrap_or(false);
        let variadic = operand["variadic"].as_bool().unwrap_or(false);
        let dots = if variadic { "..." } else { "" };
        let _ = if required {
            write!(usage, " <{value_name}>{dots}")
        } else {
            write!(usage, " [{value_name}]{dots}")
        };
    }
    let _ = writeln!(out, "{usage}\n");
    if !operands.is_empty() {
        out.push_str("Arguments:\n");
        for operand in &operands {
            let _ = writeln!(
                out,
                "  <{}>  {}",
                operand["valueName"].as_str().unwrap_or_default(),
                operand["help"].as_str().unwrap_or_default()
            );
        }
        out.push('\n');
    }
    if tool.flag("helpBullet") {
        out.push_str("  - see --limit (the usual [default: 99])\n\n");
    }
    out.push_str("Options:\n");
    for option in doc["options"].as_array().cloned().unwrap_or_default() {
        let long = option["name"].as_str().unwrap_or_default();
        let mut label = option["short"]
            .as_str()
            .map_or_else(|| "    ".to_string(), |short| format!("-{short}, "));
        let _ = write!(label, "--{long}");
        if let Some(value_name) = option["valueName"].as_str() {
            let _ = write!(label, " <{value_name}>");
        }
        let mut help = option["help"].as_str().unwrap_or_default().to_string();
        if let Some(mentioned) = tool.config.get("helpMentions").and_then(|mentions| mentions.get(long)).and_then(Value::as_str) {
            let _ = write!(help, " (see {mentioned})");
        }
        let mut line = format!("  {label:<28}{help}");
        let kind = option["valueType"].as_str().unwrap_or_default();
        if let Some(values) = option["possibleValues"].as_array() {
            let list: Vec<String> = values.iter().map(shown).collect();
            let _ = write!(line, " [possible values: {}]", list.join(", "));
        }
        if !option["default"].is_null() && !matches!(kind, "switch" | "count") {
            let mut default = shown(&option["default"]);
            if tool.flag("helpDefaultDiffers") && long == "limit" {
                default = "51".to_string();
            }
            let _ = write!(line, " [default: {default}]");
        }
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("  -h, --help                  Print help\n");
    out
}

/// The help of a group or of the root: its children and their summaries.
pub fn group_help(tool: &Tool, model: &Model, group: &str) -> String {
    let name = tool.name();
    let mut out = String::new();
    if !tool.flag("helpNoSchemaLine") && !tool.flag("noSchema") {
        out.push_str(&agents_line(tool, group));
    }
    let place = if group.is_empty() { name } else { format!("{name} {group}") };
    let _ = writeln!(out, "Usage: {place} <COMMAND>\n");
    out.push_str("Commands:\n");
    for (id, _) in model.children(group) {
        let word = id.rsplit(' ').next().unwrap_or_default();
        let summary = model
            .command(&id)
            .map(|command| command.summary)
            .or_else(|| model.group(&id).map(|group| group.summary))
            .unwrap_or_default();
        let _ = writeln!(out, "  {word:<10}{summary}");
    }
    out
}
