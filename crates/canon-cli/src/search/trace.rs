//! `canon trace <id>`: the review → finding → fix → re-review → evidence chain of one loop, in time order
//! (0.31 `canon-0-31-search`).
//!
//! The operand is one of three shapes ([`SHAPES`]). It names the starting records; the chain is every item
//! reachable from them over the joins the index holds ([`super::graph::chain_links`]), each item one
//! version of one record, the archive's included.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Serialize;

use super::index::{Index, IndexError, StoredItem};

/// The three shapes `canon trace` accepts, for the messages that refuse another.
pub const SHAPES: &str = "a review id (26 characters, the id `canon review scenario` or `canon review change` printed), a commit sha (7 to 40 hex digits; the first characters of a sha are enough), or a finding key (`<change>_<round>_<seq>`, as `canon query --kind finding` shows it)";

/// Why an operand names no start of a chain.
#[derive(Debug, PartialEq, Eq)]
pub enum TraceError {
    /// The operand has none of the three shapes.
    Unrecognized,
    /// The operand has a shape, and no record says it.
    NotFound,
    /// A sha prefix that fits more than one commit.
    Ambiguous(Vec<String>),
}

/// What the operand was read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ResolvedAs {
    Review,
    Commit,
    Finding,
}

/// One join of an item.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraceEdge {
    pub rel: String,
    /// `out` when the item is the edge's `from` end, `in` when it is the `to` end.
    pub direction: &'static str,
    /// The item at the other end.
    pub id: String,
    pub role: String,
    /// That item's version, which tells two versions of one record apart.
    pub version: String,
}

/// One version of one record in the chain.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraceItem {
    /// The version's own time.
    pub at: String,
    pub id: String,
    pub kind: String,
    /// `finding` (the version that recorded it), `fix` (the version that closed it fixed), `review` or `evidence`.
    pub role: String,
    /// The commit the item is about: reviewed, fixed in, pinned at, evidenced at.
    pub sha: Option<String>,
    /// The version's digest: with the id, what tells two versions apart.
    pub version: String,
    /// Whether the version is one the ledger's archive holds.
    pub archived: bool,
    pub edges: Vec<TraceEdge>,
}

/// The chain of an operand.
#[derive(Debug)]
pub struct Chain {
    pub resolved_as: ResolvedAs,
    pub items: Vec<TraceItem>,
}

fn is_hex(text: &str) -> bool {
    text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_ulid(text: &str) -> bool {
    text.len() == 26 && text.bytes().all(|b| b.is_ascii_digit() || (b.is_ascii_alphabetic() && !matches!(b.to_ascii_uppercase(), b'I' | b'L' | b'O' | b'U')))
}

/// `<change>_<round>_<seq>`.
fn is_finding_key(text: &str) -> bool {
    let mut parts = text.rsplitn(3, '_');
    let (seq, round, change) = (parts.next(), parts.next(), parts.next());
    matches!((seq, round, change), (Some(s), Some(r), Some(c)) if !c.is_empty() && !s.is_empty() && !r.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && r.bytes().all(|b| b.is_ascii_digit()))
}

/// Trace `operand` through `index`.
pub fn trace(index: &Index, operand: &str) -> Result<Result<Chain, TraceError>, IndexError> {
    let operand = operand.trim();
    let items = index.items()?;
    let (starts, resolved_as) = match starts_of(&items, operand) {
        Ok(found) => found,
        Err(error) => return Ok(Err(error)),
    };
    let links = index.links()?;

    let mut adjacent: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (from, to, _) in &links {
        adjacent.entry(*from).or_default().insert(*to);
        adjacent.entry(*to).or_default().insert(*from);
    }
    // The two items of one finding, the version that recorded it and the one that fixed it, are one finding:
    // reaching either reaches both. (Versions of a review or of evidence are not: each is joined on its own.)
    for a in items.iter().filter(|i| i.kind == "finding") {
        for b in items.iter().filter(|b| b.kind == "finding" && b.id == a.id && b.idx != a.idx) {
            adjacent.entry(a.idx).or_default().insert(b.idx);
        }
    }
    let mut reached: BTreeSet<usize> = starts.iter().copied().collect();
    let mut frontier: Vec<usize> = reached.iter().copied().collect();
    while let Some(at) = frontier.pop() {
        for next in adjacent.get(&at).into_iter().flatten() {
            if reached.insert(*next) {
                frontier.push(*next);
            }
        }
    }

    let by_idx: BTreeMap<usize, &StoredItem> = items.iter().map(|item| (item.idx, item)).collect();
    let mut chain: Vec<&StoredItem> = reached.iter().filter_map(|idx| by_idx.get(idx).copied()).collect();
    chain.sort_by(|a, b| (&a.at, &a.id, &a.role, &a.version).cmp(&(&b.at, &b.id, &b.role, &b.version)));
    let items = chain
        .into_iter()
        .map(|item| {
            let mut edges: Vec<TraceEdge> = Vec::new();
            for (from, to, rel) in &links {
                let (direction, other) = if *from == item.idx { ("out", *to) } else if *to == item.idx { ("in", *from) } else { continue };
                if let Some(other) = by_idx.get(&other) {
                    edges.push(TraceEdge { rel: rel.clone(), direction, id: other.id.clone(), role: other.role.clone(), version: other.version.clone() });
                }
            }
            edges.sort_by(|a, b| (&a.rel, a.direction, &a.id, &a.role, &a.version).cmp(&(&b.rel, b.direction, &b.id, &b.role, &b.version)));
            TraceItem { at: item.at.clone(), id: item.id.clone(), kind: item.kind.clone(), role: item.role.clone(), sha: item.sha.clone(), version: item.version.clone(), archived: !item.current, edges }
        })
        .collect();
    Ok(Ok(Chain { resolved_as, items }))
}

/// The items an operand starts from.
fn starts_of(items: &[StoredItem], operand: &str) -> Result<(Vec<usize>, ResolvedAs), TraceError> {
    if is_ulid(operand) {
        let reviews: Vec<usize> = items.iter().filter(|i| i.role == "review" && i.review_id.as_deref().is_some_and(|id| id.eq_ignore_ascii_case(operand))).map(|i| i.idx).collect();
        if !reviews.is_empty() {
            return Ok((reviews, ResolvedAs::Review));
        }
        if !is_hex(operand) {
            return Err(TraceError::NotFound);
        }
    }
    if (7..=40).contains(&operand.len()) && is_hex(operand) {
        let prefix = operand.to_ascii_lowercase();
        let shas: BTreeSet<&str> = items.iter().flat_map(|i| [i.sha.as_deref(), i.sha_alt.as_deref()]).flatten().filter(|sha| sha.starts_with(&prefix)).collect();
        return match shas.len() {
            0 => Err(TraceError::NotFound),
            1 => {
                let sha = *shas.iter().next().expect("one sha");
                Ok((items.iter().filter(|i| i.sha.as_deref() == Some(sha) || i.sha_alt.as_deref() == Some(sha)).map(|i| i.idx).collect(), ResolvedAs::Commit))
            }
            _ => Err(TraceError::Ambiguous(shas.into_iter().map(str::to_string).collect())),
        };
    }
    if is_finding_key(operand) {
        let id = format!("finding:{operand}");
        let found: Vec<usize> = items.iter().filter(|i| i.id == id).map(|i| i.idx).collect();
        return if found.is_empty() { Err(TraceError::NotFound) } else { Ok((found, ResolvedAs::Finding)) };
    }
    Err(TraceError::Unrecognized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(idx: usize, id: &str, role: &str, sha: Option<&str>) -> StoredItem {
        StoredItem { idx, id: id.to_string(), kind: "x".to_string(), role: role.to_string(), version: "v".to_string(), at: "t".to_string(), sha: sha.map(str::to_string), sha_alt: None, review_id: None, current: true }
    }

    #[test]
    fn the_three_shapes_are_told_apart() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let items = vec![item(0, "finding:a-b_1_2", "finding", Some(sha)), item(1, "finding:a-b_1_2", "fix", Some("89abcdef0123456789abcdef0123456789abcdef"))];
        assert_eq!(starts_of(&items, "a-b_1_2").unwrap(), (vec![0, 1], ResolvedAs::Finding));
        assert_eq!(starts_of(&items, "0123456").unwrap(), (vec![0], ResolvedAs::Commit));
        assert_eq!(starts_of(&items, &sha.to_uppercase()).unwrap(), (vec![0], ResolvedAs::Commit));
        assert_eq!(starts_of(&items, "a-b_1_3"), Err(TraceError::NotFound));
        assert_eq!(starts_of(&items, "0123450"), Err(TraceError::NotFound));
        assert_eq!(starts_of(&items, "01ARZ3NDEKTSV4RRFFQ69G5FAV"), Err(TraceError::NotFound));
        for unrecognized in ["", "abc", "change:x", "a-b", "a-b_x_2", "_1_2", "01ARZ3NDEKTSV4RRFFQ69G5FAV0"] {
            assert_eq!(starts_of(&items, unrecognized), Err(TraceError::Unrecognized), "{unrecognized:?}");
        }
        let twin = vec![item(0, "x", "evidence", Some("abcdef0123456789abcdef0123456789abcdef01")), item(1, "y", "evidence", Some("abcdef0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"))];
        assert!(matches!(starts_of(&twin, "abcdef0"), Err(TraceError::Ambiguous(shas)) if shas.len() == 2));
        assert_eq!(starts_of(&twin, "abcdef01").unwrap().1, ResolvedAs::Commit);
    }

    #[test]
    fn a_26_character_hex_operand_that_is_no_review_id_is_a_sha_prefix() {
        let mut review = item(0, "review:r", "review", Some("0123456789abcdef0123456789abcdef01234567"));
        review.review_id = Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string());
        let items = vec![review];
        assert_eq!(starts_of(&items, "01arz3ndektsv4rrffq69g5fav").unwrap().1, ResolvedAs::Review, "a review id matches in either case");
        assert_eq!(starts_of(&items, "0123456789abcdef0123456789").unwrap().1, ResolvedAs::Commit);
        assert_eq!(starts_of(&items, "01ARZ3NDEKTSV4RRFFQ69G5FAW"), Err(TraceError::NotFound));
    }
}
