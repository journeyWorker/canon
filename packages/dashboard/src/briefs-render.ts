import { h, withOn, type Child, type VNode } from "./vnode";

// The briefs page's view of `GET /api/briefs` (crates/canon-cli/src/
// dashboard_briefs.rs): the server parses each brief with `canon brief
// check`'s own parser, so this file only lays the result out.

export interface Finding {
  severity: "blocker" | "advisory";
  rule: string;
  line: number;
  message: string;
  hint: string;
  section: string | null;
  /** A blocker that stops the approval (coverage rules gate shipping, not approval). */
  blocksApproval: boolean;
}

export interface FlowStep {
  id: string;
  text: string;
  line: number;
}

export interface Section {
  key: string;
  role: string;
  heading: string | null;
  line: number;
  body: string[];
  steps: FlowStep[];
}

export interface Brief {
  changeId: string;
  subjects: string[];
  brief: string;
  text: string;
  sha256: string;
  language: string;
  languageSource?: string;
  languageNote?: string | null;
  changedSinceApproval: boolean;
  /** The relay of the approval this brief was edited after, when an agent recorded it. */
  previousRelay?: { role: string; actor: string; tool: string };
  legacy: boolean;
  sections: Section[];
  findings: Finding[];
  counts: { blockers: number; advisories: number };
  approvable: boolean;
}

/** An approved brief whose approval an agent recorded for the human (0.32). */
export interface RelayedBrief {
  changeId: string;
  brief: string;
  approval: string;
  relay: { role: string; actor: string; tool: string };
}

export const EMPTY_MESSAGE = "No brief awaits approval";

/** `code`, **bold** and the ==key sentence== marks; everything else is text. */
export function inline(text: string): Child[] {
  const out: Child[] = [];
  const parts = text.split(/(`[^`]+`|\*\*[^*]+\*\*|==)/);
  for (const part of parts) {
    if (part === "" || part === "==") continue;
    if (part.length > 4 && part.startsWith("**") && part.endsWith("**")) out.push(h("strong", {}, part.slice(2, -2)));
    else if (part.length > 2 && part.startsWith("`") && part.endsWith("`")) out.push(h("code", {}, part.slice(1, -1)));
    else out.push(part);
  }
  return out;
}

const LIST_ITEM = /^\s*(?:([-*])|(\d+)[.)])\s+(.*)$/;

/** Paragraphs, bulleted and numbered lists, and `###` sub-headings: the
 * subset of markdown a brief uses. Anything else is a paragraph of text. */
export function renderBody(lines: string[]): VNode[] {
  const out: VNode[] = [];
  let paragraph: string[] = [];
  let list: { ordered: boolean; items: VNode[] } | null = null;
  const flushParagraph = () => {
    if (paragraph.length) out.push(h("p", {}, inline(paragraph.join(" "))));
    paragraph = [];
  };
  const flushList = () => {
    if (list) out.push(h(list.ordered ? "ol" : "ul", {}, list.items));
    list = null;
  };
  for (const line of lines) {
    if (line.trim() === "") {
      flushParagraph();
      flushList();
      continue;
    }
    const heading = /^\s*#{3,6}\s+(.*)$/.exec(line);
    if (heading) {
      flushParagraph();
      flushList();
      out.push(h("h4", {}, inline(heading[1])));
      continue;
    }
    const item = LIST_ITEM.exec(line);
    if (item) {
      flushParagraph();
      const ordered = item[2] !== undefined;
      if (list && list.ordered !== ordered) flushList();
      list ??= { ordered, items: [] };
      list.items.push(h("li", {}, inline(item[3])));
      continue;
    }
    if (list && /^\s+\S/.test(line)) {
      // A wrapped list item continues the last one.
      const last = list.items[list.items.length - 1];
      last.children.push(" ", ...inline(line.trim()));
      continue;
    }
    flushList();
    paragraph.push(line.trim());
  }
  flushParagraph();
  flushList();
  return out;
}

export function renderSection(section: Section): VNode {
  const title = section.heading ?? section.key;
  const body =
    section.key === "flow" && section.steps.length > 0
      ? h(
          "ol",
          { class: "flow-steps" },
          section.steps.map((step) => h("li", { "data-step": step.id }, h("strong", {}, step.id), " ", inline(step.text.startsWith(step.id) ? step.text.slice(step.id.length).trimStart() : step.text))),
        )
      : renderBody(section.body);
  return h("section", { class: "brief-section", "data-section": section.key }, h("h3", {}, title), body);
}

export function renderFinding(finding: Finding): VNode {
  const gates = finding.severity === "blocker" && !finding.blocksApproval;
  const label = gates ? "blocker, gates shipping" : finding.severity;
  const cls = finding.blocksApproval ? "finding finding-blocker" : "finding finding-advisory";
  return h(
    "li",
    { class: cls, "data-rule": finding.rule },
    h("span", { class: "finding-label" }, label),
    " ",
    h("code", {}, finding.rule),
    ` line ${finding.line}: ${finding.message}`,
    finding.hint ? h("div", { class: "finding-hint" }, finding.hint) : null,
  );
}

export interface RuleGroup {
  rule: string;
  findings: Finding[];
}

/** The advisories grouped by rule id: the rule with the most findings
 * first, ties by rule id; findings keep their line order. */
export function groupAdvisories(findings: Finding[]): RuleGroup[] {
  const by = new Map<string, Finding[]>();
  for (const f of findings) {
    if (f.severity !== "advisory") continue;
    by.set(f.rule, [...(by.get(f.rule) ?? []), f]);
  }
  return [...by.entries()]
    .map(([rule, list]) => ({ rule, findings: [...list].sort((a, b) => a.line - b.line) }))
    .sort((a, b) => b.findings.length - a.findings.length || (a.rule < b.rule ? -1 : a.rule > b.rule ? 1 : 0));
}

export function renderRuleGroup(group: RuleGroup): VNode {
  return h(
    "section",
    { class: "rule-group", "data-rule-group": group.rule },
    h("h4", {}, h("code", {}, group.rule), " ", h("span", { class: "badge badge-advisory rule-count" }, String(group.findings.length))),
    h(
      "ul",
      { class: "findings" },
      group.findings.map((f) => h("li", { class: "finding finding-advisory", "data-rule": f.rule }, `line ${f.line}: ${f.message}`, f.hint ? h("div", { class: "finding-hint" }, f.hint) : null)),
    ),
  );
}

/** The one Approve button is off until a brief is selected, while a request
 * is in flight, and until both the name and the note hold something. */
export function approveDisabled(selected: number, by: string, note: string, busy: boolean): boolean {
  return selected === 0 || busy || by.trim() === "" || note.trim() === "";
}

/** "None selected" / "1 selected" / "3 selected". */
export function selectionText(count: number): string {
  return count === 0 ? "None selected" : `${count} selected`;
}

/** The `POST /api/briefs/approve` body: one name and answer, and for each
 * selected brief its change id and the sha256 of the file this page read. */
export function approvalRequest(selected: Pick<Brief, "changeId" | "sha256">[], by: string, note: string): { by: string; note: string; briefs: { changeId: string; sha256: string }[] } {
  return { by, note, briefs: selected.map((b) => ({ changeId: b.changeId, sha256: b.sha256 })) };
}

export interface BriefHandlers {
  /** A selection box or a field changed: the page re-reads them and sets the count and the button. */
  onChange: () => void;
  /** The one form was submitted. */
  onApprove: () => void;
}

export interface Refusal {
  changeId: string;
  code: string;
  message: string;
}

export interface ApproveFailure {
  code: string;
  message: string;
  /** Absent when the server had no hint (atip/1 omits the member; `null` is tolerated). */
  hint?: string | null;
  /** The briefs a multi-brief call refused, each with its own code. */
  refused: Refusal[];
}

/** The failure an approve answer carries: `error` of the atip/1 envelope
 * (`hint` absent when there is none) and the refused briefs in `data.refused`. */
export function approveFailure(
  json: { error?: { code: string; message: string; hint?: string | null }; data?: { refused?: Refusal[] } },
  status: number,
): ApproveFailure {
  const error = json.error ?? { code: "UNKNOWN", message: `the server answered ${status}` };
  return { code: error.code, message: error.message, hint: error.hint ?? undefined, refused: json.data?.refused ?? [] };
}

/** What a refused approval shows: the server's message and hint, and each
 * refused brief by name, with how to read it again. */
export function renderApproveError(failure: ApproveFailure, reload: VNode | null): VNode {
  return h(
    "div",
    { class: "approve-error", role: "alert" },
    h("p", {}, failure.message),
    failure.refused.length > 0
      ? h("ul", { class: "refused-briefs" }, failure.refused.map((r) => h("li", { "data-change": r.changeId }, h("strong", {}, r.changeId), ` ${r.code}: ${r.message}`)))
      : null,
    failure.hint ? h("p", { class: "finding-hint" }, failure.hint) : null,
    reload,
  );
}

/** The change ids a refusal names, for marking their cards. */
export function refusedIds(failure: Pick<ApproveFailure, "refused">): string[] {
  return failure.refused.map((r) => r.changeId);
}

export function renderApproveBar(handlers: BriefHandlers): VNode {
  const form = h(
    "form",
    { class: "approve-form approve-bar", autocomplete: "off" },
    h("p", { class: "selected-count", role: "status" }, selectionText(0)),
    h("label", {}, "Your name", h("input", { type: "text", class: "approve-by", autocomplete: "off", maxlength: "200" })),
    h("label", {}, "Your answer", h("input", { type: "text", class: "approve-note", autocomplete: "off", maxlength: "2000" })),
    h("button", { type: "submit", class: "approve-button", disabled: approveDisabled(0, "", "", false) }, "Approve selected"),
    h("div", { class: "approve-message", role: "status" }),
  );
  return withOn(form, {
    input: () => handlers.onChange(),
    submit: (event) => {
      event.preventDefault();
      handlers.onApprove();
    },
  });
}

export function renderBrief(brief: Brief, handlers: BriefHandlers): VNode {
  const blockers = brief.findings.filter((f) => f.blocksApproval).length;
  const others = brief.findings.filter((f) => f.severity !== "advisory");
  const groups = groupAdvisories(brief.findings);
  const select = h("input", { type: "checkbox", class: "brief-select", "data-change": brief.changeId, disabled: !brief.approvable, "aria-label": `Select ${brief.changeId}` });
  withOn(select, { change: () => handlers.onChange() });
  const card = h(
    "article",
    { class: "panel brief", "data-change": brief.changeId, "data-sha256": brief.sha256, "data-approvable": brief.approvable ? "true" : "false" },
    h(
      "header",
      {},
      h("label", { class: "brief-select-label" }, select, " ", h("h2", {}, brief.changeId)),
      h("p", { class: "panel-source" }, brief.brief, brief.subjects.length ? ` · subjects: ${brief.subjects.join(", ")}` : " · no subject adopts it"),
      brief.counts.blockers > 0 || brief.counts.advisories > 0
        ? h(
            "p",
            { class: "brief-counts" },
            brief.counts.blockers > 0 ? h("span", { class: "badge badge-blocker" }, `${brief.counts.blockers} blocker${brief.counts.blockers === 1 ? "" : "s"}`) : null,
            brief.counts.blockers > 0 && brief.counts.advisories > 0 ? " " : null,
            brief.counts.advisories > 0 ? h("span", { class: "badge badge-advisory" }, advisoryText(brief.counts.advisories)) : null,
          )
        : null,
      h("p", { class: "brief-language" }, languageText(brief)),
      brief.changedSinceApproval ? h("p", { class: "brief-changed" }, "Edited since it was approved: the approval no longer covers it.", brief.previousRelay ? ` (that approval was ${relayText(brief.previousRelay)})` : "") : null,
      blockers > 0 ? h("p", { class: "approve-blocked" }, `Not approvable yet: ${blockers} blocker${blockers === 1 ? "" : "s"} to clear in the brief.`) : null,
    ),
    brief.sections.length > 0 ? brief.sections.map(renderSection) : h("pre", { class: "brief-raw" }, brief.text),
    h(
      "section",
      { class: "brief-section brief-check" },
      h("h3", {}, "Checks"),
      brief.findings.length === 0 ? h("p", { class: "empty" }, "No findings.") : null,
      others.length > 0 ? h("ul", { class: "findings" }, [...others].sort((a, b) => Number(b.blocksApproval) - Number(a.blocksApproval)).map(renderFinding)) : null,
      groups.map(renderRuleGroup),
    ),
    brief.sections.length > 0 ? h("details", { class: "brief-raw-details" }, h("summary", {}, "Raw brief"), h("pre", { class: "brief-raw" }, brief.text)) : null,
  );
  return card;
}

/** "language: ko (detected)" / "language: en (configured; the text reads as ko)". */
export function languageText(brief: Pick<Brief, "language" | "languageSource" | "languageNote">): string {
  const source = brief.languageSource ?? "detected";
  return `language: ${brief.language} (${source}${brief.languageNote ? `; ${brief.languageNote}` : ""})`;
}

/** "1 advisory" / "3 advisories". */
export function advisoryText(count: number): string {
  return `${count} ${count === 1 ? "advisory" : "advisories"}`;
}

/** The page's total line: advisories across the listed briefs. */
export function advisoryTotal(briefs: Pick<Brief, "counts">[]): string {
  const total = briefs.reduce((sum, b) => sum + b.counts.advisories, 0);
  if (total === 0) return "No advisories in the listed briefs.";
  const holding = briefs.filter((b) => b.counts.advisories > 0).length;
  return `${advisoryText(total)} in ${holding} of ${briefs.length} brief${briefs.length === 1 ? "" : "s"}.`;
}

export function renderBriefs(briefs: Brief[], handlers: BriefHandlers): VNode {
  if (briefs.length === 0) return h("p", { class: "empty briefs-empty" }, EMPTY_MESSAGE);
  return h("div", { class: "briefs-list" }, h("p", { class: "briefs-total" }, advisoryTotal(briefs)), renderApproveBar(handlers), briefs.map((brief) => renderBrief(brief, handlers)));
}

/** "relayed by planner `impl-1` via ask": how an agent recorded the human's answer. */
export function relayText(relay: { role: string; actor: string; tool: string }): string {
  return `relayed by ${relay.role} \`${relay.actor}\` via ${relay.tool}`;
}

/** The approved briefs an agent recorded the approval of, so the human can tell them from their own answers. */
export function renderRelayed(relayed: RelayedBrief[]): VNode | null {
  if (relayed.length === 0) return null;
  return h(
    "section",
    { class: "panel briefs-relayed" },
    h("h2", {}, "Approved through an agent"),
    h("ul", { class: "relayed-list" }, relayed.map((one) => h("li", { "data-change": one.changeId }, h("strong", {}, one.changeId), ` · ${one.brief} · ${relayText(one.relay)}`))),
  );
}

/** The whole page body: the waiting briefs (or the empty message), then the relayed approvals. The relayed panel stays when the last waiting brief is approved. */
export function renderBriefsPage(briefs: Brief[], relayed: RelayedBrief[], handlers: BriefHandlers): VNode[] {
  const panel = renderRelayed(relayed);
  return [renderBriefs(briefs, handlers), ...(panel ? [panel] : [])];
}
