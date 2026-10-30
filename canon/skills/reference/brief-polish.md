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

Rules read the brief by section role. Intent and Questions and assumptions
get the phrase rules (`evaluative`, `connective`, `contrast`, `colloquial`,
`degree`) and the five newer ones; `decision-history` reads normative text
and Intent, not Questions. `sentence-length`, `paragraph-length` and
`key-sentence` read the normative sections only (Desired outcome, User flow,
Step detail, Acceptance; for a chore, Invariants and Checks). Out of scope
is not judged. Headings, code spans, fenced code, URLs, code-like tokens and
the `Source:` and `Approved:` lines are not counted as prose.

`canon brief check` detects the language of the text from the script of its
prose letters: Hangul is `ko`, Latin is `en`, kana or Han text is named
(`ja`, `zh`) and gets the structure rules only. A section in another script
than the rest is judged in its own language, and a section with too few
letters to tell inherits the brief's. The output prints one line,
`language: ko (detected)`; the JSON carries `language` and `languageSource`
(`detected` or `configured`). `language:` in `canon.yaml` is an override: when
set it decides, and `brief check` adds `languageNote` (one line in the human
output) if the text looks like another language. Report that note to the
human; do not translate the brief to fit the setting.

## The rules

Each finding carries the matched text or the measured value, and a hint.
Fix the sentence by saying the concrete thing; swapping in a near synonym only
moves the finding. The examples are invented.

### `prose.evaluative`

Catches a verdict standing in for a fact (en "it is important", "it is
crucial", "it is essential"; ko "중요하", "핵심이", "필요가 있", "바람직하",
"의미가 있"). Fix: state the fact the verdict stood for.

- en. Before: "It is important that export keeps the old file." After: "A failed export keeps the previous file."
- ko. 전: "내보내기가 실패해도 이전 파일을 유지하는 것이 중요하다." 후: "내보내기가 실패하면 이전 파일을 그대로 둔다."

### `prose.connective`

Catches a sentence that opens with a decorative connective (en "therefore",
"in conclusion"; ko "따라서", "결론적으로", "이처럼", "사실", "참고로"). Fix:
delete it; the order of the sentences already carries the link. Keep the
link only as a condition ("if", "unless").

- en. Before: "Therefore the list is sorted by date." After: "The list is sorted by date, newest first."
- ko. 전: "따라서 목록은 날짜순으로 정렬한다." 후: "목록은 날짜 내림차순으로 정렬한다."

### `prose.contrast`

Catches the rhetorical "not X but Y" (ko "A가 아니라 B"). Fix: state Y alone.

- en. Before: "This is not a rename but a new way to find notes." After: "Renaming a note updates its search entry at once."
- ko. 전: "이것은 이름 변경이 아니라 노트를 찾는 새로운 방식이다." 후: "노트 이름을 바꾸면 검색 항목이 바로 바뀐다."

### `prose.colloquial`

Korean only. Catches a spoken ending on a written sentence (~세요, ~어요,
~아요, ~해요, ~에요, ~네요, ~죠, …). Fix: the plain written form (`~다`).

- ko. 전: "이름이 겹치면 뒤에 숫자를 붙여 주세요." 후: "이름이 겹치면 뒤에 숫자를 붙인다."

### `prose.degree`

Catches a degree word nobody can check (en "very", "really", "extremely";
ko "매우", "겨우", "무려", "고작"). Fix: the number or the condition.

- en. Before: "The page loads very quickly." After: "The page accepts input within one second on the reference laptop."
- ko. 전: "화면이 매우 빠르게 열린다." 후: "화면은 기준 노트북에서 1초 안에 입력을 받는다."

### `prose.sentence-length`

Normative sections. Catches a sentence over 30 words (en) or 80 characters
(ko). Fix: one statement per sentence, split at the condition, the action
and the result.

- en. Before: "When a user who has not signed in opens a shared link that has already expired, the page, which also logs the attempt, shows a notice and a sign-in button and offers a way back to the list." After: "A signed-out user opens an expired shared link. The page shows the notice 'Link expired' and a sign-in button. The attempt is logged."
- ko. 전: "로그인하지 않은 사용자가 만료된 공유 링크를 열면 시도를 기록하면서 안내 문구와 로그인 버튼을 함께 보여 주고 이후 목록 화면으로 돌아가는 경로도 제공한다." 후: "로그인하지 않은 사용자가 만료된 공유 링크를 연다. 화면에 '링크가 만료되었습니다' 문구와 로그인 버튼이 뜬다. 시도는 기록한다."

### `prose.paragraph-length`

Normative sections. Catches a paragraph of more than five sentences. Fix: the
compression pass in `canon skills show role-planner`: keep the sentences that state a
condition, an action, a result or a number; make three or more parallel
items a list; delete or move the rest.

### `prose.key-sentence`

Normative sections. Each carries exactly one key sentence marked `==**…**==`:
the line a reader must not miss. Fix: mark the single rule or result the
section turns on, and nothing else.

- en: `==**A deleted note stays recoverable for 30 days.**==`
- ko: `==**삭제한 노트는 30일 동안 복구할 수 있다.**==`

### `prose.code-citation`

Catches code references in sentences: paths with a known extension,
`path:line`, paths of two or more segments (`a/b/c`, `./x`), and backticked
code-shaped identifiers (`a::b`, `f()`, `snake_case`, `obj.member`,
`camelCase`). It does not catch UI labels, product names (`Next.js`, `iPhone`),
flags, commands, dates, routes or URLs, or the `Source:` line. Fix: say what the code does as behavior;
provenance goes on the one `Source:` line.

- en. Before: "`NoteStore.save` in `src/store/notes.ts:212` drops the tag list." After: "Saving a note drops its tags." and, once, `Source: the note store's save path.`
- ko. 전: "`src/store/notes.ts:212`의 `NoteStore.save`가 태그를 버린다." 후: "노트를 저장하면 태그가 사라진다." 그리고 한 번 `Source: 노트 저장소의 저장 경로.`

### `prose.parenthetical`

Catches a section with three or more parenthetical asides at a density over
four per 1000 characters. Labels like `(F1)` or `(30초)` do not count. Fix:
if the aside is a condition or a result, make it its own sentence; if it is
background, delete it; if it is a doubt, move it to Questions and assumptions.

- en. Before: "Sync runs (every hour), on open (when online), and retries (at most twice)." After: "Sync runs every hour and when the app opens while online. A failed sync retries twice."
- ko. 전: "동기화는 (한 시간마다), 앱을 열 때 (온라인이면 언제나) 실행되고 (최대 두 번) 재시도한다." 후: "동기화는 매시간, 그리고 온라인 상태에서 앱을 열 때 실행한다. 실패한 동기화는 두 번 재시도한다."

### `prose.decision-history`

Normative text, Intent and table cells. One finding per sentence that holds
a calendar date, in any language (`2026-10-21`, `2026년 10월`, `10월 21일`,
`Oct 21`, `21 October`, `October 2026`), or, in Korean text, one of
`결정했다`, `결정했고`, `결정하여`, `결정됐다`, `결정되었다`, `정해졌다`, `합의했다`,
`확정했다`, `~기로 했다`, `논의 결과`, `운영자 결정`, or, in English text, "we
decided", "we agreed", "was decided", "it was agreed". There is no gating: a
date next to 부터 or after is a finding too. Fix: delete the history and write
the decision as present behavior; a decision still open goes to Questions and
assumptions. When the date is part of the behavior (an effective date, a
deadline) or a quoted label, keep it and waive it with `match=` set to the
date (it compares the date or phrase the finding matched and the excerpt, nothing else), so the waiver clears that date only and every other note in the section
still draws a finding. Never waive the rule for the whole section.

- en. Before: "On 3 March the team agreed on a grid as the default view." After: "The default view is a grid."
- ko. 전: "3월 3일 회의에서 기본 보기를 격자로 하기로 했다." 후: "기본 보기는 격자다."
- waive one date: frontmatter entry `rule: prose.decision-history`, `section: detail`, `reason: "연체 요금은 2026-12-01부터 적용되는 동작이다"`, `match: "2026-12-01"`

What it misses: a decision written in other words ("회의에서 정한 대로" is not
listed on purpose, it reads as behavior). Over-reports: a date that is
behavior, and the closed-list phrases in behavior text; waive those with `match=`.

### `prose.ai-vocabulary`

Catches a short closed list of stock words and metaphors (en "seamless",
"robust", "leverage", "streamline", "empower", "north star", "game-changer",
…; ko "원활", "견고", "시너지", "패러다임", "북극성", "토대", …). Fix: the plain
verb and its object.

- en. Before: "This leverages the sync engine for a seamless experience." After: "A note edited on one device appears on the others within a minute."
- ko. 전: "동기화 엔진으로 원활한 사용 경험을 제공한다." 후: "한 기기에서 고친 노트가 1분 안에 다른 기기에 나타난다."

### `prose.ko-ending`

Korean only. Flagged everywhere, list items included: polite `~ㅂ니다`,
`~ㅂ니까`, `~ㅂ시다` (합니다, 입니다, 없습니다; a plain word like 아니다 is not
polite), `~십시오`, and a plain imperative: a terminated, unlabeled sentence
whose last word ends in 라 after a syllable with no final consonant (하라,
해라, 두라, 오라, 넣으라, 남겨라, 줘라). A syllable with a final consonant
(달라, 갈라) never counts. The `~요` forms in `prose.colloquial`'s list
(`~세요`, `~해요`, …) belong to that rule. Outside list items any terminated
sentence not ending in `~다` is flagged, a plain hortative `~자` included. In a
list item the plain `~자` is not detected: it is the noun suffix in 참가자,
후보자, 하자, 주자, so keep a hortative out of lists yourself. Not flagged:
questions, `없음.`, sentences ending in code or a quote, flow steps, list
fragments without a verb, a `label:` item, and an unterminated list item.

What it over-reports: a class of nouns, often loanwords, that end in an open
syllable plus 라 (보라 'purple', 카메라, 오페라, 인프라) as a terminated list
item; keep the sentence and waive with `match="<the noun>"`. What it misses: the
prohibitive `~말라` (`지우지 말라`: 말 has a final consonant), any consonant-stem
imperative written `~라` (달라 as a verb), and any hortative inside a list.
Fix: end the statement in `~다`.

- ko. 전: "목록이 비어 있으면 안내를 보여 줍니다." 후: "목록이 비어 있으면 안내를 보여 준다."
- ko. 전: "- 재고를 확인해라." 후: "- 재고를 확인한다."

### `prose.length`

Catches a section or the whole brief over its character budget; the finding
states the measured value and the budget. Section budgets (ko / en
characters): Intent 400 / 700, Outcome 500 / 900, Flow 800 / 1500, Detail
2500 / 3500, Acceptance 1000 / 1800, Questions 800 / 1200, Invariants and
Checks 800 / 1500 each (Out of scope has none); whole brief 4000 / 6000. Fix: the
compression pass in `canon skills show role-planner`: delete sentences that restate another
section, turn parallel items into a list or table, move assumptions to
Questions and assumptions and a not-doing list to Out of scope. A brief this
long usually says several things twice. A waiver for the whole-brief budget
uses the section key `brief`.

## Inspect

- `canon brief check --json`: read the `prose.*` findings, then the sections they sit in. Fix by section, not by rule.
- A finding is a prompt to reread, not a verdict. When the flagged text is the precise term (a state name, a quoted label, a command), keep it and waive that rule in that section:
  an entry of the frontmatter `waive` list: `- rule: <rule-id>`, `section: <section-key>`, `reason: "<reason>"`.
  To waive one finding and keep the rest of the rule live in that section, add the text the finding names as `match: "<text>"` (an empty reason or match, an unknown rule id or an unknown key is a blocker).
  A waiver is right when the text is a quotation or a fixed term, and wrong when it excuses a sentence you could simply rewrite. One rule, one section, a reason the human can read.

## The loop

1. Fix the mode problems above, then the findings, section by section.
2. Run `canon brief check <slug> --json`: the findings are in `data.findings[]`.
3. Repeat until no advisory remains, or each remaining one is waived with a reason.
4. Stop and report to the human, in the brief's language. Do not loop further: a clean run says nothing about whether the brief is good, only that these counts are within budget.

## NEVER

- Never change meaning: no flow step, rule, limit or failure path added, removed or softened while rewording.
- Never rewrite to dodge a rule (a near synonym, a list used to hide a long sentence); say the concrete thing.
- Never touch flow ids or subheadings, and never polish the headings.
- Never translate the brief to match a `language:` setting.
- Never treat a clean run as a claim about who wrote the brief.

## Verify

`canon brief check` no longer lists the `prose.*` findings you rewrote, or lists them as waived with a reason; no blockers appeared.

**Done when:** `canon brief check <slug> --json` lists no `prose.*` finding (read `data.findings[]`) except those waived with a reason (`data.waivers[]`), and `data.counts` shows no blocker.

## Next

Show the brief to the human and ask (`canon brief show <slug> --body`, then the harness question tool); record an explicit approval with `brief approve` with `--relayed-via <tool>` (`canon skills show role-planner`).

## Invariants (every brief pass)

- Never invent policy. An edit that could change what the gate demands, who approves, or what ships goes to the brief's Questions and assumptions, not into the text as a decision.
- Explaining writes nothing. Asked what a finding means or what a pass would do, answer in chat and leave the brief untouched.
- No pass requires another as a prerequisite. Run the one the findings or the human point at, in any order.
- A pass closes only the findings it rechecked: rerun `canon brief check` after the edit and report a finding as closed only when it is gone from that run. Never claim a rule you did not rerun.
- Write in the brief's language (detected from the text, or `language:` in `canon.yaml` when set). Edits touch `.canon/changes/<slug>/brief.md` only. A brief edited after approval is not approved: it needs a new human answer, shown and asked again, and a new `canon brief approve` (recorded with `--relayed-via` from that answer).
