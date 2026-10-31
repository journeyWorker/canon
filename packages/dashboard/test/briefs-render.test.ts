import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { advisoryTotal, approvalRequest, approveDisabled, approveFailure, EMPTY_MESSAGE, groupAdvisories, inline, refusedIds, renderApproveError, renderBody, renderBrief, renderBriefs, renderRelayed, renderBriefsPage, relayText, selectionText, type Brief, type BriefHandlers } from "../src/briefs-render";
import { elements, escapeHtml, toHtml } from "../src/vnode";

const handlers: BriefHandlers = { onChange: () => {}, onApprove: () => {} };

function brief(over: Partial<Brief> = {}): Brief {
  return {
    changeId: "add-login",
    subjects: ["demo"],
    brief: ".canon/changes/add-login/brief.md",
    text: "# Add login\n",
    sha256: "a".repeat(64),
    language: "en",
    changedSinceApproval: false,
    legacy: false,
    sections: [
      { key: "intent", role: "context", heading: "Intent", line: 3, body: ["Add login", "", "Visitors lose **their cart**."], steps: [] },
      {
        key: "flow",
        role: "normative",
        heading: "User flow",
        line: 9,
        body: ["1. F1 sees the form", "2. F2 submits a wrong password"],
        steps: [
          { id: "F1", text: "F1 sees the form", line: 10 },
          { id: "F2", text: "F2 submits a wrong password", line: 11 },
        ],
      },
      { key: "questions", role: "question", heading: "Questions and assumptions", line: 20, body: ["- Does `SSO` count?", "- None other."], steps: [] },
    ],
    findings: [
      { severity: "advisory", rule: "prose.long-sentence", line: 5, message: "a long one", hint: "split it", section: "outcome", blocksApproval: false },
      { severity: "blocker", rule: "brief.placeholder", line: 7, message: "TODO", hint: "write it", section: "intent", blocksApproval: true },
    ],
    counts: { blockers: 1, advisories: 1 },
    approvable: false,
    ...over,
  };
}

const HOSTILE = `<script>alert(1)</script><img src=x onerror=alert(2)> & "quoted"`;

test("brief text is shown as text: markup in any field never becomes an element", () => {
  const hostile = brief({
    changeId: HOSTILE,
    subjects: [HOSTILE],
    brief: HOSTILE,
    text: HOSTILE,
    sections: [
      { key: "intent", role: "context", heading: HOSTILE, line: 1, body: [HOSTILE, `- ${HOSTILE}`, `### ${HOSTILE}`, `**${HOSTILE}** and \`${HOSTILE}\``], steps: [] },
      { key: "flow", role: "normative", heading: null, line: 2, body: [], steps: [{ id: "F1", text: `F1 ${HOSTILE}`, line: 3 }] },
    ],
    findings: [{ severity: "blocker", rule: HOSTILE, line: 1, message: HOSTILE, hint: HOSTILE, section: null, blocksApproval: true }],
  });
  const tree = renderBrief(hostile, handlers);
  const tags = new Set(elements(tree).map((e) => e.tag));
  for (const forbidden of ["script", "img", "iframe", "a", "style"]) expect(tags.has(forbidden)).toBe(false);
  for (const node of elements(tree)) {
    for (const name of Object.keys(node.attrs)) expect(name.startsWith("on")).toBe(false);
  }
  const html = toHtml(tree);
  expect(html).not.toContain("<script>");
  expect(html).not.toContain("<img");
  expect(html).toContain(escapeHtml(HOSTILE));
});

test("the page code never assigns HTML", () => {
  for (const file of ["briefs.ts", "briefs-render.ts", "vnode.ts", "nav.ts"]) {
    const source = readFileSync(join(import.meta.dir, "..", "src", file), "utf-8");
    expect(source.replace(/\/\/.*$/gm, "")).not.toMatch(/innerHTML|outerHTML|insertAdjacentHTML|document\.write/);
  }
});

test("a brief shows its change id, subjects, and each section with its heading", () => {
  const html = toHtml(renderBrief(brief(), handlers));
  expect(html).toContain("<h2>add-login</h2>");
  expect(html).toContain("subjects: demo");
  expect(html).toContain("<h3>Intent</h3>");
  expect(html).toContain("<h3>User flow</h3>");
  expect(html).toContain("<h3>Questions and assumptions</h3>");
  expect(html).toContain("<strong>their cart</strong>");
  expect(html).toContain("<code>SSO</code>");
});

test("flow steps keep their ids in an ordered list", () => {
  const flow = elements(renderBrief(brief(), handlers)).find((e) => e.attrs["data-section"] === "flow")!;
  const steps = elements(flow).filter((e) => e.tag === "li");
  expect(steps.map((s) => s.attrs["data-step"])).toEqual(["F1", "F2"]);
  expect(toHtml(steps[0])).toContain("<strong>F1</strong> sees the form");
});

test("a section with a Korean heading is shown under that heading", () => {
  const ko = brief({ sections: [{ key: "flow", role: "normative", heading: "사용자 흐름", line: 1, body: [], steps: [{ id: "F1", text: "F1 로그인", line: 2 }] }] });
  expect(toHtml(renderBrief(ko, handlers))).toContain("<h3>사용자 흐름</h3>");
});

test("blockers are red and listed first, advisories muted", () => {
  const findings = elements(renderBrief(brief(), handlers)).filter((e) => e.tag === "li" && e.attrs["data-rule"]);
  expect(findings.map((f) => f.attrs["data-rule"])).toEqual(["brief.placeholder", "prose.long-sentence"]);
  expect(findings[0].attrs.class).toContain("finding-blocker");
  expect(findings[1].attrs.class).toContain("finding-advisory");
});

test("a coverage blocker does not stop approval and is not drawn red", () => {
  const coverage = brief({
    approvable: true,
    findings: [{ severity: "blocker", rule: "brief.flow-uncovered", line: 10, message: "F1", hint: "tag a scenario", section: "flow", blocksApproval: false }],
  });
  const item = elements(renderBrief(coverage, handlers)).find((e) => e.attrs["data-rule"] === "brief.flow-uncovered")!;
  expect(item.attrs.class).toContain("finding-advisory");
  expect(toHtml(item)).toContain("blocker, gates shipping");
});

test("report.dashboard.13: a brief with blockers cannot be selected, an approvable one can", () => {
  const blocked = elements(renderBrief(brief(), handlers)).find((e) => e.tag === "input" && e.attrs.type === "checkbox")!;
  expect(blocked.attrs.disabled).toBe(true);
  expect(toHtml(renderBrief(brief(), handlers))).toContain("Not approvable yet: 1 blocker");
  const open = elements(renderBrief(brief({ approvable: true }), handlers)).find((e) => e.tag === "input" && e.attrs.type === "checkbox")!;
  expect(open.attrs.disabled).toBe(false);
  expect(open.attrs["data-change"]).toBe("add-login");
});

test("report.dashboard.13: the page has one name, one answer and one button, and shows how many are selected", () => {
  const list = [brief({ approvable: true }), brief({ changeId: "b", approvable: true }), brief({ changeId: "c", approvable: true })];
  const page = renderBriefs(list, handlers);
  const els = elements(page);
  expect(els.filter((e) => e.tag === "button" && e.attrs.class === "approve-button")).toHaveLength(1);
  expect(els.filter((e) => e.tag === "input" && e.attrs.class === "approve-by")).toHaveLength(1);
  expect(els.filter((e) => e.tag === "input" && e.attrs.class === "approve-note")).toHaveLength(1);
  expect(els.filter((e) => e.tag === "input" && e.attrs.type === "checkbox")).toHaveLength(3);
  expect(toHtml(page)).toContain("None selected");
  expect(selectionText(0)).toBe("None selected");
  expect(selectionText(3)).toBe("3 selected");
});

test("report.dashboard.13: Approve waits for a selection, the name and the answer, and for a request in flight", () => {
  expect(approveDisabled(0, "Ana", "Go.", false)).toBe(true);
  expect(approveDisabled(2, "", "Go.", false)).toBe(true);
  expect(approveDisabled(2, "Ana", "  ", false)).toBe(true);
  expect(approveDisabled(2, "Ana", "Go.", true)).toBe(true);
  expect(approveDisabled(2, "Ana", "Go.", false)).toBe(false);
});

test("report.dashboard.13: the request carries the name and answer once and each selected brief's change id and sha256", () => {
  const picked = [brief({ changeId: "a", sha256: "1".repeat(64) }), brief({ changeId: "b", sha256: "2".repeat(64) })];
  expect(approvalRequest(picked, "Ana", "Go.")).toEqual({
    by: "Ana",
    note: "Go.",
    briefs: [
      { changeId: "a", sha256: "1".repeat(64) },
      { changeId: "b", sha256: "2".repeat(64) },
    ],
  });
});

test("report.dashboard.14: a refused multi-brief approval names each refused brief and says to read it again", () => {
  const failure = {
    code: "BRIEF_APPROVE_REFUSED",
    message: "1 of 3 briefs refused, so none was approved",
    hint: "reload this page to read the briefs named as they are now",
    refused: [{ changeId: "b", code: "BRIEF_CHANGED_SINCE_PAGE_LOAD", message: "b changed after this page loaded it" }],
  };
  expect(refusedIds(failure)).toEqual(["b"]);
  const html = toHtml(renderApproveError(failure, null));
  expect(html).toContain("<strong>b</strong> BRIEF_CHANGED_SINCE_PAGE_LOAD: b changed after this page loaded it");
  expect(html).toContain("reload this page");
  expect(html).toContain("none was approved");
});

test("report.dashboard.14: refusal text from the server is shown as text, never markup", () => {
  const html = toHtml(renderApproveError({ code: "X", message: HOSTILE, hint: HOSTILE, refused: [{ changeId: HOSTILE, code: "C", message: HOSTILE }] }, null));
  expect(html).not.toContain("<script>");
  expect(html).toContain(escapeHtml(HOSTILE));
});

test("atip/1: an error without a hint member has no hint, and a null hint is tolerated", () => {
  const absent = approveFailure({ error: { code: "BRIEF_APPROVE_REFUSED", message: "none approved" }, data: { refused: [{ changeId: "b", code: "C", message: "m" }] } }, 409);
  expect(absent.hint).toBeUndefined();
  expect(absent.refused).toEqual([{ changeId: "b", code: "C", message: "m" }]);
  expect(toHtml(renderApproveError(absent, null))).not.toContain("finding-hint");
  const nulled = approveFailure({ error: { code: "X", message: "m", hint: null } }, 400);
  expect(nulled.hint).toBeUndefined();
  expect(toHtml(renderApproveError(nulled, null))).not.toContain("finding-hint");
  expect(approveFailure({ error: { code: "X", message: "m", hint: "reload" } }, 400).hint).toBe("reload");
  expect(approveFailure({}, 502)).toEqual({ code: "UNKNOWN", message: "the server answered 502", hint: undefined, refused: [] });
});

test("atip/1: no dashboard source reads an old canon JSON key", () => {
  const old = /canonVersion|hasMore|nextCursor|\.why\b|PLAN_CHANGED|INVALID_CURSOR|INPUT_ELEMENT_FAILED|meta\.(unchanged|waitingOn|nextOmitted|returned|total|context)/;
  for (const file of ["briefs.ts", "briefs-render.ts", "status.ts", "banner.ts", "snapshot.ts", "main.ts"]) {
    expect(readFileSync(join(import.meta.dir, "..", "src", file), "utf-8")).not.toMatch(old);
  }
});

test("the selection boxes are empty on load and the fields hold no value, autofill off", () => {
  const els = elements(renderBriefs([brief({ approvable: true })], handlers));
  for (const input of els.filter((e) => e.tag === "input" && e.attrs.type === "text")) {
    expect("value" in input.attrs).toBe(false);
    expect(input.attrs.autocomplete).toBe("off");
  }
  expect(els.filter((e) => e.tag === "input" && e.attrs.type === "checkbox").every((e) => !("checked" in e.attrs))).toBe(true);
});

test("no briefs shows the empty message", () => {
  expect(toHtml(renderBriefs([], handlers))).toContain(EMPTY_MESSAGE);
  expect(EMPTY_MESSAGE).toBe("No brief awaits approval");
});

test("a brief with no sections (pre-0.19) shows its text", () => {
  const legacy = brief({ sections: [], legacy: true, text: "Just prose <b>here</b>" });
  const html = toHtml(renderBrief(legacy, handlers));
  expect(html).toContain("Just prose &lt;b&gt;here&lt;/b&gt;");
});

test("markdown subset: paragraphs join, lists group, sub-headings, key-sentence marks drop", () => {
  const nodes = renderBody(["First line", "second line", "", "1. one", "2. two", "- bullet", "### F1 Submit", "==**Key.**=="]);
  expect(nodes.map((n) => n.tag)).toEqual(["p", "ol", "ul", "h4", "p"]);
  expect(toHtml(nodes[0])).toBe("<p>First line second line</p>");
  expect(toHtml(nodes[4])).toBe("<p><strong>Key.</strong></p>");
  expect(inline("a `b` **c** ==d==").map((c) => (typeof c === "string" ? c : toHtml(c)))).toEqual(["a ", "<code>b</code>", " ", "<strong>c</strong>", " ", "d"]);
});

const advisory = (rule: string, line: number, message: string) => ({ severity: "advisory" as const, rule, line, message, hint: "", section: null, blocksApproval: false });

test("report.dashboard.11: advisories are grouped by rule, with the count, line and matched text", () => {
  const findings = [
    advisory("prose.evaluative", 9, "obviously"),
    advisory("prose.key-sentence", 30, "no key sentence"),
    advisory("prose.evaluative", 4, "very"),
  ];
  const groups = groupAdvisories(findings);
  expect(groups.map((g) => [g.rule, g.findings.map((f) => f.line)])).toEqual([
    ["prose.evaluative", [4, 9]],
    ["prose.key-sentence", [30]],
  ]);
  const card = renderBrief(brief({ findings, counts: { blockers: 0, advisories: 3 }, approvable: true }), handlers);
  const html = toHtml(card);
  expect(html).toContain("3 advisories");
  const groupNodes = elements(card).filter((e) => e.tag === "section" && e.attrs["data-rule-group"]);
  expect(groupNodes.map((g) => g.attrs["data-rule-group"])).toEqual(["prose.evaluative", "prose.key-sentence"]);
  expect(toHtml(groupNodes[0])).toContain("line 4: very");
  expect(toHtml(groupNodes[0])).toContain("line 9: obviously");
  const counts = elements(groupNodes[0]).find((e) => e.attrs.class?.toString().includes("rule-count"))!;
  expect(toHtml(counts)).toContain(">2<");
});

test("report.dashboard.11: the badge sits next to the blocker count and is absent with none", () => {
  const both = toHtml(renderBrief(brief(), handlers));
  expect(both).toContain("1 blocker</span> <span class=\"badge badge-advisory\">1 advisory</span>");
  const clean = brief({ findings: [], counts: { blockers: 0, advisories: 0 }, approvable: true });
  expect(toHtml(renderBrief(clean, handlers))).not.toContain("badge");
});

test("report.dashboard.11: advisory text is a text node, never markup", () => {
  const hostile = brief({ findings: [advisory("prose.evaluative", 1, HOSTILE)], counts: { blockers: 0, advisories: 1 } });
  const card = renderBrief(hostile, handlers);
  expect(elements(card).some((e) => e.tag === "script" || e.tag === "img")).toBe(false);
  expect(toHtml(card)).toContain(escapeHtml(HOSTILE));
});

test("report.dashboard.12: the page total counts advisories across briefs and says when there are none", () => {
  const two = [brief({ counts: { blockers: 0, advisories: 3 } }), brief({ changeId: "b", counts: { blockers: 0, advisories: 0 } })];
  expect(advisoryTotal(two)).toBe("3 advisories in 1 of 2 briefs.");
  expect(toHtml(renderBriefs(two, handlers))).toContain("3 advisories in 1 of 2 briefs.");
  expect(advisoryTotal([brief({ counts: { blockers: 1, advisories: 1 } })])).toBe("1 advisory in 1 of 1 brief.");
  expect(advisoryTotal([brief({ counts: { blockers: 0, advisories: 0 } })])).toBe("No advisories in the listed briefs.");
});

test("each brief card names the language it was judged in and where that came from", () => {
  const detected = toHtml(renderBrief(brief({ language: "ko", languageSource: "detected" }), handlers));
  expect(detected).toContain("language: ko (detected)");
  const configured = toHtml(renderBrief(brief({ language: "en", languageSource: "configured", languageNote: "the text reads as ko" }), handlers));
  expect(configured).toContain("language: en (configured; the text reads as ko)");
});

test("subject.lifecycle.52: the page lists the approvals an agent recorded for the human, and says so on an edited brief", () => {
  const relay = { role: "planner", actor: "plan-1", tool: "ask" };
  expect(relayText(relay)).toBe("relayed by planner `plan-1` via ask");
  expect(renderRelayed([])).toBeNull();
  const html = toHtml(renderRelayed([{ changeId: "add-login", brief: ".canon/changes/add-login/brief.md", approval: "Ana 2026-10-30T00:00:00Z", relay }])!);
  expect(html).toContain("Approved through an agent");
  expect(html).toContain("add-login");
  expect(html).toContain("relayed by planner");
  const edited = toHtml(renderBrief(brief({ changedSinceApproval: true, previousRelay: relay }), handlers));
  expect(edited).toContain("that approval was relayed by planner");
});

test("subject.lifecycle.52: the relayed panel stays on the page when the last waiting brief is approved", () => {
  const relayed = [{ changeId: "add-login", brief: ".canon/changes/add-login/brief.md", approval: "Ana", relay: { role: "planner", actor: "plan-1", tool: "ask" } }];
  const html = (nodes: ReturnType<typeof renderBriefsPage>) => nodes.map((n) => toHtml(n)).join("");
  const waiting = html(renderBriefsPage([brief()], relayed, handlers));
  const afterLast = html(renderBriefsPage([], relayed, handlers));
  expect(waiting).toContain("Approved through an agent");
  expect(afterLast).toContain(EMPTY_MESSAGE);
  expect(afterLast).toContain("Approved through an agent");
  expect(afterLast).toContain("add-login");
});
