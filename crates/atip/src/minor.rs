//! The minors of atip/1 this crate knows (spec 15.7).
//!
//! A tool declares the minor it implements in `meta.contractVersion`; that declaration is the tool's own constant,
//! because a tool states what it prints, not what this crate could help it print. [`Minor::LATEST`] is the newest
//! minor the crate implements, so a rule that depends on the declared minor ([`Minor::at_least`]) is written once here
//! and a tool picks it by its own declaration.

use std::fmt;

/// A minor of atip/1: the `1.<minor>` of `meta.contractVersion` (a document that omits the member declares 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Minor {
    V1_0,
    V1_1,
    V1_2,
}

/// A text that is not a minor this crate knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownMinor(pub String);

impl fmt::Display for UnknownMinor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is not a minor of atip/1 this crate knows (it knows {})", self.0, Minor::ALL.map(Minor::as_str).join(", "))
    }
}

impl std::error::Error for UnknownMinor {}

impl Minor {
    /// Every minor the crate knows, oldest first.
    pub const ALL: [Minor; 3] = [Minor::V1_0, Minor::V1_1, Minor::V1_2];
    /// The newest minor the crate implements. It is not a tool's declaration.
    pub const LATEST: Minor = Minor::V1_2;

    /// The minor written `<major>.<minor>`, or [`UnknownMinor`] for any other text (a later minor, another major, `"1"`, `""`).
    pub fn parse(text: &str) -> Result<Self, UnknownMinor> {
        Self::ALL.into_iter().find(|minor| minor.as_str() == text).ok_or_else(|| UnknownMinor(text.to_string()))
    }

    /// The text of `meta.contractVersion`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Minor::V1_0 => "1.0",
            Minor::V1_1 => "1.1",
            Minor::V1_2 => "1.2",
        }
    }

    /// Whether this minor is `other` or a later one: `V1_2.at_least(V1_1)`.
    pub const fn at_least(self, other: Minor) -> bool {
        self as u8 >= other as u8
    }
}

impl fmt::Display for Minor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atip_crate12_01_the_minors_are_1_0_1_1_and_1_2_and_the_constant_is_the_newest() {
        assert_eq!(Minor::parse("1.0"), Ok(Minor::V1_0));
        assert_eq!(Minor::parse("1.1"), Ok(Minor::V1_1));
        assert_eq!(Minor::parse("1.2"), Ok(Minor::V1_2));
        for minor in Minor::ALL {
            assert_eq!(Minor::parse(minor.as_str()), Ok(minor));
            assert_eq!(minor.to_string(), minor.as_str());
        }
        assert_eq!(crate::CONTRACT_VERSION, "1.2");
        assert_eq!(crate::CONTRACT_VERSION, Minor::LATEST.as_str());
    }

    #[test]
    fn atip_crate12_03_a_text_that_is_not_a_known_minor_is_refused() {
        for text in ["1.3", "1.9", "2.0", "1", "", "1.2.0", "1.02", " 1.2", "v1.2"] {
            assert_eq!(Minor::parse(text), Err(UnknownMinor(text.to_string())), "{text:?}");
        }
        assert!(Minor::parse("1.3").expect_err("refused").to_string().contains("1.0, 1.1, 1.2"));
    }

    #[test]
    fn atip_crate12_01_at_least_orders_the_minors() {
        assert!(Minor::V1_2.at_least(Minor::V1_1));
        assert!(Minor::V1_2.at_least(Minor::V1_2));
        assert!(Minor::V1_1.at_least(Minor::V1_0));
        assert!(!Minor::V1_1.at_least(Minor::V1_2));
        assert!(!Minor::V1_0.at_least(Minor::V1_1));
        assert_eq!(Minor::ALL.iter().copied().max(), Some(Minor::LATEST));
    }
}
