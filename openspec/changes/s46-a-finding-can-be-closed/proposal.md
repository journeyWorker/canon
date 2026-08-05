# s46 — a-finding-can-be-closed

> Found while cutting the release after s45: the generated release
> narrative said canon had 8 open blockers it had already fixed, and no
> command existed that could say otherwise.

## Why

`.canon/REPORT.md`'s review totals panel reports this today:

```
| change_id                 | rounds_recorded | findings | severity_blocker | disposition_open | disposition_fixed |
| s44-spec-derived-worklist | 1               | 14       | 8                | 14               | 0                 |
```

All fourteen were fixed. Twelve of the fourteen are cited by name in
s44's own `tasks.md`, and s44's 27 checkboxes are closed against
committed `EvidenceRecord`s. The corpus says otherwise because nothing
can write the other answer:

```
$ canon finding add --change-id s44-spec-derived-worklist --round 1 --seq 1 \
      --disposition fixed --resolution-sha <40-hex>
canon finding add: refused — s44-spec-derived-worklist__0001__0001 is already
occupied by a committed finding (`s44-spec-derived-worklist`, round 1, seq 1);
pick the next free --seq rather than authoring a second record under one
finding's identity
```

`canon finding` has one verb, `add`. There is no other.

That refusal is correct and must stay. Its own rule says why
(`crates/canon-gate/src/promote.rs`, `NaturalKeyRule::Unique`): the key
is `{change_id}__{round}__{seq}`, a reviewer's own numbering of their
own round, so two DISTINCT findings under one key are two reviewers'
work collapsed into one identity — and every reader that folds by
natural key silently drops one of them, by digest order, leaving the
count s43 exists to make trustworthy short by one with nothing saying
so.

But the rule was enforced one notch wider than it was written. It
refuses a second body; what it means to refuse is a second FINDING. A
finding is raised `open` and closed later, and the readers already
assume exactly that. `mart_review_rounds` says so in its own prose,
committed in s43:

> folded to the latest version of each `{change_id}__{round}__{seq}`
> finding first, so a finding re-authored from `open` to `fixed` is
> counted once and in one disposition bucket

The view folds by `version_rank DESC` (`views.sql:1770-1773`) and has
always been able to read that pair. The CLI could never write it. So
the disposition columns could only ever report the state each finding
was BORN in, and a repo that records its findings at review time — the
honest moment, while the reviewer still has them — could never record
that it fixed them.

s43's forty-seven findings hide this: they were all authored `fixed`
in one pass, after the fixing, with zero duplicate natural keys. That
is recording the outcome, not the review. It works only for a reviewer
who never writes anything down until the work is finished.

## What changes

One record-level predicate, one exemption at the authority, one CLI
verb, one reader added to an existing fold list.

`Finding::is_disposition_transition_of` (`canon-model`) answers whether
one finding is the SAME finding as another with only its disposition
moved: every identity/content field byte-equal — `change_id`, `round`,
`seq`, `severity`, `reviewer`, `summary`, `reviewed_sha`,
`introduced_by`, `file_ref` — and the disposition actually different.
The envelope is excluded, deliberately: a transition is authored at a
later instant by whoever closed it. It lives on the record because only
the kind knows which of its fields carry identity and which carry
state.

`canon_gate::promote` admits a second body at a `Unique` key only when
that predicate holds against the CURRENT committed version. Enforced at
the authority, not in a CLI verb: a hand-written body must clear the
same bar. Two transitions of one key in a single drain are still
refused — they have no defined order between them, and picking by
sorted path would be the meaningless judgement the duplicate refusal
exists to avoid.

`canon finding close` stages that transition. A separate verb, not a
flag on `add`, for one structural reason: it READS the committed record
and copies every content field, so a close CANNOT alter the severity,
the reviewer, the summary, or either sourced sha. Re-typing them would
turn a typo into promote's two-records-one-identity refusal — the
correct refusal for the wrong reason.

The committed record STAYS. The ledger is append-only and the pair IS
the history: `open` at one instant, `fixed` at another, each with its
own author and timestamp.

`canon query --kind finding` joins `subject` in the git-routed-but-
re-written fold list. Both kinds now legitimately carry more than one
version per key, and a reader that returned all of them would report a
closed finding as still open.

## What does NOT change

- `NaturalKeyRule::Unique` is not weakened to `Versioned`. `Versioned`
  admits ANY new body at an occupied key, so the two-reviewers-one-seq
  collapse would sail through it. Under `Unique` that body is still
  refused, byte for byte.
- `canon finding add` still refuses an occupied key. Its refusal now
  names `close` for the case the author actually meant.
- `mart_review_rounds` and `mart_review_totals` are untouched. Their
  fold was already correct; this change makes the input they always
  described actually authorable.
- No command can edit a recorded finding's severity, reviewer, or
  summary. A finding recorded wrongly stays wrongly recorded. That is
  the append-only ledger working, not a gap.

## What this does NOT establish

`close` records that someone, at a stamped time, declared a finding
closed by a named commit. canon does not check that the commit fixes
anything, that it touches the code the finding named, or that the
finding was real. `--resolution-sha` is checked for EXISTENCE in this
repository and nothing else — the same bar `canon finding add` already
applies. The author of a close and its beneficiary are the same party,
with no signature and no second party: the gap `canon evidence add`
states about attestations applies here unchanged.
