Feature: skills install
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:skills-install
  @skills.install.01
  Scenario: Installing materializes both agent conventions and records what it wrote
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a source tree holding one agent guide
    When the guides are installed into a target repo
    Then the guide is reported installed at version one as changed
    And its file under the Claude convention is byte-verbatim against the source
    And its file under the Codex convention is flattened, leading with the guide's name and carrying no frontmatter delimiters
    And the lock records that guide's content hash and its version
    And no third agent directory is created

  @subject:skills-install
  @skills.install.02
  Scenario: A rerun with no source change reports unchanged rather than rewriting
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a target repo where the guides have already been installed once
    When install is run a second time with nothing changed in the source
    Then both materialized files and the lock are byte-identical to the first run
    And the guide is reported unchanged, because its content hash matched the lock entry
    And its recorded version did not move

  @subject:skills-install
  @skills.install.03
  Scenario: An edited guide bumps its recorded version by exactly one
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a guide installed at version one
    When its source content is edited and install is run again
    Then the guide is reported changed
    And its recorded version is two, incremented by exactly one

  @subject:skills-install
  @skills.install.04
  Scenario: The lock keys on content, never on the clock
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the same bytes hashed twice and one differing byte hashed once
    When the hashes are compared
    Then the two identical inputs produce the same hash and the differing input a different one
    And the hash names its algorithm, so an install decision is reproducible from content alone

  @subject:skills-install
  @skills.install.05
  Scenario: The Codex form is a flattening, not a second authored document
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a guide's name, its description and its body
    When the Codex form is rendered
    Then it leads with the guide's name as a heading
    And it carries the description as a quoted line
    And it ends with the body, so nothing authored is dropped in translation

  @subject:skills-install
  @skills.install.06
  Scenario: A guide with no frontmatter delimiters parses to a fallback name and an empty description
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a guide whose content carries no frontmatter delimiters at all
    When its frontmatter is parsed with a fallback name supplied
    Then the fallback name is used and the description is empty
    And the whole content is kept as the body, unmodified
