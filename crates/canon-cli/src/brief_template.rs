//! The PRD brief `canon change new` writes (0.19 D1, 0.31): Intent → Desired
//! outcome → User flow → Step detail → Acceptance → Questions and
//! assumptions → Out of scope, each an empty `##` section.
//!
//! The file holds a title, the sections' English headings and nothing else
//! (a chore brief starts with a frontmatter holding `kind: chore`): no HTML
//! comment, no hint, no approval note. An empty section is a section not yet
//! written, and `canon brief check` reports it with the section's guidance
//! ([`guidance`]), which `canon brief show` and the brief-shape skill show
//! too, in the repo's `language:`. Headings are always English: readers find
//! a section by its heading ([`canon_ingest::brief`]), and `language:` only
//! picks the language of the prose rules, the guidance and this template's
//! own wording.
//!
//! The Intent section's first paragraph is the change's summary
//! (`Change.summary`): the title.

use crate::language::{Language, Shipped};

pub use canon_ingest::brief::{canonical_role, Role, SectionDef};
use canon_ingest::brief::{CHORE_SECTIONS, FEATURE_SECTIONS};

/// The PRD sections, in the order a brief carries them.
pub const PRD_SECTIONS: &[SectionDef] = &FEATURE_SECTIONS;

/// `--kind` for clap, with `feature` and `chore` known to it.
pub fn kind_parser() -> impl clap::builder::TypedValueParser<Value = ChangeKind> {
    crate::cli_error::closed_set(["feature", "chore"], parse_kind)
}

/// `--kind` named neither `feature` nor `chore`.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChangeKindError {
    #[error("unknown change kind `{0}`; expected `feature` or `chore`")]
    Unknown(String),
}

/// `--kind` for clap: `feature` (the default) or `chore`.
pub fn parse_kind(raw: &str) -> Result<ChangeKind, ChangeKindError> {
    ChangeKind::parse(raw).ok_or_else(|| ChangeKindError::Unknown(raw.to_string()))
}

/// What a change is (0.19 D5). A `chore` changes how the subject is
/// built or kept, never what it does; its brief is the short one below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// The PRD brief.
    Feature,
    /// The chore brief.
    Chore,
}

impl ChangeKind {
    pub fn parse(raw: &str) -> Option<ChangeKind> {
        match raw {
            "feature" => Some(ChangeKind::Feature),
            "chore" => Some(ChangeKind::Chore),
            _ => None,
        }
    }

    /// The sections a brief of this kind carries, in order.
    pub fn sections(self) -> &'static [SectionDef] {
        match self {
            ChangeKind::Feature => &FEATURE_SECTIONS,
            ChangeKind::Chore => &CHORE_SECTIONS,
        }
    }
}

/// The values a chore's `category` section may hold.
pub const CHORE_CATEGORIES: [&str; 5] = ["refactor", "dependency", "build", "ci", "infra"];

/// The frontmatter a chore brief starts with, so a reader knows the change is
/// a chore without reading its sections.
pub const CHORE_FRONTMATTER: &str = "---\nkind: chore\n---\n\n";

/// What goes in section `key`, in `language`: shown by `canon brief show`
/// and in the hint of the empty-section blocker; empty for a key canon does
/// not know.
pub fn guidance(language: &Language, key: &str) -> &'static str {
    match (language.template(), key) {
        (Shipped::En, "intent") => "What problem this solves, for whom, and why now. One to three sentences.",
        (Shipped::En, "outcome") => "What is observably true once this ships, from the user's side.",
        (Shipped::En, "flow") => "A numbered list; each item starts with its id and reads sees → does → becomes: `1. F1 The user sees … → does … → becomes …`.",
        (Shipped::En, "detail") => "One `### F1 …` subheading per flow step, in the flow's order: rules, states and edge cases of that step.",
        (Shipped::En, "acceptance") => "Per step: what must be observable, and which failure path is covered. Do not write scenario ids here.",
        (Shipped::En, "questions") => "What is unresolved and what you assumed; anything that could change policy goes here. Write `None.` if nothing.",
        (Shipped::En, "out-of-scope") => "What this change deliberately does not do. Write `None.` if nothing.",
        (Shipped::En, "category") => "Exactly one of `refactor`, `dependency`, `build`, `ci`, `infra`.",
        (Shipped::En, "invariants") => "What must not change: behavior, public interfaces, data, outputs. A change that alters any of them is a feature, not a chore.",
        (Shipped::En, "checks") => "How you show the invariants held: the suite that is run again at the chore commit, and anything else you compare.",
        (Shipped::Ko, "category") => "`refactor`, `dependency`, `build`, `ci`, `infra` 중 정확히 하나.",
        (Shipped::Ko, "invariants") => "바뀌면 안 되는 것: 동작, 공개 인터페이스, 데이터, 출력. 이 중 하나라도 바꾸면 chore가 아니라 기능 변경이다.",
        (Shipped::Ko, "checks") => "불변 조건이 지켜졌음을 보이는 방법: chore 커밋에서 다시 돌리는 스위트와 그 밖에 비교하는 것.",
        (Shipped::Ko, "intent") => "어떤 문제를, 누구를 위해, 왜 지금 푸는지. 한 문장에서 세 문장.",
        (Shipped::Ko, "outcome") => "출시되면 사용자 입장에서 눈에 보이는 상태.",
        (Shipped::Ko, "flow") => "번호 목록. 각 항목은 id로 시작하고 \"본다 → 한다 → 된다\"로 쓴다: `1. F1 사용자는 … 을 보고 → … 을 하면 → … 이 된다`.",
        (Shipped::Ko, "detail") => "흐름의 각 단계마다 `### F1 …` 소제목을 같은 순서로 하나씩 두고, 그 단계의 규칙·상태·예외를 쓴다.",
        (Shipped::Ko, "acceptance") => "단계마다 관찰되어야 할 것과 다루는 실패 경로를 쓴다. 시나리오 id는 여기에 쓰지 않는다.",
        (Shipped::Ko, "questions") => "정해지지 않은 것과 가정한 것. 정책을 바꿀 수 있는 내용은 여기에 쓴다. 없으면 `없음.`이라고 쓴다.",
        (Shipped::Ko, "out-of-scope") => "이 변경이 일부러 하지 않는 것. 없으면 `없음.`이라고 쓴다.",
        _ => "",
    }
}

/// The brief `canon change new` writes for `title`: the PRD brief (0.19 D1)
/// or the chore brief (0.19 D5), as empty sections (module doc).
pub fn brief(kind: ChangeKind, title: &str) -> String {
    let mut out = String::new();
    if kind == ChangeKind::Chore {
        out.push_str(CHORE_FRONTMATTER);
    }
    out.push_str(&format!("# {title}\n"));
    for section in kind.sections() {
        out.push_str(&format!("\n## {}\n", section.heading));
        if section.key == "intent" {
            out.push_str(&format!("\n{title}\n"));
        }
    }
    out
}

/// The heading `canon brief check` and its hints name a section by.
pub fn heading_of(key: &str) -> &'static str {
    FEATURE_SECTIONS.iter().chain(CHORE_SECTIONS.iter()).find(|s| s.key == key).map_or("", |s| s.heading)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_change_kind_is_refused_in_its_old_words() {
        assert_eq!(parse_kind("epic").unwrap_err().to_string(), "unknown change kind `epic`; expected `feature` or `chore`");
        assert_eq!(parse_kind("chore").unwrap(), ChangeKind::Chore);
    }

    #[test]
    fn a_new_brief_is_headings_and_the_title_only() {
        let feature = brief(ChangeKind::Feature, "Add login");
        assert_eq!(feature, "# Add login\n\n## Intent\n\nAdd login\n\n## Desired outcome\n\n## User flow\n\n## Step detail\n\n## Acceptance\n\n## Questions and assumptions\n\n## Out of scope\n");
        let chore = brief(ChangeKind::Chore, "Tidy");
        assert!(chore.starts_with("---\nkind: chore\n---\n\n# Tidy\n\n## Intent\n\nTidy\n\n## Category\n"), "{chore}");
        assert!(!feature.contains("<!--") && !chore.contains("<!--"));
    }

    #[test]
    fn every_section_of_both_kinds_has_guidance_in_both_languages() {
        for language in [Language { tag: "en".to_string() }, Language { tag: "ko".to_string() }] {
            for def in FEATURE_SECTIONS.iter().chain(CHORE_SECTIONS.iter()) {
                assert!(!guidance(&language, def.key).is_empty(), "{} in {}", def.key, language.tag);
            }
        }
    }
}
