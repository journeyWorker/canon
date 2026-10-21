import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { approveDisabled, EMPTY_MESSAGE, inline, renderBody, renderBrief, renderBriefs, type Brief, type BriefHandlers } from "../src/briefs-render";
import { elements, escapeHtml, toHtml } from "../src/vnode";

const handlers: BriefHandlers = { onInput: () => {}, onApprove: () => {} };

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

test("Approve is disabled while the brief has blockers, even with both fields filled", () => {
  expect(approveDisabled({ approvable: false }, "Ana", "Go.", false)).toBe(true);
  const button = elements(renderBrief(brief(), handlers)).find((e) => e.tag === "button" && e.attrs.class === "approve-button")!;
  expect(button.attrs.disabled).toBe(true);
  expect(toHtml(renderBrief(brief(), handlers))).toContain("Not approvable yet: 1 blocker");
});

test("Approve waits for both the name and the answer, and for a request in flight", () => {
  const ok = { approvable: true };
  expect(approveDisabled(ok, "", "Go.", false)).toBe(true);
  expect(approveDisabled(ok, "Ana", "  ", false)).toBe(true);
  expect(approveDisabled(ok, "Ana", "Go.", true)).toBe(true);
  expect(approveDisabled(ok, "Ana", "Go.", false)).toBe(false);
});

test("the form is empty on load: no value, autofill off", () => {
  const inputs = elements(renderBrief(brief({ approvable: true }), handlers)).filter((e) => e.tag === "input");
  expect(inputs).toHaveLength(2);
  for (const input of inputs) {
    expect("value" in input.attrs).toBe(false);
    expect(input.attrs.autocomplete).toBe("off");
  }
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
