//! The one internal model of the fake tool's commands: the printed documents (`doc.rs`), `--help` (`help.rs`) and
//! the enforcement at run time (`parse.rs`) are all derived from these values, so they agree by construction.

use serde_json::{json, Value};

/// Whether an argument is an operand or an option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Operand,
    Option,
}

/// One operand or option of a command (spec 10.6, 10.7).
#[derive(Debug, Clone)]
pub struct Arg {
    pub name: &'static str,
    pub kind: Kind,
    /// `switch`, `count`, `string`, `integer`, `number`, `path`, `enum`, `json`.
    pub value_type: &'static str,
    pub value_name: Option<&'static str>,
    pub short: Option<char>,
    pub required: bool,
    /// An option given once per value, or a variadic operand.
    pub repeatable: bool,
    pub default: Value,
    pub default_note: Option<&'static str>,
    pub possible: Vec<&'static str>,
    pub domain: Option<Value>,
    pub flag_only: bool,
    pub help: &'static str,
}

impl Arg {
    fn new(kind: Kind, name: &'static str, value_type: &'static str, help: &'static str) -> Self {
        let default = match value_type {
            "switch" => json!(false),
            "count" => json!(0),
            _ => Value::Null,
        };
        Self {
            name,
            kind,
            value_type,
            value_name: None,
            short: None,
            required: false,
            repeatable: value_type == "count",
            default,
            default_note: None,
            possible: Vec::new(),
            domain: None,
            flag_only: false,
            help,
        }
    }

    pub fn option(name: &'static str, value_type: &'static str, help: &'static str) -> Self {
        Self::new(Kind::Option, name, value_type, help)
    }

    pub fn operand(name: &'static str, value_type: &'static str, help: &'static str) -> Self {
        Self::new(Kind::Operand, name, value_type, help)
    }

    pub fn value(mut self, value_name: &'static str) -> Self {
        self.value_name = Some(value_name);
        self
    }

    pub fn short(mut self, letter: char) -> Self {
        self.short = Some(letter);
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn repeatable(mut self) -> Self {
        self.repeatable = true;
        self
    }

    pub fn default(mut self, default: Value) -> Self {
        self.default = default;
        self
    }

    pub fn default_note(mut self, note: &'static str) -> Self {
        self.default_note = Some(note);
        self
    }

    pub fn possible(mut self, values: &[&'static str]) -> Self {
        self.possible = values.to_vec();
        self
    }

    pub fn domain(mut self, domain: Value) -> Self {
        self.domain = Some(domain);
        self
    }

    pub fn flag_only(mut self) -> Self {
        self.flag_only = true;
        self
    }

    /// The name a failure's `field` gives: the option as written, an operand's value name.
    pub fn field(&self) -> String {
        match self.kind {
            Kind::Option => format!("--{}", self.name),
            Kind::Operand => self.value_name.unwrap_or(self.name).to_string(),
        }
    }

    /// Whether the argument takes a value on the command line.
    pub fn takes_value(&self) -> bool {
        self.kind == Kind::Operand || !matches!(self.value_type, "switch" | "count")
    }
}

/// One constraint (spec 10.8.1).
#[derive(Debug, Clone)]
pub struct Rule {
    pub rule: &'static str,
    pub names: Vec<&'static str>,
    /// `if`: the argument and, for a rule on a value, the values that make it hold.
    pub when: Option<(&'static str, Vec<Value>)>,
    pub then: Vec<&'static str>,
    pub code: Option<&'static str>,
}

impl Rule {
    fn set(rule: &'static str, names: &[&'static str]) -> Self {
        Self {
            rule,
            names: names.to_vec(),
            when: None,
            then: Vec::new(),
            code: None,
        }
    }

    pub fn at_least_one(names: &[&'static str]) -> Self {
        Self::set("atLeastOne", names)
    }

    pub fn exactly_one(names: &[&'static str]) -> Self {
        Self::set("exactlyOne", names)
    }

    pub fn at_most_one(names: &[&'static str]) -> Self {
        Self::set("atMostOne", names)
    }

    fn relation(rule: &'static str, name: &'static str, values: &[&str], then: &[&'static str]) -> Self {
        Self {
            rule,
            names: Vec::new(),
            when: Some((name, values.iter().map(|value| json!(value)).collect())),
            then: then.to_vec(),
            code: None,
        }
    }

    /// `name` (holding one of `values` when given) requires every name of `then`.
    pub fn requires(name: &'static str, values: &[&str], then: &[&'static str]) -> Self {
        Self::relation("requires", name, values, then)
    }

    /// `name` (holding one of `values` when given) forbids every name of `then`.
    pub fn forbids(name: &'static str, values: &[&str], then: &[&'static str]) -> Self {
        Self::relation("forbids", name, values, then)
    }

    pub fn code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }
}

/// The stateless checks that a domain and a constraint cannot express (spec 10.8.3).
#[derive(Debug, Clone)]
pub enum Check {
    /// The value of the argument is a calendar date that exists, `YYYY-MM-DD`.
    CalendarDate(&'static str),
}

/// A validator: what it reads, the code it refuses with, how to word it, and invocations it refuses.
#[derive(Debug, Clone)]
pub struct Validator {
    pub arguments: Vec<&'static str>,
    pub code: &'static str,
    pub description: &'static str,
    pub rejects: Vec<&'static str>,
    pub check: Check,
}

/// One output of a command.
#[derive(Debug, Clone)]
pub struct Output {
    /// The name of its schema in `schemas`.
    pub schema: &'static str,
    pub when: Option<&'static str>,
    pub fields: Option<Value>,
    /// The `schemaVersion` of the output: `1.0` for a report, the format version for a `schema` answer.
    pub version: &'static str,
}

/// A leaf command.
#[derive(Debug, Clone)]
pub struct Command {
    pub id: &'static str,
    pub summary: &'static str,
    pub effect: &'static str,
    pub idempotency: Option<&'static str>,
    pub effect_note: Option<&'static str>,
    pub args: Vec<Arg>,
    pub rules: Vec<Rule>,
    pub validators: Vec<Validator>,
    pub errors: Vec<&'static str>,
    pub outputs: Vec<Output>,
    /// Complete invocations, `{tool}` standing for the tool name.
    pub examples: Vec<&'static str>,
    pub schemas: Vec<(&'static str, Value)>,
    /// The `input` member of the document, with `{tool}` free text only in `example`.
    pub input: Option<Value>,
}

impl Command {
    pub fn new(id: &'static str, summary: &'static str) -> Self {
        Self {
            id,
            summary,
            effect: "read",
            idempotency: None,
            effect_note: None,
            args: Vec::new(),
            rules: Vec::new(),
            validators: Vec::new(),
            errors: Vec::new(),
            outputs: Vec::new(),
            examples: Vec::new(),
            schemas: Vec::new(),
            input: None,
        }
    }

    pub fn writes(mut self, idempotency: &'static str) -> Self {
        self.effect = "write";
        self.idempotency = Some(idempotency);
        self
    }

    pub fn operand(mut self, arg: Arg) -> Self {
        self.args.push(arg);
        self
    }

    /// Adds options, then the tool-wide `--json` and `--repo`.
    pub fn options(mut self, args: Vec<Arg>) -> Self {
        self.args.extend(args);
        self.args.push(
            Arg::option("json", "switch", "Print the answer as one JSON document")
                .flag_only(),
        );
        self.args.push(
            Arg::option("repo", "path", "The repository directory the command works in")
                .value("PATH")
                .default_note("the current directory"),
        );
        self
    }

    pub fn rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn errors(mut self, codes: &[&'static str]) -> Self {
        self.errors = codes.to_vec();
        self
    }

    pub fn output(mut self, schema: &'static str, schema_value: Value) -> Self {
        self.outputs.push(Output {
            schema,
            when: None,
            fields: None,
            version: "1.0",
        });
        self.schemas.push((schema, schema_value));
        self
    }

    pub fn output_when(mut self, schema: &'static str, when: &'static str, schema_value: Value) -> Self {
        self.outputs.push(Output {
            schema,
            when: Some(when),
            fields: None,
            version: "1.0",
        });
        self.schemas.push((schema, schema_value));
        self
    }

    /// Declares `fields` on the last output.
    pub fn fields(mut self, fields: Value) -> Self {
        if let Some(output) = self.outputs.last_mut() {
            output.fields = Some(fields);
        }
        self
    }

    /// Sets the version of the last output.
    pub fn version(mut self, version: &'static str) -> Self {
        if let Some(output) = self.outputs.last_mut() {
            output.version = version;
        }
        self
    }

    pub fn schema(mut self, name: &'static str, schema: Value) -> Self {
        self.schemas.push((name, schema));
        self
    }

    pub fn examples(mut self, examples: &[&'static str]) -> Self {
        self.examples = examples.to_vec();
        self
    }

    pub fn input(mut self, input: Value) -> Self {
        self.input = Some(input);
        self
    }

    pub fn arg(&self, name: &str) -> Option<&Arg> {
        self.args.iter().find(|arg| arg.name == name)
    }

    pub fn operands(&self) -> impl Iterator<Item = &Arg> {
        self.args.iter().filter(|arg| arg.kind == Kind::Operand)
    }

    pub fn flags(&self) -> impl Iterator<Item = &Arg> {
        self.args.iter().filter(|arg| arg.kind == Kind::Option)
    }

    /// The path words of the command id.
    pub fn path(&self) -> Vec<&'static str> {
        self.id.split(' ').collect()
    }
}

/// A group: its id and summary; its children are the commands (and groups) whose path extends it by one word.
#[derive(Debug, Clone, Copy)]
pub struct Group {
    pub id: &'static str,
    pub summary: &'static str,
}

/// What the tool's command set depends on: the claim and the seeded extras.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    /// Whether the tool claims 1.1 (the warning registry, `--warning`).
    pub is_1_1: bool,
    /// Whether the tree has a path of three words (`team roster list`).
    pub deep: bool,
}

pub fn groups(shape: Shape) -> Vec<Group> {
    let mut groups = vec![
        Group {
            id: "member",
            summary: "Find and tag the members of the directory",
        },
        Group {
            id: "team",
            summary: "Manage teams",
        },
    ];
    if shape.deep {
        groups.push(Group {
            id: "team roster",
            summary: "Read the roster of a team",
        });
    }
    groups
}

const DATE_PATTERN: &str = "^[0-9]{4}-[0-9]{2}-[0-9]{2}$";
const TAG_PATTERN: &str = "^[a-z][a-z0-9-]*$";

fn state_report(name: &'static str) -> (&'static str, Value) {
    (
        name,
        json!({
            "type": "object",
            "required": ["items", "state", "remote"],
            "properties": {
                "items": {"type": "integer"},
                "state": {"type": "string"},
                "remote": {"type": "integer"},
                "ci": {"type": "boolean"}
            },
            "additionalProperties": false
        }),
    )
}

fn paged_read(id: &'static str, summary: &'static str, report: &'static str) -> Command {
    let (name, schema) = state_report(report);
    Command::new(id, summary)
        .options(vec![Arg::option(
            "cursor",
            "string",
            "Resume after the last item of the previous page (meta.page.nextCursor)",
        )
        .value("CURSOR")])
        .errors(&["CURSOR_INVALID", "STATE_ODD", "USAGE"])
        .output(name, schema)
}

fn member_list(is_1_1: bool) -> Command {
    let mut command = Command::new("member list", "List the members of the directory, a page at a time")
        .options(vec![
            Arg::option("team", "string", "Only the members of this team")
                .value("TEAM"),
            Arg::option("sort", "enum", "The order of the members")
                .value("SORT")
                .possible(&["name", "id"])
                .default(json!("name")),
            Arg::option("limit", "integer", "Return at most this many members")
                .value("N")
                .default(json!(50))
                .domain(json!({"minimum": 1})),
            Arg::option(
                "cursor",
                "string",
                "Resume after the last member of the previous page (meta.page.nextCursor)",
            )
            .value("CURSOR"),
            Arg::option("all", "switch", "Return every member from the cursor on"),
            Arg::option(
                "fields",
                "string",
                "Keep only these member fields (comma-separated paths); see outputs[0].fields",
            )
            .value("FIELDS"),
        ])
        .rule(Rule::at_most_one(&["limit", "all"]))
        .errors(&["CURSOR_INVALID", "USAGE"])
        .output(
            "MemberListReport",
            json!({
                "type": "object",
                "required": ["members"],
                "properties": {
                    "members": {"type": "array", "items": {"$ref": "#/schemas/MemberRow"}}
                },
                "additionalProperties": false
            }),
        )
        .fields(json!({
            "records": "/members",
            "keep": ["id"],
            "paths": [
                "name", "nickname", "profile", "profile.name", "profile.email", "tags", "owners",
                "owners[].name", "owners[].email", "attributes"
            ]
        }))
        .schema(
            "MemberRow",
            json!({
                "type": "object",
                "description": "One member of the directory.",
                "required": ["id", "name", "profile", "tags", "owners", "attributes"],
                "properties": {
                    "id": {"type": "integer"},
                    "name": {"type": "string"},
                    "nickname": {"type": "string"},
                    "profile": {"anyOf": [{"$ref": "#/schemas/Profile"}, {"type": "null"}]},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "owners": {"type": "array", "items": {"$ref": "#/schemas/Owner"}},
                    "attributes": {
                        "description": "Free-form directory attributes: a map, so each key is selectable (spec 6.10).",
                        "type": "object",
                        "additionalProperties": {"type": "string"}
                    }
                },
                "additionalProperties": false
            }),
        )
        .schema(
            "Profile",
            json!({
                "type": "object",
                "required": ["name", "email", "timezone"],
                "properties": {
                    "name": {"type": "string"},
                    "email": {"type": "string"},
                    "timezone": {"type": "string"}
                },
                "additionalProperties": false
            }),
        )
        .schema(
            "Owner",
            json!({
                "type": "object",
                "required": ["name", "email"],
                "properties": {"name": {"type": "string"}, "email": {"type": "string"}},
                "additionalProperties": false
            }),
        )
        .examples(&[
            "{tool} member list --team core --limit 2 --fields profile.name,owners[].email --json",
            "{tool} member list --team=core --sort id --json",
        ]);
    if is_1_1 {
        command.examples.push(
            "{tool} member list --fields 'attributes[\"http.status-code\"]' --json",
        );
    }
    command
}

fn member_search() -> Command {
    Command::new("member search", "Search the members by name, tag and age")
        .operand(
            Arg::operand("query", "string", "Text the member's name contains")
                .value("QUERY")
                .domain(json!({"minLength": 2, "maxLength": 40})),
        )
        .options(vec![
            Arg::option("tag", "string", "Only members with this tag; repeat for several")
                .value("TAG")
                .repeatable()
                .domain(json!({"pattern": TAG_PATTERN})),
            Arg::option("filter", "json", "A filter document: {\"team\": <text>, \"min\": <integer>}")
                .value("JSON")
                .domain(json!({
                    "type": "object",
                    "properties": {
                        "team": {"type": "string", "minLength": 1},
                        "min": {"type": "integer", "minimum": 0}
                    },
                    "additionalProperties": false
                })),
            Arg::option("min-score", "integer", "Only matches with at least this score")
                .value("N")
                .domain(json!({"minimum": 0, "maximum": 100})),
            Arg::option("age", "integer", "Only members younger than this many years")
                .value("YEARS")
                .domain(json!({"exclusiveMinimum": 0, "exclusiveMaximum": 150})),
            Arg::option("step", "integer", "Group the scores in steps of this size")
                .value("N")
                .domain(json!({"minimum": 0, "multipleOf": 5})),
            Arg::option("since", "string", "Only members who joined on or after this date, YYYY-MM-DD")
                .value("DATE")
                .domain(json!({"pattern": DATE_PATTERN})),
            Arg::option("sort", "enum", "The order of the matches")
                .value("SORT")
                .possible(&["name", "score", "age"])
                .default(json!("name")),
            Arg::option("exact", "switch", "Match the whole name"),
            Arg::option("prefix", "switch", "Match the start of the name"),
            Arg::option("quiet", "switch", "Print the names only"),
            Arg::option("verbose", "count", "Say more; once per level")
                .short('v')
                .domain(json!({"maximum": 3})),
            Arg::option("total", "switch", "Print only how many members match"),
        ])
        .rule(Rule::at_least_one(&["query", "tag", "filter"]))
        .rule(Rule::exactly_one(&["exact", "prefix"]))
        .rule(Rule::at_most_one(&["quiet", "verbose"]))
        .rule(Rule::requires("sort", &["score"], &["min-score"]))
        .rule(Rule::forbids("sort", &["age"], &["step"]).code("SORT_CONFLICT"))
        .validator(Validator {
            arguments: vec!["since"],
            code: "DATE_INVALID",
            description: "The date must exist on the calendar: 2026-02-30 matches the pattern and is refused.",
            rejects: vec!["{tool} member search ada --exact --since=2026-02-30 --json"],
            check: Check::CalendarDate("since"),
        })
        .errors(&["DATE_INVALID", "SORT_CONFLICT", "USAGE"])
        .output(
            "MemberSearchReport",
            json!({
                "type": "object",
                "required": ["count", "matches"],
                "properties": {
                    "count": {"type": "integer", "minimum": 0},
                    "matches": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["id", "name", "score"],
                            "properties": {
                                "id": {"type": "integer"},
                                "name": {"type": "string"},
                                "score": {"type": "integer"}
                            },
                            "additionalProperties": false
                        }
                    }
                },
                "additionalProperties": false
            }),
        )
        .output_when(
            "MemberTotalReport",
            "--total",
            json!({
                "type": "object",
                "required": ["count"],
                "properties": {"count": {"type": "integer", "minimum": 0}},
                "additionalProperties": false
            }),
        )
        .examples(&[
            "{tool} member search ada --tag lead --min-score 10 --age 30 --step 10 --since 2026-02-01 --sort name --exact --json",
            "{tool} member search --filter '{\"team\":\"core\"}' --prefix --total --json",
        ])
}

fn member_tag() -> Command {
    Command::new("member tag", "Tag a member; the tags go to tags/<MEMBER>.json")
        .writes("replay-safe")
        .operand(
            Arg::operand("member", "integer", "The member to tag")
                .value("MEMBER")
                .required()
                .domain(json!({"minimum": 1})),
        )
        .options(vec![
            Arg::option("tag", "string", "The tag to add; repeat for several")
                .value("TAG")
                .required()
                .repeatable()
                .domain(json!({"pattern": TAG_PATTERN})),
            Arg::option("note", "string", "A note kept with the tags")
                .value("TEXT")
                .domain(json!({"maxLength": 80})),
            Arg::option("clear-note", "switch", "Remove the note kept with the tags"),
        ])
        .rule(Rule::requires("tag", &[], &["member"]))
        .rule(Rule::at_most_one(&["note", "clear-note"]))
        .rule(Rule::forbids("note", &[], &["clear-note"]))
        .errors(&["MEMBER_NOT_FOUND", "USAGE"])
        .output(
            "MemberTagReport",
            json!({
                "type": "object",
                "required": ["member", "tags", "status"],
                "properties": {
                    "member": {"type": "integer"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "note": {"type": "string"},
                    "status": {"enum": ["written", "unchanged"]}
                },
                "additionalProperties": false
            }),
        )
        .examples(&["{tool} member tag 3 --tag lead --tag core --note promoted --json"])
}

fn schema_command(shape: Shape) -> Command {
    let mut options = vec![
        Arg::option("errors", "switch", "Print the error registry"),
        Arg::option("error", "string", "Print one error registry entry").value("CODE"),
    ];
    let mut names = vec!["path", "errors", "error"];
    if shape.is_1_1 {
        options.push(Arg::option("warnings", "switch", "Print the warning registry"));
        options.push(Arg::option("warning", "string", "Print one warning registry entry").value("CODE"));
        names.extend(["warnings", "warning"]);
    }
    Command::new("schema", "Describe the commands of the tool, or its registries, as JSON")
        .operand(
            Arg::operand("path", "string", "Words of a command path, or one quoted string")
                .value("PATH")
                .repeatable(),
        )
        .options(options)
        .rule(Rule::at_most_one(&names))
        .errors(&["UNKNOWN_COMMAND", "USAGE"])
        .output(
            "SchemaDocument",
            json!({
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": {"enum": ["tree", "group", "command", "errors", "error", "warnings", "warning"]}
                }
            }),
        )
        .version(if shape.is_1_1 { "1.1" } else { "1.0" })
        .examples(&[
            "{tool} schema --json",
            "{tool} schema member list --json",
            "{tool} schema --errors --json",
        ])
}

fn touch() -> Command {
    Command::new("touch", "Write touched.txt")
        .writes("replay-safe")
        .options(vec![
            Arg::option("name", "string", "What to touch").value("NAME").required(),
            Arg::option("input", "string", "The arguments as inline JSON").value("JSON").flag_only(),
        ])
        .errors(&["INPUT_CONFLICT", "INPUT_INVALID", "INPUT_TOO_LARGE", "USAGE"])
        .output(
            "TouchReport",
            json!({
                "type": "object",
                "required": ["touched"],
                "properties": {"touched": {"type": "boolean"}},
                "additionalProperties": false
            }),
        )
        .schema(
            "TouchInput",
            json!({
                "type": "object",
                "properties": {"name": {"type": "string"}},
                "additionalProperties": false
            }),
        )
        .schema(
            "TouchArguments",
            json!({
                "type": "object",
                "required": ["name"],
                "properties": {"name": {"type": "string"}},
                "additionalProperties": false
            }),
        )
        .input(json!({
            "option": "--input",
            "schemaRef": "#/schemas/TouchInput",
            "argumentsRef": "#/schemas/TouchArguments",
            "batch": false,
            "maxBytes": MAX_INPUT_BYTES,
            "example": {"name": "log"}
        }))
        .examples(&[
            "{tool} touch --name report --json",
            "{tool} touch --input '{\"name\":\"log\"}' --json",
        ])
}

/// The most bytes `touch --input` accepts.
pub const MAX_INPUT_BYTES: usize = 4096;

/// Every leaf command of the tool, in the order of the tree.
pub fn commands(shape: Shape, schema_offered: bool) -> Vec<Command> {
    let mut commands = vec![
        Command::new("init", "Create the state file")
            .writes("replay-safe")
            .options(Vec::new())
            .errors(&["USAGE"])
            .output(
                "InitReport",
                json!({
                    "type": "object",
                    "required": ["initialised"],
                    "properties": {"initialised": {"type": "boolean"}},
                    "additionalProperties": false
                }),
            )
            .examples(&["{tool} init --json"]),
        paged_read("items", "Count the items", "ItemsReport").examples(&["{tool} items --json"]),
        member_list(shape.is_1_1),
        member_search(),
        member_tag(),
        Command::new("reject", "Send a request the backend always rejects")
            .writes("replay-safe")
            .options(Vec::new())
            .errors(&["USAGE"])
            .output(
                "RejectReport",
                json!({"type": "object", "additionalProperties": false}),
            )
            .examples(&["{tool} reject --json"]),
        paged_read("status", "Report the state of the tool", "StatusReport")
            .examples(&["{tool} status --json"]),
        Command::new("team add", "Add a team; writes team.txt")
            .writes("replay-safe")
            .options(Vec::new())
            .errors(&["USAGE"])
            .output(
                "TeamAddReport",
                json!({
                    "type": "object",
                    "required": ["added"],
                    "properties": {"added": {"type": "boolean"}},
                    "additionalProperties": false
                }),
            )
            .examples(&["{tool} team add --json"]),
        touch(),
    ];
    if schema_offered {
        commands.push(schema_command(shape));
    }
    if shape.deep {
        commands.push(
            Command::new("team roster list", "List the members of the roster of a team")
                .options(Vec::new())
                .errors(&["USAGE"])
                .output(
                    "RosterReport",
                    json!({
                        "type": "object",
                        "required": ["roster"],
                        "properties": {"roster": {"type": "array", "items": {"type": "string"}}},
                        "additionalProperties": false
                    }),
                )
                .examples(&["{tool} team roster list --json"]),
        );
    }
    commands.sort_by_key(|command| command.id);
    commands
}
