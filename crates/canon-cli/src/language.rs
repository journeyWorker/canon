//! `canon.yaml`'s `language:` (0.19 D6): the language the repo's briefs
//! are written in. It picks the brief template's headings and hints
//! ([`crate::brief_template`]) and the prose rule lists `canon brief
//! check` applies ([`crate::brief_check`]).
//!
//! Templates and prose rules ship for `en` and `ko`. Any other tag is
//! valid: it gets the `en` headings and no prose rules (the structure
//! rules still apply). An unset `language:` reads as `en`, which is what
//! a repo without the key had before 0.19.

use std::path::Path;

/// The languages canon ships a template and prose rule list for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shipped {
    En,
    Ko,
}

/// A repo's configured language tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    /// The tag as written, lowercased (`ko`, `en-us`, `fr`).
    pub tag: String,
}

impl Language {
    /// The default for a `canon.yaml` with no `language:`.
    pub fn unset() -> Self {
        Language { tag: "en".to_string() }
    }

    /// Parses a tag: an ASCII language subtag of two or three letters,
    /// optionally followed by `-` subtags of letters and digits
    /// (`ko`, `en-US`, `zh-Hant`).
    pub fn parse(raw: &str) -> Result<Self, String> {
        let tag = raw.trim().to_ascii_lowercase();
        let mut parts = tag.split('-');
        let primary = parts.next().unwrap_or("");
        let primary_ok = (2..=3).contains(&primary.len()) && primary.bytes().all(|b| b.is_ascii_lowercase());
        let rest_ok = parts.all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()));
        if primary_ok && rest_ok {
            Ok(Language { tag })
        } else {
            Err(format!("`{}` is not a language tag (expected a BCP 47 tag such as `en` or `ko`)", raw.trim()))
        }
    }

    /// The shipped language this tag's primary subtag names, if any.
    pub fn shipped(&self) -> Option<Shipped> {
        match self.tag.split('-').next() {
            Some("ko") => Some(Shipped::Ko),
            Some("en") => Some(Shipped::En),
            _ => None,
        }
    }

    /// The language whose headings and hints the brief template uses:
    /// the tag's own when shipped, otherwise `en`.
    pub fn template(&self) -> Shipped {
        self.shipped().unwrap_or(Shipped::En)
    }

    /// The language whose prose rule lists apply; `None` for a tag canon
    /// ships none for.
    pub fn prose_rules(&self) -> Option<Shipped> {
        self.shipped()
    }
}

/// `canon.yaml`'s `language:` under `repo`: unset reads as `en`. `Err`
/// names a `canon.yaml` that cannot be read or parsed, or a `language:`
/// that is not a tag.
pub fn configured(repo: &Path) -> Result<Language, String> {
    let path = repo.join("canon.yaml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Language::unset()),
        Err(e) => return Err(format!("cannot read `{}`: {e}", path.display())),
    };
    from_yaml(&text)
}

/// [`configured`] over the text of a `canon.yaml`.
pub fn from_yaml(text: &str) -> Result<Language, String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(text).map_err(|e| format!("canon.yaml does not parse: {e}"))?;
    match doc.get("language") {
        None | Some(serde_yaml::Value::Null) => Ok(Language::unset()),
        Some(serde_yaml::Value::String(tag)) => Language::parse(tag).map_err(|e| format!("canon.yaml `language:` {e}")),
        Some(_) => Err("canon.yaml `language:` must be a string language tag such as `en` or `ko`".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_route_to_shipped_languages_by_primary_subtag() {
        assert_eq!(Language::parse("ko").unwrap().shipped(), Some(Shipped::Ko));
        assert_eq!(Language::parse("en-US").unwrap().shipped(), Some(Shipped::En));
        let french = Language::parse("fr").unwrap();
        assert_eq!(french.shipped(), None);
        assert_eq!(french.template(), Shipped::En);
        assert_eq!(french.prose_rules(), None);
    }

    #[test]
    fn a_malformed_tag_or_value_is_an_error() {
        assert!(Language::parse("korean language").is_err());
        assert!(Language::parse("k").is_err());
        assert!(from_yaml("language: [ko]\n").is_err());
        assert_eq!(from_yaml("tiers: {}\n").unwrap(), Language::unset());
        assert_eq!(from_yaml("language: ko\n").unwrap().tag, "ko");
    }
}
