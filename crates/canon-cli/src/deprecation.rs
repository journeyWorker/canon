//! Deprecated command forms (0.21 D1): one table, one mechanism.
//!
//! A renamed command keeps running for one release as a hidden clap
//! subcommand wired to the canonical handler. Running it calls [`warn`],
//! which prints `canon <old>: deprecated — use canon <new>` on stderr and
//! remembers the pair so [`crate::json_out::envelope`] adds
//! `meta.deprecated: { "use": "<new>", "removedIn": "0.22.0" }`. `canon
//! schema` reads [`DEPRECATED`] to list the old paths with the replacement
//! (hidden from the bare listing unless `--all`), and the drift test keeps
//! the table and the clap tree in step. 0.22 deletes the hidden variants and
//! their rows.
//!
//! A deprecated *form* of a surviving command (`subject status <id>
//! <state>`) has no path of its own; it calls [`warn`] with a descriptive
//! `old`/`new` pair and is documented where it is parsed.

use std::sync::OnceLock;

use serde_json::{json, Value};

/// The release that deletes everything in [`DEPRECATED`].
pub const REMOVED_IN: &str = "0.22.0";

/// A renamed command: `old` still runs, `new` is canonical. Both are
/// space-separated command paths without the leading `canon`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deprecation {
    pub old: &'static str,
    pub new: &'static str,
}

/// Every deprecated command path.
pub const DEPRECATED: &[Deprecation] = &[
    Deprecation { old: "context-pack create", new: "context-pack new" },
    Deprecation { old: "demo attest", new: "demo add" },
    Deprecation { old: "divergence stage", new: "divergence add" },
    Deprecation { old: "prompt register", new: "prompt add" },
];

/// The two-positional `subject status <id> <state>` form, replaced by `--to`.
pub const SUBJECT_STATUS_POSITIONAL: Deprecation = Deprecation { old: "subject status <id> <state>", new: "subject status <id> --to <state>" };

/// The deprecation row whose old path is `path`.
pub fn lookup(path: &str) -> Option<&'static Deprecation> {
    DEPRECATED.iter().find(|d| d.old == path)
}

/// The pair the running command was invoked under, set by [`warn`].
static ACTIVE: OnceLock<Deprecation> = OnceLock::new();

/// Announce that the running invocation uses a deprecated form: one stderr
/// line, and `meta.deprecated` on the JSON envelope.
pub fn warn(deprecation: &Deprecation) {
    eprintln!("canon {}: deprecated — use canon {}", deprecation.old, deprecation.new);
    let _ = ACTIVE.set(*deprecation);
}

/// [`warn`] for the deprecated command path `old`.
///
/// # Panics
/// When `old` is not in [`DEPRECATED`]: the table and the dispatch must agree.
pub fn warn_path(old: &str) {
    warn(lookup(old).unwrap_or_else(|| panic!("`{old}` is not in the deprecation table")));
}

/// `meta.deprecated` for the running command, when it was invoked deprecated.
pub fn meta() -> Option<Value> {
    ACTIVE.get().map(|d| json!({ "use": d.new, "removedIn": REMOVED_IN }))
}
