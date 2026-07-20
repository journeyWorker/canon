//! `canon-learn`'s own row ids — `TrajectoryId`/`StrategyId`, both
//! ULIDs, mirroring `canon-model::ids::RunId`'s exact pattern (a ULID's
//! own parser is the grammar check; its canonical Crockford-base32
//! `Display`/`FromStr` is already what "join key" means here). These
//! are NOT join-spine keys (the join spine's own eight keys live in
//! `canon-model`) — they identify a row within THIS crate's two
//! stores, referenced by `StrategyItem::source_trajectory_ids`
//! provenance.

use std::fmt;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::error::LearnError;

macro_rules! ulid_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(Ulid);

        impl $name {
            /// A fresh, time-sortable id (ULIDs embed a millisecond
            /// timestamp — two ids minted in the same call sort in
            /// generation order, a useful property for a raw/append
            /// tier even before `recorded_at` is consulted).
            pub fn new() -> Self {
                Self(Ulid::new())
            }

            pub fn parse(s: &str) -> Result<Self, LearnError> {
                Ulid::from_string(s).map(Self).map_err(|e| LearnError::InvalidId { value: s.to_string(), reason: e.to_string() })
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = LearnError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::parse(&s)
            }
        }

        impl From<$name> for String {
            fn from(v: $name) -> String {
                v.0.to_string()
            }
        }
    };
}

ulid_id!(TrajectoryId, "Identifies one raw [`crate::trajectory::Trajectory`] row.");

ulid_id!(StrategyId, "Identifies one distilled [`crate::strategy::StrategyItem`] row.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trajectory_id_display_parse_round_trips() {
        let id = TrajectoryId::new();
        let s = id.to_string();
        assert_eq!(TrajectoryId::parse(&s).unwrap(), id);
    }

    #[test]
    fn strategy_id_serde_round_trips() {
        let id = StrategyId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: StrategyId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn two_freshly_minted_ids_are_distinct() {
        assert_ne!(TrajectoryId::new(), TrajectoryId::new());
    }

    #[test]
    fn malformed_id_is_rejected() {
        assert!(TrajectoryId::parse("not-a-ulid").is_err());
    }
}
