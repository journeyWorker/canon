//! The record graph the index stores (0.31 `canon-0-31-search`): nodes, edges, and the review chain.
//!
//! # Nodes
//! One per current record, `<kind>:<natural key>` with the key `canon query` shows in its ID column (a
//! review that shares its key with another current review is `<kind>:<key>@<digest>`, the digest being the
//! last part of its file name); one per brief section and flow step, `brief:<change>#<section or F-id>`;
//! one per scenario of the `.feature` files, `scenario:<project>_<scenario id>`; and one marked placeholder
//! per id that a finding, evidence or subject names and no committed source holds ([`crate::references`],
//! the same references `canon query` reports).
//!
//! # Edges
//! The explicit references ([`references_of`]), named by their field; the commit joins of the review loop
//! ([`chain_links`]); and `flow`, from a scenario to the brief step its `@flow:` tag names. Each edge is
//! stored once, in the direction its rule says; a search reads it both ways.
//!
//! # The chain
//! `canon trace` follows [`ChainLink`]s between [`Item`]s, one per record version, the archive's included.
//! A finding is two items, the version that recorded it (role `finding`, the commit it reviewed) and the
//! version that closed it `fixed` (role `fix`, the commit that fixed it).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use canon_model::RecordKind;
use serde_json::Value;

use super::inputs::{BriefNode, Corpus, FeatureScenario, Version};

/// A node of the graph.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub placeholder: bool,
    /// For a placeholder, how many references name it.
    pub refs: usize,
}

/// An edge of the graph, `rel` the reference field or rule that joins `from` to `to`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub rel: String,
}

/// One version of a finding, review or evidence record, as a trace sees it.
#[derive(Debug, Clone)]
pub struct Item {
    /// `<kind>:<natural key>`.
    pub id: String,
    pub kind: RecordKind,
    /// `finding`, `fix`, `review` or `evidence`.
    pub role: &'static str,
    /// The digest of the version: with `id`, what tells two versions of one record apart.
    pub version: String,
    pub at: String,
    /// The commit this item is about: the one a finding reviewed, the one a fix landed in, a review's pin,
    /// evidence's commit.
    pub sha: Option<String>,
    /// For a finding, the commit that introduced the defect.
    pub sha_alt: Option<String>,
    pub review_id: Option<String>,
    /// Whether this is a current version (not the archive's).
    pub current: bool,
    /// The node this item is a version of, when the graph holds one.
    pub node: Option<String>,
    pub body: Value,
}

/// A join of two items of the chain.
#[derive(Debug, Clone)]
pub struct ChainLink {
    /// Indexes into the items.
    pub from: usize,
    pub to: usize,
    pub rel: &'static str,
}

/// The nodes, edges, items and chain links of one corpus.
#[derive(Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub items: Vec<Item>,
    pub links: Vec<ChainLink>,
}

fn text(body: &Value, field: &str) -> Option<String> {
    body.get(field).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

fn strings(body: &Value, field: &str) -> Vec<String> {
    body.get(field).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
}

/// Every string of `value`, in document order, one per line: the text a record is searched by. The
/// envelope's bookkeeping (`schema`, `kind`, `at`, `staging_id`) is not text.
fn flatten(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) if !s.is_empty() => out.push(s.clone()),
        Value::Array(items) => {
            for item in items {
                flatten(item, out);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                if !matches!(key.as_str(), "schema" | "kind" | "at" | "staging_id") {
                    flatten(item, out);
                }
            }
        }
        _ => {}
    }
}

fn record_title(version: &Version) -> String {
    let body = &version.body;
    let pick = |field: &str| text(body, field);
    match version.kind {
        RecordKind::Finding => pick("summary"),
        RecordKind::Review => {
            let target = pick("scenario_id").or_else(|| pick("change_id")).unwrap_or_default();
            Some(format!("{} reviewed {target}", pick("reviewer").unwrap_or_default()).trim().to_string())
        }
        RecordKind::Evidence => {
            let subject = pick("scenario_id").or_else(|| pick("task_id")).or_else(|| pick("run_id")).unwrap_or_default();
            let class = body.get("evidence").and_then(|e| text(e, "kind")).unwrap_or_default();
            Some(format!("{subject} {class} {}", pick("verdict").unwrap_or_default()).split_whitespace().collect::<Vec<_>>().join(" "))
        }
        _ => pick("title").or_else(|| pick("summary")),
    }
    .filter(|title| !title.is_empty())
    .unwrap_or_else(|| version.key.clone())
}

fn record_body(version: &Version) -> String {
    let mut lines = Vec::new();
    flatten(&version.body, &mut lines);
    lines.join("\n")
}

/// What a reference field names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Target {
    Change,
    Subject,
    Task,
    Session,
    Run,
    Handoff,
    Scenario,
    Review,
}

/// One reference of a record: the field it is in, what it names, and the id.
struct Reference {
    field: &'static str,
    target: Target,
    id: String,
    /// The project a scenario reference is scoped to, when the record names one.
    project: Option<String>,
}

/// The reference fields of a record of `kind`: the join keys the typed records carry. Every record also
/// names the session that wrote it.
fn references_of(kind: RecordKind, body: &Value) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut one = |field: &'static str, target: Target, project: Option<String>| {
        if let Some(id) = text(body, field) {
            out.push(Reference { field, target, id, project });
        }
    };
    match kind {
        RecordKind::Run => {
            one("parent_run_id", Target::Run, None);
            one("session_id", Target::Session, None);
            one("task_id", Target::Task, None);
        }
        RecordKind::Event => one("run_id", Target::Run, None),
        RecordKind::Review => {
            one("scenario_id", Target::Scenario, text(body, "project_id"));
            one("change_id", Target::Change, None);
        }
        RecordKind::Divergence => one("scenario_id", Target::Scenario, text(body, "project_id")),
        RecordKind::Trajectory => {
            one("run_id", Target::Run, None);
            one("task_id", Target::Task, None);
            one("session_id", Target::Session, None);
        }
        RecordKind::StrategyItem => one("source_task_id", Target::Task, None),
        RecordKind::Evidence => {
            one("task_id", Target::Task, None);
            one("scenario_id", Target::Scenario, text(body, "project_id"));
            one("run_id", Target::Run, None);
        }
        RecordKind::Finding => {
            one("change_id", Target::Change, None);
            one("review_id", Target::Review, None);
        }
        RecordKind::Handoff => {
            one("parent_handoff_id", Target::Handoff, None);
            one("openspec_change_slug", Target::Change, None);
        }
        RecordKind::Scenario => one("subject_id", Target::Subject, None),
        RecordKind::Change | RecordKind::Subject | RecordKind::Task | RecordKind::Session => {}
    }
    let lists: &[(&'static str, Target)] = match kind {
        RecordKind::Change => &[("subject_ids", Target::Subject)],
        RecordKind::Subject => &[("change_ids", Target::Change)],
        RecordKind::Task => &[("scenario_refs", Target::Scenario), ("depends_on", Target::Task)],
        RecordKind::Finding => &[("scenario_ids", Target::Scenario)],
        _ => &[],
    };
    for (field, target) in lists {
        for id in strings(body, field) {
            out.push(Reference { field, target: *target, id, project: None });
        }
    }
    if let Some(id) = body.get("actor").and_then(|actor| text(actor, "session_id")) {
        out.push(Reference { field: "actor.session_id", target: Target::Session, id, project: None });
    }
    out
}

/// The field of a node's record that its natural key is read from, per kind that others name.
fn target_of(kind: RecordKind) -> Option<Target> {
    match kind {
        RecordKind::Change => Some(Target::Change),
        RecordKind::Subject => Some(Target::Subject),
        RecordKind::Task => Some(Target::Task),
        RecordKind::Session => Some(Target::Session),
        RecordKind::Run => Some(Target::Run),
        RecordKind::Handoff => Some(Target::Handoff),
        _ => None,
    }
}

/// The `scenario:` node of a scenario of a spec root.
fn scenario_node_id(project: &str, scenario_id: &str) -> String {
    format!("scenario:{project}_{scenario_id}")
}

/// Build the graph of `corpus`.
pub fn build(corpus: &Corpus) -> Graph {
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut edges: BTreeSet<Edge> = BTreeSet::new();

    // Current records. A natural key held by two current records (two reviews of one scenario at two
    // commits) names each by its digest too.
    let mut per_key: HashMap<(RecordKind, &str), usize> = HashMap::new();
    for version in corpus.versions.iter().filter(|v| v.current) {
        *per_key.entry((version.kind, version.key.as_str())).or_insert(0) += 1;
    }
    let node_id = |version: &Version| -> String {
        if per_key.get(&(version.kind, version.key.as_str())).copied().unwrap_or(0) > 1 {
            format!("{}:{}@{}", version.kind.as_str(), version.key, version.digest)
        } else {
            format!("{}:{}", version.kind.as_str(), version.key)
        }
    };
    let mut current: Vec<(String, &Version)> = Vec::new();
    for version in corpus.versions.iter().filter(|v| v.current) {
        let id = node_id(version);
        nodes.insert(id.clone(), Node { id: id.clone(), kind: version.kind.as_str().to_string(), title: record_title(version), body: record_body(version), placeholder: false, refs: 0 });
        current.push((id, version));
    }

    // Scenarios of the `.feature` files: a scenario's node is `scenario:<project>_<scenario id>`, the id of
    // its index record too, so the file's text replaces the record's and a scenario without a record is a
    // node of its own.
    let mut scenario_nodes: Vec<(String, &FeatureScenario)> = Vec::new();
    for scenario in &corpus.scenarios {
        let id = scenario_node_id(&scenario.project, &scenario.scenario_id);
        nodes.insert(id.clone(), Node { id: id.clone(), kind: "scenario".to_string(), title: scenario.title.clone(), body: scenario.text.clone(), placeholder: false, refs: 0 });
        scenario_nodes.push((id, scenario));
    }

    // Briefs.
    let brief_id = |brief: &BriefNode| format!("brief:{}#{}", brief.change, brief.name);
    for brief in &corpus.briefs {
        let id = brief_id(brief);
        nodes.insert(id.clone(), Node { id: id.clone(), kind: "brief".to_string(), title: brief.title.clone(), body: brief.text.clone(), placeholder: false, refs: 0 });
    }

    // Lookup tables for the reference fields.
    let mut by_key: HashMap<(Target, String), Vec<String>> = HashMap::new();
    let mut scenarios_by_id: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut reviews_by_id: HashMap<String, Vec<String>> = HashMap::new();
    for (id, version) in &current {
        if let Some(target) = target_of(version.kind) {
            by_key.entry((target, version.key.clone())).or_default().push(id.clone());
        }
        if version.kind == RecordKind::Review {
            if let Some(review_id) = text(&version.body, "review_id") {
                reviews_by_id.entry(review_id).or_default().push(id.clone());
            }
        }
    }
    for (id, scenario) in &scenario_nodes {
        scenarios_by_id.entry(scenario.scenario_id.clone()).or_default().push((scenario.project.clone(), id.clone()));
    }
    for (id, version) in current.iter().filter(|(_, v)| v.kind == RecordKind::Scenario) {
        if let (Some(project), Some(scenario_id)) = (text(&version.body, "project_id"), text(&version.body, "scenario_id")) {
            let entry = scenarios_by_id.entry(scenario_id).or_default();
            // The record's node and the file's node of one scenario are one scenario.
            if !entry.iter().any(|(p, _)| *p == project) {
                entry.push((project, id.clone()));
            }
        }
    }
    // Explicit references.
    for (id, version) in &current {
        if !nodes.contains_key(id) {
            continue;
        }
        for reference in references_of(version.kind, &version.body) {
            let targets: Vec<String> = match reference.target {
                Target::Scenario => scenarios_by_id.get(&reference.id).into_iter().flatten().filter(|(project, _)| reference.project.as_ref().is_none_or(|wanted| wanted == project)).map(|(_, node)| node.clone()).collect(),
                Target::Review => reviews_by_id.get(&reference.id).cloned().unwrap_or_default(),
                other => by_key.get(&(other, reference.id.clone())).cloned().unwrap_or_default(),
            };
            for to in targets {
                if to != *id && nodes.contains_key(&to) {
                    edges.insert(Edge { from: id.clone(), to, rel: reference.field.to_string() });
                }
            }
        }
    }

    // Flow tags.
    for (id, scenario) in &scenario_nodes {
        for tag in &scenario.flows {
            let Some((change, step)) = tag.rsplit_once('.') else { continue };
            let to = format!("brief:{change}#{step}");
            if nodes.contains_key(&to) {
                edges.insert(Edge { from: id.clone(), to, rel: "flow".to_string() });
            }
        }
    }

    // Placeholders: the references `canon query` reports as having no committed source.
    let mut placeholder_refs: BTreeMap<String, usize> = BTreeMap::new();
    for found in &corpus.dangling {
        let placeholder = format!("{}:{}", found.target, found.id);
        let from = current.iter().find(|(_, v)| v.kind == found.kind && v.key == found.key && v.digest == found.digest).map(|(id, _)| id.clone());
        let Some(from) = from else { continue };
        if !nodes.contains_key(&placeholder) || nodes[&placeholder].placeholder {
            *placeholder_refs.entry(placeholder.clone()).or_insert(0) += 1;
            nodes.entry(placeholder.clone()).or_insert_with(|| Node { id: placeholder.clone(), kind: found.target.to_string(), title: format!("{} {} (no committed source)", found.target, found.id), body: String::new(), placeholder: true, refs: 0 });
        }
        edges.insert(Edge { from, to: placeholder, rel: found.field.to_string() });
    }
    for (id, refs) in placeholder_refs {
        if let Some(node) = nodes.get_mut(&id) {
            node.refs = refs;
        }
    }

    // The review loop.
    let node_of: HashMap<(RecordKind, &str, &str), String> = current.iter().map(|(id, v)| ((v.kind, v.key.as_str(), v.digest.as_str()), id.clone())).collect();
    let items = items_of(&corpus.versions, &node_of);
    let links = chain_links(&items);
    for link in &links {
        if link.rel == "review_id" {
            continue;
        }
        let (from, to) = (&items[link.from], &items[link.to]);
        let counts = |item: &Item| item.kind == RecordKind::Finding || item.current;
        if let (Some(a), Some(b)) = (&from.node, &to.node) {
            if counts(from) && counts(to) && a != b {
                edges.insert(Edge { from: a.clone(), to: b.clone(), rel: link.rel.to_string() });
            }
        }
    }

    edges.retain(|edge| nodes.contains_key(&edge.from) && nodes.contains_key(&edge.to));
    Graph { nodes: nodes.into_values().collect(), edges: edges.into_iter().collect(), items, links }
}

/// The items of the review loop: every version of every finding, review and evidence record.
fn items_of(versions: &[Version], node_of: &HashMap<(RecordKind, &str, &str), String>) -> Vec<Item> {
    let mut items = Vec::new();
    let mut findings: BTreeMap<&str, Vec<&Version>> = BTreeMap::new();
    for version in versions {
        let id = format!("{}:{}", version.kind.as_str(), version.key);
        let node = node_of.get(&(version.kind, version.key.as_str(), version.digest.as_str())).cloned();
        let body = &version.body;
        match version.kind {
            RecordKind::Finding => findings.entry(version.key.as_str()).or_default().push(version),
            RecordKind::Review => items.push(Item { id, kind: version.kind, role: "review", version: version.digest.clone(), at: version.at.clone(), sha: text(body, "pin").filter(|pin| is_sha(pin)), sha_alt: None, review_id: text(body, "review_id"), current: version.current, node, body: body.clone() }),
            RecordKind::Evidence => items.push(Item { id, kind: version.kind, role: "evidence", version: version.digest.clone(), at: version.at.clone(), sha: text(body, "evidence_sha"), sha_alt: None, review_id: None, current: version.current, node, body: body.clone() }),
            _ => {}
        }
    }
    for (key, mut group) in findings {
        group.sort_by(|a, b| (&a.at, &a.digest).cmp(&(&b.at, &b.digest)));
        let id = format!("finding:{key}");
        let node = group.iter().find(|v| v.current).and_then(|v| node_of.get(&(v.kind, v.key.as_str(), v.digest.as_str())).cloned());
        let first = group[0];
        items.push(Item { id: id.clone(), kind: RecordKind::Finding, role: "finding", version: first.digest.clone(), at: first.at.clone(), sha: text(&first.body, "reviewed_sha"), sha_alt: text(&first.body, "introduced_by"), review_id: text(&first.body, "review_id"), current: first.current, node: node.clone(), body: first.body.clone() });
        if let Some(fixed) = group.iter().rev().find(|v| text(&v.body, "disposition").as_deref() == Some("fixed")) {
            items.push(Item { id, kind: RecordKind::Finding, role: "fix", version: fixed.digest.clone(), at: fixed.at.clone(), sha: text(&fixed.body, "resolution_sha"), sha_alt: text(&fixed.body, "introduced_by"), review_id: text(&fixed.body, "review_id"), current: fixed.current, node, body: fixed.body.clone() });
        }
    }
    items.sort_by(|a, b| (&a.at, &a.id, a.role, &a.version).cmp(&(&b.at, &b.id, b.role, &b.version)));
    items
}

fn is_sha(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A finding's `(round, seq)` and change: the order of its rounds.
fn round_of(item: &Item) -> Option<(String, u64, u64)> {
    let change = text(&item.body, "change_id")?;
    Some((change, item.body.get("round")?.as_u64()?, item.body.get("seq")?.as_u64()?))
}

/// Whether `review` covers the change or a scenario of the finding `item`.
fn covers(review: &Item, finding: &Item) -> bool {
    let same_change = text(&review.body, "change_id").is_some_and(|change| round_of(finding).is_some_and(|(c, _, _)| c == change));
    let scenario = text(&review.body, "scenario_id");
    same_change || scenario.is_some_and(|id| strings(&finding.body, "scenario_ids").contains(&id))
}

/// Whether the evidence `item` is about a scenario that `other`, a finding or a review, points at.
fn about_same_scenario(evidence: &Item, other: &Item) -> bool {
    let Some(scenario) = text(&evidence.body, "scenario_id") else { return false };
    let project = text(&evidence.body, "project_id");
    match other.kind {
        RecordKind::Finding => strings(&other.body, "scenario_ids").contains(&scenario),
        _ => text(&other.body, "scenario_id").is_some_and(|id| id == scenario) && project.is_none_or(|p| text(&other.body, "project_id").is_none_or(|q| p == q)),
    }
}

/// The joins of the review loop between `items`.
///
/// - `review_id`: a finding's review id to the reviews that carry it and cover its change or a scenario of
///   it; when none does, every review of the run;
/// - `re-review`: the commit a finding's fix landed in is the commit a later finding of the change reviewed;
/// - `fix-reviewed`: a review pinned at a finding's fix commit, covering its change or a scenario of it;
/// - `fix-of-fix`: the commit a finding's fix landed in is the commit that introduced a later finding's defect;
/// - `evidence-at-commit`: evidence at a commit a finding reviewed or fixed in, or a review pinned at, when
///   that finding or review points at the evidence's scenario.
pub fn chain_links(items: &[Item]) -> Vec<ChainLink> {
    let mut links: BTreeSet<(usize, usize, &'static str)> = BTreeSet::new();
    let by_role = |role: &str| -> Vec<usize> { items.iter().enumerate().filter(|(_, i)| i.role == role).map(|(n, _)| n).collect() };
    let (findings, fixes, reviews, evidence) = (by_role("finding"), by_role("fix"), by_role("review"), by_role("evidence"));

    for &f in &findings {
        let Some(run) = &items[f].review_id else { continue };
        let carrying: Vec<usize> = reviews.iter().copied().filter(|&r| items[r].review_id.as_ref() == Some(run)).collect();
        let covering: Vec<usize> = carrying.iter().copied().filter(|&r| covers(&items[r], &items[f])).collect();
        for r in if covering.is_empty() { carrying } else { covering } {
            links.insert((f, r, "review_id"));
        }
    }
    for &x in &fixes {
        let Some(sha) = &items[x].sha else { continue };
        let Some(order) = round_of(&items[x]).map(|(c, round, seq)| (c, (round, seq))) else { continue };
        for &f in &findings {
            let Some((change, round, seq)) = round_of(&items[f]) else { continue };
            if change != order.0 || (round, seq) <= order.1 {
                continue;
            }
            if items[f].sha.as_ref() == Some(sha) {
                links.insert((x, f, "re-review"));
            }
            if items[f].sha_alt.as_ref() == Some(sha) {
                links.insert((x, f, "fix-of-fix"));
            }
        }
        for &r in &reviews {
            if items[r].sha.as_ref() == Some(sha) && covers(&items[r], &items[x]) {
                links.insert((x, r, "fix-reviewed"));
            }
        }
    }
    for &e in &evidence {
        let Some(sha) = &items[e].sha else { continue };
        for &other in findings.iter().chain(&fixes).chain(&reviews) {
            if items[other].sha.as_ref() == Some(sha) && about_same_scenario(&items[e], &items[other]) {
                links.insert((e, other, "evidence-at-commit"));
            }
        }
    }
    links.into_iter().map(|(from, to, rel)| ChainLink { from, to, rel }).collect()
}
