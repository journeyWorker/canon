import "./style.css";
import { renderNav } from "./nav";
import { advisoryTotal, approvalRequest, approveDisabled, refusedIds, renderApproveError, renderBriefsPage, type RelayedBrief, selectionText, type Brief, type BriefHandlers, type Refusal } from "./briefs-render";
import { h, toDom } from "./vnode";

// The briefs page: the changes whose brief awaits the human, and the form
// that records the approval of every selected brief through one `POST
// /api/briefs/approve`, each bound to the sha256 of the brief file this page
// rendered. The name and the answer are typed once, here and nowhere else:
// never prefilled, never stored.

interface ApiError {
  error: { code: string; message: string; hint: string | null };
}

const main = document.querySelector<HTMLElement>("#briefs");
const notices = document.querySelector<HTMLElement>("#notices");
const nav = document.querySelector<HTMLElement>("#nav");
if (!main || !notices || !nav) throw new Error("missing briefs page containers");
renderNav(nav, "briefs");

interface ApprovalEntry {
  change: string;
  outcome: "written" | "unchanged" | "failed" | "skipped";
  brief?: string;
  digest?: string;
  error?: string;
}

interface ApprovalsData {
  approvedBy: string;
  policyApproved: boolean;
  approvals: ApprovalEntry[];
}

function bar(): HTMLElement {
  return main!.querySelector<HTMLElement>(".approve-bar") as HTMLElement;
}

function input(cls: string): HTMLInputElement {
  return bar().querySelector<HTMLInputElement>(`.${cls}`) as HTMLInputElement;
}

function say(node: ReturnType<typeof h> | null): void {
  const slot = bar().querySelector<HTMLElement>(".approve-message") as HTMLElement;
  slot.replaceChildren(...(node ? [toDom(node)] : []));
}

/** The briefs whose selection box is ticked. */
function selected(): Brief[] {
  const ticked = new Set([...main!.querySelectorAll<HTMLInputElement>(".brief-select:checked")].map((box) => box.dataset.change));
  return shown.filter((b) => ticked.has(b.changeId));
}

function refresh(busy = false): void {
  const form = bar();
  const picked = selected();
  (form.querySelector(".selected-count") as HTMLElement).textContent = selectionText(picked.length);
  (form.querySelector(".approve-button") as HTMLButtonElement).disabled = approveDisabled(picked.length, input("approve-by").value, input("approve-note").value, busy);
}

function afterApproval(data: ApprovalsData): void {
  const done = new Set(data.approvals.filter((a) => a.outcome === "written" || a.outcome === "unchanged").map((a) => a.change));
  for (const card of main!.querySelectorAll<HTMLElement>(".brief")) if (done.has(card.dataset.change ?? "")) card.remove();
  notices!.append(
    toDom(
      h(
        "div",
        { class: "banner notice-approved", role: "status" },
        h("strong", {}, `Approved ${[...done].join(", ")}`),
        ` by ${data.approvedBy}${data.policyApproved ? "; the policy approval is recorded too." : "."}`,
      ),
    ),
  );
  shown = shown.filter((b) => !done.has(b.changeId));
  const total = main!.querySelector(".briefs-total");
  if (total) total.textContent = advisoryTotal(shown);
  if (shown.length === 0) render([]);
  else {
    say(null);
    refresh();
  }
}

async function approve(): Promise<void> {
  const picked = selected();
  if (picked.length === 0) return;
  refresh(true);
  say(null);
  const body = JSON.stringify(approvalRequest(picked, input("approve-by").value, input("approve-note").value));
  try {
    const response = await fetch("/api/briefs/approve", { method: "POST", headers: { "content-type": "application/json" }, body });
    const json = (await response.json()) as { data?: ApprovalsData & { refused?: Refusal[] } } & Partial<ApiError>;
    if (response.ok && json.data && json.data.approvals) return afterApproval(json.data);
    const error = json.error ?? { code: "UNKNOWN", message: `the server answered ${response.status}`, hint: null };
    const failure = { ...error, refused: json.data?.refused ?? [] };
    for (const id of refusedIds(failure)) main!.querySelector(`.brief[data-change="${CSS.escape(id)}"]`)?.classList.add("brief-refused");
    const stale = failure.refused.length > 0 || error.code === "BRIEF_CHANGED_SINCE_PAGE_LOAD";
    say(renderApproveError(failure, stale ? withReload() : null));
  } catch (e) {
    say(h("div", { class: "approve-error", role: "alert" }, h("p", {}, `The request failed: ${e instanceof Error ? e.message : String(e)}`)));
  }
  refresh();
}

function withReload() {
  const button = h("button", { type: "button", class: "reload-button" }, "Reload the page");
  button.on.click = () => window.location.reload();
  return button;
}

const handlers: BriefHandlers = {
  onChange: () => refresh(),
  onApprove: () => void approve(),
};

let shown: Brief[] = [];

let relayedShown: RelayedBrief[] = [];

function render(briefs: Brief[], relayed: RelayedBrief[] = relayedShown): void {
  shown = briefs;
  relayedShown = relayed;
  main!.replaceChildren(...renderBriefsPage(briefs, relayed, handlers).map(toDom));
}

async function load(): Promise<void> {
  try {
    const response = await fetch("/api/briefs", { headers: { accept: "application/json" } });
    const json = (await response.json()) as { data?: { briefs: Brief[]; relayed?: RelayedBrief[]; unreadable: { changeId: string; message: string }[] } } & Partial<ApiError>;
    if (!response.ok || !json.data) throw new Error(json.error ? `${json.error.code}: ${json.error.message}` : `the server answered ${response.status}`);
    render(json.data.briefs, json.data.relayed ?? []);
    for (const skipped of json.data.unreadable) {
      notices!.append(toDom(h("div", { class: "banner" }, `${skipped.changeId}: ${skipped.message}`)));
    }
    document.body.dataset.status = "ready";
  } catch (error) {
    main!.replaceChildren(toDom(h("div", { class: "banner banner-error" }, `Failed to load the briefs: ${error instanceof Error ? error.message : String(error)}`)));
    document.body.dataset.status = "error";
  }
}

void load();
