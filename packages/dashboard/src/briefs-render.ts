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
  changedSinceApproval: boolean;
  legacy: boolean;
  sections: Section[];
  findings: Finding[];
  counts: { blockers: number; advisories: number };
  approvable: boolean;
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

/** The Approve button is off while the brief has blockers, while a request
 * is in flight, and until both the name and the note hold something. */
export function approveDisabled(brief: Pick<Brief, "approvable">, by: string, note: string, busy: boolean): boolean {
  return !brief.approvable || busy || by.trim() === "" || note.trim() === "";
}

export interface BriefHandlers {
  /** Both fields changed: the page re-reads them and sets the button. */
  onInput: (card: HTMLElement) => void;
  onApprove: (brief: Brief, card: HTMLElement) => void;
}

export function renderBrief(brief: Brief, handlers: BriefHandlers): VNode {
  const blockers = brief.findings.filter((f) => f.blocksApproval).length;
  const form = h(
    "form",
    { class: "approve-form", autocomplete: "off" },
    h("label", {}, "Your name", h("input", { type: "text", class: "approve-by", autocomplete: "off", maxlength: "200" })),
    h("label", {}, "Your answer", h("input", { type: "text", class: "approve-note", autocomplete: "off", maxlength: "2000" })),
    h("button", { type: "submit", class: "approve-button", disabled: approveDisabled(brief, "", "", false) }, "Approve"),
    blockers > 0 ? h("p", { class: "approve-blocked" }, `Not approvable yet: ${blockers} blocker${blockers === 1 ? "" : "s"} to clear in the brief.`) : null,
  );
  const card = h(
    "article",
    { class: "panel brief", "data-change": brief.changeId, "data-sha256": brief.sha256, "data-approvable": brief.approvable ? "true" : "false" },
    h(
      "header",
      {},
      h("h2", {}, brief.changeId),
      h("p", { class: "panel-source" }, brief.brief, brief.subjects.length ? ` · subjects: ${brief.subjects.join(", ")}` : " · no subject adopts it"),
      brief.changedSinceApproval ? h("p", { class: "brief-changed" }, "Edited since it was approved: the approval no longer covers it.") : null,
    ),
    brief.sections.length > 0 ? brief.sections.map(renderSection) : h("pre", { class: "brief-raw" }, brief.text),
    h(
      "section",
      { class: "brief-section brief-check" },
      h("h3", {}, "Checks"),
      brief.findings.length === 0
        ? h("p", { class: "empty" }, "No findings.")
        : h("ul", { class: "findings" }, [...brief.findings].sort((a, b) => Number(b.blocksApproval) - Number(a.blocksApproval)).map(renderFinding)),
    ),
    brief.sections.length > 0 ? h("details", { class: "brief-raw-details" }, h("summary", {}, "Raw brief"), h("pre", { class: "brief-raw" }, brief.text)) : null,
    h("div", { class: "approve-message", role: "status" }),
    form,
  );
  const formNode = form;
  withOn(formNode, {
    input: (event) => handlers.onInput((event.currentTarget as HTMLElement).closest("article") as HTMLElement),
    submit: (event) => {
      event.preventDefault();
      handlers.onApprove(brief, (event.currentTarget as HTMLElement).closest("article") as HTMLElement);
    },
  });
  return card;
}

export function renderBriefs(briefs: Brief[], handlers: BriefHandlers): VNode {
  if (briefs.length === 0) return h("p", { class: "empty briefs-empty" }, EMPTY_MESSAGE);
  return h("div", { class: "briefs-list" }, briefs.map((brief) => renderBrief(brief, handlers)));
}
