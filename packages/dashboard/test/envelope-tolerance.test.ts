import { expect, test } from "bun:test";
import { approvedChanges, approveFailure, refusedIds, renderApproveError, renderBrief, type Brief, type BriefHandlers } from "../src/briefs-render";
import { toHtml } from "../src/vnode";

// atip/1 15.6: a caller ignores the members it does not know, in every object, and does not reject a
// document for an enum string it does not know: it leaves out that item and reads the rest. canon 0.34 adds
// `meta.contractVersion`, `error.reason` and new registries, and a later canon will add more: the dashboard
// reads what it understands and shows the rest as it is, never throwing.

const handlers: BriefHandlers = { onChange: () => {}, onApprove: () => {} };

const failure = {
  ok: false,
  error: {
    code: "BRIEF_INVALID",
    reason: "A_REASON_FROM_A_LATER_CANON",
    message: "refused",
    retryable: false,
    field: "/briefs/0",
    received: "",
    hint: "run it again",
    "x-future": { anything: true },
    futureMember: [1, 2, 3],
    details: [{ code: "BRIEF_CHECK_FAILED", message: "`add-login`: blockers", also: "unknown member of a detail" }],
  },
  data: { newMember: 1 },
  meta: { schemaVersion: "1.9", tool: "canon", toolVersion: "9.9.9", contract: "atip/1", contractVersion: "1.9", command: "brief approve", "x-canon": {}, futureMeta: { a: 1 } },
};

test("a failure with members it does not know is read: code, message, hint and each refused brief", () => {
  const read = approveFailure(failure, 422);
  expect(read.code).toBe("BRIEF_INVALID");
  expect(read.message).toBe("refused");
  expect(read.hint).toBe("run it again");
  expect(refusedIds(read)).toEqual(["add-login"]);
  const html = toHtml(renderApproveError(read, null));
  expect(html).toContain("BRIEF_CHECK_FAILED: blockers");
  expect(html).toContain("run it again");
});

test("a failure with no hint and a reason it does not know still reads", () => {
  const { hint: _hint, details: _details, ...error } = failure.error;
  const read = approveFailure({ error, data: undefined }, 500);
  expect(read.hint).toBeUndefined();
  expect(read.refused).toEqual([]);
  expect(read.code).toBe("BRIEF_INVALID");
});

test("an approval whose outcome is a string this page does not know is left out, the others count", () => {
  const approvals = [
    { change: "add-login", outcome: "written" },
    { change: "add-cart", outcome: "queued-for-a-later-canon", note: "a member the page does not know" },
    { change: "add-pay", outcome: "unchanged" },
    { change: "add-ship", outcome: "failed" },
  ];
  expect(approvedChanges({ approvals })).toEqual(["add-login", "add-pay"]);
});

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
      { key: "intent", role: "context", heading: "Intent", line: 3, body: ["Add login"], steps: [] },
      { key: "appendix", role: "a-role-from-a-later-canon", heading: "Appendix", line: 9, body: ["read it"], steps: [] },
    ],
    findings: [
      { severity: "advisory", rule: "prose.long-sentence", line: 5, message: "a long one", hint: "split it", section: "intent", blocksApproval: false },
      { severity: "catastrophic", rule: "brief.from-a-later-canon", line: 7, message: "new kind", hint: "fix it", section: "intent", blocksApproval: true },
    ],
    counts: { blockers: 1, advisories: 1 },
    approvable: false,
    ...over,
  };
}

test("a brief with a finding severity and a section role it does not know renders every item", () => {
  const html = toHtml(renderBrief(brief(), handlers));
  expect(html).toContain("brief.from-a-later-canon");
  expect(html).toContain("catastrophic");
  expect(html).toContain("Appendix");
  expect(html).toContain("Intent");
});
