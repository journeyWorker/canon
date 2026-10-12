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

  # canon: {"schema":1,"at":"2026-10-10T16:35:35Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:edge
  @skills.install.11
  Scenario: The projected skill claims build work in a canon repo and opens with the working loop
    Given canon's own skill source and its committed Claude, Codex, OMP and Pi projections
    When each projected SKILL.md is read
    Then it is the source byte for byte
    And its description claims build, implement, feature and fix work in a repo that has canon.yaml, ahead of generic site, app or game builder skills
    And its description stays within the 1024-character limit skill loaders enforce
    And its first section is the working loop, naming brief, subject, scenarios, units, implement, evidence, independent review and transition in that order
    And the command reference comes after the loop
    And the AGENTS.md block canon init writes names the same steps in the same order

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:skills-install
  @case:happy
  @skills.install.20
  Scenario: The skill and the AGENTS.md block carry one human approval per scope
    Given canon's own skill source and the AGENTS.md block canon init writes
    When each is read with its wrapped lines joined
    Then each says a new subject or a scope change needs one human answer approving the brief and the policy
    And each tells the agent to record that answer at once, as an "Approved: <who> <when>" line in the change's proposal.md and as canon policy approve --by <the human> --note, and never to approve without a human answer
    And each says that inside an approved scope the agent never stops for a human, that independent review happens on every change, and that when no human can answer it writes the assumption and proceeds
    And each says a fix or follow-up stays in the approved change as tasks in its tasks.md, never a canon change new, because a new change is a new scope
    And each says the agent moves the subject to shipped itself once the guard passes, unless the brief says the human ships

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:skills-install
  @case:failure
  @skills.install.21
  Scenario: The skill and the AGENTS.md block name no second human stop
    Given canon's own skill source and the AGENTS.md block canon init writes
    When each is read with its wrapped lines joined
    Then neither keeps a fixed pair of human stops, a brief approval for every request, or a rule that the agent never runs the policy approval for the human

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.40
  Scenario: The install lock records the canon version and the source commit
    Given a canonical skill source inside a git checkout
    When canon skills install writes the projections
    Then the install lock records this canon's version and the checkout's HEAD commit
    And a rerun with nothing changed reports unchanged and leaves the lock byte-identical, even after a new commit that leaves the bundle as it was
    And a source outside any git checkout records no source commit

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.41
  Scenario: A lock another canon version wrote is reported with the install fix
    Given projections whose install lock records another canon version
    When the read-only check and the doctor inspect the target
    Then both say "installed by canon <that version>, this canon is <this version>" and name `canon skills install` for the selected providers
    And the check exits 1 while the doctor exits 0
    And after the install both are clean and the lock records this canon's version

  # canon: {"schema":1,"at":"2026-10-11T14:43:37Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.42
  Scenario: A lock from before 0.16 is reported as an unknown canon version with the same fix
    Given projections whose install lock was written before canon 0.16 and records no canon version
    When the read-only check and the doctor inspect the target
    Then both report the projections as installed by an unknown canon version and name the same install command
    And the check exits 1 until the install rewrites the lock

  # canon: {"schema":1,"at":"2026-10-11T14:43:37Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.43
  Scenario: Doctor warns about co-installed skills that claim build work and never edits them
    Given a target with canon's projections and, in the provider skill roots, a skill whose description claims site, app or game work and a skill that claims none
    When canon skills doctor runs
    Then it names the claiming skill, its directory and the build words its description holds, and warns that if an agent picks it for build work canon's loop will not run
    And it says nothing about canon's own skill or the skill that claims no build work
    And it exits 0 and still reports the projections as healthy
    And every other skill's files are byte-identical afterwards

  # canon: {"schema":1,"at":"2026-10-12T03:41:11Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.22
  Scenario: The skill, the AGENTS.md block and the write help name no bookkeeping step the commands already do
    Given canon's own skill source, the AGENTS.md block canon init writes, and the help of canon evidence, canon finding and canon finding add
    When each is read with its wrapped lines joined
    Then the working loop never tells the agent to run canon format specs or canon ingest plans
    And it names canon inventory sync only for a command that printed "NOT refreshed"
    And every mention of canon gate promote is conditioned on write.promote manual
    And no help loop lists canon gate promote as a step of its own
