import type { SnapshotManifest } from "./snapshot";

// Freshness banner (task 5.4): surfaces manifest.json's own provenance
// fields. design.md's Risk section is explicit that "matches `canon
// report`" is scoped to THIS recorded snapshot input, not the live
// checkout — the banner states that scoping directly rather than
// implying the numbers are current.
export function renderFreshnessBanner(banner: HTMLElement, manifest: SnapshotManifest): void {
  banner.className = "banner";
  banner.replaceChildren();

  const summary = document.createElement("p");
  summary.textContent =
    "Numbers below match canon report for the snapshot recorded at the timestamp below — not necessarily the live checkout.";
  summary.style.margin = "0";

  const dl = document.createElement("dl");
  const fields: [string, string][] = [
    ["generated_at", manifest.generated_at],
    ["source_git_sha", manifest.source_git_sha],
    ["source_digest", manifest.source_digest],
  ];
  for (const [label, value] of fields) {
    const dt = document.createElement("dt");
    dt.textContent = label;
    const dd = document.createElement("dd");
    dd.textContent = value;
    dl.append(dt, dd);
  }

  banner.append(summary, dl);
}

export function renderErrorBanner(banner: HTMLElement, error: unknown): void {
  banner.className = "banner banner-error";
  banner.textContent = `Failed to load snapshot: ${error instanceof Error ? error.message : String(error)}`;
}

/** The repo has no snapshot (`canon dashboard`'s /api/status says so): why,
 * and the fix, in place of panels. */
export function renderUnavailableBanner(banner: HTMLElement, reason: string | null, fix: string | null): void {
  banner.className = "banner banner-error";
  banner.replaceChildren();
  const title = document.createElement("p");
  title.style.margin = "0";
  title.textContent = "No snapshot to show.";
  const why = document.createElement("p");
  why.textContent = reason ?? "The repo's snapshot is not available.";
  const how = document.createElement("p");
  how.style.margin = "0";
  how.textContent = fix ?? "";
  banner.append(title, why, how);
}
