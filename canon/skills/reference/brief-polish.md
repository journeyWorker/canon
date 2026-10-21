# Brief pass: polish

Make the brief something a human reads once and understands. Run it whenever
`canon brief check` reports `prose.*` advisories; `canon status` and
`meta.next` name it before the human stop. It never blocks approval, and it
is not finished by silencing rules: it is finished when the brief reads
plainly and each advisory is gone or waived with a reason.

## Mode first, style second

Fix what the document is before touching a word. Style rules cannot rescue a
brief that says the wrong thing.

- **Intent** states the current behavior: what a user sees and does today, and the gap. Not the history of how it got there.
- **Desired outcome** states what is true afterwards, as behavior a user can observe.
- **Interfaces** are described by behavior: inputs, outputs, errors, limits. Not by file, line or function.
- **Acceptance** gives, per step, a check a reader can run without asking the author, and one failure path naming the condition and what the user sees.
- **Provenance** goes on one line, `Source: <where this came from>`. That line is the only place a path or an identifier belongs.
- **No** paths, line numbers, identifiers, dates or decision history in the prose. A fact stated in one section is not restated in another; point to the section instead.

## Scope and language

The sections judged are all of them except flow ids and subheadings: the
normative sections (Desired outcome, User flow, Step detail, Acceptance; for a
chore, Invariants and Checks) and the context sections (Intent, Questions and
assumptions). Length and key-sentence rules stay on the normative sections.

`canon brief check` detects the language of the brief's text from its
letters: Hangul is `ko`, Latin is `en`. Other scripts get the structure rules
only. The output prints one `language:` line with its source (`detected` or
`configured`); the JSON carries `language` and `languageSource`, plus
`languageNote` when `language:` in `canon.yaml` and the detected language
disagree. A `language:` in `canon.yaml` overrides detection. If the note says
the configured language is wrong for this text, tell the human; do not
translate the brief to fit the setting.

## The rules

Each finding carries the matched text or the measured value and the budget.
Rewrite to say the concrete thing; swapping in a near synonym only moves the
finding.

### `prose.evaluative`

Catches judgment words that stand for a fact: "seamless", "robust", "자연스럽게", "깔끔하게".
Fix: name what the user observes.

- en: Before: "Export feels fast and reliable." After: "Export of a 10,000-row table finishes in under five seconds; a failed export keeps the previous file."
- ko: 전: "내보내기가 빠르고 안정적으로 동작한다." 후: "행 10,000개 표의 내보내기는 5초 안에 끝나고, 실패하면 이전 파일을 그대로 둔다."

### `prose.connective`

Catches linking words that decorate ("moreover", "furthermore", "또한", "아울러"). Fix: delete them; keep a connective only when it carries a condition ("if", "otherwise", "unless").

- en: Before: "Moreover, the list is sorted by date." After: "The list is sorted by date, newest first."
- ko: 전: "또한 목록은 날짜순으로 정렬된다." 후: "목록은 날짜 내림차순으로 정렬한다."

### `prose.contrast`

Catches rhetorical contrast ("not just X but Y", "X가 아니라 Y다" used as flourish). Fix: state the Y alone.

- en: Before: "This is not only a rename but a new way to find notes." After: "Renaming a note updates its search entry at once."
- ko: 전: "단순한 이름 변경이 아니라 노트를 찾는 새로운 방식이다." 후: "노트 이름을 바꾸면 검색 항목이 바로 바뀐다."

### `prose.colloquial`

Catches spoken register in written text (ko): fillers and sentence-final spoken forms. Fix: the plain written form of the same statement.

- ko: 전: "이름이 겹치면 그냥 뒤에 숫자를 붙여 주면 돼요." 후: "이름이 겹치면 뒤에 숫자를 붙인다."
- en: Before: "Just tack a number on if the name clashes." After: "If the name already exists, append a number."

### `prose.degree`

Catches degree words that cannot be checked ("very", "quite", "매우", "상당히"). Fix: the number or the condition.

- en: Before: "The page loads very quickly." After: "The page is interactive within one second on the reference laptop."
- ko: 전: "화면이 매우 빠르게 열린다." 후: "화면은 기준 노트북에서 1초 안에 입력을 받는다."

### `prose.sentence-length`

Catches a sentence over the budget. Fix: one statement per sentence; split at the condition, the action and the result. A list of three or more parallel items becomes a list.

- en: Before: "When a user who has not signed in opens a shared link that has expired, the page, which also logs the attempt, shows a notice and a sign-in button." After: "A signed-out user opens an expired shared link. The page shows the notice 'Link expired' and a sign-in button. The attempt is logged."
- ko: 전: "로그인하지 않은 사용자가 만료된 공유 링크를 열면 시도를 기록하면서 안내 문구와 로그인 버튼을 함께 보여 준다." 후: "로그인하지 않은 사용자가 만료된 공유 링크를 연다. 화면에 '링크가 만료되었습니다' 문구와 로그인 버튼이 뜬다. 시도는 기록한다."

### `prose.paragraph-length`

Catches a paragraph over the budget. Fix: keep the sentences that state a condition, an action, a result or a number (the compression pass in `role-planner.md`); move the rest to its own section or delete it.

### `prose.key-sentence`

A normative section carries exactly one key sentence marked `==**…**==`: the one line a reader must not miss. Fix: pick the single rule or result the section turns on, and mark only it.

- en: `==**A deleted note stays recoverable for 30 days.**==`
- ko: `==**삭제한 노트는 30일 동안 복구할 수 있다.**==`

### `prose.code-citation`

Catches `path:line` citations, file paths and backticked identifiers inside sentences. Fix: say what the code does as behavior; put provenance on the one `Source:` line, which is exempt.

- en: Before: "`NoteStore.save` in `src/store/notes.ts:212` drops the tag list." After: "Saving a note drops its tags. Source: store module, save path."
- ko: 전: "`src/store/notes.ts:212`의 `NoteStore.save`가 태그를 버린다." 후: "노트를 저장하면 태그가 사라진다. Source: 저장소 모듈의 저장 경로."

A quoted command, flag or error code the user types or reads is a term, not a citation: keep it and waive if the rule fires on it.

### `prose.parenthetical`

Catches parenthetical asides above the section's density. Fix: if the aside is a condition or a result, make it its own sentence; if it is background, delete it; if it is a doubt, move it to Questions and assumptions.

- en: Before: "Sync runs (every hour, or on open (if online)) and retries (twice)." After: "Sync runs every hour and when the app opens while online. A failed sync retries twice."
- ko: 전: "동기화는 (매시간, 앱을 열 때(온라인이면)) 실행되고 (두 번) 재시도한다." 후: "동기화는 매시간, 그리고 온라인 상태에서 앱을 열 때 실행한다. 실패한 동기화는 두 번 재시도한다."

### `prose.decision-history`

Catches dates and "decided / changed on …" notes in normative or Intent text. Fix: delete the history; state the decision as present behavior. A decision that is still open belongs in Questions and assumptions. The brief describes the product as it will be, not the discussion that led there.

- en: Before: "On 3 March the team changed the default view to a grid." After: "The default view is a grid."
- ko: 전: "3월 3일 회의에서 기본 보기를 격자로 바꾸기로 했다." 후: "기본 보기는 격자다."

### `prose.ai-vocabulary`

Catches a short closed list of metaphors and stock words ("leverage", "landscape", "journey", "unlock", "원활한", "견고한", "한 단계 끌어올리다"). Fix: the plain verb and the object.

- en: Before: "This unlocks a smoother note-taking journey." After: "A user can create a note in one tap from the lock screen."
- ko: 전: "노트 작성 여정을 한층 원활하게 만든다." 후: "잠금 화면에서 한 번 눌러 노트를 만든다."

### `prose.ko-ending`

Catches Korean sentences that do not end in the plain declarative `~다` (polite `~요`/`~니다`-style, colloquial, imperative or hortative). Flow steps and list fragments are exempt. Fix: end each statement in `~다`.

- ko: 전: "목록이 비어 있으면 안내를 보여 줘요." 후: "목록이 비어 있으면 안내를 보여 준다."

### `prose.length`

Catches a brief or a section over its character budget; the finding states the measured value and the budget. Fix: run the compression pass in `role-planner.md`: delete sentences that restate another section, turn parallel items into a list or table, move assumptions to Questions and assumptions and a not-doing list to Out of scope. A long brief is usually a brief that says several things twice.

## Inspect

- `canon brief check --json`: read the `prose.*` findings, then the sections they sit in. Fix by section, not by rule.
- A finding is a prompt to reread, not a verdict. When the flagged text is the precise term (a state name, a quoted label, a command), keep it and waive that rule in that section:
  `<!-- canon:waive <rule-id> <section-key> "<reason>" -->`
  A waiver is right when the text is a quotation or a fixed term, and wrong when it excuses a sentence you could simply rewrite. One rule, one section, a reason the human can read.

## The loop

1. Fix the mode problems above, then the findings, section by section.
2. Run `canon brief check --change-id <slug> --json`.
3. Repeat until no advisory remains, or each remaining one is waived with a reason.
4. Stop and report to the human, in the brief's language. Do not loop further: a clean run says nothing about whether the brief is good, only that these counts are within budget.

## NEVER

- Never change meaning: no flow step, rule, limit or failure path added, removed or softened while rewording.
- Never rewrite to dodge a rule (a near synonym, a list used to hide a long sentence); say the concrete thing.
- Never touch flow ids or subheadings, and never polish the markers.
- Never translate the brief to match a `language:` setting.
- Never treat a clean run as a claim about who wrote the brief.

## Verify

`canon brief check` no longer lists the `prose.*` findings you rewrote, or lists them as waived with a reason; no blockers appeared.

## Next

Report to the human and `canon brief approve` from their answer.

## Invariants (every brief pass)

- Never invent policy. An edit that could change what the gate demands, who approves, or what ships goes to the brief's Questions and assumptions, not into the text as a decision.
- Explaining writes nothing. Asked what a finding means or what a pass would do, answer in chat and leave the brief untouched.
- No pass requires another as a prerequisite. Run the one the findings or the human point at, in any order.
- A pass closes only the findings it rechecked: rerun `canon brief check` after the edit and report a finding as closed only when it is gone from that run. Never claim a rule you did not rerun.
- Write in the brief's language (detected from the text, or `language:` in `canon.yaml` when set). Edits touch `.canon/changes/<slug>/brief.md` only. A brief edited after approval is not approved: it needs a new human answer and `canon brief approve` again.
