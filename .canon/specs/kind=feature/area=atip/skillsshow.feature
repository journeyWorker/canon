Feature: ATIP: skills show, the guides and their command lines
  # canon: {"schema":1,"at":"2026-11-02T10:15:13Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}

  # canon: {"schema":1,"at":"2026-11-02T10:15:14Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-skills-show.F1
  @atip.skillsshow.01
  Scenario: skills show without a name answers the default guide, canon, with every guide name in guides, in an empty directory
    Given a built canon and an empty directory
    When canon skills show --json runs without a name
    Then exit is 0, data.name is canon, data.guides lists every guide name in order, and data.text names each of them

  # canon: {"schema":1,"at":"2026-11-02T10:15:48Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-skills-show.F1
  @atip.skillsshow.02
  Scenario: A call without a name prints the default guide's bytes in text mode and neither form writes anything, with or without a repository
    Given a directory with a canon.yaml and state, and an empty one
    When skills show runs without a name, with --full and with a name, and with an unknown name
    Then the bare text equals skills show canon and the guide's bytes, and the directory is byte for byte as it was

  # canon: {"schema":1,"at":"2026-11-02T10:16:13Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-skills-show.F2
  @atip.skillsshow.03
  Scenario: Every guide's --json is name, text, guides and full, passes the contract's guide schema, and carries x-canon version
    Given a built canon and every guide name, and no name
    When skills show runs with --json for each, and with --full on canon and on another guide
    Then data passes the contract's guide schema, name is the one asked for, x-canon.version is canon's, and full says whether --full material is in text

  # canon: {"schema":1,"at":"2026-11-02T10:16:40Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-skills-show.F2
  @atip.skillsshow.04
  Scenario: The old members markdown and version are gone and a changed document fails the guide schema
    Given the output of skills show --json and documents of the old and changed shape
    When the members are listed and the documents are checked against the guide schema
    Then no markdown and no top-level version member exists, the old shape and each changed document fail the schema, and the command document refers to the schema by its id

  # canon: {"schema":1,"at":"2026-11-02T10:16:57Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-skills-show.F3
  @atip.skillsshow.05
  Scenario: An unknown name, an upper-case name and a name with a space refuse with USAGE, field NAME and the guide names as expected
    Given a built canon and values nosuch-xyz, Canon, a name with a space, and an underscored name
    When skills show runs with each
    Then each exits 2 with USAGE, field NAME and expected the guide names, and a known name is not refused

  # canon: {"schema":1,"at":"2026-11-02T10:17:32Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-skills-show.F4
  @atip.skillsshow.06
  Scenario: The first line of every help names schema and skills show
    Given a built canon
    When the first line of the root, a group and a command help is read
    Then each names schema and skills show, and the introduction stays below it

  # canon: {"schema":1,"at":"2026-11-02T10:17:54Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-skills-show.F5
  @atip.skillsshow.07
  Scenario: Every command line in the fenced blocks of every guide parses by the command document and by canon's parser
    Given every command line in the fenced blocks of every guide
    When its placeholders are replaced by valid values and each line is parsed by the command document and canon's parser
    Then none is refused, an --input document is read element by element, and lines with shell syntax are outside the rule

  # canon: {"schema":1,"at":"2026-11-02T10:18:31Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-skills-show.F5
  @atip.skillsshow.08
  Scenario: A guide line with a wrong option, a missing required argument, a value outside its domain or an unknown command is found
    Given a guide line that is correct
    When an option is misspelled, a required argument is dropped, a value leaves its domain, or the command is unknown
    Then the test reports the guide and the line number of each, and a pipe line and a line outside a fence are left alone

  # canon: {"schema":1,"at":"2026-11-02T10:18:55Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-35-skills-show.F6
  @atip.skillsshow.09
  Scenario: No guide, script, hook or page reads the old members of the skills show output
    Given the guides, scripts, hooks, website pages and docs of the repository
    When they are searched for readers of the output of skills show
    Then none reads markdown or a top-level version of it
