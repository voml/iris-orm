/**
 * W2 — Minimal logical row store in IndexedDB (entity + identity key).
 *
 * Not a full VOS query executor: upsert / get / delete / list-by-entity only.
 * Durable writes that must participate in the local commit protocol go through
 * journal prepare → commit (mutations applied atomically with journal status).
 */

import { idbRequest, idbTransactionDone, STORE_ROWS } from "./idb.ts";

export type RowRecord = {
    /** Entity / type name from VOS schema (opaque string to the adapter). */
    entity: string;
    /** Primary identity serialized as string. */
    id: string;
    /** Small / indexed field map (JSON-cloneable). */
    fields: Record<string, unknown>;
    /** Optional field → CAS hash for large values stored in OPFS. */
    objectRefs?: Record<string, string>;
    schemaFingerprint: string;
    /** ISO-8601 */
    updatedAt: string;
};

export type RowUpsert = {
    kind: "upsert";
    entity: string;
    id: string;
    fields: Record<string, unknown>;
    objectRefs?: Record<string, string>;
};

export type RowDelete = {
    kind: "delete";
    entity: string;
    id: string;
};

export type RowMutation = RowUpsert | RowDelete;

function nowIso(): string {
    return new Date().toISOString();
}

export class WebRows {
    constructor(readonly db: IDBDatabase) {}

    async get(entity: string, id: string): Promise<RowRecord | undefined> {
        const tx = this.db.transaction(STORE_ROWS, "readonly");
        const row = await idbRequest<RowRecord | undefined>(tx.objectStore(STORE_ROWS).get([entity, id]));
        await idbTransactionDone(tx);
        return row;
    }

    async list(entity: string): Promise<RowRecord[]> {
        const tx = this.db.transaction(STORE_ROWS, "readonly");
        const idx = tx.objectStore(STORE_ROWS).index("by_entity");
        const rows = await idbRequest<RowRecord[]>(idx.getAll(entity));
        await idbTransactionDone(tx);
        return rows;
    }

    /**
     * Immediate upsert outside a journal (debug / bootstrap only).
     * Prefer `journal.prepare` + mutations for durable protocol writes.
     */
    async upsert(input: Omit<RowRecord, "updatedAt"> & { updatedAt?: string }): Promise<RowRecord> {
        const record: RowRecord = {
            entity: input.entity,
            id: input.id,
            fields: input.fields,
            objectRefs: input.objectRefs,
            schemaFingerprint: input.schemaFingerprint,
            updatedAt: input.updatedAt ?? nowIso(),
        };
        const tx = this.db.transaction(STORE_ROWS, "readwrite");
        tx.objectStore(STORE_ROWS).put(record);
        await idbTransactionDone(tx);
        return record;
    }

    async delete(entity: string, id: string): Promise<boolean> {
        const existing = await this.get(entity, id);
        if (!existing) return false;
        const tx = this.db.transaction(STORE_ROWS, "readwrite");
        tx.objectStore(STORE_ROWS).delete([entity, id]);
        await idbTransactionDone(tx);
        return true;
    }

    /** Apply mutations inside an already-opened readwrite transaction that includes STORE_ROWS. */
    applyMutationsInTransaction(store: IDBObjectStore, mutations: readonly RowMutation[], schemaFingerprint: string, updatedAt: string): void {
        for (const m of mutations) {
            if (m.kind === "delete") {
                store.delete([m.entity, m.id]);
                continue;
            }
            const record: RowRecord = {
                entity: m.entity,
                id: m.id,
                fields: m.fields,
                objectRefs: m.objectRefs,
                schemaFingerprint,
                updatedAt,
            };
            store.put(record);
        }
    }
}
