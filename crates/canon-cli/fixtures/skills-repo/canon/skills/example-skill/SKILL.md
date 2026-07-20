---
name: example-skill
description: Fixture skill exercising `canon skills install`'s materializer (S0 task 6.2) — not a real companion skill.
---

# example-skill

This is a fixture `SKILL.md` used only by `canon-cli`'s
`tests/skills_install.rs` integration test. It asserts that `canon skills
install` materializes this file verbatim into `.claude/skills/example-skill/
SKILL.md` and flattened into `.codex/skills/example-skill.md`, and that the
`canon/skills/.install-lock.json` lock records a content hash and version 1
with no `generatedAt` field.
