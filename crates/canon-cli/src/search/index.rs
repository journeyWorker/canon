//! The SQLite file behind `canon search` and `canon trace` (0.31 `canon-0-31-search`).
//!
//! # A derived file
//! `.canon/index/search.db` is rebuilt from the inputs ([`super::inputs`]) and never read back as a source
//! of records. Every run reads the inputs, hashes them, and compares the hash with the one the file
//! holds: any difference makes the file stale, and the run builds a whole new file before it reads
//! anything. The new file is written beside the old one and renamed over it, so a reader never sees a
//! half-written index and runs that start at the same time each leave a complete file.
//!
//! The first build also writes `.canon/index/.gitignore`, holding only `*`, so the file (and the `-wal`
//! and `-shm` files SQLite may add) is never committed before `canon init` has put the directory in the
//! repo's own `.gitignore`.
//!
//! # Failure paths
//! A file that is not a database is deleted and rebuilt once, with a warning. A directory that cannot be
//! written makes the run build the index in memory, with a warning naming the path.
//!
//! # Tables
//! `node` (one row per node, `fts` its full-text index over title and body with the `trigram` tokenizer,
//! which finds any run of three or more characters in any language), `edge`, `item` and `link` (the review
//! chain, [`super::graph`]), and `meta`.

use std::path::Path;

use canon_model::paths;
use rusqlite::{params, Connection, OpenFlags};
use schemars::JsonSchema;
use serde::Serialize;

use super::graph::Graph;
use super::inputs::Corpus;

/// The index could not be built or read.
#[derive(Debug)]
pub struct IndexError(pub String);

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<rusqlite::Error> for IndexError {
    fn from(error: rusqlite::Error) -> Self {
        IndexError(format!("the index database failed: {error}"))
    }
}

/// What a report says about the index.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    /// The index file, relative to the repo.
    pub path: String,
    /// Nodes in the graph, placeholders included.
    pub nodes: usize,
    pub edges: usize,
    /// Whether this run built the index (it was missing, stale or corrupt, or lives in memory).
    pub rebuilt: bool,
    /// The backends of the stores no record was read from (`postgres`, `s3`).
    pub unread_stores: Vec<String>,
    /// Whether the index lives in memory for this run because the index directory cannot be written.
    pub in_memory: bool,
}

const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE node (id TEXT PRIMARY KEY, kind TEXT NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL, placeholder INTEGER NOT NULL, refs INTEGER NOT NULL);
CREATE VIRTUAL TABLE fts USING fts5(title, body, tokenize='trigram');
CREATE TABLE edge (src TEXT NOT NULL, dst TEXT NOT NULL, rel TEXT NOT NULL, PRIMARY KEY (src, dst, rel)) WITHOUT ROWID;
CREATE INDEX edge_dst ON edge (dst);
CREATE TABLE item (idx INTEGER PRIMARY KEY, id TEXT NOT NULL, kind TEXT NOT NULL, role TEXT NOT NULL, version TEXT NOT NULL, at TEXT NOT NULL, sha TEXT, sha_alt TEXT, review_id TEXT, current INTEGER NOT NULL);
CREATE TABLE link (src INTEGER NOT NULL, dst INTEGER NOT NULL, rel TEXT NOT NULL, PRIMARY KEY (src, dst, rel)) WITHOUT ROWID;
";

/// An open index.
#[derive(Debug)]
pub struct Index {
    conn: Connection,
    pub info: IndexInfo,
}

/// How an existing index file stands against the inputs.
enum Existing {
    Fresh(Connection, usize, usize),
    Missing,
    Stale,
    Broken(String),
}

/// Open the index for `corpus`: reuse the file when its fingerprint is the corpus's, else build it.
/// Warnings (a corrupt file, an unwritable directory) are appended to `warnings` as `(code, message)`.
pub fn open(repo: &Path, corpus: &Corpus, warnings: &mut Vec<(&'static str, String)>) -> Result<Index, IndexError> {
    let dir = repo.join(paths::INDEX_DIR);
    let file = repo.join(paths::INDEX_FILE);
    let unread_stores = corpus.unread.clone();
    let info = |nodes, edges, rebuilt, in_memory| IndexInfo { path: paths::INDEX_FILE.to_string(), nodes, edges, rebuilt, unread_stores: unread_stores.clone(), in_memory };

    // The directory and its ignore file come first, so the file is never untracked-and-visible. A failure
    // matters only when the file has to be built.
    let unwritable = prepare_dir(&dir).err();

    let mut broken = None;
    match existing(&file, &corpus.fingerprint) {
        Existing::Fresh(conn, nodes, edges) => return Ok(Index { conn, info: info(nodes, edges, false, false) }),
        Existing::Broken(reason) => broken = Some(reason),
        Existing::Missing | Existing::Stale => {}
    }

    let graph = super::graph::build(corpus);
    let (nodes, edges) = (graph.nodes.len(), graph.edges.len());
    if let Some(reason) = broken {
        match std::fs::remove_file(&file) {
            Ok(()) => warnings.push(("SEARCH_INDEX_REBUILT", format!("the index file `{}` is not a usable database ({reason}); deleted it and rebuilt it", paths::INDEX_FILE))),
            Err(error) => return in_memory(&graph, corpus, warnings, &format!("the index file `{}` is not a usable database ({reason}) and cannot be deleted ({error})", paths::INDEX_FILE), info(nodes, edges, true, true)),
        }
    }
    let written = match unwritable {
        Some(error) => Err(error),
        None => write_file(&dir, &file, &graph, corpus),
    };
    match written {
        Ok(conn) => Ok(Index { conn, info: info(nodes, edges, true, false) }),
        Err(error) => in_memory(&graph, corpus, warnings, &format!("the index directory `{}` cannot be written ({error})", paths::INDEX_DIR), info(nodes, edges, true, true)),
    }
}

fn in_memory(graph: &Graph, corpus: &Corpus, warnings: &mut Vec<(&'static str, String)>, why: &str, info: IndexInfo) -> Result<Index, IndexError> {
    let conn = Connection::open_in_memory()?;
    populate(&conn, graph, corpus)?;
    warnings.push(("SEARCH_INDEX_IN_MEMORY", format!("{why}; built the index in memory for this run")));
    Ok(Index { conn, info })
}

/// Create `dir` and its self-ignoring `.gitignore` (a `*` and nothing else).
fn prepare_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let ignore = dir.join(".gitignore");
    if std::fs::read_to_string(&ignore).ok().as_deref() != Some("*\n") {
        std::fs::write(&ignore, "*\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn existing(file: &Path, fingerprint: &str) -> Existing {
    match std::fs::symlink_metadata(file) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Existing::Missing,
        Err(e) => return Existing::Broken(e.to_string()),
    }
    let read = || -> rusqlite::Result<Option<(Connection, usize, usize)>> {
        let conn = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        let meta = |key: &str| conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| row.get::<_, String>(0));
        if meta("fingerprint")? != fingerprint {
            return Ok(None);
        }
        let count = |key: &str| meta(key).map(|v| v.parse::<usize>().unwrap_or(0));
        let (nodes, edges) = (count("nodes")?, count("edges")?);
        Ok(Some((conn, nodes, edges)))
    };
    match read() {
        Ok(Some((conn, nodes, edges))) => Existing::Fresh(conn, nodes, edges),
        Ok(None) => Existing::Stale,
        Err(error) => Existing::Broken(error.to_string()),
    }
}

/// Build the index beside `file`, then rename it over `file` and open it for reading.
fn write_file(dir: &Path, file: &Path, graph: &Graph, corpus: &Corpus) -> Result<Connection, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let temp = dir.join(format!("search.db.{}.{nanos}.tmp", std::process::id()));
    let built = (|| -> Result<(), String> {
        let conn = Connection::open(&temp).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;").map_err(|e| e.to_string())?;
        populate(&conn, graph, corpus).map_err(|e| e.to_string())?;
        conn.close().map_err(|(_, e)| e.to_string())?;
        std::fs::rename(&temp, file).map_err(|e| e.to_string())
    })();
    if let Err(error) = built {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(|e| e.to_string())
}

/// Fill an empty database with the graph.
fn populate(conn: &Connection, graph: &Graph, corpus: &Corpus) -> Result<(), IndexError> {
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch("BEGIN")?;
    let mut node = conn.prepare("INSERT INTO node (id, kind, title, body, placeholder, refs) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    let mut fts = conn.prepare("INSERT INTO fts (rowid, title, body) VALUES (?1, ?2, ?3)")?;
    for n in &graph.nodes {
        node.execute(params![n.id, n.kind, n.title, n.body, n.placeholder, n.refs])?;
        if !n.placeholder || !n.body.is_empty() {
            fts.execute(params![conn.last_insert_rowid(), n.title, n.body])?;
        }
    }
    let mut edge = conn.prepare("INSERT INTO edge (src, dst, rel) VALUES (?1, ?2, ?3)")?;
    for e in &graph.edges {
        edge.execute(params![e.from, e.to, e.rel])?;
    }
    let mut item = conn.prepare("INSERT INTO item (idx, id, kind, role, version, at, sha, sha_alt, review_id, current) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")?;
    for (idx, i) in graph.items.iter().enumerate() {
        item.execute(params![idx, i.id, i.kind.as_str(), i.role, i.version, i.at, i.sha, i.sha_alt, i.review_id, i.current])?;
    }
    let mut link = conn.prepare("INSERT INTO link (src, dst, rel) VALUES (?1, ?2, ?3)")?;
    for l in &graph.links {
        link.execute(params![l.from, l.to, l.rel])?;
    }
    let mut meta = conn.prepare("INSERT INTO meta (key, value) VALUES (?1, ?2)")?;
    meta.execute(params!["fingerprint", corpus.fingerprint])?;
    meta.execute(params!["nodes", graph.nodes.len().to_string()])?;
    meta.execute(params!["edges", graph.edges.len().to_string()])?;
    conn.execute_batch("COMMIT")?;
    Ok(())
}

/// A node a search found by its text.
#[derive(Debug, Clone)]
pub struct Seed {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    /// The full-text score, larger for a better match; zero for a substring match.
    pub score: f64,
}

/// A node as a neighbor or an edge end shows it.
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub kind: String,
    pub title: String,
    pub placeholder: bool,
    pub refs: usize,
}

/// One edge incident to a node, seen from that node.
#[derive(Debug, Clone)]
pub struct Incident {
    pub other: String,
    pub rel: String,
    /// The edge as stored.
    pub from: String,
    pub to: String,
}

/// One item of the review chain as the index stores it.
#[derive(Debug, Clone)]
pub struct StoredItem {
    pub idx: usize,
    pub id: String,
    pub kind: String,
    pub role: String,
    pub version: String,
    pub at: String,
    pub sha: Option<String>,
    pub sha_alt: Option<String>,
    pub review_id: Option<String>,
    pub current: bool,
}

impl Index {
    /// The nodes whose text holds `query`, best first, at most `limit`, and how many held it in all. A
    /// query of three or more characters is matched by the trigram index; a shorter one as a substring of
    /// a node's title or body, in id order. Placeholders have no text and never match.
    pub fn seeds(&self, query: &str, kinds: &[String], limit: usize) -> Result<(Vec<Seed>, usize), IndexError> {
        let kind_filter = if kinds.is_empty() { String::new() } else { format!(" AND n.kind IN ({})", kinds.iter().map(|k| format!("'{}'", k.replace('\'', "''"))).collect::<Vec<_>>().join(", ")) };
        if query.chars().count() >= 3 {
            // The query is one phrase of literal text: a quote inside it is doubled, nothing else is syntax.
            let phrase = format!("\"{}\"", query.replace('"', "\"\""));
            let matched: usize = self.conn.query_row(&format!("SELECT count(*) FROM fts JOIN node n ON n.rowid = fts.rowid WHERE fts MATCH ?1{kind_filter}"), [&phrase], |row| row.get(0))?;
            let mut statement = self.conn.prepare(&format!("SELECT n.id, n.kind, n.title, n.body, bm25(fts, 4.0, 1.0) AS score FROM fts JOIN node n ON n.rowid = fts.rowid WHERE fts MATCH ?1{kind_filter} ORDER BY score, n.id LIMIT ?2"))?;
            let seeds = statement
                .query_map(params![phrase, limit], |row| Ok(Seed { id: row.get(0)?, kind: row.get(1)?, title: row.get(2)?, body: row.get(3)?, score: -row.get::<_, f64>(4)? }))?
                .collect::<Result<Vec<_>, _>>()?;
            return Ok((seeds, matched));
        }
        let wanted = query.to_lowercase();
        // A placeholder node with text is a record no committed source backs; its text is searched like any record's.
        let mut statement = self.conn.prepare(&format!("SELECT n.id, n.kind, n.title, n.body FROM node n WHERE (n.placeholder = 0 OR n.body <> ''){kind_filter} ORDER BY n.id"))?;
        let mut seeds = Vec::new();
        let mut matched = 0;
        for row in statement.query_map([], |row| Ok(Seed { id: row.get(0)?, kind: row.get(1)?, title: row.get(2)?, body: row.get(3)?, score: 0.0 }))? {
            let seed = row?;
            if seed.title.to_lowercase().contains(&wanted) || seed.body.to_lowercase().contains(&wanted) {
                matched += 1;
                if seeds.len() < limit {
                    seeds.push(seed);
                }
            }
        }
        Ok((seeds, matched))
    }

    /// What the index holds of one node.
    pub fn node(&self, id: &str) -> Result<Option<NodeInfo>, IndexError> {
        match self.conn.query_row("SELECT kind, title, placeholder, refs FROM node WHERE id = ?1", [id], |row| Ok(NodeInfo { kind: row.get(0)?, title: row.get(1)?, placeholder: row.get::<_, i64>(2)? != 0, refs: row.get::<_, usize>(3)? })) {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// The edges incident to `id`, either way, by other end and rel.
    pub fn incident(&self, id: &str) -> Result<Vec<Incident>, IndexError> {
        let mut out = Vec::new();
        let mut outgoing = self.conn.prepare("SELECT dst, rel FROM edge WHERE src = ?1")?;
        for row in outgoing.query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))? {
            let (other, rel) = row?;
            out.push(Incident { from: id.to_string(), to: other.clone(), other, rel });
        }
        let mut incoming = self.conn.prepare("SELECT src, rel FROM edge WHERE dst = ?1")?;
        for row in incoming.query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))? {
            let (other, rel) = row?;
            out.push(Incident { from: other.clone(), to: id.to_string(), other, rel });
        }
        out.sort_by(|a, b| (&a.other, &a.rel, &a.from).cmp(&(&b.other, &b.rel, &b.from)));
        Ok(out)
    }

    /// Every item of the review chain, in the order they were stored (time, then id).
    pub fn items(&self) -> Result<Vec<StoredItem>, IndexError> {
        let mut statement = self.conn.prepare("SELECT idx, id, kind, role, version, at, sha, sha_alt, review_id, current FROM item ORDER BY idx")?;
        let items = statement
            .query_map([], |row| {
                Ok(StoredItem { idx: row.get(0)?, id: row.get(1)?, kind: row.get(2)?, role: row.get(3)?, version: row.get(4)?, at: row.get(5)?, sha: row.get(6)?, sha_alt: row.get(7)?, review_id: row.get(8)?, current: row.get::<_, i64>(9)? != 0 })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    /// Every join of the review chain: `(from, to, rel)` over item indexes.
    pub fn links(&self) -> Result<Vec<(usize, usize, String)>, IndexError> {
        let mut statement = self.conn.prepare("SELECT src, dst, rel FROM link ORDER BY src, dst, rel")?;
        let links = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(links)
    }
}

/// A search hit's snippet: about a line of the text around the first place `query` occurs in `body`, else
/// the start of the title or body.
pub fn snippet(query: &str, title: &str, body: &str) -> String {
    const BEFORE: usize = 40;
    const AFTER: usize = 80;
    let fold = |text: &str| -> Vec<char> { text.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect() };
    let wanted = fold(query);
    let find = |text: &str| -> Option<(Vec<char>, usize)> {
        let original: Vec<char> = text.chars().collect();
        let folded = fold(text);
        if wanted.is_empty() || folded.len() < wanted.len() {
            return None;
        }
        (0..=folded.len() - wanted.len()).find(|&at| folded[at..at + wanted.len()] == wanted[..]).map(|at| (original, at))
    };
    let (chars, at) = find(body).or_else(|| find(title)).unwrap_or_else(|| (if body.is_empty() { title } else { body }.chars().collect(), 0));
    let start = at.saturating_sub(BEFORE);
    let end = (at + wanted.len() + AFTER).min(chars.len());
    let window: String = chars[start..end].iter().collect();
    let line = window.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}{line}{}", if start > 0 { "…" } else { "" }, if end < chars.len() { "…" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snippet_is_the_text_around_the_first_match_whatever_its_case_or_script() {
        let body = format!("{} Token is logged in the error path {}", "x".repeat(100), "y".repeat(200));
        let shown = snippet("TOKEN IS", "title", &body);
        assert!(shown.starts_with('…') && shown.ends_with('…'), "{shown}");
        assert!(shown.contains("Token is logged"), "{shown}");
        assert!(shown.chars().count() < 140, "{shown}");
        assert_eq!(snippet("타임아웃", "", "재시도 로직이 타임아웃을 무시한다"), "재시도 로직이 타임아웃을 무시한다");
        assert_eq!(snippet("absent", "a title", "a body"), "a body");
        assert_eq!(snippet("absent", "a title", ""), "a title");
        assert_eq!(snippet("in the title", "match in the title only", "a body"), "match in the title only");
    }
}
