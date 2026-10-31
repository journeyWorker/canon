//! `canon search` and `canon trace` (0.31 `canon-0-31-search`): full-text search over the records, the
//! briefs and the scenario text, joined to the graph the records form, and the review loop of a finding.
//!
//! Both read a derived index, `.canon/index/search.db` ([`index`]), which they bring up to date first: it
//! is gitignored, never a source of records, and deleting it changes nothing but `data.index.rebuilt`. The
//! inputs are in [`inputs`], the graph in [`graph`], the chain in [`trace`].
//!
//! # Exits
//! `0` for a search, found or not, and for a chain. `2` for an empty query, an operand `canon trace` cannot
//! read as one of its three shapes ([`trace::SHAPES`]) or no record names, and a canon.yaml that cannot
//! be read. `1` when the records cannot be read, or the index cannot be built even in memory.

pub mod graph;
pub mod index;
pub mod inputs;
pub mod trace;

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use canon_model::RecordKind;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::top::{SearchArgs, TraceArgs};
use crate::cli_error::{CliError, CliErrorExt as _};
use crate::json_out::Report;
use index::{snippet, Index, IndexError, IndexInfo, Seed};
use inputs::{Corpus, InputError};

/// The node kinds: every record kind `canon query` reads, and `brief` (a section or a flow step).
pub fn kind_names() -> Vec<&'static str> {
    RecordKind::ALL.iter().map(|kind| kind.as_str()).chain(std::iter::once("brief")).collect()
}

/// One node a search found.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    /// `<kind>:<natural key>`.
    pub id: String,
    pub kind: String,
    /// 1 for the best match.
    pub rank: usize,
    /// The full-text score, larger for a better match; 0 for a query shorter than three characters.
    pub score: f64,
    pub title: String,
    /// The text around the match.
    pub snippet: String,
    /// Always false: a placeholder has no text, so no query finds it.
    pub placeholder: bool,
}

/// A node next to a hit.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchNeighbor {
    pub id: String,
    pub kind: String,
    /// Edges between the nearest hit and this node.
    pub hops: usize,
    /// The rel of the edge that reached it.
    pub via: String,
    /// The node that edge came from.
    pub from: String,
    pub title: String,
    /// True for a node standing for a reference no committed source holds.
    pub placeholder: bool,
    /// For a placeholder, how many references name it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refs: Option<usize>,
}

/// An edge between two nodes of the result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchEdge {
    pub from: String,
    pub to: String,
    /// The reference field, or `re-review`, `fix-reviewed`, `fix-of-fix`, `evidence-at-commit`, `flow`.
    pub rel: String,
}

/// How many of each a search found.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchCounts {
    /// Nodes that matched the query, before `--limit`.
    pub matched: usize,
    pub hits: usize,
    pub neighbors: usize,
    pub edges: usize,
}

/// `canon search --json`'s report (`SearchReport`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchReport {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub neighbors: Vec<SearchNeighbor>,
    pub edges: Vec<SearchEdge>,
    pub counts: SearchCounts,
    pub index: IndexInfo,
}

impl Report for SearchReport {}

/// How many items of each kind a chain holds.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraceCounts {
    pub items: usize,
    /// Of those, the archive's versions.
    pub archived: usize,
}

/// `canon trace --json`'s report (`TraceReport`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraceReport {
    pub operand: String,
    pub resolved_as: trace::ResolvedAs,
    /// The chain, oldest first (ties by id).
    pub chain: Vec<trace::TraceItem>,
    pub counts: TraceCounts,
    pub index: IndexInfo,
}

impl Report for TraceReport {}

/// A leaf's `.gitignore` gap: the index directory is kept out of git by its own ignore file until `canon
/// init` puts the line in the repo's.
fn gitignore_warning(repo: &std::path::Path) -> Option<String> {
    crate::init::index_ignore_gap(repo)
}

/// Read the inputs and bring the index up to date; the failure is the exit code already emitted.
fn open_index(command: &str, repo: &std::path::Path, json: bool) -> Result<Index, ExitCode> {
    if let Some(gap) = gitignore_warning(repo) {
        crate::json_out::warn("SEARCH_GITIGNORE_MISSING", format!("{command}: {gap}"));
    }
    let corpus: Corpus = match inputs::load(repo) {
        Ok(corpus) => corpus,
        Err(InputError::Config(why)) => {
            return Err(ExitCode::from(CliError::refused("CONFIG_UNREADABLE", why).hint("fix canon.yaml so it parses and names usable spec roots (`canon init --check` reports what is wrong), then run the command again").emit(command, json)));
        }
        Err(InputError::Unreadable(why)) => {
            return Err(ExitCode::from(CliError::failed("SEARCH_INDEX_FAILED", format!("the records the index is built from cannot be read: {why}")).hint("fix or restore what the message names, then run the command again; `canon gate check` shows the state of the ledger").emit(command, json)));
        }
    };
    let mut warnings = Vec::new();
    let opened = index::open(repo, &corpus, &mut warnings);
    for (code, warning) in warnings {
        crate::json_out::warn(code, format!("{command}: {warning}"));
    }
    if let Some(why) = &corpus.references_unchecked {
        crate::json_out::warn("REFERENCES_UNCHECKED", format!("{command}: the references of the records were not checked, so the index holds no placeholder node ({why})"));
    }
    opened.map_err(|error| ExitCode::from(CliError::failed("SEARCH_INDEX_FAILED", error.to_string()).hint("delete `.canon/index` and run the command again; the index is rebuilt from the records").emit(command, json)))
}

fn index_failed(command: &str, json: bool, error: &IndexError) -> ExitCode {
    ExitCode::from(CliError::failed("SEARCH_INDEX_FAILED", error.to_string()).hint("delete `.canon/index` and run the command again; the index is rebuilt from the records").emit(command, json))
}

/// `canon search "<text>"`.
pub(crate) fn run_search(args: SearchArgs) -> ExitCode {
    let SearchArgs { text, kind, expand, limit, repo, json } = args;
    let command = "canon search";
    let query = text.trim();
    if query.is_empty() {
        let error = CliError::refused("SEARCH_QUERY_EMPTY", "the search text is empty or only spaces").field("text").hint("pass the text to find, e.g. `canon search \"timeout\"`");
        return ExitCode::from(error.emit("canon search", json));
    }
    let index = match open_index(command, &repo, json) {
        Ok(index) => index,
        Err(code) => return code,
    };
    let kinds: Vec<String> = kind.into_iter().collect::<BTreeSet<_>>().into_iter().collect();
    let found = index.seeds(query, &kinds, limit as usize).and_then(|(seeds, matched)| expand_from(&index, &seeds, expand as usize).map(|(neighbors, edges)| (seeds, matched, neighbors, edges)));
    let (seeds, matched, neighbors, edges) = match found {
        Ok(found) => found,
        Err(error) => return index_failed(command, json, &error),
    };
    let hits: Vec<SearchHit> = seeds.iter().enumerate().map(|(n, seed)| SearchHit { id: seed.id.clone(), kind: seed.kind.clone(), rank: n + 1, score: (seed.score * 1e6).round() / 1e6, title: seed.title.clone(), snippet: snippet(query, &seed.title, &seed.body), placeholder: false }).collect();
    let counts = SearchCounts { matched, hits: hits.len(), neighbors: neighbors.len(), edges: edges.len() };
    let report = SearchReport { query: query.to_string(), hits, neighbors, edges, counts, index: index.info.clone() };
    if json {
        crate::json_out::data("search", &report);
    } else {
        print!("{}", format_search(&report));
    }
    ExitCode::SUCCESS
}

/// The neighbors of `seeds` out to `hops` edges, and the edges walked.
fn expand_from(index: &Index, seeds: &[Seed], hops: usize) -> Result<(Vec<SearchNeighbor>, Vec<SearchEdge>), IndexError> {
    let mut seen: BTreeMap<String, (usize, String, String)> = seeds.iter().map(|s| (s.id.clone(), (0, String::new(), String::new()))).collect();
    let mut edges: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut frontier: Vec<String> = seeds.iter().map(|s| s.id.clone()).collect();
    for hop in 1..=hops {
        let mut next = Vec::new();
        for id in &frontier {
            for edge in index.incident(id)? {
                edges.insert((edge.from.clone(), edge.to.clone(), edge.rel.clone()));
                if !seen.contains_key(&edge.other) {
                    seen.insert(edge.other.clone(), (hop, edge.rel.clone(), id.clone()));
                    next.push(edge.other);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    let mut neighbors = Vec::new();
    for (id, (hops, via, from)) in seen.into_iter().filter(|(_, (hops, _, _))| *hops > 0) {
        let Some(node) = index.node(&id)? else { continue };
        neighbors.push(SearchNeighbor { id, kind: node.kind, hops, via, from, title: node.title, placeholder: node.placeholder, refs: node.placeholder.then_some(node.refs) });
    }
    neighbors.sort_by(|a, b| (a.hops, &a.id).cmp(&(b.hops, &b.id)));
    let edges = edges.into_iter().map(|(from, to, rel)| SearchEdge { from, to, rel }).collect();
    Ok((neighbors, edges))
}

fn format_search(report: &SearchReport) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "canon search \"{}\": {} hit(s) of {} match(es), {} neighbor(s), {} edge(s)", report.query, report.counts.hits, report.counts.matched, report.counts.neighbors, report.counts.edges);
    let _ = writeln!(out, "index {}: {} nodes, {} edges, {}{}", report.index.path, report.index.nodes, report.index.edges, if report.index.rebuilt { "rebuilt" } else { "reused" }, if report.index.unread_stores.is_empty() { String::new() } else { format!("; not read: {}", report.index.unread_stores.join(", ")) });
    for hit in &report.hits {
        let _ = writeln!(out, "\n{:>3}. {}  [{}]  score {:.3}\n     {}\n     {}", hit.rank, hit.id, hit.kind, hit.score, hit.title, hit.snippet);
    }
    if !report.neighbors.is_empty() {
        let _ = writeln!(out, "\nneighbors:");
        for n in &report.neighbors {
            let _ = writeln!(out, "  {}  [{}]  {} hop(s), via {} from {}{}  {}", n.id, n.kind, n.hops, n.via, n.from, if n.placeholder { format!("  (no committed source, {} reference(s))", n.refs.unwrap_or(0)) } else { String::new() }, n.title);
        }
    }
    out
}

/// `canon trace <id>`.
pub(crate) fn run_trace(args: TraceArgs) -> ExitCode {
    let TraceArgs { id, repo, json } = args;
    let command = "canon trace";
    let index = match open_index(command, &repo, json) {
        Ok(index) => index,
        Err(code) => return code,
    };
    let chain = match trace::trace(&index, &id) {
        Ok(Ok(chain)) => chain,
        Ok(Err(refusal)) => {
            let shapes = trace::SHAPES;
            let error = match &refusal {
                trace::TraceError::Unrecognized => CliError::refused("TRACE_OPERAND_INVALID", format!("`{id}` is none of the three things canon trace takes; it takes {shapes}")).hint("pass a review id, a commit sha or a finding key"),
                trace::TraceError::NotFound => CliError::refused("TRACE_NOT_FOUND", format!("no record says `{id}`; it takes {shapes}")).hint("pass an id a record carries: `canon query --kind finding` shows finding keys, `canon query --kind review --json` review ids"),
                trace::TraceError::Ambiguous(shas) => CliError::refused("TRACE_NOT_FOUND", format!("`{id}` is the start of more than one commit ({}); it takes {shapes}", shas.join(", "))).hint("pass more characters of the sha"),
            };
            return ExitCode::from(error.field("id").emit("canon trace", json));
        }
        Err(error) => return index_failed(command, json, &error),
    };
    let archived = chain.items.iter().filter(|item| item.archived).count();
    let report = TraceReport { operand: id.trim().to_string(), resolved_as: chain.resolved_as, counts: TraceCounts { items: chain.items.len(), archived }, chain: chain.items, index: index.info.clone() };
    if json {
        crate::json_out::data("trace", &report);
    } else {
        print!("{}", format_trace(&report));
    }
    ExitCode::SUCCESS
}

fn format_trace(report: &TraceReport) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "canon trace {}: {} item(s) from the {} {}", report.operand, report.counts.items, serde_json::to_value(report.resolved_as).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(), report.operand);
    let _ = writeln!(out, "index {}: {}", report.index.path, if report.index.rebuilt { "rebuilt" } else { "reused" });
    for item in &report.chain {
        let sha = item.sha.as_deref().map_or("-".to_string(), |sha| sha.chars().take(10).collect());
        let _ = writeln!(out, "{}  {:<8} {:<8} {}  {sha}{}", item.at, item.kind, item.role, item.id, if item.archived { "  (archived)" } else { "" });
        for edge in &item.edges {
            let arrow = if edge.direction == "out" { "->" } else { "<-" };
            let _ = writeln!(out, "      {arrow} {}  {} ({})", edge.rel, edge.id, edge.role);
        }
    }
    out
}
