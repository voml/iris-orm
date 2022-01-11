/**
 * W5 — Multi-tab coordination via Web Locks + BroadcastChannel.
 *
 * Writer critical sections (prepare/commit/writeThrough/outbox advance) take an
 * exclusive lock. Other tabs receive commit/schema events over BroadcastChannel.
 * This is coordination, not a distributed transaction.
 */

export type CoordEvent =
  | {
      kind: "commit";
      sourceName: string;
      journalId: string;
      schemaFingerprint: string;
      at: string;
    }
  | {
      kind: "schema";
      sourceName: string;
      schemaId: string;
      fingerprint: string;
      at: string;
    }
  | {
      kind: "outbox";
      sourceName: string;
      seq: number;
      at: string;
    }
  | {
      kind: "recover";
      sourceName: string;
      at: string;
    };

export type CoordCapabilities = {
  webLocks: boolean;
  broadcast: boolean;
  /** Effective mode reported by probe. */
  mode: "none" | "web-locks" | "broadcast" | "web-locks+broadcast";
};

export type CoordListener = (event: CoordEvent) => void;

export type CoordPublishInput =
  | { kind: "commit"; journalId: string; schemaFingerprint: string; at?: string }
  | { kind: "schema"; schemaId: string; fingerprint: string; at?: string }
  | { kind: "outbox"; seq: number; at?: string }
  | { kind: "recover"; at?: string };

function nowIso(): string {
  return new Date().toISOString();
}

export function detectCoordCapabilities(): CoordCapabilities {
  const webLocks =
    typeof navigator !== "undefined" &&
    typeof (navigator as Navigator & { locks?: LockManager }).locks?.request === "function";
  const broadcast = typeof BroadcastChannel !== "undefined";
  let mode: CoordCapabilities["mode"] = "none";
  if (webLocks && broadcast) mode = "web-locks+broadcast";
  else if (webLocks) mode = "web-locks";
  else if (broadcast) mode = "broadcast";
  return { webLocks, broadcast, mode };
}

export class WebCoordinator {
  readonly lockName: string;
  readonly channelName: string;
  readonly capabilities: CoordCapabilities;
  #channel: BroadcastChannel | null = null;
  #listeners = new Set<CoordListener>();

  constructor(readonly sourceName: string) {
    this.lockName = `iris-web:lock:${sourceName}`;
    this.channelName = `iris-web:bc:${sourceName}`;
    this.capabilities = detectCoordCapabilities();
  }

  open(): this {
    if (this.capabilities.broadcast && !this.#channel) {
      this.#channel = new BroadcastChannel(this.channelName);
      this.#channel.onmessage = (ev: MessageEvent<CoordEvent>) => {
        const data = ev.data;
        if (!data || typeof data !== "object" || !("kind" in data)) return;
        for (const listener of this.#listeners) listener(data);
      };
    }
    return this;
  }

  close(): void {
    this.#channel?.close();
    this.#channel = null;
    this.#listeners.clear();
  }

  subscribe(listener: CoordListener): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  publish(event: CoordPublishInput): void {
    const full = {
      ...event,
      sourceName: this.sourceName,
      at: event.at ?? nowIso(),
    } as CoordEvent;
    for (const listener of this.#listeners) listener(full);
    this.#channel?.postMessage(full);
  }

  /**
   * Run `fn` under an exclusive Web Lock when available.
   * Without locks, runs immediately (single-tab / unsupported environments).
   */
  async withWriteLock<T>(fn: () => Promise<T>, options?: { signal?: AbortSignal }): Promise<T> {
    const locks = (navigator as Navigator & { locks?: LockManager }).locks;
    if (!locks?.request) {
      return fn();
    }
    return locks.request(
      this.lockName,
      { mode: "exclusive", signal: options?.signal },
      async () => fn(),
    );
  }
}
