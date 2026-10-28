Feature: authoring brief check
  # canon: {"schema":1,"at":"2026-10-21T16:39:57Z","actor":{"agent_id":"canon-impl-026"}}

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.brief-check.01
  Scenario: A brief is judged in the language its text is written in, and the output says which
    Given a repo whose canon.yaml sets no language and a brief written in Korean prose with English product terms, code spans and a Source line
    When canon brief check runs, in text and with --json
    Then the brief is judged with the Korean rules and the text output prints one line, "language: ko (detected)"
    And the JSON carries language "ko" and languageSource "detected" and no languageNote, an English brief reads "en (detected)", and canon brief show reads the same resolution

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.02
  Scenario: A configured language overrides the detected one and a disagreeing text is noted in one line
    Given a repo whose canon.yaml sets a language, and a brief whose prose reads as another language
    When canon brief check runs
    Then the configured language decides, the findings are judged with its rules, and the text output prints one line, "language: en (configured; the text reads as ko)"
    And the JSON carries languageSource "configured" and a languageNote, and a configured language the text agrees with carries no note
    And the configured language decides which phrase lists run, but a date is language-independent: with en configured a Korean sentence with a date draws one prose.decision-history finding for the date, and a Korean decision phrase with no date draws none

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.brief-check.03
  Scenario: A section written in another script is judged in its own language
    Given a brief in one language with one section written in another script, with no language set
    When canon brief check runs
    Then the section is judged in its own language and the other sections in the brief's
    And when the repo sets a language, every section is judged in it

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.brief-check.04
  Scenario: Japanese and Chinese text is named and gets the structure rules and no prose rules
    Given a brief written in Japanese or in Chinese, and one of its sections removed
    When canon brief check runs
    Then the language is named "ja" or "zh", detected, and no prose rule reads the text
    And the structure rules still apply: the missing section is a blocker and the command exits 1

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.brief-check.05
  Scenario: Intent and Questions are judged with the style rules, and the length and key-sentence rules stay on normative sections
    Given a brief whose Intent and Questions hold evaluative, connective and degree words, a code citation, a dated decision note and a long sentence, and an out of scope section with the same
    When canon brief check runs
    Then Intent and Questions get the phrase rules and the new rules, and a decision note is flagged in Intent but not in a question
    And no sentence-length, paragraph-length or key-sentence finding is reported for either, and out of scope is not judged

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.06
  Scenario: A code citation is flagged with what it matched, and a Source line is exempt
    Given a brief citing a file with a line, a code identifier in a span, a plain file path, and the same file on a Source line
    When canon brief check runs
    Then each citation in a sentence is one prose.code-citation finding whose hint names what matched and says to put provenance on one Source line
    And a Source line inside a judged section, a quoted UI label, a command, a slash between words, a product name such as Next.js or iPhone (App.js is a file and is flagged) and a src/dst pair are not flagged

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.07
  Scenario: Parenthetical asides above the density are flagged with their count and threshold
    Given a brief with a section carrying three or more parenthetical asides at more than four per 1000 characters
    When canon brief check runs
    Then one prose.parenthetical finding names the section, its first aside, the count of asides and characters, and the threshold
    And fewer asides, or labels such as (F1) and (30초), report nothing

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.08
  Scenario: Every calendar date and a closed list of decision phrases in normative or Intent text is flagged as decision history
    Given a brief with ISO, Korean and English dates, undated decision phrases (결정했다, 기로 했다, we decided), behavior sentences that use a meeting, an approval or a decision field (회의에서 나가면, approved by a manager, decision on the request, decided to leave), dates next to 부터, 이후 or after, a quoted label next to a date, and a decision note in a question
    When canon brief check runs
    Then each such sentence is one prose.decision-history finding, naming the matched date or phrase, in normative sections, Intent and table cells but not in a question
    And there is no boundary or verb gating, so a date that is part of the behavior (an effective date) or a quoted label is a finding too, and the hint says to keep it and waive it with a reason
    And a sentence with a date and a phrase is one finding, a waiver clears it, and sentences with no date and no listed phrase (a conditional 합의하면, the behavior sentences above, "agreed to the terms", 결정 버튼) report nothing

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.09
  Scenario: Stock AI vocabulary is named from a closed list in each language
    Given a brief with a stock AI word or metaphor from the closed list, in English and in Korean
    When canon brief check runs
    Then one prose.ai-vocabulary finding per sentence names the matched word, and plain wording reports nothing

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.10
  Scenario: A Korean sentence that does not end in the plain declarative is flagged, with fragments and labels exempt
    Given a Korean brief with polite (ㅂ니다, ㅂ시다), 십시오 and plain imperative (해라, 하라, 두라, 넣으라, 남겨라, 줘라) sentences in list items and paragraphs, a colloquial ~세요 sentence, noun fragments (참가자, 후보자, 상품 하자, 선두 주자), a label item, unlabeled noun fragments ending in 라 after a final consonant (달라, 갈라) and flow steps in polite endings
    When canon brief check runs
    Then each polite, 십시오 or imperative sentence (no final consonant before 라) is a prose.ko-ending finding naming the ending, list items included, and the open-syllable nouns 보라, 카메라 and 오페라 are known false positives to waive with match, and a plain word that ends in 니다 (아니다, 지니다) is not polite
    And the ~세요 sentence is prose.colloquial's only, and fragments, a label: item, flow steps, questions and none answers report nothing
    And a plain hortative ~자 is flagged outside list items by the generic rule and is not detected in a list item, where it is the noun suffix of 참가자 and 하자, and English text never gets the rule

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.11
  Scenario: A section or brief over its character budget is flagged with the measured value and the budget
    Given a Korean brief with an Intent over its section budget, and another with the sum of its sections over the brief budget
    When canon brief check runs
    Then prose.length names the section with the measured characters and the budget, and the whole-brief finding has no section and names its budget
    And out of scope has no budget of its own, and an English brief over the Korean budget but under the English one is flagged only when ko is configured

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.brief-check.12
  Scenario: A frontmatter waiver clears each new rule in its section and a whole-brief finding with the key brief
    Given a bad Korean brief and a frontmatter waive entry for each new rule and section it shows, with the key brief for the whole-brief length
    When canon brief check runs
    Then every new finding is waived, the output lists each waiver as an advisory, and a waiver for another section clears nothing
    And a waiver with match="<text>" clears only the rule's findings in that section whose matched text (the finding's matched field) or excerpt contains it, never the hint's words (an effective date is waived and the dated decision note beside it stays), one that fits nothing waives nothing, and an entry with an empty reason or match, an unknown rule id or an unknown key is brief.waiver-invalid, while a reason that merely mentions match= is a plain waiver

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.brief-check.13
  Scenario: Good briefs stay free of the new rules and bad briefs show every mechanism
    Given two good Korean and two good English fixture briefs, and three bad Korean fixture briefs written in the style the rules were measured on
    When canon brief check runs on each
    Then the good briefs report no finding at all
    And each bad brief reports a finding for code citations, asides, decision history, stock vocabulary and Korean endings, and for length

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.brief-check.14
  Scenario: canon status counts the findings of the detected language
    Given a repo with no language set, a subject adopting a change whose brief is written in Korean with prose advisories
    When canon status runs
    Then the brief line counts the same advisories canon brief check reports, judged in the detected language

  # canon: {"schema":1,"at":"2026-10-21T17:10:01Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.brief-check.15
  Scenario: A markdown table is not measured as a sentence
    Given a Korean detail section holding a table of six rows, one row longer than the sentence-length threshold
    When canon brief check runs
    Then no sentence-length, paragraph-length, parenthetical, phrase or ending finding reads the table rows, while its cells are read for code citations, decision history and stock vocabulary
    And prose.length still counts the table's characters

  # canon: {"schema":1,"at":"2026-10-23T11:48:11Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.16
  Scenario: A brief with blockers exits 1 and keeps its report, and a check that is refused exits 2
    Given a change whose brief has a structure blocker, and a slug that names no change
    When canon brief check runs on each, with --json
    Then the brief with the blocker exits 1 with ok false and error code BRIEF_HAS_BLOCKERS, and data still carries the findings and counts
    And the same brief with its blocker fixed exits 0 even while advisories remain
    And the unknown slug exits 2 with BRIEF_NOT_FOUND and no report

  # canon: {"schema":1,"at":"2026-10-27T12:04:37Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:happy
  @flow:canon-0-31-brief-format.F1
  @authoring.brief-check.17
  Scenario: A brief of English headings with no comments passes brief check, and every reader finds the same sections
    Given a brief with a frontmatter-free title, the seven English headings and no HTML comment, in English and in Korean prose, written for a change
    When canon brief check, brief show, status and the dashboard brief page read it
    Then every reader finds the same seven sections with the key and role the heading names, brief check exits 0, and the flow steps F1 and F2 read the same as in a 0.30 brief
    And the Intent summary of a plan import and a chore's review and ship read the new brief the same way

  # canon: {"schema":1,"at":"2026-10-27T12:04:38Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-31-brief-format.F1
  @authoring.brief-check.18
  Scenario: A misspelt or differently cased heading is named with the nearest heading, and a deleted or moved section is reported
    Given a brief whose Intent heading is written `## Intnet`, another whose heading differs only in case, one with a deleted section and one with two sections swapped
    When canon brief check runs on each
    Then the first two report brief.section-missing for the section and name the nearest heading, `Intent` or the right case, in the hint
    And the deleted section is brief.section-missing, the swap is brief.section-order, and a `##` line that is no known heading stays body text of the section before it
    And a brief with a shipped Korean heading and no markers is old-format and gets no near-heading hint

  # canon: {"schema":1,"at":"2026-10-27T12:04:40Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:edge
  @flow:canon-0-31-brief-format.F1
  @authoring.brief-check.19
  Scenario: The language setting changes the prose rules and never the headings, and a note with one known heading is a pre-PRD brief
    Given a Korean brief and the same brief with language set to en, and a note with exactly one known heading
    When canon brief check runs on each
    Then the headings are judged the same whatever the language, only the prose rules and the guidance language change
    And a heading inside a code fence is no heading, and the note with one known heading is read as a pre-PRD brief, brief.legacy, not as a pile of missing sections

  # canon: {"schema":1,"at":"2026-10-27T12:04:41Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:happy
  @flow:canon-0-31-brief-format.F2
  @authoring.brief-check.20
  Scenario: A frontmatter waiver clears one rule in one section, is listed, and adding one after the approval is a change after the approval
    Given a brief whose frontmatter waive list holds a rule, a section, a reason and a match
    When canon brief check runs, and the brief is approved and then a waiver entry is added
    Then the entry clears that rule in that section, or only the findings containing its match, and the output lists the waiver with its rule severity and whether it applied
    And the frontmatter is not prose: no sentence rule, length count or language detection reads it, and the reported lines are the file's real lines
    And the approval digest covers the frontmatter, so the added entry reads as BRIEF_CHANGED_SINCE_APPROVAL

  # canon: {"schema":1,"at":"2026-10-27T12:04:42Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-31-brief-format.F2
  @authoring.brief-check.21
  Scenario: A waiver with an empty reason or unknown rule or key, and a frontmatter that does not read, are blockers that waive nothing
    Given waiver entries with an empty reason, an unknown rule id, a missing section or an unknown key, a frontmatter that is not YAML, one with an unknown top-level key, and an entry for a section key canon does not know
    When canon brief check runs on each
    Then each invalid entry is brief.waiver-invalid and waives nothing, the broken or unknown-key frontmatter is brief.frontmatter-invalid and no waiver of it is read
    And the entry for an unknown section key is no blocker and is listed as a waiver that matches nothing

  # canon: {"schema":1,"at":"2026-10-27T12:04:43Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:edge
  @flow:canon-0-31-brief-format.F2
  @authoring.brief-check.22
  Scenario: kind chore in the frontmatter makes a chore brief and its absence a feature brief
    Given one brief with `kind: chore` in its frontmatter and one without
    When canon brief check, the chore review and the ship check read each
    Then the first is judged by the chore rule set (intent, category, invariants, checks, out-of-scope) and the second as a feature brief
    And a chore brief that moved to the frontmatter still reads as a chore in the chore review and the ship check

  # canon: {"schema":1,"at":"2026-10-27T12:04:45Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:happy
  @flow:canon-0-31-brief-format.F3
  @authoring.brief-check.23
  Scenario: brief show prints each empty section's guidance in the brief's language and check reports each empty section
    Given a brief written by canon change new, whose sections are empty, in a repo with language ko and in one without
    When canon brief check and canon brief show run
    Then brief check reports brief.placeholder for each empty section with that section's guidance in the hint, and Intent holding the title is not empty
    And brief show lists the guidance of each empty section in Korean for ko and in English otherwise, in text and in JSON emptySections

  # canon: {"schema":1,"at":"2026-10-27T12:04:46Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-31-brief-format.F3
  @authoring.brief-check.24
  Scenario: A section filled with a leftover TODO is still a placeholder blocker
    Given a brief whose empty sections are filled but one still says TODO
    When canon brief check runs
    Then the section with TODO keeps its brief.placeholder blocker and the guidance text is not a placeholder

  # canon: {"schema":1,"at":"2026-10-27T12:04:47Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-31-brief-format.F4
  @authoring.brief-check.25
  Scenario: A 0.30 brief gets one old-format blocker and its waiver comments are not read
    Given a brief in the 0.30 format with canon:section comments and a canon:waive comment
    When canon brief check runs
    Then it reports one blocker, brief.old-format, naming canon migrate, no section rule is applied and the waiver comment is not read
    And brief show says old-format and its first next step is migrate
