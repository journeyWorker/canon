# canon-authoring

A `.feature` corpus is the join spine everything else in canon hangs
off: `canon inventory sync` indexes it, `canon review`/`canon evidence`
attest against it, `canon gate check` measures coverage over it. A
corpus that is organized badly does not fail `canon format` — it
produces a green gate that measures the wrong thing. This skill is the
organizing rule.

## Vocabulary

| Term | Is | Is NOT |
|---|---|---|
| **area** | One bounded context of the product, a kebab noun (`checkout`, `world-map`, `dm`). The directory `area=<area>/`. | A sprint, a team, a theme ("experience", "quality"). |
| **surface** | One screen, flow, or API a user or caller actually meets (`cart`, `place-lock`, `session-create`). Exactly ONE `.feature` file per surface. | A grab-bag ("misc", "experience", "review") — see anti-patterns. |
| **scenario** | One observable behavior of that surface: a refusal, a transition, a boundary, a derived value, an idempotence property. Readable by someone who has never seen the code. | A function name, an implementation step, a rule about a document or a report. |
| **`<area>.<surface>.<nn>`** | The scenario's durable id. Assigned once, never renumbered; gaps are fine. | A sort key you tidy up. |

A `Feature:` title names the surface. If you cannot name the surface,
you do not have one yet.

## Lanes: what kind of requirement is this?

Every scenario is one of a small set of lanes, tagged `@lane:<value>`:

| Lane | The scenario is about | Attested by |
|---|---|---|
| `behavior` | What the product does — state, input, data, navigation, errors. The default. | A test or an observed run. |
| `design` | How it looks and moves — tokens, spacing, motion, easing. | A design review. |
| `architecture` | How the code is shaped — layering, ownership, error surfacing. | An architecture review. |
| `process` | How the TEAM works — what a review report may claim, how acceptance is run, what a build must check. | Nothing in the product. |

`process` rules are real, but they have no code surface and can never
be `faithful` from a test. Keep them out of the coverage number:

```yaml
# .canon/policy.yaml
spec_coverage:
  require_evidence: true
  scope: [building, verifying]
  exclude_lanes: [process]
```

The base set comes from `canon.core`; a repo declares its own `lane`
enum in `.canon/vocab/<id>/enums.yaml` to replace it (see
`canon-vocab`). `canon context` shows the set in force. A value outside
a declared set is refused at the scaffold and dropped, with a counted
diagnostic, at `canon inventory sync`.

**The test for a `behavior` scenario:** name the test or run that would
attest it. If you can't, it is a `process` rule, or it is a gap — record
the gap, do not write the scenario.

## Cases: which path of the behavior is this?

A second, independent axis, tagged `@case:<value>`:

| Case | The scenario specifies |
|---|---|
| `happy` | The expected path: valid input, the outcome the user wants. |
| `failure` | A refusal, an error, a denied or blocked action — what happens when the input is wrong, missing, unauthorized, or stale. |
| `edge` | A boundary that is neither: empty sets, limits, duplicates, re-runs, ordering. |

A spec can be fully attested and still describe only its golden path.
Evidence presence cannot see that gap; `require_cases` can:

```yaml
# .canon/policy.yaml
spec_coverage:
  require_evidence: true
  scope: [building, verifying]
  require_cases: [failure]
```

Every feature surface (`<area>.<surface>`) with in-scope scenarios must
then carry at least one scenario of each listed case, or `canon gate
check` reports it as `uncovered-cell`, and `canon subject status <id>
shipped` refuses for a subject that owns such a surface. An untagged
scenario counts toward its surface but satisfies no case, so an
untagged corpus reports every surface instead of passing silently. The
base set comes from `canon.core`; a repo replaces it with its own `case`
enum, like `lane`.

## Subject pinning

Every scenario belongs to a Subject (the durable product unit, see
`canon-subject`). Write the Subject first, then pin with `--subject`;
`canon inventory sync` carries the tag onto the index record and
`spec_coverage.scope` filters on it. An unpinned scenario is invisible to
a scoped gate.

## The commands — and the one thing you never type

```bash
canon feature new checkout.cart --title "Cart"                       # once per surface
canon scenario new checkout.cart.01 --title "Adding an item raises the count" \
  --subject checkout-core --lane behavior --case happy               # once per behavior
canon scenario new checkout.cart.04 --title "Adding an out-of-stock item is refused" \
  --subject checkout-core --lane behavior --case failure             # and its failure path
canon format specs                                                   # clean?
canon inventory sync                                                 # index it
```

- `--actor <id>` (or `CANON_ACTOR` in the environment) puts YOUR id on the
  provenance line. Default is `canon-scaffold`.
- **Never hand-type, move, or edit a `# canon:` line.** The scaffold
  writes it directly above the scenario's tags:

  ```gherkin
    # canon: {"schema":1,"at":"2026-09-16T10:02:11Z","actor":{"agent_id":"Main"}}
    @subject:checkout-core
    @lane:behavior
    @case:happy
    @checkout.cart.01
    Scenario: Adding an item raises the count
      Given an empty cart
      When one item is added
      Then the count reads 1
  ```

  A line directly UNDER the header (older corpora) is also accepted.
  Anywhere else — separated by a blank line, above `Feature:` with
  scenario tags in between — is `missing-provenance`, and the whole
  root's sync aborts.
- Steps are yours to write; the scaffold's `Given a step` is a
  placeholder, not a spec.

## Anti-patterns, by name

- **The grab-bag surface.** One `experience.feature` holding pointer
  capture, i18n fallback, a compiler tag rule, and "the review report
  must not claim video quality". Four surfaces and a process rule. Split
  by what the user meets; move the rule to `lane:process`.
- **A Then about a document.** "Then the report distinguishes X from Y."
  The product has no report. `lane:process`, or `policy.yaml`.
- **A Feature title that is a theme.** "Fidelity of the original
  experience" names no surface. Name the screen.
- **Hand-stamped provenance.** Eighty lines with one timestamp and the
  same `agent_id`. Use `canon scenario new --actor`.
- **Coverage by splitting.** One behavior as two scenarios so two rows go
  green. One behavior, one id.
- **Runner tags as classification.** `@design`, `@p2`, `@wip` are
  invisible to canon — they index nothing. Only `@lane:`, `@case:`, and
  `@subject:` are read; any other `@name:value` namespace is counted as a
  diagnostic at sync so you can see it was dropped.
- **The golden-path-only surface.** Five scenarios, all of the form
  "doing X works". Nothing says what happens on a missing field, a
  denied user, a duplicate submit. Write the refusal as its own
  `@case:failure` scenario; `require_cases` makes the gap a gate failure.

## Worked example

```
specs/features/kind=feature/area=checkout/
  cart.feature        # surface: the cart screen
  payment.feature     # surface: the payment step
```

```gherkin
Feature: Cart
  # canon: {"schema":1,"at":"2026-09-16T10:02:11Z","actor":{"agent_id":"Main"}}

  # canon: {"schema":1,"at":"2026-09-16T10:02:11Z","actor":{"agent_id":"Main"}}
  @subject:checkout-core
  @lane:behavior
  @checkout.cart.01
  Scenario: Adding an item raises the count
    Given an empty cart
    When one item is added
    Then the count reads 1

  # canon: {"schema":1,"at":"2026-09-16T10:02:40Z","actor":{"agent_id":"Main"}}
  @subject:checkout-core
  @lane:design
  @checkout.cart.02
  Scenario: The count badge uses the accent token and a 120ms ease-out
    Given a cart with one item
    When a second item is added
    Then the badge animates over 120ms with ease-out in the accent color

  # canon: {"schema":1,"at":"2026-09-16T10:03:02Z","actor":{"agent_id":"Main"}}
  @subject:checkout-core
  @lane:process
  @checkout.cart.03
  Scenario: A cart review names the device it was run on
    Given a cart design review
    When the report is written
    Then it names the device and OS the review ran on
```

`.03` is excluded from coverage by `exclude_lanes: [process]`; `.01` is
attested by a test, `.02` by a design review.