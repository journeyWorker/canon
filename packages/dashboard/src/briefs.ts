import "./style.css";
import { renderNav } from "./nav";
import { advisoryTotal, renderBriefs, approveDisabled, type Brief, type BriefHandlers } from "./briefs-render";
import { h, toDom } from "./vnode";

// The briefs page: the changes whose brief awaits the human, and the form
// that records the approval through `POST /api/briefs/approve`, bound to the
// sha256 of the brief file this page rendered. The name and the answer are
// typed here and nowhere else: never prefilled, never stored.

interface ApiError {
  error: { code: string; message: string; hint: string | null };
}

const main = document.querySelector<HTMLElement>("#briefs");
const notices = document.querySelector<HTMLElement>("#notices");
const nav = document.querySelector<HTMLElement>("#nav");
if (!main || !notices || !nav) throw new Error("missing briefs page containers");
renderNav(nav, "briefs");

function field(card: HTMLElement, cls: string): HTMLInputElement {
  return card.querySelector<HTMLInputElement>(`.${cls}`) as HTMLInputElement;
}

function say(card: HTMLElement, node: ReturnType<typeof h> | null): void {
  const slot = card.querySelector<HTMLElement>(".approve-message") as HTMLElement;
  slot.replaceChildren(...(node ? [toDom(node)] : []));
}

function refreshButton(card: HTMLElement, brief: Pick<Brief, "approvable">, busy = false): void {
  const button = card.querySelector<HTMLButtonElement>(".approve-button") as HTMLButtonElement;
  button.disabled = approveDisabled(brief, field(card, "approve-by").value, field(card, "approve-note").value, busy);
}

function afterApproval(brief: Brief, card: HTMLElement, data: { brief: string; digest: string; approvedBy: string; policyApproved: boolean }): void {
  card.remove();
  notices!.append(
    toDom(
      h(
        "div",
        { class: "banner notice-approved", role: "status" },
        h("strong", {}, `Approved ${brief.changeId}`),
        ` by ${data.approvedBy}: ${data.brief} is now bound to ${data.digest}`,
        data.policyApproved ? "; the policy approval is recorded too." : ".",
      ),
    ),
  );
  shown = shown.filter((b) => b.changeId !== brief.changeId);
  const total = main!.querySelector(".briefs-total");
  if (total) total.textContent = advisoryTotal(shown);
  if (!main!.querySelector(".brief")) render([]);
}

async function approve(brief: Brief, card: HTMLElement): Promise<void> {
  refreshButton(card, brief, true);
  say(card, null);
  const body = JSON.stringify({ changeId: brief.changeId, by: field(card, "approve-by").value, note: field(card, "approve-note").value, sha256: brief.sha256 });
  try {
    const response = await fetch("/api/briefs/approve", { method: "POST", headers: { "content-type": "application/json" }, body });
    const json = (await response.json()) as { data?: { brief: string; digest: string; approvedBy: string; policyApproved: boolean } } & Partial<ApiError>;
    if (response.ok && json.data) return afterApproval(brief, card, json.data);
    const error = json.error ?? { code: "UNKNOWN", message: `the server answered ${response.status}`, hint: null };
    const changed = error.code === "BRIEF_CHANGED_SINCE_PAGE_LOAD";
    say(
      card,
      h(
        "div",
        { class: "approve-error", role: "alert" },
        h("p", {}, error.message),
        error.hint ? h("p", { class: "finding-hint" }, error.hint) : null,
        changed ? withReload() : null,
      ),
    );
  } catch (e) {
    say(card, h("div", { class: "approve-error", role: "alert" }, h("p", {}, `The request failed: ${e instanceof Error ? e.message : String(e)}`)));
  }
  refreshButton(card, brief);
}

function withReload() {
  const button = h("button", { type: "button", class: "reload-button" }, "Reload the page");
  button.on.click = () => window.location.reload();
  return button;
}

const handlers: BriefHandlers = {
  onInput: (card) => refreshButton(card, { approvable: card.dataset.approvable === "true" }),
  onApprove: (brief, card) => void approve(brief, card),
};

let shown: Brief[] = [];

function render(briefs: Brief[]): void {
  shown = briefs;
  main!.replaceChildren(toDom(renderBriefs(briefs, handlers)));
}

async function load(): Promise<void> {
  try {
    const response = await fetch("/api/briefs", { headers: { accept: "application/json" } });
    const json = (await response.json()) as { data?: { briefs: Brief[]; unreadable: { changeId: string; message: string }[] } } & Partial<ApiError>;
    if (!response.ok || !json.data) throw new Error(json.error ? `${json.error.code}: ${json.error.message}` : `the server answered ${response.status}`);
    render(json.data.briefs);
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
