//! The PRD brief `canon change new` writes (0.19 D1): Intent → Desired
//! outcome → User flow → Step detail → Acceptance → Questions and
//! assumptions → Out of scope, then the approval comment.
//!
//! Each section heading is preceded by a marker comment,
//! `<!-- canon:section <key> role=<normative|context|question> -->`.
//! Readers find sections by marker, never by heading text, so the
//! headings are in the repo's `language:` ([`crate::language`]). A
//! section starts with a `<!-- canon:hint … -->` comment naming what
//! goes there; `canon brief check` reports a hint that is still in the
//! brief as a placeholder. The approval line is named in a plain comment,
//! which the approval parse never reads, so the file starts unapproved.
//!
//! The Intent section's first paragraph is the change's summary
//! (`Change.summary`): the title.

use crate::language::{Language, Shipped};

/// How a section's text is judged: `normative` sections carry the
/// requirements and get the prose rules; `context` and `question`
/// sections do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Normative,
    Context,
    Question,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Normative => "normative",
            Role::Context => "context",
            Role::Question => "question",
        }
    }

    pub fn parse(raw: &str) -> Option<Role> {
        match raw {
            "normative" => Some(Role::Normative),
            "context" => Some(Role::Context),
            "question" => Some(Role::Question),
            _ => None,
        }
    }
}

/// One PRD section: its marker key and role, in brief order.
pub struct PrdSection {
    pub key: &'static str,
    pub role: Role,
}

/// The PRD sections, in the order a brief carries them.
pub const PRD_SECTIONS: [PrdSection; 7] = [
    PrdSection { key: "intent", role: Role::Context },
    PrdSection { key: "outcome", role: Role::Normative },
    PrdSection { key: "flow", role: Role::Normative },
    PrdSection { key: "detail", role: Role::Normative },
    PrdSection { key: "acceptance", role: Role::Normative },
    PrdSection { key: "questions", role: Role::Question },
    PrdSection { key: "out-of-scope", role: Role::Context },
];

/// The marker comment opening a section.
pub fn marker(key: &str, role: Role) -> String {
    format!("<!-- canon:section {key} role={} -->", role.as_str())
}

/// A section's heading and hint in `language`.
fn heading_and_hint(language: Shipped, key: &str) -> (&'static str, &'static str) {
    match (language, key) {
        (Shipped::En, "intent") => ("Intent", "What problem this solves, for whom, and why now. One to three sentences."),
        (Shipped::En, "outcome") => ("Desired outcome", "What is observably true once this ships, from the user's side."),
        (Shipped::En, "flow") => ("User flow", "A numbered list; each item starts with its id and reads sees → does → becomes: `1. F1 The user sees … → does … → becomes …`."),
        (Shipped::En, "detail") => ("Step detail", "One `### F1 …` subheading per flow step, in the flow's order: rules, states and edge cases of that step."),
        (Shipped::En, "acceptance") => ("Acceptance", "Per step: what must be observable, and which failure path is covered. Do not write scenario ids here."),
        (Shipped::En, "questions") => ("Questions and assumptions", "What is unresolved and what you assumed; anything that could change policy goes here. Write `None.` if nothing."),
        (Shipped::En, "out-of-scope") => ("Out of scope", "What this change deliberately does not do. Write `None.` if nothing."),
        (Shipped::Ko, "intent") => ("의도", "어떤 문제를, 누구를 위해, 왜 지금 푸는지. 한 문장에서 세 문장."),
        (Shipped::Ko, "outcome") => ("원하는 결과", "출시되면 사용자 입장에서 눈에 보이는 상태."),
        (Shipped::Ko, "flow") => ("사용자 흐름", "번호 목록. 각 항목은 id로 시작하고 \"본다 → 한다 → 된다\"로 쓴다: `1. F1 사용자는 … 을 보고 → … 을 하면 → … 이 된다`."),
        (Shipped::Ko, "detail") => ("단계 상세", "흐름의 각 단계마다 `### F1 …` 소제목을 같은 순서로 하나씩 두고, 그 단계의 규칙·상태·예외를 쓴다."),
        (Shipped::Ko, "acceptance") => ("인수 조건", "단계마다 관찰되어야 할 것과 다루는 실패 경로를 쓴다. 시나리오 id는 여기에 쓰지 않는다."),
        (Shipped::Ko, "questions") => ("질문과 가정", "정해지지 않은 것과 가정한 것. 정책을 바꿀 수 있는 내용은 여기에 쓴다. 없으면 `없음.`이라고 쓴다."),
        (Shipped::Ko, "out-of-scope") => ("범위 밖", "이 변경이 일부러 하지 않는 것. 없으면 `없음.`이라고 쓴다."),
        _ => ("", ""),
    }
}

/// The approval comment, in `language`.
fn approval_comment(language: Shipped) -> &'static str {
    match language {
        Shipped::En => "<!-- Once the human answers, and only from that answer, record it on its own line: Approved: <who> <when> — \"<their words>\" -->",
        Shipped::Ko => "<!-- 사람이 답한 뒤, 그 답에서만, 한 줄로 기록한다: Approved: <누가> <언제> — \"<그 사람의 말>\" -->",
    }
}

/// The PRD brief for `title` in `language`.
pub fn prd_brief(title: &str, language: &Language) -> String {
    let shipped = language.template();
    let mut out = format!("# {title}\n");
    for section in &PRD_SECTIONS {
        let (heading, hint) = heading_and_hint(shipped, section.key);
        out.push_str(&format!("\n{}\n## {heading}\n\n", marker(section.key, section.role)));
        if section.key == "intent" {
            out.push_str(&format!("{title}\n\n"));
        }
        out.push_str(&format!("<!-- canon:hint {hint} -->\n"));
    }
    out.push_str(&format!("\n{}\n", approval_comment(shipped)));
    out
}

/// The heading `canon brief check` and its hints name a section by, in
/// `language`.
pub fn heading_of(language: &Language, key: &str) -> &'static str {
    heading_and_hint(language.template(), key).0
}
