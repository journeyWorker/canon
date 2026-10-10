Feature: skills install
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:skills-install
  @case:happy
  @skills.install.01
  Scenario: Installing materializes both agent conventions and records what it wrote
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a source tree holding one agent guide
    When the guides are installed into a target repo
    Then the guide is reported installed at version one as changed
    And its file under the Claude convention is byte-verbatim against the source
    And its file under the Codex convention, the skill directory Codex discovers, is byte-verbatim against the source as well
    And the lock records that guide's content hash and its version
    And no third agent directory is created

  @subject:skills-install
  @case:edge
  @skills.install.02
  Scenario: A rerun with no source change reports unchanged rather than rewriting
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a target repo where the guides have already been installed once
    When install is run a second time with nothing changed in the source
    Then both materialized files and the lock are byte-identical to the first run
    And the guide is reported unchanged, because its content hash matched the lock entry
    And its recorded version did not move

  @subject:skills-install
  @case:happy
  @skills.install.03
  Scenario: An edited guide bumps its recorded version by exactly one
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a guide installed at version one
    When its source content is edited and install is run again
    Then the guide is reported changed
    And its recorded version is two, incremented by exactly one

  @subject:skills-install
  @case:edge
  @skills.install.04
  Scenario: The lock keys on content, never on the clock
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the same bytes hashed twice and one differing byte hashed once
    When the hashes are compared
    Then the two identical inputs produce the same hash and the differing input a different one
    And the hash names its algorithm, so an install decision is reproducible from content alone

  # canon: {"schema":1,"at":"2026-10-08T14:02:45Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.07
  Scenario: An unknown provider is refused before anything is written
    Given a canonical skill source and an empty target
    When the skill is installed for a provider canon does not know
    Then the install fails with the provider named
    And no projection or lock file is written to the target

  # canon: {"schema":1,"at":"2026-10-10T14:03:19Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.08
  Scenario: Codex receives the skill where Codex discovers skills, frontmatter intact
    Given a canonical skill source whose frontmatter names the skill and describes it, with reference and script sidecars
    When the skill is installed for Codex
    Then Codex's skill file is the source byte for byte, in the canon skill directory under the agents skills root Codex scans
    And its frontmatter opens the file and carries the skill's name and a non-empty description
    And the reference and script sidecars sit beside it
    And nothing is written under the Codex config directory
    And a target holding only the agents root or only the Codex config directory selects Codex when no provider is named
    And a second install reports unchanged and the read-only check is clean

  # canon: {"schema":1,"at":"2026-10-10T14:03:19Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:edge
  @skills.install.09
  Scenario: Install migrates a legacy Codex projection, removing only what canon's install lock proves it wrote
    Given a target holding the flattened Codex projection an earlier canon wrote under the Codex config directory, with an install lock recording each file's hash
    And a file of the user's own beside it, and a legacy file the user edited after install
    When the skill is installed
    Then the Codex skill lands under the agents skills root
    And every legacy file whose bytes still match its recorded hash is removed, along with each legacy directory left empty
    And the user's own file and the edited legacy file are kept
    And the legacy skills directory is removed only when it ends up empty, and the Codex config directory itself is kept
    And a rerun reports unchanged and leaves the user's file in place

  # canon: {"schema":1,"at":"2026-10-10T14:03:19Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.10
  Scenario: A leftover legacy Codex projection is reported as a remnant with its fix, never deleted
    Given a target where a legacy Codex projection remains that no install lock proves canon wrote
    When the read-only check and the doctor inspect the target
    Then the check lists the legacy path as a remnant, names the install command that migrates it, and exits non-zero
    And the doctor reports it as a legacy remnant naming the same fix
    And a following install keeps the file, because canon cannot prove it wrote it
