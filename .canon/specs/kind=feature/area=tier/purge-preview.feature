Feature: tier purge-preview
  # canon: {"schema":1,"at":"2026-11-09T10:12:29Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:12:29Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-40-retire-followups.F2
  @tier.purge-preview.01
  Scenario: A purge preview in a repository with a tier that cannot attach lists the removal set and names the tier, and offers no commit command
    Given a canon.yaml that declares a cold s3 tier whose bucket variable is unset, and old rows of the retired session kind
    When canon purge --kind session --before T runs without --commit
    Then it exits 0 with the removal set, data.unavailable naming the cold tier, and a first meta.next step that names the variable to set (CANON_R2_BUCKET), or, for a tier that is configured and did not answer, the reachability check to retry
    And no meta.next entry is a --commit command and nothing is written

  # canon: {"schema":1,"at":"2026-11-09T10:12:37Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-40-retire-followups.F2
  @tier.purge-preview.02
  Scenario: Committing the digest of that preview is refused as RETENTION_TIERS_REQUIRED and deletes nothing
    Given the preview of the unattached-tier repository
    When canon purge --kind session --before T --commit --plan-digest D runs with its digest
    Then it exits 2 with RETENTION_TIERS_REQUIRED naming the cold tier
    And no row is deleted

  # canon: {"schema":1,"at":"2026-11-09T10:12:49Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-40-retire-followups.F2
  @tier.purge-preview.03
  Scenario: Once the tier attaches the preview offers the commit command and that commit deletes
    Given the same repository with the bucket variable set to a bucket the tier can attach
    When canon purge --kind session --before T runs, then commits the plan it printed
    Then the preview offers the --commit command as its first step and that commit deletes the rows
