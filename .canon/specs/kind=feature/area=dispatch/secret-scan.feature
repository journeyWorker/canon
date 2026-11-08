Feature: Context pack secret scan and repository state
  # canon: {"schema":1,"at":"2026-11-08T06:19:21Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-08T06:29:00Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F1
  @dispatch.secret-scan.01
  Scenario: Lines that only look like secrets, and lines just below a rule's minimum length, become packs
    Given a repo and the plain lines of the brief's table: risk-high, task-list, Akiashi, sk-live-123456, a long run of words after sk- with no digit, ghp_ alone, a certificate header, api_key placeholders, and each rule's line just below its minimum length
    When each is selected into a context-pack add
    Then every one becomes a pack with a sha256 id
    And the scanner finds no span in any of them

  # canon: {"schema":1,"at":"2026-11-08T06:29:08Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F1
  @dispatch.secret-scan.02
  Scenario: Words that merely start like a token are not secrets
    Given the lookalike lines risk-high, RISK-HIGH, task-list, disk-usage, mask-all, ask-user, Akiashi, AKIATSU, sk-live-123456, sk-documentation-for-onboarding-flow, ghp_ and github_pat_ alone, the certificate and public key headers, empty and placeholder api_key values, api_key: string and max_api_key_len = 4
    When the scanner reads each
    Then it finds no span in any of them
    And so a word that merely contains risk-, task- or akia is never refused

  # canon: {"schema":1,"at":"2026-11-08T06:29:16Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F1
  @dispatch.secret-scan.03
  Scenario: No markdown file of the repository holds a secret span
    Given the .md files under canon/, docs/ and .canon/ of this repository
    When the scanner reads each
    Then no file holds a span
    And the walk found more than a hundred files, so a moved directory fails the test

  # canon: {"schema":1,"at":"2026-11-08T06:29:24Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.04
  Scenario: An AWS access key id needs sixteen characters and both boundaries
    Given AKIA followed by sixteen upper-case letters and digits
    When the scanner reads it with different neighbours
    Then it is one span of the AWS access key id kind at the right byte range
    And fifteen or seventeen characters, a letter or underscore before it and lower case are not found, and a hyphen before it still is

  # canon: {"schema":1,"at":"2026-11-08T06:30:13Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.05
  Scenario: A GitHub token needs thirty-six characters after ghp_
    Given ghp_ followed by letters and digits
    When the scanner reads thirty-five, thirty-six and a hundred characters, with a letter before it and with a hyphen in the run
    Then thirty-six or more is one span of the GitHub token kind
    And thirty-five, a left neighbour that is a letter, a bare ghp_ and a run ended by a hyphen are not found

  # canon: {"schema":1,"at":"2026-11-08T06:30:26Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.06
  Scenario: A GitHub fine-grained token takes underscores from twenty-two characters after github_pat_
    Given github_pat_ followed by letters, digits and underscores
    When the scanner reads twenty-one and twenty-two characters, and with a letter before it
    Then twenty-two is one span of the GitHub fine-grained token kind
    And twenty-one, a left neighbour that is a letter and a bare github_pat_ are not found

  # canon: {"schema":1,"at":"2026-11-08T06:30:36Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.07
  Scenario: An API secret key needs twenty characters with a digit, hyphens and underscores included
    Given sk- followed by letters, digits, underscores and hyphens
    When the scanner reads nineteen and twenty characters, the sk-proj- form, sk-live-123456 and a long run of words
    Then twenty characters with a digit is one span of the API secret key kind, and so is the sk-proj- form
    And nineteen characters, sk-live-123456, a run with no digit and a left neighbour that is a letter are not found, nor are risk-, task-, disk-, mask- and ask- words

  # canon: {"schema":1,"at":"2026-11-08T06:30:42Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.08
  Scenario: A private key is found by its header and runs to its end line
    Given a private key header with RSA, OPENSSH, ENCRYPTED, PGP BLOCK or DSA in it, and an end line
    When the scanner reads it
    Then the span runs from the header to the end of the end line, or is the header alone when there is no end line
    And certificate and public key headers, a lower-case header, a header without its closing dashes and a header longer than eighty bytes are not found

  # canon: {"schema":1,"at":"2026-11-08T06:30:48Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.09
  Scenario: A private key whose end line is unreadable is the header alone
    Given a private key header followed by an end line whose name is not readable
    When the scanner reads it
    Then the span is the header alone
    And the end line is not taken as part of the key

  # canon: {"schema":1,"at":"2026-11-08T06:30:55Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.10
  Scenario: A value after a key name is read, with blanks, quotes, an environment prefix and a JSON pair
    Given api_key, apikey, access_token and secret_key followed by = or : and a value of eight or more characters
    When the scanner reads them in different forms: upper case name, blanks around =, a quote, an environment variable prefix, a JSON pair, a colon with a quoted value
    Then the span is the value alone, of the named-value kind
    And seven characters, a colon without a quote, a letter or digit before the name and a longer name are not found

  # canon: {"schema":1,"at":"2026-11-08T06:31:01Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.11
  Scenario: Placeholders and empty values after a key name are not secrets
    Given a key name followed by an empty value, <your key>, ${API_KEY}, YOUR_API_KEY, PLACEHOLDER, string or $TOKEN_VALUE
    When the scanner reads each
    Then it finds no span in any of them
    And a value with a digit or in mixed case is a value and is found

  # canon: {"schema":1,"at":"2026-11-08T06:31:07Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.12
  Scenario: A value takes at most one candidate window from its key name
    Given a key name followed by two thousand characters
    When the scanner reads it
    Then it finds one span
    And the span ends one candidate window after the name

  # canon: {"schema":1,"at":"2026-11-08T06:31:13Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:failure
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.13
  Scenario: A refusal names the file, the line and the rule, never the value, and stores nothing
    Given a file with an AWS key id on its third line, and a CRLF file with a named value before a key id
    When the file is selected into a context pack
    Then the refusal is CONTEXT_INPUT_INVALID with reason SECRET, not retryable, and its message names the file, the line and the rule
    And the message does not carry the value, the first secret of the file is the one named, and the pack store holds nothing

  # canon: {"schema":1,"at":"2026-11-08T06:31:19Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:failure
  @flow:canon-0-39-secret-scan.F2
  @dispatch.secret-scan.14
  Scenario: Every line of the table that matches a rule is refused with the code, the reason and the exit value of 0.38
    Given the lines of the brief's table that match a rule: a key id, a token, a private key header, a named value, each at its minimum length
    When each is selected into a context pack
    Then each is refused with CONTEXT_INPUT_INVALID, reason SECRET and exit 2, naming the rule
    And the scanner names the same rule for each

  # canon: {"schema":1,"at":"2026-11-08T06:31:25Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:failure
  @flow:canon-0-39-secret-scan.F3
  @dispatch.secret-scan.15
  Scenario: Injected guidance that holds a secret is refused naming its strategy, and plain guidance becomes a pack
    Given a strategy whose content holds an AWS key id on its second line, and another whose content is only risk-high
    When dispatch begin injects guidance
    Then the first is refused with CONTEXT_INPUT_INVALID naming injected guidance and its strategy id, and no pack is stored
    And the second becomes a pack and its guidance carries risk-high

  # canon: {"schema":1,"at":"2026-11-08T06:31:32Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F3
  @dispatch.secret-scan.16
  Scenario: The title of an injected guidance is scanned too
    Given an injected guidance whose title holds an sk-proj- key on its second line and whose content is plain
    When the pack is created
    Then it is refused naming the injected guidance, line 2 and the api-secret-key rule
    And nothing is stored

  # canon: {"schema":1,"at":"2026-11-08T06:31:44Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:failure
  @flow:canon-0-39-secret-scan.F3
  @dispatch.secret-scan.17
  Scenario: A text gets the same verdict as a file and as an injected guidance, a quoted value included
    Given the same text held by a file and held by an injected guidance, among them a quoted JSON value, an AWS key id, a named value and a private key
    When each is selected into a pack both ways
    Then both ways give one verdict, and a refusal reads the same apart from its file name or guidance id
    And the quoted value is judged as written, not as its JSON text, whose escaped quote would hide it

  # canon: {"schema":1,"at":"2026-11-08T06:31:52Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F4
  @dispatch.secret-scan.18
  Scenario: Spans carry their start, end and kind and come in start order whatever the rule order
    Given a text holding an sk- key, an AWS key id, a private key header and a named value, in that order
    When the scanner reads it
    Then it returns four spans of those kinds, each with its start, end and kind
    And the starts are in increasing order whatever order the rules run in

  # canon: {"schema":1,"at":"2026-11-08T06:31:58Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F4
  @dispatch.secret-scan.19
  Scenario: Overlapping spans merge into the earlier kind and touching spans stay apart
    Given an api_key whose value is an sk- key, a named value inside a private key, and two key ids with and without a gap
    When the scanner reads them
    Then the overlapping spans are one span of the earlier kind
    And spans that only touch stay two, and two key ids with no gap are not a key id

  # canon: {"schema":1,"at":"2026-11-08T06:32:05Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F4
  @dispatch.secret-scan.20
  Scenario: Spans lie on character boundaries in text with multibyte characters
    Given a text with Korean, an emoji and a quoted api_key value
    When the scanner reads it
    Then every span starts and ends on a character boundary
    And the text holds exactly one span

  # canon: {"schema":1,"at":"2026-11-08T06:32:11Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F4
  @dispatch.secret-scan.21
  Scenario: The line of a span counts from one
    Given a text with LF and CRLF line ends and a named value on its third line
    When line_of is asked for the span and for offset 0 and for an empty text
    Then the span is on line 3
    And offset 0 and an empty text are line 1, so lines count from one

  # canon: {"schema":1,"at":"2026-11-08T06:32:17Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F4
  @dispatch.secret-scan.22
  Scenario: A 4 MiB input of repeated candidates is scanned to its end
    Given 4 MiB of sk-, AKIA, -----BEGIN , sk-a- or a private key header repeated
    When the scanner reads each
    Then it ends
    And no span reaches past the end of the text

  # canon: {"schema":1,"at":"2026-11-08T06:32:23Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:failure
  @flow:canon-0-39-secret-scan.F4
  @dispatch.secret-scan.23
  Scenario: The scanner and context-pack add judge every line of the table alike, and a line they judge differently fails
    Given every line of the brief's table, with the rule that refuses it or none
    When the scanner and context-pack add each judge the line
    Then they agree on each line and with the table
    And a line one of them judges differently fails the test and names the line

  # canon: {"schema":1,"at":"2026-11-08T06:32:29Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F5
  @dispatch.secret-scan.24
  Scenario: A clean repository's pack is not dirty, also when an earlier pack left files in the pack store
    Given a clean git repository with one commit
    When a context pack is created twice
    Then its repo state is not dirty and its state digest is the digest of clean
    And the files the first pack left under the pack store do not make the second dirty, both have the same id, and show and verify answer as before

  # canon: {"schema":1,"at":"2026-11-08T06:32:36Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:failure
  @flow:canon-0-39-secret-scan.F5
  @dispatch.secret-scan.25
  Scenario: A change outside the pack stores makes the pack dirty and each status entry of its digest ends in one NUL
    Given a clean git repository, then an untracked file outside the pack stores, then a changed tracked file
    When a pack is created in each state
    Then each is dirty
    And the digest is the diff, one NUL, and each status entry followed by exactly one NUL

  # canon: {"schema":1,"at":"2026-11-08T06:32:42Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F5
  @dispatch.secret-scan.26
  Scenario: The status entries of the pack stores and the empty entries are dropped
    Given a status list holding entries of the pack stores, ordinary entries and empty entries
    When the pack store entries are filtered out
    Then the empty entries are dropped with them
    And only the ordinary entries remain, each followed by its NUL

  # canon: {"schema":1,"at":"2026-11-08T06:32:49Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:edge
  @flow:canon-0-39-secret-scan.F5
  @dispatch.secret-scan.27
  Scenario: No git repository and no commit are read as no change
    Given a directory that is no git repository, and a git repository with no commit
    When a pack is created in each
    Then neither has a commit and neither is dirty
    And the state digest is the digest of clean

  # canon: {"schema":1,"at":"2026-11-08T06:32:55Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:happy
  @flow:canon-0-39-secret-scan.F6
  @dispatch.secret-scan.28
  Scenario: The CHANGELOG announces the scanner, the fixed refusals, the new refusals and the new pack ids
    Given the CHANGELOG entry of canon-0-39-secret-scan
    When the release check reads its Fixed and Changed parts
    Then Fixed names risk-high and dirty: false
    And Changed names the api_key = form, JSON, sk-, the pack id and stateDigest

  # canon: {"schema":1,"at":"2026-11-08T06:33:01Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:failure
  @flow:canon-0-39-secret-scan.F6
  @dispatch.secret-scan.29
  Scenario: A release without the announcement fails the release check
    Given a CHANGELOG with no entry of canon-0-39-secret-scan, one without risk-high and one without stateDigest
    When the release check reads each
    Then it names the missing section or the missing item
    And an entry that has them all is named by nothing

  # canon: {"schema":1,"at":"2026-11-08T08:20:14Z","actor":{"agent_id":"impl-0-39","role":"implementer","session_id":"s-0-39-impl"}}
  @subject:dispatch-manifest
  @lane:behavior
  @case:failure
  @flow:canon-0-39-secret-scan.F1
  @dispatch.secret-scan.30
  Scenario: A selection of a plain document and a document that holds a secret is refused as a whole and stores no pack
    Given a repo, a plain document, and a second document with an AWS key id on its second line, both selected by one manifest
    When context-pack add runs on the manifest
    Then it is refused with CONTEXT_INPUT_INVALID, reason SECRET, naming the second file and its line and no value
    And the plain document alone becomes a pack, and the refused selection left no pack behind
