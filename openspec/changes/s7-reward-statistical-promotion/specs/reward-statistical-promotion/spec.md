## ADDED Requirements

### Requirement: Role-specific reward function registry
The system SHALL maintain a per-role reward function registry mapping S4
verdict-stream events to a reward score in `[0, 1]`, with each role's
function derived from that role's own verdict sources (git signals for
`dev`, review records for `content`/`design`/`review`, etc), never a single
formula applied uniformly across roles.

#### Scenario: dev role reward reflects PR/CI/rollback signals
- **WHEN** a `dev`-role trajectory's verdict stream shows PR merged, CI
  passed, and the no-rollback window elapsed with no rollback
- **THEN** the computed reward matches the registered `dev` weighted
  formula's full-positive-signal value

#### Scenario: A non-dev role never uses the dev weight formula
- **WHEN** a `content`- or `design`-role trajectory receives a verdict
  event
- **THEN** the reward is computed by that role's own registered function,
  not the `dev` role's PR/CI weights

### Requirement: Trajectory verdict write-back
The system SHALL provide `mark_trajectory_verdict` to record a verdict
(`pending | success | failure | rolled-back`) and a `[0, 1]`-clamped
reward on a stored trajectory, and SHALL never leave a trajectory at
`pending` once a covering verdict event has arrived.

#### Scenario: A covering verdict event clears pending
- **WHEN** a trajectory is stored with verdict `pending` and a later
  verdict event covers it
- **THEN** `mark_trajectory_verdict` updates it to a non-`pending` verdict
  with a reward inside `[0, 1]`

#### Scenario: Reward is clamped, never out of range
- **WHEN** a role's reward function computes a value outside `[0, 1]`
- **THEN** `mark_trajectory_verdict` clamps the stored reward to `[0, 1]`

### Requirement: Paired-CRN statistical promotion gate
For roles whose domain supports deterministic replay, the system SHALL
gate strategy promotion on a paired common-random-number contrast that
decomposes measured effect into a between-config term and a between-panel
noise term, and SHALL refuse promotion when the residual degrees of
freedom fall below the documented minimum.

#### Scenario: A corroborated effect promotes
- **WHEN** a CRN-gated role's contrast batch shows a between-config effect
  that clears the df-aware significance threshold with sufficient
  residual degrees of freedom
- **THEN** `corroborated_effect` returns true and the candidate strategy
  is eligible for promotion

#### Scenario: A single-panel batch never promotes
- **WHEN** a CRN-gated role's contrast batch has fewer panels than the
  documented minimum for significance
- **THEN** `corroborated_effect` returns false regardless of the observed
  panel-mean difference

#### Scenario: The documented MaTTS counter-example stays rejected
- **WHEN** a two-config, two-panel batch with per-panel differences
  `[0.1, 0.3]` is evaluated
- **THEN** `corroborated_effect` returns false

### Requirement: N-occurrence + zero-contradiction promotion gate
The system SHALL gate strategy promotion, for roles whose domain does not
support deterministic replay, on at least `n_min` corroborating
`success`-verdict trajectories sharing a `regime_key`, with zero
`failure`-verdict trajectories for that `regime_key` inside the
configured observation window, and SHALL reset the corroboration count on
any contradicting failure within the window.

#### Scenario: Enough successes with no contradiction promotes
- **WHEN** an occurrence-gated role's `regime_key` accumulates `n_min`
  `success`-verdict trajectories with zero `failure`-verdict trajectories
  in the observation window
- **THEN** the candidate strategy for that regime becomes eligible for
  promotion

#### Scenario: A contradicting failure resets the count
- **WHEN** an occurrence-gated regime has `n_min - 1` corroborating
  successes and then receives one `failure`-verdict trajectory inside the
  window
- **THEN** the corroboration count resets and promotion does not proceed

### Requirement: Strategy demotion on contradiction
The system SHALL demote a previously-promoted strategy when a
contradicting trajectory arrives for its regime, writing a demotion
evidence record and applying the repo's configured demotion policy
(soft-flag by default, hard-delete optionally) to the git-tier file.

#### Scenario: A contradicting trajectory demotes a promoted strategy
- **WHEN** a strategy previously promoted for a regime later receives a
  `failure`-verdict trajectory in the same regime
- **THEN** `demote_strategy` writes a demotion evidence record and the
  git-tier file is updated per the repo's demotion policy

#### Scenario: Default policy soft-flags rather than deletes
- **WHEN** a repo has not configured a hard-delete demotion policy
- **THEN** demotion annotates the existing git-tier file `status:
  demoted` with a reason, and does not delete the file

### Requirement: Webhook receiver PR/CI ingestion
The system SHALL normalize inbound PR/CI webhook payloads into S4's
verdict-event shape and route them through the reward function registry
and `mark_trajectory_verdict`, without duplicating S4's own ingest
adapters.

#### Scenario: A merged PR with passing CI produces a success verdict
- **WHEN** the webhook receiver ingests a `pull_request.merged` event
  followed by a passing `workflow_run.conclusion` for the same PR/SHA
- **THEN** the corresponding trajectory's verdict is marked `success`
  with the `dev` role's positive-signal reward

#### Scenario: Webhook ingestion is opt-in per repo
- **WHEN** a consumer repo has not enabled `canon.yaml`'s
  `webhook.enabled` setting
- **THEN** no webhook receiver runs, and the repo continues to function
  with fixture/manual verdict marking only

### Requirement: Golden fixture verdict streams
The system SHALL ship fixture verdict streams whose expected promotions
and rejections are golden-file checked, and SHALL demonstrate at least one
contradicted candidate being demoted.

#### Scenario: A fixture stream's promotions match the golden file
- **WHEN** the fixture verdict stream runs through the reward + promotion
  pipeline
- **THEN** the set of promoted strategies matches the golden expectations
  file exactly

#### Scenario: A fixture stream's rejections match the golden file
- **WHEN** the fixture verdict stream includes candidates below the
  promotion threshold
- **THEN** those candidates are absent from the promoted set, matching
  the golden expectations file
