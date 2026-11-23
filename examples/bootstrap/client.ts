/** Minimal dependency-free client for the unreleased bootstrap API; amounts remain exact JSON strings. */
export type Contract = {
  contractId: string;
  template: { package_id: string; module: string; entity: string };
  payload: { type: "raw_json"; value: string };
  createEventId: string;
};
export type Event = {
  eventId: string;
  contractId: string;
  kind: "create" | "archive";
  template: Contract["template"];
  payload: Contract["payload"] | null;
};
export type Registration = {
  id: string;
  cursor: string;
  snapshotUrl: string;
  expiresAtSeconds: string;
  formatVersion: 1;
  atomicHandoff: true;
};
export type Snapshot = {
  items: Contract[];
  visibleTotal: string;
  nextCursor: string | null;
  formatVersion: 1;
};
export type Page = {
  events: Event[];
  nextCursor: string | null;
  more: boolean;
  formatVersion: 1;
};
/** Callers handle explicit coverage loss; this client never silently registers a replacement baseline. */
export class ApiError extends Error {
  constructor(readonly status: number, readonly coverage?: string) {
    super(`CantonGrid request failed (${status}${coverage ? `, ${coverage}` : ""})`);
  }
}
/** Construct paths locally so a response cannot redirect an organization bearer to another origin. */
function subscription(id: string): string {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(id)) {
    throw new Error("Invalid subscription identity");
  }
  return `/v1/subscriptions/${id}`;
}
/** The token callback may refresh organization credentials independently of the source Ledger token. */
export class BootstrapApi {
  private readonly origin: URL;
  constructor(base: string, private readonly token: () => Promise<string>) {
    this.origin = new URL(base);
    if (this.origin.username || this.origin.password || this.origin.search || this.origin.hash ||
        !(this.origin.protocol === "https:" ||
          (this.origin.protocol === "http:" && this.origin.hostname === "127.0.0.1"))) {
      throw new Error("Use HTTPS or the numeric-loopback development API");
    }
  }
  /** Share transport, authorization, response bounds and redacted failures across the small API surface. */
  private async request<T>(path: string, signal?: AbortSignal, body?: unknown): Promise<T> {
    const response = await fetch(new URL(path, this.origin), {
      method: body === undefined ? "GET" : "POST",
      redirect: "error",
      cache: "no-store",
      headers: { Authorization: `Bearer ${await this.token()}`, "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal,
    });
    const reader = response.body?.getReader();
    const decoder = new TextDecoder();
    let text = "", bytes = 0;
    if (reader) {
      for (;;) {
        const chunk = await reader.read();
        if (chunk.done) break;
        bytes += chunk.value.byteLength;
        if (bytes > 2 * 1024 * 1024) {
          await reader.cancel();
          throw new Error("API response exceeds its bounded transport size");
        }
        text += decoder.decode(chunk.value, { stream: true });
      }
      text += decoder.decode();
    }
    let value: unknown;
    try { value = JSON.parse(text); } catch { throw new ApiError(response.status); }
    if (!response.ok) {
      const coverage = typeof value === "object" && value !== null && "coverage" in value &&
        (value.coverage === "gap" || value.coverage === "unavailable") ? value.coverage : undefined;
      throw new ApiError(response.status, coverage);
    }
    return value as T;
  }
  /** Retry a fresh name only within the same live, unconsumed bootstrap request. */
  register(name: string, signal?: AbortSignal) {
    return this.request<Registration>("/v1/subscriptions/bootstrap", signal, { name });
  }
  /** The opaque subscription identifies the baseline; no client-selected source offset is accepted. */
  snapshot(id: string, cursor: string | null, signal?: AbortSignal) {
    const query = new URLSearchParams({ limit: "25" });
    if (cursor) query.set("cursor", cursor);
    return this.request<Snapshot>(`${subscription(id)}/snapshot?${query}`, signal);
  }
  /** Omit resume cursors to replay from the durable server ACK after a crash or lost response. */
  events(id: string, signal?: AbortSignal) {
    return this.request<Page>(`${subscription(id)}/events?limit=25&waitSeconds=10`, signal);
  }
  /** ACK only after the application transaction containing every event in this prefix committed. */
  acknowledge(id: string, cursor: string, signal?: AbortSignal) {
    return this.request<{ ok: true }>(`${subscription(id)}/acknowledge`, signal, { cursor });
  }
}
