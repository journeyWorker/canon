//! `canon.yaml`'s `language:` (0.19 D6): the language the repo's briefs
//! are written in. It picks the brief template's headings and hints
//! ([`crate::brief_template`]) and the prose rule lists `canon brief
//! check` applies ([`crate::brief_check`]).
//!
//! Templates and prose rules ship for `en` and `ko`. Any other tag is
//! valid: it gets the `en` headings and no prose rules (the structure
//! rules still apply). An unset `language:` reads as `en` for the
//! template `canon change new` writes, which is what a repo without the
//! key had before 0.19.
//!
//! # Judging a brief (0.26 D1)
//! A brief is judged in the language its text is written in, not the one
//! `canon.yaml` names: [`for_brief`] is the one resolution every judge
//! (`canon brief check`, `canon status`, the ship guard, the dashboard)
//! goes through. A set `language:` is an override and decides; the text
//! is then only read to say so when it disagrees ([`Resolved::note`]).
//! Unset, the script of the prose letters decides ([`Tally::script`]):
//! Hangul is `ko`, kana (or Han alone) is `ja` (`zh`), Latin is `en`.
//! Code spans, fenced code, comments and markers, URLs, code-like tokens,
//! headings and the `Approved:` / `Source:` lines are not prose and are
//! not counted ([`Tally::add_line`]). A section whose own script differs
//! from the brief's is judged in its own language ([`section_language`]).

use std::path::Path;

use serde::Serialize;

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

/// `canon.yaml`'s `language:` under `repo`, or `None` when it sets none
/// (a missing `canon.yaml` sets none). `Err` names a `canon.yaml` that
/// cannot be read or parsed, or a `language:` that is not a tag.
pub fn declared(repo: &Path) -> Result<Option<Language>, String> {
    let path = repo.join("canon.yaml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read `{}`: {e}", path.display())),
    };
    declared_from_yaml(&text)
}

/// [`declared`] over the text of a `canon.yaml`.
pub fn declared_from_yaml(text: &str) -> Result<Option<Language>, String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(text).map_err(|e| format!("canon.yaml does not parse: {e}"))?;
    match doc.get("language") {
        None | Some(serde_yaml::Value::Null) => Ok(None),
        Some(serde_yaml::Value::String(tag)) => Language::parse(tag).map(Some).map_err(|e| format!("canon.yaml `language:` {e}")),
        Some(_) => Err("canon.yaml `language:` must be a string language tag such as `en` or `ko`".to_string()),
    }
}

/// The language `canon change new` writes the template in: `language:`
/// under `repo`, `en` when unset. Judging a brief goes through
/// [`for_brief`], never this.
pub fn configured(repo: &Path) -> Result<Language, String> {
    declared(repo).map(|language| language.unwrap_or_else(Language::unset))
}

/// [`configured`] over the text of a `canon.yaml`.
pub fn from_yaml(text: &str) -> Result<Language, String> {
    declared_from_yaml(text).map(|language| language.unwrap_or_else(Language::unset))
}

// ───────────────────────────── detection ─────────────────────────────

/// Where a resolved language came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Read from the script of the text.
    Detected,
    /// `canon.yaml`'s `language:`.
    Configured,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Detected => "detected",
            Source::Configured => "configured",
        }
    }
}

/// The language a brief is judged in, where it came from, and, for a
/// configured language the text does not read as, one line saying so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub language: Language,
    pub source: Source,
    /// `the text reads as ko`: only when `source` is `Configured` and the
    /// detected script differs.
    pub note: Option<String>,
}

impl Resolved {
    /// `ko (detected)` / `en (configured; the text reads as ko)`: what
    /// the one `language:` line of the human output says.
    pub fn describe(&self) -> String {
        match &self.note {
            Some(note) => format!("{} ({}; {note})", self.language.tag, self.source.as_str()),
            None => format!("{} ({})", self.language.tag, self.source.as_str()),
        }
    }
}

/// Fewest prose letters a text needs before its script is read; below it
/// the text has no language of its own (a section inherits the brief's).
pub const MIN_LETTERS: usize = 12;

/// Hangul's share of the prose letters, in percent, from which a text is
/// `ko`. Korean prose names products and terms in Latin letters (a
/// Korean sentence with an English word is 10-25% Latin), and English
/// prose quotes the odd Korean UI string (a few percent Hangul), so the
/// line sits at a meaningful share, not a majority.
pub const HANGUL_PERCENT: usize = 30;

/// The same share for kana plus Han (`ja`), and for Han alone (`zh`).
pub const CJK_PERCENT: usize = 30;

/// Counts of the prose letters of a text, by script.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub hangul: usize,
    pub kana: usize,
    pub han: usize,
    pub latin: usize,
}

/// The script a text's prose reads in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Hangul,
    Japanese,
    Chinese,
    Latin,
}

impl Script {
    /// The language tag a detected script names.
    pub fn tag(self) -> &'static str {
        match self {
            Script::Hangul => "ko",
            Script::Japanese => "ja",
            Script::Chinese => "zh",
            Script::Latin => "en",
        }
    }
}

/// Lines starting with one of these are brief bookkeeping, not prose.
const META_PREFIXES: [&str; 2] = ["Approved:", "Source:"];

/// Whether `line` is an `Approved:` or `Source:` line.
pub fn is_meta_line(line: &str) -> bool {
    let t = line.trim_start();
    META_PREFIXES.iter().any(|p| t.starts_with(p))
}

/// A whitespace token that is code, not prose: ASCII with a slash, an
/// underscore, `::`, an `@`, or a dot between alphanumerics (a path, an
/// identifier, an address).
fn is_code_token(token: &str) -> bool {
    if !token.is_ascii() {
        return false;
    }
    let bytes = token.as_bytes();
    token.contains(['/', '_', '@']) || token.contains("::") || bytes.windows(3).any(|w| w[1] == b'.' && w[0].is_ascii_alphanumeric() && w[2].is_ascii_alphanumeric())
}

impl Tally {
    /// Counts the prose letters of one body line: headings and meta lines
    /// are skipped; code spans, URLs and code-like tokens are not prose.
    pub fn add_line(&mut self, line: &str) {
        let t = line.trim_start();
        if t.starts_with('#') || is_meta_line(t) {
            return;
        }
        let mut prose = String::with_capacity(line.len());
        let mut in_code = false;
        for c in line.chars() {
            match c {
                '`' => {
                    in_code = !in_code;
                    prose.push(' ');
                }
                _ if in_code => {}
                _ => prose.push(c),
            }
        }
        for token in prose.split_whitespace() {
            if token.contains("://") || is_code_token(token) {
                continue;
            }
            for c in token.chars() {
                match c {
                    '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' => self.hangul += 1,
                    '\u{3040}'..='\u{30FF}' => self.kana += 1,
                    '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' => self.han += 1,
                    c if c.is_ascii_alphabetic() => self.latin += 1,
                    _ => {}
                }
            }
        }
    }

    pub fn total(&self) -> usize {
        self.hangul + self.kana + self.han + self.latin
    }

    /// The script the letters read in; `None` below [`MIN_LETTERS`].
    pub fn script(&self) -> Option<Script> {
        let total = self.total();
        if total < MIN_LETTERS {
            return None;
        }
        let share = |n: usize, percent: usize| n * 100 >= total * percent;
        Some(if share(self.hangul, HANGUL_PERCENT) {
            Script::Hangul
        } else if self.kana > 0 && share(self.kana + self.han, CJK_PERCENT) {
            Script::Japanese
        } else if share(self.han, CJK_PERCENT) {
            Script::Chinese
        } else {
            Script::Latin
        })
    }
}

/// The script group a configured tag belongs to, for comparing it with
/// what the text reads as: `ko`, `ja` and `zh` are their own, every other
/// tag is read as a Latin-script language.
fn script_of(language: &Language) -> Script {
    match language.tag.split('-').next() {
        Some("ko") => Script::Hangul,
        Some("ja") => Script::Japanese,
        Some("zh") => Script::Chinese,
        _ => Script::Latin,
    }
}

/// Resolves the language a brief is judged in: `declared` (the repo's
/// `language:`) when set, else the script of `brief`'s prose, `en` when
/// it has too few letters to read.
pub fn resolve(declared: Option<&Language>, brief: &Tally) -> Resolved {
    let detected = brief.script();
    match declared {
        Some(language) => {
            let note = detected.filter(|script| *script != script_of(language)).map(|script| format!("the text reads as {}", script.tag()));
            Resolved { language: language.clone(), source: Source::Configured, note }
        }
        None => {
            let language = detected.map_or_else(Language::unset, |script| Language { tag: script.tag().to_string() });
            Resolved { language, source: Source::Detected, note: None }
        }
    }
}

/// The language one section of a brief is judged in: the brief's own
/// when the language is configured or the section has too few letters to
/// read, else the section's script (`ko` for a Korean section of an
/// English brief and the other way round).
pub fn section_language(brief: &Resolved, section: &Tally) -> Language {
    if brief.source == Source::Configured {
        return brief.language.clone();
    }
    section.script().map_or_else(|| brief.language.clone(), |script| Language { tag: script.tag().to_string() })
}

/// The language `text`, the brief of a repo at `repo`, is judged in: the
/// one resolution (module doc). `Err` is [`declared`]'s.
pub fn for_brief(repo: &Path, text: &str) -> Result<Resolved, String> {
    Ok(resolve(declared(repo)?.as_ref(), &crate::brief_check::prose_tally(text)))
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
        Language::parse("korean language").unwrap_err();
        Language::parse("k").unwrap_err();
        from_yaml("language: [ko]\n").unwrap_err();
        assert_eq!(from_yaml("tiers: {}\n").unwrap(), Language::unset());
        assert_eq!(from_yaml("language: ko\n").unwrap().tag, "ko");
        assert_eq!(declared_from_yaml("tiers: {}\n").unwrap(), None);
        assert_eq!(declared_from_yaml("language: ko\n").unwrap(), Some(Language::parse("ko").unwrap()));
    }

    fn tally(text: &str) -> Tally {
        let mut t = Tally::default();
        for line in text.lines() {
            t.add_line(line);
        }
        t
    }

    #[test]
    fn the_script_of_the_prose_letters_names_the_language() {
        let script = |text: &str| tally(text).script().map(Script::tag);
        assert_eq!(script("방문자가 로그인하면 저장된 장바구니가 보인다."), Some("ko"));
        assert_eq!(script("A visitor who signs in sees the saved cart."), Some("en"));
        assert_eq!(script("訪問者がログインすると保存したカートが表示される。"), Some("ja"));
        assert_eq!(script("访客登录后可以看到已保存的购物车内容。"), Some("zh"));
        assert_eq!(script("Add login"), None);
        // An English sentence quoting one Korean UI string stays English; a Korean sentence naming an English product stays Korean.
        assert_eq!(script("The button reads 저장 and the page reloads after the save completes."), Some("en"));
        assert_eq!(script("Checkout 화면에서 사용자가 Apple Pay 버튼을 누르면 결제가 시작된다."), Some("ko"));
    }

    #[test]
    fn code_urls_headings_and_meta_lines_are_not_prose() {
        let t = tally("## Heading in English\nSource: docs/some-plan.md\nApproved: someone\n`parseConfig()` https://example.com/a/b src/main.rs 로그인한 사용자는 목록을 본다\n");
        assert_eq!((t.latin, t.hangul > 0), (0, true), "{t:?}");
    }

    #[test]
    fn a_configured_language_decides_and_a_disagreeing_text_is_noted() {
        let ko_text = tally("방문자가 로그인하면 저장된 장바구니가 보인다.");
        let en = Language::parse("en").unwrap();
        let ko = Language::parse("ko").unwrap();
        let over = resolve(Some(&en), &ko_text);
        assert_eq!((over.language.tag.as_str(), over.source, over.note.as_deref()), ("en", Source::Configured, Some("the text reads as ko")));
        assert_eq!(over.describe(), "en (configured; the text reads as ko)");
        assert_eq!(resolve(Some(&ko), &ko_text).note, None);
        let detected = resolve(None, &ko_text);
        assert_eq!((detected.language.tag.as_str(), detected.describe().as_str()), ("ko", "ko (detected)"));
        // A configured Latin-script tag the text agrees with by script gets no note.
        assert_eq!(resolve(Some(&Language::parse("fr").unwrap()), &tally("Le visiteur voit le panier enregistré.")).note, None);
    }

    #[test]
    fn a_section_with_a_script_of_its_own_is_judged_in_it() {
        let brief = resolve(None, &tally("A visitor who signs in sees the saved cart and the order history."));
        let ko = tally("방문자가 로그인하면 저장된 장바구니가 보인다.");
        assert_eq!(section_language(&brief, &ko).tag, "ko");
        assert_eq!(section_language(&brief, &tally("None.")).tag, "en");
        let configured = resolve(Some(&Language::parse("en").unwrap()), &ko);
        assert_eq!(section_language(&configured, &ko).tag, "en");
    }
}
