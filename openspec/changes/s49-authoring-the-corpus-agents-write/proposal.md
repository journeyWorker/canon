# s49 — authoring-the-corpus-agents-write

> An agent given `canon init` and nothing else wrote 84 scenarios into
> seven grab-bag surfaces, hand-typed every provenance line, tagged no
> subject, and mixed "the review report must not claim X" in with
> "pointerup must not cancel the draft". `canon format` said 0
> violations. This change makes the tool say what a good corpus is,
> and makes the scaffold the easier path to one.

## Why

A real consumer repo (`eevee`, `specs/features/kind=feature/area=icecream/`)
was read end to end. What it shows is not an author's failure; every
symptom traces to a gap in what canon says or does:

1. **No authoring guidance exists.** `canon/skills/` has fifteen skills
   and none covers writing a `.feature`. `canon-inventory` begins at
   "author .feature corpus" as a given. `canon init` writes `canon.yaml`
   and stops — it never suggests `canon skills install`, so the consumer
   repo had zero canon skills when the corpus was written.
2. **The provenance comment's position is enforced, not preferred.**
   `gherkin::scan` accepts `# canon:` ONLY as the first non-blank line
   AFTER a header (`crates/canon-fmt/src/gherkin.rs:89-92`). Placed above
   the tag, between tag and `Scenario:`, or above `Feature:` — all three
   report `missing-provenance` and abort the whole root. Every reader
   finds the after-header position odd (a comment between a header and
   its steps, at the header's own indent), and the one edit that fixes
   the reading breaks the sync.
3. **The scaffold cannot attribute.** `canon scenario new` stamps a fixed
   `canon-scaffold` actor with no `--actor`. An agent that wants its
   name on the line hand-types the comment instead, and loses the
   scaffold's shape guarantee and duplicate-tag guard. The consumer
   corpus carries `"agent_id":"Main"` on all 84 lines, in timestamp
   batches — hand-written.
4. **There is no axis for what KIND of requirement a scenario is.** The
   scanner collects `@<area>.<surface>.<nn>` and `@subject:` and silently
   drops everything else (`gherkin.rs:104-112`). An author who tags
   `@design` or `@process` has classified nothing: not indexed, not
   queryable, not visible to the gate. Meanwhile every indexed scenario
   is a `spec_coverage` target, so a process rule ("the review report
   must distinguish frame review from video quality") sits uncovered
   forever or collects `not_applicable` — the coverage number is
   polluted by rows that never had a code surface.
5. **`@subject:` is documented as mandatory and impossible to scaffold.**
   `canon-subject` says "pin every scenario"; `canon scenario new` has no
   `--subject`. The consumer corpus has zero subject tags.

## What changes

**Provenance may lead the header.** `gherkin::scan` accepts a `# canon:`
line in the contiguous block immediately ABOVE a `Feature:`/`Scenario:`
header (tag lines and comment lines, no blank line) as well as the
existing first-line-after position. Both scaffolds emit the leading
form:

```
  # canon: {"schema":1,"at":"...","actor":{"agent_id":"..."}}
  @world.hotdeal.01
  Scenario: Apply a hotdeal coupon
    Given a step
```

Existing corpora with the trailing form stay clean. A header with
provenance in BOTH positions counts once.

**The scaffold attributes, and stamps to the second.** `canon scenario
new` / `canon feature new` gain `--actor <id>` (falls back to
`CANON_ACTOR`, then the existing `canon-scaffold`). The `at` stamp is
truncated to whole seconds — sub-second precision only churned
`source_digest`.

**`@lane:<value>` is a recognized scenario axis.** The scanner collects
`@lane:` exactly as it collects `@subject:`; `canon inventory sync` maps
it onto a new additive `Scenario.lane: Option<String>` (absent key when
unset, like `subject_id`). The value is a kebab slug; when the repo's
vocabulary declares a `lane` enum the value must be a member (the same
shape-then-membership rule `subject --domain` uses), otherwise any slug.
`canon.core` ships the base set `behavior`, `design`, `architecture`,
`process`. `canon scenario new --lane <v>` emits the tag. Two `@lane:`
tags: first wins, counted diagnostic — the `@subject:` rule.

**`spec_coverage` can exclude lanes.** `policy.yaml`'s `spec_coverage:`
gains optional `exclude_lanes: [<lane>, …]`. A scenario whose `lane` is
listed is out of scope for the coverage check — the sanctioned home for
process/review rules that have no code surface. Absent → nothing
excluded, unchanged behavior.

**`canon scenario new --subject <id>`** emits the `@subject:` tag. The
skill's rule finally has a command behind it.

**Unrecognized tag NAMESPACES are counted.** A `@name:value`-shaped tag
whose namespace is neither `subject` nor `lane` is reported by `canon
inventory sync` as a diagnostic (counted, printed, non-fatal — the
`@subject:` duplicate rule). Plain tags (`@p2`, `@wip`) are a Gherkin
runner's business and stay silent.

**A `canon-authoring` skill.** area = one bounded context, a noun;
surface = one screen/flow/API a user or caller meets, one file; scenario
= one observable behavior; `nn` never renumbered; process and review
rules go to `lane:process` (excluded from coverage) or `policy.yaml`,
never into a behavior surface; use the scaffold, do not hand-type `#
canon:`; the grab-bag surface as the named anti-pattern. `canon init`
prints `canon skills install` as its next step. `canon-fmt`'s
`missing-provenance` row and `canon-subject`'s scenario-pinning line are
updated to match.

## What does NOT change

- `missing-provenance` stays a violation and stays whole-root-fatal. The
  comment is still required on every header; only WHERE it may sit
  widens.
- `FmtFailureClass` is not widened. The unknown-namespace report is a
  sync diagnostic, not a format violation — a runner tag must never
  break a corpus.
- `Scenario.schema` stays `1`. `lane` is additive with the exact
  `subject_id` wire contract (`default`, `skip_serializing_if`).
- `spec_coverage.scope` semantics are unchanged. `exclude_lanes` is a
  second, independent filter applied after scope.
- No opinion is shipped on how a team slices lanes beyond the base set;
  a repo replaces it in its own vocabulary, exactly as `domain`.

## What this does NOT establish

Recognizing `@lane:` does not make an author classify well, and a
`canon-authoring` skill does not make an agent read it. What this buys
is that the classification an author DOES make is carried, queryable,
and honored by the gate — and that the path of least resistance
(`canon scenario new --lane --subject --actor`) produces a line the
tool can read back, instead of a line the author had to fake.
