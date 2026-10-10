//! How a write command says where its write landed.
//!
//! Some writes are STAGED: the record waits in a staging directory and
//! lands in the ledger only when a promote runs. The rest are written
//! directly. An operator who cannot tell the two apart either skips a
//! promote that was needed or runs one that has nothing to do. So every
//! write command ends its success line with exactly one of these
//! suffixes, after ` — `.

/// A record staged for `canon gate promote` (`evidence add`, `finding
/// add`, `finding close`).
pub const STAGED_FOR_GATE_PROMOTE: &str = "run `canon gate promote` to commit it";

/// A divergence candidate staged for `canon divergence promote`.
pub const STAGED_FOR_DIVERGENCE_PROMOTE: &str = "run `canon divergence promote` to commit it";

/// A write that is already final: a ledger record committed directly
/// (`review add`, `divergence resolve|defer`), a subject record, or a
/// file in the working tree (`scenario new`, `feature new`).
pub const DIRECT: &str = "written directly; nothing to promote";
