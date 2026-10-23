//! "Did you mean" for usage errors and refused values (0.28 D3).
//!
//! The candidates come from the clap tree (commands, long options, possible
//! values) or from an error's own `expected` set; the measure is edit
//! distance. A `destructive` command ([`crate::schema::Effect`]) is suggested
//! only when the typo is within distance 1 of it: a slip must never turn into
//! a deletion the caller did not mean to type.

use crate::schema::{Effect, META};

/// At most this many suggestions.
const LIMIT: usize = 3;

/// The Levenshtein distance between two words, by characters.
pub fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ca == *cb { diagonal } else { 1 + diagonal.min(above).min(row[j]) };
            diagonal = above;
        }
    }
    row[b.len()]
}

/// How far a word may be from a candidate and still be a likely typo of it.
fn allowed(word: &str) -> usize {
    (word.chars().count() / 3).clamp(1, 3)
}

/// The candidates within typo range of `received`, closest first (ties in
/// candidate order), at most [`LIMIT`].
pub fn nearest<'a>(received: &str, candidates: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    ranked(received, candidates.into_iter().map(|candidate| (candidate, candidate, usize::MAX)))
}

/// [`nearest`] over `(shown, compared, cap)` triples: `compared` is measured
/// against `received`, `shown` is what the suggestion says, `cap` lowers the
/// allowed distance for that candidate.
fn ranked<'a>(received: &str, candidates: impl Iterator<Item = (&'a str, &'a str, usize)>) -> Vec<String> {
    let received = received.to_ascii_lowercase();
    let limit = allowed(&received);
    let mut found: Vec<(usize, usize, &str)> = Vec::new();
    for (order, (shown, compared, cap)) in candidates.enumerate() {
        let d = distance(&received, &compared.to_ascii_lowercase());
        if d <= limit.min(cap) {
            found.push((d, order, shown));
        }
    }
    found.sort();
    found.dedup_by(|a, b| a.2 == b.2);
    found.into_iter().take(LIMIT).map(|(_, _, shown)| shown.to_string()).collect()
}

/// Whether `path` (a canonical command path such as `purge`) is destructive.
fn destructive(path: &str) -> bool {
    META.iter().any(|row| row.path == path && row.effect == Effect::Destructive)
}

/// The commands below the group at `parent` (words, `[]` for the root) that
/// `word` is a typo of, as full command lines (`canon gate check`). Commands
/// that are destructive count only within distance 1.
pub fn commands(root: &clap::Command, parent: &[String], word: &str) -> Vec<String> {
    let Some(node) = node_at(root, parent) else { return Vec::new() };
    let prefix: String = parent.iter().map(|w| format!("{w} ")).collect();
    let names: Vec<(String, String, usize)> = node
        .get_subcommands()
        .filter(|c| !c.is_hide_set() && c.get_name() != "help")
        .map(|c| {
            let path = format!("{prefix}{}", c.get_name());
            let cap = if destructive(&path) { 1 } else { usize::MAX };
            (format!("canon {path}"), c.get_name().to_string(), cap)
        })
        .collect();
    ranked(word, names.iter().map(|(shown, compared, cap)| (shown.as_str(), compared.as_str(), *cap)))
}

/// The long options of the command at `path` that `word` (`--flag`) is a typo of.
pub fn options(root: &clap::Command, path: &[String], word: &str) -> Vec<String> {
    let Some(node) = node_at(root, path) else { return Vec::new() };
    let names = long_options(node);
    nearest(word.trim_start_matches('-'), names.iter().map(|n| n.trim_start_matches('-'))).into_iter().map(|n| format!("--{n}")).collect()
}

/// Every `--long` option of `node`, in declaration order, without clap's own help and version.
pub fn long_options(node: &clap::Command) -> Vec<String> {
    node.get_arguments().filter(|a| !a.is_hide_set()).filter_map(|a| a.get_long()).filter(|l| *l != "help" && *l != "version").map(|l| format!("--{l}")).collect()
}

/// The command at `path` in the tree, `None` when a word is not a subcommand.
pub fn node_at<'a>(root: &'a clap::Command, path: &[String]) -> Option<&'a clap::Command> {
    let mut node = root;
    for word in path {
        node = node.find_subcommand(word)?;
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_counts_edits() {
        assert_eq!(distance("gate", "gate"), 0);
        assert_eq!(distance("gat", "gate"), 1);
        assert_eq!(distance("kitten", "sitting"), 3);
        assert_eq!(distance("", "abc"), 3);
    }

    #[test]
    fn nearest_orders_by_distance_and_drops_far_words() {
        assert_eq!(nearest("fnding", ["finding", "feature", "format"]), vec!["finding".to_string()]);
        assert!(nearest("zzzz", ["finding", "feature"]).is_empty());
    }

    #[test]
    fn a_destructive_command_needs_distance_one() {
        let root = crate::cli::command();
        assert!(commands(&root, &[], "purgee").contains(&"canon purge".to_string()));
        assert!(!commands(&root, &[], "purgxx").contains(&"canon purge".to_string()));
    }
}
