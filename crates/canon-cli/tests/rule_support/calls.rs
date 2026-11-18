//! The calls the search finds, kept as data (0.42 `canon-0-42-test-cost`).
//!
//! [`super::search::solve`] looks at every set of the arguments the rules of a command name: for `evidence add` 262,144 sets, about ten
//! seconds a search, and two test binaries asked it for the same rules. What it finds is a pure function of its inputs, so it is kept in
//! `tests/atip/rule_calls.jsonl` and the sweeps read it. A row stores the finding (the arguments and the other rules the call breaks), not the
//! words of the call: the words are made when a sweep reads it, so a required argument no rule names can change its value without making the
//! table stale.
//!
//! A row is keyed by (command, goal, source of the example). The source is the document (the first example of the command document, which
//! `atip_check_order` uses: violations and pairs of rules) or the story (the example with the ids the story repo gives it, which `arg_rules`
//! uses: violations and calls that meet a rule). Every row carries a digest of what the search read: the rules of the command, the
//! arguments they name with their candidate values, the required arguments, the example words the search used and the revision of `atip` in
//! `Cargo.lock`. A digest that is no longer the one of the inputs makes the
//! read fail, saying how to make the table again; a table never falls back to searching.
//!
//! `CANON_BLESS_RULE_CALLS=1 cargo test -p canon-cli --test arg_rules the_stored_rule_calls` (`just rule-calls`) writes the table.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use atip::doc::ConstraintDoc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::given_from;
use super::search::{candidates, constraints, doc_example, exercised, names_of, parse_call, solve, walk_pairs, Found, Goal};

/// The environment variable that makes the stored table again (the family of `CANON_BLESS_*`).
pub const BLESS: &str = "CANON_BLESS_RULE_CALLS";

/// What a failure of the table says to do.
pub const REMEDY: &str = "make the table again with `just rule-calls` (CANON_BLESS_RULE_CALLS=1 cargo test -p canon-cli --test arg_rules the_stored_rule_calls) and commit it";

/// The table, relative to the crate.
pub fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/atip/rule_calls.jsonl")
}

/// Where the example of a search came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Doc,
    Story,
}

impl Source {
    fn name(self) -> &'static str {
        match self {
            Source::Doc => "doc",
            Source::Story => "story",
        }
    }
}

/// The key of a row: command, source and goal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub command: String,
    pub source: Source,
    pub goal: Goal,
}

impl Key {
    fn describe(&self) -> String {
        format!("`{}` ({}) {:?}", self.command, self.source.name(), self.goal)
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    pub digest: String,
    pub found: Option<Found>,
}

#[derive(Debug)]
pub struct Table {
    pub atip: String,
    pub rows: BTreeMap<Key, Row>,
}

// ------------------------------------------------------------------------------------------------ the inputs

/// The revision of `atip` that `Cargo.lock` names.
pub fn lock_atip_rev() -> String {
    let lock = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock")).expect("Cargo.lock");
    let block = lock.split("[[package]]").find(|block| block.trim_start().starts_with("name = \"atip\"")).expect("Cargo.lock names atip");
    let source = block.lines().find_map(|line| line.strip_prefix("source = ")).expect("atip has a source");
    source.trim_matches('"').to_string()
}

fn hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// What a check reads besides the documents.
pub struct World<'a> {
    pub docs: &'a [Value],
    /// The example of a command as the story runs it.
    pub story_example: &'a (dyn Fn(&Value) -> Vec<String> + Sync),
    pub atip: &'a str,
}

fn example_of(doc: &Value, source: Source, world: &World) -> Vec<String> {
    match source {
        Source::Doc => doc_example(doc),
        Source::Story => (world.story_example)(doc),
    }
}

/// The digest of everything the search of `doc` from `example` reads.
pub fn digest_of(doc: &Value, rules: &[ConstraintDoc], example: &[String], atip: &str) -> String {
    let names = names_of(rules);
    let parsed = parse_call(doc, example).map_or_default(|parsed| parsed.text);
    let arguments: Vec<Value> = names
        .iter()
        .map(|name| {
            let argument = super::search::argument(doc, name).cloned().unwrap_or(Value::Null);
            json!({
                "name": name,
                "valueType": argument["valueType"],
                "valueName": argument["valueName"],
                "possibleValues": argument["possibleValues"],
                "required": argument["required"],
                "candidates": candidates(doc, example, rules, name),
                "example": parsed.get(name),
            })
        })
        .collect();
    let required: Vec<&Value> = super::search::arguments(doc).into_iter().filter(|argument| argument["required"] == true).map(|argument| &argument["name"]).collect();
    let input = json!({ "atip": atip, "command": doc["command"], "constraints": doc["constraints"], "required": required, "arguments": arguments });
    hex(&input.to_string())[..16].to_string()
}

// ------------------------------------------------------------------------------------------ text and rows

fn goal_json(goal: Goal) -> (&'static str, Vec<usize>) {
    match goal {
        Goal::Violate(at) => ("violate", vec![at]),
        Goal::Both(a, b) => ("both", vec![a, b]),
        Goal::Satisfy(at) => ("satisfy", vec![at]),
    }
}

/// The table as text: a header line, then one row a line in key order.
pub fn render(atip: &str, rows: &BTreeMap<Key, Row>) -> String {
    let header = json!({ "_doNotEdit": format!("GENERATED by tests/rule_support/calls.rs: {REMEDY}."), "atip": atip });
    let mut out = format!("{header}\n");
    for (key, row) in rows {
        let (goal, rules) = goal_json(key.goal);
        let found = row.found.as_ref().map(|found| json!({ "alsoBreaks": found.also_breaks, "assignment": found.assignment }));
        out.push_str(&json!({ "command": key.command, "digest": row.digest, "found": found, "goal": goal, "rules": rules, "source": key.source.name() }).to_string());
        out.push('\n');
    }
    out
}

impl Table {
    pub fn parse(text: &str) -> Result<Table, String> {
        let mut lines = text.lines();
        let header: Value = serde_json::from_str(lines.next().unwrap_or_default()).map_err(|why| format!("the rule-call table has no header ({why}); {REMEDY}"))?;
        let atip = header["atip"].as_str().ok_or_else(|| format!("the rule-call table header names no atip revision; {REMEDY}"))?.to_string();
        let mut rows = BTreeMap::new();
        for (at, line) in lines.enumerate() {
            let bad = |what: &str| format!("the rule-call table line {} is not a row ({what}); {REMEDY}", at + 2);
            let row: Value = serde_json::from_str(line).map_err(|why| bad(&why.to_string()))?;
            let rules: Vec<usize> = row["rules"].as_array().ok_or_else(|| bad("rules"))?.iter().filter_map(|n| n.as_u64().and_then(|n| usize::try_from(n).ok())).collect();
            let goal = match (row["goal"].as_str(), rules.as_slice()) {
                (Some("violate"), [at]) => Goal::Violate(*at),
                (Some("satisfy"), [at]) => Goal::Satisfy(*at),
                (Some("both"), [a, b]) => Goal::Both(*a, *b),
                _ => return Err(bad("goal")),
            };
            let source = match row["source"].as_str() {
                Some("doc") => Source::Doc,
                Some("story") => Source::Story,
                _ => return Err(bad("source")),
            };
            let found = match &row["found"] {
                Value::Null => None,
                found => {
                    let assignment = serde_json::from_value(found["assignment"].clone()).map_err(|why| bad(&why.to_string()))?;
                    let also_breaks = serde_json::from_value(found["alsoBreaks"].clone()).map_err(|why| bad(&why.to_string()))?;
                    Some(Found { assignment, also_breaks })
                }
            };
            let key = Key { command: row["command"].as_str().ok_or_else(|| bad("command"))?.to_string(), source, goal };
            let digest = row["digest"].as_str().ok_or_else(|| bad("digest"))?.to_string();
            rows.insert(key, Row { digest, found });
        }
        Ok(Table { atip, rows })
    }

    /// The finding stored for `goal` of `doc` searched from `example`, or why the table cannot answer: no row, or a row made from other
    /// inputs. It never searches.
    pub fn found(&self, doc: &Value, example: &[String], source: Source, goal: Goal) -> Result<Option<Found>, String> {
        let command = doc["command"].as_str().unwrap_or_default().to_string();
        let key = Key { command, source, goal };
        let Some(row) = self.rows.get(&key) else { return Err(format!("the rule-call table has no row for {}; {REMEDY}", key.describe())) };
        let rules = constraints(doc);
        let digest = digest_of(doc, &rules, example, &self.atip);
        if row.digest != digest {
            return Err(format!("the rule-call table row for {} was made from other inputs (digest {} now {digest}); {REMEDY}", key.describe(), row.digest));
        }
        Ok(row.found.clone())
    }
}

/// The stored table of the repository.
pub fn table() -> &'static Table {
    static TABLE: LazyLock<Result<Table, String>> = LazyLock::new(|| {
        let text = std::fs::read_to_string(path()).map_err(|why| format!("cannot read {}: {why}; {REMEDY}", path().display()))?;
        let table = Table::parse(&text)?;
        let lock = lock_atip_rev();
        if table.atip != lock {
            return Err(format!("the rule-call table was made with atip `{}` and Cargo.lock names `{lock}`; {REMEDY}", table.atip));
        }
        Ok(table)
    });
    TABLE.as_ref().unwrap_or_else(|why| panic!("{why}"))
}

/// The call stored for `goal`, read from the repository's table; a failure of the table fails the test and names the remedy.
pub fn read(doc: &Value, example: &[String], source: Source, goal: Goal) -> Option<Found> {
    table().found(doc, example, source, goal).unwrap_or_else(|why| panic!("{why}"))
}

// ------------------------------------------------------------------------------------------------ judging

fn set(items: impl IntoIterator<Item = usize>) -> Vec<usize> {
    items.into_iter().collect::<BTreeSet<_>>().into_iter().collect()
}

/// What is wrong with a stored call: it must break what its goal says, and the other rules it breaks are the ones it records.
fn judge_call(doc: &Value, rules: &[ConstraintDoc], key: &Key, found: &Found, out: &mut Vec<String>) {
    let given = given_from(doc, &found.assignment);
    let broken = set(atip::constraint::violations(rules, &given).into_iter().map(|(at, _)| at));
    let (target, wanted): (Vec<usize>, bool) = match key.goal {
        Goal::Violate(at) => (vec![at], true),
        Goal::Both(a, b) => (vec![a, b], true),
        Goal::Satisfy(_) => (Vec::new(), false),
    };
    let tag = key.describe();
    if wanted {
        if !target.iter().all(|at| broken.contains(at)) {
            out.push(format!("{tag}: the stored call does not break {target:?} now (it breaks {broken:?})"));
            return;
        }
    } else if !broken.is_empty() || !matches!(key.goal, Goal::Satisfy(at) if exercised(&rules[at], &given)) {
        out.push(format!("{tag}: the stored call should break no rule and exercise its rule; it breaks {broken:?}"));
        return;
    }
    let others = set(broken.iter().copied().filter(|at| !target.contains(at)));
    if others != set(found.also_breaks.iter().copied()) {
        out.push(format!("{tag}: the stored call also breaks {others:?} now; the table records {:?}", found.also_breaks));
    }
}

/// What is stale in `text` against the documents, the story's examples, the searcher and `atip`: empty when the table is current. A
/// non-empty answer starts with the remedy.
pub fn problems(text: &str, world: &World) -> Vec<String> {
    let table = match Table::parse(text) {
        Ok(table) => table,
        Err(why) => return vec![why],
    };
    let mut out = Vec::new();
    if table.atip != world.atip {
        out.push(format!("the table was made with atip `{}` and Cargo.lock names `{}`", table.atip, world.atip));
    }
    let mut wanted: BTreeSet<Key> = BTreeSet::new();
    for doc in world.docs {
        let rules = constraints(doc);
        if rules.is_empty() {
            continue;
        }
        let command = doc["command"].as_str().unwrap_or_default().to_string();
        for source in [Source::Doc, Source::Story] {
            let example = example_of(doc, source, world);
            let digest = digest_of(doc, &rules, &example, world.atip);
            let mut visit = |goal: Goal, out: &mut Vec<String>| -> Option<Found> {
                let key = Key { command: command.clone(), source, goal };
                wanted.insert(key.clone());
                let Some(row) = table.rows.get(&key) else {
                    out.push(format!("{}: no row", key.describe()));
                    return None;
                };
                if row.digest != digest {
                    out.push(format!("{}: made from other inputs (digest {} now {digest})", key.describe(), row.digest));
                    return None;
                }
                match &row.found {
                    Some(found) => judge_call(doc, &rules, &key, found, out),
                    None if matches!(goal, Goal::Both(..)) => {}
                    None => out.push(format!("{}: has no call; only a pair of rules may have none", key.describe())),
                }
                row.found.clone()
            };
            for at in 0..rules.len() {
                visit(Goal::Violate(at), &mut out);
                if source == Source::Story {
                    visit(Goal::Satisfy(at), &mut out);
                }
            }
            if source == Source::Doc {
                walk_pairs(&rules, |first, second| visit(Goal::Both(first, second), &mut out));
            }
        }
    }
    for key in table.rows.keys().filter(|key| !wanted.contains(*key)) {
        out.push(format!("{}: no document declares this row", key.describe()));
    }
    if !out.is_empty() {
        out.insert(0, format!("the stored rule calls are stale: {REMEDY}"));
    }
    out
}

// ------------------------------------------------------------------------------------------------ making

/// Runs `jobs` on six threads; the answers come back in the order of the jobs.
fn parallel<T: Sync, R: Send>(jobs: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = Mutex::new(0usize);
    let done: Mutex<Vec<Option<R>>> = Mutex::new(jobs.iter().map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let at = {
                    let mut next = next.lock().expect("the counter");
                    let at = *next;
                    *next += 1;
                    at
                };
                let Some(job) = jobs.get(at) else { return };
                let answer = work(job);
                done.lock().expect("the answers")[at] = Some(answer);
            });
        }
    });
    done.into_inner().expect("the answers").into_iter().map(|answer| answer.expect("every job answered")).collect()
}

/// The table made by searching: every rule of every command with rules, from both sources.
pub fn make(world: &World) -> String {
    enum Job<'a> {
        One(&'a Value, Source, Goal),
        Pairs(&'a Value),
    }
    let docs: Vec<&Value> = world.docs.iter().filter(|doc| !constraints(doc).is_empty()).collect();
    let mut jobs = Vec::new();
    for doc in &docs {
        for at in 0..constraints(doc).len() {
            jobs.push(Job::One(doc, Source::Doc, Goal::Violate(at)));
            jobs.push(Job::One(doc, Source::Story, Goal::Violate(at)));
            jobs.push(Job::One(doc, Source::Story, Goal::Satisfy(at)));
        }
        jobs.push(Job::Pairs(doc));
    }
    let answers = parallel(&jobs, |job| {
        let finish = |doc: &Value, source: Source, goal: Goal, found: Option<Found>| {
            let rules = constraints(doc);
            let example = example_of(doc, source, world);
            let digest = digest_of(doc, &rules, &example, world.atip);
            (Key { command: doc["command"].as_str().unwrap_or_default().to_string(), source, goal }, Row { digest, found })
        };
        match job {
            Job::One(doc, source, goal) => {
                let found = solve(doc, &example_of(doc, *source, world), &constraints(doc), *goal);
                vec![finish(doc, *source, *goal, found)]
            }
            Job::Pairs(doc) => {
                let rules = constraints(doc);
                let example = doc_example(doc);
                walk_pairs(&rules, |a, b| solve(doc, &example, &rules, Goal::Both(a, b))).into_iter().map(|((a, b), found)| finish(doc, Source::Doc, Goal::Both(a, b), found)).collect()
            }
        }
    });
    let rows: BTreeMap<Key, Row> = answers.into_iter().flatten().collect();
    render(world.atip, &rows)
}
