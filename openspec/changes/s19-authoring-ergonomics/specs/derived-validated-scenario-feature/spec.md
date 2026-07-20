## ADDED Requirements

### Requirement: `--feature` is optional on `canon scenario new`, defaulting to the tag-derived path
`canon scenario new <tag> --title <label>` (with `--feature` omitted) SHALL
resolve the target `.feature` file's path by deriving it from `<tag>`'s
own `area`/`surface` segments (`ScenarioId::area()`/`ScenarioId::surface()`)
through the SAME `features/kind=feature/area=<area>/<surface>.feature`
join `canon feature new` already builds — never a second, independently
hand-typed derivation. Resolution SHALL require exactly ONE configured
`specs.roots[]` entry; a repo configured with more than one root SHALL
refuse loud (exit `2`, naming the ambiguity) rather than guess which root
the derived file belongs under, mirroring `canon feature new`'s own
existing multi-root refusal.

#### Scenario: Omitting --feature derives the exact path feature new would scaffold
- **WHEN** `canon scenario new wall.render.01 --title 'renders the wall'`
  runs (no `--feature`) against a repo with one configured spec root
- **THEN** the command appends/creates
  `<root>/features/kind=feature/area=wall/render.feature` — byte-identical
  to the path `canon feature new wall.render --title '…'` would have
  scaffolded — and exits `0`

#### Scenario: Omitting --feature under an ambiguous multi-root config refuses loud
- **WHEN** `canon scenario new wall.render.01 --title '…'` runs (no
  `--feature`) against a repo whose `canon.yaml` `specs.roots[]` declares
  two entries
- **THEN** the command refuses with a named ambiguity error and exits `2`
  — zero bytes written, mirroring `canon feature new`'s own refusal for
  the identical multi-root shape

### Requirement: An explicit --feature must resolve under a configured specs.roots[] entry
When `--feature <path>` IS supplied, `canon scenario new` SHALL validate
that the resolved absolute path falls under at least one configured
`specs.roots[]` entry's root directory (a canonicalized, path-component-wise
prefix check — never a naive string prefix, so a root named `specs` never
falsely accepts a sibling directory like `specs2`). A path resolving
OUTSIDE every configured root SHALL be refused (exit `2`, naming the
attempted absolute path and every configured root) with ZERO bytes
written — never a silent write outside the validated corpus. This
validation runs BEFORE the existing duplicate-tag and target-file
checks, which are otherwise unchanged.

#### Scenario: A --feature path outside every configured root is refused
- **WHEN** `canon scenario new wall.render.03 --title guess --feature
  wall.render` runs against a repo whose only configured spec root is
  `specs/`
- **THEN** the command refuses with an error naming the attempted path
  (resolving outside `specs/`) and the configured root(s), exits `2`, and
  writes NO file anywhere — the repo-root orphan this construct used to
  produce no longer occurs

#### Scenario: A --feature path under a configured root, in a non-canonical subpath, is accepted
- **WHEN** `canon scenario new wall.render.04 --title '…' --feature
  features/kind=feature/area=wall/misc.feature` runs against a repo whose
  configured spec root is `specs/` and the resolved path falls under it
- **THEN** the command proceeds exactly as before this change (subject to
  the existing duplicate-tag/target-file guards) — root-membership
  validation does not force the derived canonical filename when a path is
  explicitly given

### Requirement: Existing duplicate-tag and target-file guards are unchanged
Introducing the default-derivation and root-membership checks above SHALL
NOT alter `canon scenario new`'s existing refusal behavior for a tag that
already exists somewhere under a configured spec root, or for a tag that
already exists in the specific target `.feature` file — both guards SHALL
continue to run, in the same order, with the same exit code and message
shape, whether the target path was derived or explicitly supplied.

#### Scenario: A duplicate tag is still refused when the path is derived
- **WHEN** `canon scenario new wall.render.01 --title '…'` runs a second
  time (no `--feature`) against a repo where `@wall.render.01` already
  exists under a configured spec root
- **THEN** the command refuses exactly as it does today for an explicit
  `--feature` pointing at the same duplicate — exit `2`, naming the tag
  and the spec root, zero bytes written
