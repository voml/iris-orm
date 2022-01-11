/**
 * W6 — Local outbox + CommitToken / AppliedWatermark (sync-ready, not remote sync).
 *
 * Authority commit may append outbox records in the same IDB transaction as row
 * mutations. A future sync worker drains pending records; this module only
 * persists and advances local watermarks.
 */

import {
  getMeta,
  idbRequest,
  idbTransactionDone,
  putMeta,
  STORE_META,
  STORE_OUTBOX,
  STORE_WATERMARK,
} from "./idb.ts";

export type OutboxEffect = "upsert" | "delete";

export type CommitToken = {
    shard: string;
    seq: number;
};

export type AppliedWatermark = {
    shard: string;
    seq: number;
    /** ISO-8601 when this watermark was applied locally. */
    appliedAt: string;
};

export type OutboxRecord = {
    /** Monotonic seq within shard (also IDB key with shard prefix). */
    id: string;
    shard: string;
    seq: number;
    entity: string;
    entityId: string;
    effect: OutboxEffect;
    schemaFingerprint: string;
    journalId: string;
    /** Opaque payload / field snapshot hint (never secrets). */
    payload?: unknown;
    createdAt: string;
    /** pending | acked | failed */
    status: "pending" | "acked" | "failed";
    errorMessage?: string;
};

export type OutboxAppend = {
    entity: string;
    entityId: string;
    effect: OutboxEffect;
    schemaFingerprint: string;
    journalId: string;
    payload?: unknown;
    shard?: string;
};

export const DEFAULT_SHARD = "default" as const;

function nowIso(): string {
    return new Date().toISOString();
}

function outboxId(shard: string, seq: number): string {
    return `${shard}:${seq}`;
}

export class WebOutbox {
    constructor(readonly db: IDBDatabase) {}

    async getCommitToken(shard: string = DEFAULT_SHARD): Promise<CommitToken> {
        const key = `commitToken:${shard}`;
        const token = await getMeta<CommitToken>(this.db, key);
        return token ?? { shard, seq: 0 };
    }

    async setCommitToken(token: CommitToken): Promise<void> {
        await putMeta(this.db, `commitToken:${token.shard}`, token);
    }

    async getAppliedWatermark(shard: string = DEFAULT_SHARD): Promise<AppliedWatermark | undefined> {
        const tx = this.db.transaction(STORE_WATERMARK, "readonly");
        const row = await idbRequest<AppliedWatermark | undefined>(tx.objectStore(STORE_WATERMARK).get(shard));
        await idbTransactionDone(tx);
        return row;
    }

    async setAppliedWatermark(wm: AppliedWatermark): Promise<void> {
        const tx = this.db.transaction(STORE_WATERMARK, "readwrite");
        tx.objectStore(STORE_WATERMARK).put(wm);
        await idbTransactionDone(tx);
    }

    /**
     * Advance authority CommitToken and append outbox rows.
     * Caller should hold the W5 write lock.
     */
    async appendMany(appends: readonly OutboxAppend[]): Promise<{
        token: CommitToken;
        records: OutboxRecord[];
    }> {
        if (appends.length === 0) {
            const token = await this.getCommitToken();
            return { token, records: [] };
        }

        const shard = appends[0]?.shard ?? DEFAULT_SHARD;
        const current = await this.getCommitToken(shard);
        let seq = current.seq;
        const ts = nowIso();
        const records: OutboxRecord[] = [];

    const tx = this.db.transaction([STORE_OUTBOX, STORE_META], "readwrite");
    const outStore = tx.objectStore(STORE_OUTBOX);
    const metaStore = tx.objectStore(STORE_META);

    for (const a of appends) {
      seq += 1;
      const record: OutboxRecord = {
        id: outboxId(shard, seq),
        shard,
        seq,
        entity: a.entity,
        entityId: a.entityId,
        effect: a.effect,
        schemaFingerprint: a.schemaFingerprint,
        journalId: a.journalId,
        payload: a.payload,
        createdAt: ts,
        status: "pending",
      };
      outStore.put(record);
      records.push(record);
    }

    const token: CommitToken = { shard, seq };
    metaStore.put({ key: `commitToken:${shard}`, value: token });
    await idbTransactionDone(tx);
    return { token, records };
  }

  async listPending(options?: { shard?: string; limit?: number }): Promise<OutboxRecord[]> {
    const shard = options?.shard ?? DEFAULT_SHARD;
    const limit = options?.limit ?? 100;
    const tx = this.db.transaction(STORE_OUTBOX, "readonly");
    const idx = tx.objectStore(STORE_OUTBOX).index("by_status");
    const rows = await idbRequest<OutboxRecord[]>(idx.getAll("pending"));
    await idbTransactionDone(tx);
    return rows.filter((r) => r.shard === shard).slice(0, limit);
  }

    async after(shard: string, afterSeq: number, limit = 100): Promise<OutboxRecord[]> {
        const tx = this.db.transaction(STORE_OUTBOX, "readonly");
        const idx = tx.objectStore(STORE_OUTBOX).index("by_shard_seq");
        const range = IDBKeyRange.bound([shard, afterSeq + 1], [shard, Number.MAX_SAFE_INTEGER]);
        const rows = await idbRequest<OutboxRecord[]>(idx.getAll(range, limit));
        await idbTransactionDone(tx);
        return rows;
    }

    async ack(ids: readonly string[]): Promise<number> {
        if (ids.length === 0) return 0;
        const tx = this.db.transaction(STORE_OUTBOX, "readwrite");
        const store = tx.objectStore(STORE_OUTBOX);
        const done = idbTransactionDone(tx);
        let n = 0;
        for (const id of ids) {
            const row = await idbRequest<OutboxRecord | undefined>(store.get(id));
            if (!row) continue;
            store.put({ ...row, status: "acked" });
            n += 1;
        }
        await done;
        return n;
    }

    /**
     * Advance AppliedWatermark monotonically (same shard). Used by local projector /
     * sync consumer after applying outbox records.
     */
    async advanceApplied(token: CommitToken): Promise<AppliedWatermark> {
        const existing = await this.getAppliedWatermark(token.shard);
        if (existing && existing.shard === token.shard && existing.seq > token.seq) {
            return existing;
        }
        const wm: AppliedWatermark = {
            shard: token.shard,
            seq: token.seq,
            appliedAt: nowIso(),
        };
        await this.setAppliedWatermark(wm);
        return wm;
    }
}
