import { h, toDom } from "./vnode";

/** The Panels / Briefs links both pages carry. `canon dashboard` serves the
 * live snapshot at /live-snapshot/, which the panels page reads from its URL. */
export function renderNav(into: HTMLElement, current: "panels" | "briefs"): void {
  const link = (href: string, label: string, page: "panels" | "briefs") =>
    h("a", { href, ...(current === page ? { "aria-current": "page", class: "nav-current" } : {}) }, label);
  into.replaceChildren(
    toDom(
      h(
        "nav",
        { class: "nav" },
        link(current === "panels" ? `./${window.location.search}` : "./?snapshot=/live-snapshot/", "Panels", "panels"),
        link("briefs.html", "Briefs", "briefs"),
      ),
    ),
  );
}
