// `GET /api/status` of `canon dashboard` (crates/canon-cli/src/dashboard.rs):
// whether the repo's snapshot exists, and if not why and how to get one.
// Any other server (vite dev or preview) has no such endpoint: `null`.

export interface SnapshotStatus {
  available: boolean;
  reason: string | null;
  fix: string | null;
}

export async function fetchSnapshotStatus(): Promise<SnapshotStatus | null> {
  try {
    const response = await fetch("/api/status", { headers: { accept: "application/json" } });
    if (!response.ok || !(response.headers.get("content-type") ?? "").includes("application/json")) return null;
    const body = (await response.json()) as { data?: { snapshot?: SnapshotStatus } };
    return body.data?.snapshot ?? null;
  } catch {
    return null;
  }
}
