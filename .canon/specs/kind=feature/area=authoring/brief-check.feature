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
    And the JSON carries language "ko" and languageSource "detected" and no languageNote, an English brief reads "en (detected)", and bare canon brief reads the same resolution

  # canon: {"schema":1,"at":"2026-10-21T16:40:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.brief-check.02
  Scenario: A configured language overrides the detected one and a disagreeing text is noted in one line
    Given a repo whose canon.yaml sets a language, and a brief whose prose reads as another language
    When canon brief check runs
    Then the configured language decides, the findings are judged with its rules, and the text output prints one line, "language: en (configured; the text reads as ko)"
    And the JSON carries languageSource "configured" and a languageNote, and a configured language the text agrees with carries no note

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
    And the structure rules still apply: the missing section is a blocker and the command exits 2

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
    And a Source line inside a judged section, a quoted UI label, a command, a slash between words, a product name such as Next.js or iPhone and a src/dst pair are not flagged

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
  Scenario: Dates and decision notes in normative or Intent text are flagged as decision history
    Given a brief with a dated decision note, an undated one ("we decided", 결정했다) and behavior sentences that only mention agreeing, a meeting or a conditional (합의하면, 변경했으면) in its normative and Intent text, and a decision note in a question
    When canon brief check runs
    Then each note is a prose.decision-history finding naming the matched date or phrase, except the question's
    And a sentence that only says someone agreed to terms, sat in a meeting, changed something on the server or releases on May 3, or a Korean conditional, is not flagged

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
  Scenario: A Korean sentence that does not end in the plain declarative is flagged, with fragments and flow steps exempt
    Given a Korean brief with polite, hortative and imperative sentences, a colloquial ~세요 sentence, noun fragments in list items (참가자, 하자, 보라) and flow steps in polite endings
    When canon brief check runs
    Then each polite, hortative and imperative sentence is a prose.ko-ending finding naming the ending, and the ~세요 sentence is prose.colloquial's only
    And fragments, flow steps, questions and none answers report nothing, and English text never gets the rule
    And a plain form that ends in 니다 (아니다, 지니다, 다닌다) is not polite, while 합니다, 입니다 and 없습니다 are flagged

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
  Scenario: A waiver comment clears each new rule in its section and a whole-brief finding with the key brief
    Given a bad Korean brief and a waiver comment for each new rule and section it shows, with the key brief for the whole-brief length
    When canon brief check runs
    Then every new finding is waived, the output lists each waiver as an advisory, and a waiver for another section clears nothing

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
    Then no sentence-length, paragraph-length, parenthetical, phrase or ending finding reads the table rows
    And prose.length still counts the table's characters
