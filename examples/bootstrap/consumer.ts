/** Application storage is deliberately supplied by the consumer, rather than guessed by the indexer. */
import { BootstrapApi, Contract, Event } from "./client";

/** Implement these callbacks with durable transactions in the consuming application. */
export interface Sink {
  /** Begin a private staging area; do not replace the live application baseline page by page. */
  beginBaseline(subscription: string): Promise<void>;
  stage(contracts: Contract[]): Promise<void>;
  /** Publish staged rows and persist the subscription identity together in one application transaction. */
  commitBaseline(subscription: string): Promise<void>;
  /** Discard incomplete staging, leaving the last committed application view intact. */
  abortBaseline(subscription: string): Promise<void>;
  /** Atomically deduplicate by configured source generation + eventId, then apply the complete page. */
  applyPage(subscription: string, events: Event[]): Promise<void>;
}

/** Install a complete pinned baseline; concurrent ledger events wait in the paired durable feed. */
export async function installBaseline(
  api: BootstrapApi, sink: Sink, name = `baseline-${crypto.randomUUID()}`, signal?: AbortSignal,
): Promise<string> {
  const registration = await api.register(name, signal);
  if (registration.formatVersion !== 1 || !registration.atomicHandoff) {
    throw new Error("Unsupported bootstrap protocol");
  }
  const id = registration.id;
  await sink.beginBaseline(id);
  try {
    let cursor: string | null = null;
    for (let page = 0; ; page++) {
      if (page >= 512) throw new Error("Baseline exceeded the example's page budget");
      const snapshot = await api.snapshot(id, cursor, signal);
      if (snapshot.formatVersion !== 1) throw new Error("Unsupported snapshot protocol");
      await sink.stage(snapshot.items);
      cursor = snapshot.nextCursor;
      if (cursor === null) break;
    }
    await sink.commitBaseline(id);
    return id;
  } catch (error) {
    await sink.abortBaseline(id);
    throw error;
  }
}

/** Call repeatedly after baseline publication, or resume with the subscription stored by commitBaseline. */
export async function consumePage(api: BootstrapApi, sink: Sink, id: string, signal?: AbortSignal) {
  const page = await api.events(id, signal);
  if (page.formatVersion !== 1) throw new Error("Unsupported delivery protocol");
  if (page.events.length !== 0) {
    if (!page.nextCursor) throw new Error("Delivery page has no acknowledgement cursor");
    await sink.applyPage(id, page.events);
    await api.acknowledge(id, page.nextCursor, signal);
  }
  return page.more;
}
