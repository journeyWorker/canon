---
name: canon
description: Grid Manual — an International Typographic Style engineering manual shared by the canon landing page and the Starlight docs.
colors:
  ground: "#f0f3f0"
  ink: "#0b0b0b"
  ink-2: "#4a4a45"
  signal: "#ff4a1c"
  rule: "#b4b6b1"
  rule-soft: "#d9dad4"
  paper: "#ffffff"
  code-hairline: "#2a2a28"
  code-muted: "#a3a39b"
typography:
  display:
    fontFamily: "Spline Sans Variable, Noto Sans KR Variable, system-ui, sans-serif"
    fontSize: "calc(103 * var(--u))"
    fontWeight: 700
    lineHeight: 1.15
    letterSpacing: "-0.045em"
  figure-numeral:
    fontFamily: "Spline Sans Variable, system-ui, sans-serif"
    fontSize: "calc(111 * var(--u))"
    fontWeight: 700
    lineHeight: 0.8
    letterSpacing: "-0.05em"
  headline:
    fontFamily: "Spline Sans Variable, Noto Sans KR Variable, system-ui, sans-serif"
    fontSize: "clamp(40px, 4.3vw, 66px)"
    fontWeight: 700
    lineHeight: 1.04
    letterSpacing: "-0.035em"
  title:
    fontFamily: "Spline Sans Variable, Noto Sans KR Variable, system-ui, sans-serif"
    fontSize: "24px"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "-0.02em"
  lead:
    fontFamily: "Spline Sans Variable, Noto Sans KR Variable, system-ui, sans-serif"
    fontSize: "clamp(20px, 1.6vw, 25px)"
    fontWeight: 400
    lineHeight: 1.4
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Spline Sans Variable, Noto Sans KR Variable, system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Inconsolata Variable, Noto Sans KR Variable, ui-monospace, Menlo, monospace"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.4
  mono:
    fontFamily: "Inconsolata Variable, Noto Sans KR Variable, ui-monospace, Menlo, monospace"
    fontSize: "17px"
    fontWeight: 400
    lineHeight: 1.6
rounded:
  none: "0"
spacing:
  margin: "calc(32 * var(--u))"
  gutter: "calc(25 * var(--u))"
  margin-mobile: "16px"
  gutter-mobile: "12px"
components:
  command-bar:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ground}"
    typography: "{typography.mono}"
    rounded: "{rounded.none}"
    padding: "0 calc(21 * var(--u))"
    height: "calc(68 * var(--u))"
  copy-button:
    textColor: "{colors.ground}"
    typography: "{typography.mono}"
  copy-button-hover:
    textColor: "{colors.signal}"
  guide-link:
    textColor: "{colors.ink}"
  guide-link-hover:
    textColor: "{colors.signal}"
  figure-panel:
    backgroundColor: "{colors.signal}"
    textColor: "{colors.ink}"
    rounded: "{rounded.none}"
    padding: "calc(22 * var(--u)) calc(24 * var(--u)) calc(27 * var(--u)) calc(25 * var(--u))"
  code-block:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ground}"
    typography: "{typography.mono}"
    rounded: "{rounded.none}"
    padding: "22px 26px"
---

# Design System: canon

## Overview

**Creative North Star: "The Grid Manual"**

canon reads as an engineering standard set in the International Typographic Style: every claim sits flush-left on a strict twelve-column grid, and every proof is a numbered figure the reader can check. The landing page and the Starlight docs are one world: the same off-white ground, near-black ink, grey hairline column rules, and one signal-orange field.

The first viewport is scaled from the approved 1536x1024 comp through a single unit (`--u`, one comp pixel, bounded by viewport width, height, and 1.35px), so the composition holds its proportions rather than reflowing. Below the fold the page switches to fluid `clamp()` sizes on the same twelve columns. Density is low and typographic; there are no cards, no shadows, no gradients, no rounded corners.

The world refuses the dev-tool hero with a glowing terminal and a feature-card grid. Proof is drawn from real build-time data (Fig. 1 is generated from canon's own corpus), never illustrated.

**Key Characteristics:**
- Twelve visible hairline column rules over the whole page (four on mobile).
- One signal-orange field that owns a whole region (Fig. 1), plus orange as hover, focus, and selection.
- Spline Sans bold at poster scale with tight negative tracking; Inconsolata for commands, captions, and data.
- Square corners everywhere; black command bar and black code blocks.
- Numbered "Fig." captions and black rules that carry stations and section heads.

## Colors

A near-monochrome engineering palette with one loud signal.

### Primary
- **Signal Orange** (signal): the Fig. 1 panel field, the active state of links and Copy, the focus outline, text selection, the Expressive Code active-tab indicator and copy tooltip, and Starlight's accent in both themes. Text on it is always ink.

### Neutral
- **Manual Ground** (ground): page background on the landing and the light docs theme; text colour inside black fields.
- **Press Ink** (ink): text, rules that carry structure, the command bar, code blocks, and the dark docs background.
- **Graphite** (ink-2): secondary text and Starlight's mid grey in light theme.
- **Column Rule** (rule): the always-visible twelve-column hairlines and the docs hairline.
- **Soft Rule** (rule-soft): lighter docs dividers.
- **Paper** (paper): reserved white; rarely used.
- **Code Hairline** (code-hairline) and **Code Muted** (code-muted): divider and titlebar text inside black code frames.

### Named Rules
**The One Field Rule.** Signal orange fills exactly one region per surface (Fig. 1 on the landing); elsewhere it appears only as state: hover, focus, selection, active tab.

**The Visible Grid Rule.** Column rules are always drawn, behind content, never hidden at any breakpoint.

## Typography

**Display Font:** Spline Sans Variable (with Noto Sans KR Variable, Apple SD Gothic Neo, Malgun Gothic, system-ui)
**Body Font:** Spline Sans Variable
**Label/Mono Font:** Inconsolata Variable (with Noto Sans KR Variable, ui-monospace, Menlo)

**Character:** A geometric grotesk set bold and tightly tracked for statements, regular for reading; a plain monospace marks anything a reader could copy, count, or check.

### Hierarchy
- **Display** (700, `calc(103 * var(--u))`, 1.15, -0.045em): the hero headline only. Carries a 0.014em text-stroke to match the comp's heavier weight. Fits seven columns on desktop; on mobile it is sized from the viewport width.
- **Figure numeral** (700, `calc(111 * var(--u))`, 0.8, -0.05em): the gate count at the foot of Fig. 1.
- **Headline** (700, clamp(40px, 4.3vw, 66px), 1.04, -0.035em): section heads, set on a 1.5px ink rule.
- **Title** (700, 20–24px, 1.2, -0.02em): sub-heads, docs-list titles, Fig. 3 product names.
- **Lead** (400, clamp(20px, 1.6vw, 25px), 1.4): section introductions, max 44ch.
- **Body** (400, 18px, 1.5): running text, max 52–62ch; docs body at 1.65 line-height in a 46rem column.
- **Label / Mono** (Inconsolata, 14–17px): figure captions, table notes, commands, code.

### Named Rules
**The Tight Display Rule.** Bold display sizes always carry negative tracking (-0.02em to -0.05em); text sizes stay near zero.

**The Mono Means Checkable Rule.** Monospace is for commands, data, and captions, never for headings or prose.

## Layout

A twelve-column grid inside page margins (`spacing.margin`), with each column's content offset by `spacing.gutter` from the column rule on its left. The first viewport is a fixed composition in comp units: headline, subline, command bar, and guide link in columns 1–7; the orange Fig. 1 panel in columns 8–12; a full-width black rule with six stations on column lines at the foot. Sections below use generous vertical rhythm (clamp(96px, 10vw, 168px) top padding) and place headings on the gutter line.

At ≤900px the grid drops to four columns, margins to 16px and gutters to 12px; the hero stacks, Fig. 1 follows the copy, stations wrap to three columns, and multi-column lists collapse to one.

### Named Rules
**The Flush-Left Rule.** Every text block starts on a column gutter line; nothing is centred.

## Elevation & Depth

Flat. There are no shadows anywhere: Starlight's `--sl-shadow-*` are set to none and Expressive Code's frame shadow is removed. Depth comes only from fills (ink, signal) against the ground and from ink rules of 1–2px.

### Named Rules
**The Flat Page Rule.** Separation is a rule or a fill, never a shadow, blur, or gradient.

## Shapes

Square corners throughout (`rounded.none`), including code frames, tabs, and table cells. Structure is drawn with lines: grey hairline column rules for the grid, 1–2px ink rules for section heads, table rows, and the station rule. Links are underlined (1px at rest, 2px on hover) or carry a 2-unit bottom border on the guide link.

## Components

### Command Bar
Black field holding the install command in mono with a `$ ` prompt and a mono "Copy" button flush right. Copy turns signal on hover and after copying; a visually hidden status span announces the result. This is the primary action.

### Guide Link
Inline link with an arrow SVG, underlined by a 2-unit currentColor border; turns signal on hover.

### Figure Panel (Fig. 1)
Signal-orange field spanning five columns, mono caption at the top, the corpus drawn as one ink hairline per scenario (canvas, SVG fallback), and the figure numeral at its foot. Built from `src/data/corpus-figure.json`; never hand-drawn.

### Station Rule (Fig. 2)
A black rule carrying six stations aligned to column lines; each station a numbered mono label with a bold title.

### Comparison Table (Fig. 3)
Full-width table with 2px ink top rule per row group, 1px ink row rules, bold product names, and 14px source notes with underlined primary-source links. Experimental canon features are marked in text.

### Code Blocks
Black field, mono 17px at 1.6, square, no shadow, comments in a muted tone. Docs code uses Expressive Code with the same black field, signal active-tab indicator, and hidden terminal dots.

### Navigation
Bold wordmark at left; plain text links at right, underlined on hover. Docs use Starlight's chrome re-coloured to ground/ink with signal accent.

## Do's and Don'ts

### Do:
- **Do** keep the twelve column rules visible behind every surface.
- **Do** put signal orange on one full region per surface and use it otherwise only for hover, focus (2px outline, 3px offset), and selection.
- **Do** set every proof as a numbered "Fig." with a mono caption and real data.
- **Do** keep corners square and separation flat: rules and fills only.
- **Do** start every text block on a gutter line, flush-left.

### Don't:
- **Don't** add shadows, gradients, blur, or rounded corners.
- **Don't** use a glowing-terminal hero or a grid of feature cards.
- **Don't** set headings or prose in the monospace.
- **Don't** put white text on signal orange; text on it is ink.
