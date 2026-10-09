---
version: 1
slug: "src-pages-index-astro"
primary_target: "src/pages/index.astro"
related_targets: ["src/pages/ko/index.astro"]
---

# Landing surface brief

Scope: the canon site landing (`/` and `/ko/`), Persuade mode. The Starlight docs (Read mode) are re-themed into the same world.

Audience: leads and engineers on teams shipping with AI coding agents, deciding whether canon fits and how to install it.
Job: see in one viewport that canon refuses "done" without evidence, copy the install command, then read the flow, the comparison, and the docs.
Proof: the hero's PR panel uses the comp's words. Below it, every claim reuses existing site, README, or docs content, and other-tool claims link primary sources.
Constraints: comp words verbatim; never invent copy, taglines, or subtitles; the comparison is a real `<table>` at every width (horizontal scroll and a sticky first column on mobile); no vertical grid hairlines; EN and KO (KO is a faithful translation); experimental features labeled.
Approved comp: `.impeccable/comps/landing-pr.png` (Checks Panel, 1774x887). The previous Grid Manual comp `.impeccable/comps/landing.png` was rejected.

## Direction contract

THESIS: canon's promise is shown as the screen its audience already trusts: a pull request whose checks block the merge because canon's gate found scenarios with no evidence. Refuses the marketing hero with a feature grid, and refuses the rejected Grid Manual's vertical hairlines.

OWN-WORLD: GitHub-dark ground #0d1117, text #e6edf3, panel surfaces with 1px #30363d borders and 10–12px radii, horizontal row dividers only. State colours always come with a word: green passed, red failed and Merge blocked, grey waiting. Mona Sans bold for statements, Geist Mono for commands and logs.

STORY: the visitor reads "Agents say done. canon makes them prove it.", sees the canon gate check row red with its log open and the merge blocked, and copies `bunx @journeykit/canon init`. Then they follow the loop as a PR timeline, compare tools in a sourced capability matrix, run the 60-second demo, read what canon does and does not prove, and enter the docs.

FIRST VIEWPORT: left column: headline at about 92px, three lines; below it the command field with a divider and a Copy button (the primary action). Right column: the PR panel `agent/feat-cart → main` with a red Merge blocked badge and four check rows; the third row is expanded with its log `uncovered-cell cart.add.04 — spec scenario has no evidence record` and `exit 1`. Wordmark at top left, nav Docs, CLI, Concepts, GitHub at top right.

FORM: Checks Panel, the native grammar of pull-request checks (GitHub-style). Approved by the user as a comp. Seed key: not recorded; the build started with `build-phase start --comp`.

FINISH: unreviewed and undocumented is unfinished. This build ends with the finish review, the verdict, a new DESIGN.md plus `.impeccable/design.json`, and provenance on any shipping raster (none ship).

## Memorable moment

The checks resolve one by one, the canon gate turns red with its log open, and the Merge blocked badge answers once.

## Unresolved

- Only the canon gate check row toggles; the other chevrons are markers, because their logs would be invented copy.
