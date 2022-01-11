/**
 * W3 — Local commit journal types + IDB-backed prepare / commit / abort / recover.
 *
 * Protocol design: see ../documentation/local-commit-protocol.md
 *
 * Prepare may record row mutations (W2) and object_pending refs (W4 CAS).
 * Commit applies mutations atomically with journal status in one IDB txn.
 * OPFS GC of state=gc objects is driven by OpfsCas after recover/abort.
 */

import { idbRequest, idbTransactionDone, STORE_JOURNAL, STORE_OBJECT_PENDING, STORE_ROWS } from "./idb.ts";
import { WebRows, type RowMutation } from "./rows.ts";

export type JournalStatus = "pending" | "prepared" | "committed" | "aborted" | "failed";

export type PendingObjectState = "intended" | "written" | "linked" | "gc";

export type PendingObjectRef = {
    /** Content hash (hex). */
    hash: string;
    journalId: string;
    /** Relative OPFS path under the source opfsDir. */
    opfsPath: string;
    state: PendingObjectState;
    /** ISO-8601 */
    updatedAt: string;
};

export type JournalEntry = {
    id: string;
    /** Operation / plan semantic hash from Iris (opaque to the adapter). */
    semanticHash: string;
    schemaFingerprint: string;
    status: JournalStatus;
    createdAt: string;
    updatedAt: string;
    /** Consistency intent label from Iris (string for now). */
    consistency?: string;
    /** Opaque intent / plan fragment for replay diagnostics. */
    intent?: unknown;
    /** Row mutations applied on commit (W2). */
    mutations?: RowMutation[];
    errorMessage?: string;
};

export type PrepareJournalInput = {
    semanticHash: string;
    schemaFingerprint: string;
    consistency?: string;
    intent?: unknown;
    /** Objects that will be / have been written to OPFS before commit. */
    objects?: readonly {
        hash: string;
        opfsPath: string;
        /** Default `intended`; use `written` after OPFS put succeeds (W4). */
        state?: PendingObjectState;
    }[];
    /** Applied on commit inside the same IDB transaction as journal status. */
    mutations?: readonly RowMutation[];
};

export type RecoveryAction =
    | { kind: "finalize_commit"; journalId: string }
    | { kind: "abort_stale_prepare"; journalId: string }
    | { kind: "mark_orphan_object"; hash: string }
    | { kind: "noop" };

export type RecoveryReport = {
    scanned: number;
    actions: RecoveryAction[];
    prepared: JournalEntry[];
    failed: JournalEntry[];
};

function nowIso(): string {
    return new Date().toISOString();
}

function newId(): string {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
        return crypto.randomUUID();
    }
    return `j-${Date.now().toString(16)}-${Math.random().toString(16).slice(2)}`;
}

export class WebJournal {
    readonly #rows: WebRows;

    constructor(readonly db: IDBDatabase) {
        this.#rows = new WebRows(db);
    }

    async get(id: string): Promise<JournalEntry | undefined> {
        const tx = this.db.transaction(STORE_JOURNAL, "readonly");
        const row = await idbRequest<JournalEntry | undefined>(tx.objectStore(STORE_JOURNAL).get(id));
        await idbTransactionDone(tx);
        return row;
    }

    async listByStatus(status: JournalStatus): Promise<JournalEntry[]> {
        const tx = this.db.transaction(STORE_JOURNAL, "readonly");
        const idx = tx.objectStore(STORE_JOURNAL).index("by_status");
        const rows = await idbRequest<JournalEntry[]>(idx.getAll(status));
        await idbTransactionDone(tx);
        return rows;
    }

    async listPendingObjects(state?: PendingObjectState): Promise<PendingObjectRef[]> {
        const tx = this.db.transaction(STORE_OBJECT_PENDING, "readonly");
        const store = tx.objectStore(STORE_OBJECT_PENDING);
        const rows = state
            ? await idbRequest<PendingObjectRef[]>(store.index("by_state").getAll(state))
            : await idbRequest<PendingObjectRef[]>(store.getAll());
        await idbTransactionDone(tx);
        return rows;
    }

    /**
     * Prepare phase: record journal + pending object refs (+ deferred mutations) in one IDB txn.
     * Prefer OPFS put first, then prepare with object state `written`.
     */
    async prepare(input: PrepareJournalInput): Promise<JournalEntry> {
        const ts = nowIso();
        const entry: JournalEntry = {
            id: newId(),
            semanticHash: input.semanticHash,
            schemaFingerprint: input.schemaFingerprint,
            status: "prepared",
            createdAt: ts,
            updatedAt: ts,
            consistency: input.consistency,
            intent: input.intent,
            mutations: input.mutations ? [...input.mutations] : undefined,
        };

        const tx = this.db.transaction([STORE_JOURNAL, STORE_OBJECT_PENDING], "readwrite");
        tx.objectStore(STORE_JOURNAL).put(entry);
        for (const obj of input.objects ?? []) {
            const ref: PendingObjectRef = {
                hash: obj.hash,
                journalId: entry.id,
                opfsPath: obj.opfsPath,
                state: obj.state ?? "intended",
                updatedAt: ts,
            };
            tx.objectStore(STORE_OBJECT_PENDING).put(ref);
        }
        await idbTransactionDone(tx);
        return entry;
    }

    /**
     * Commit phase: mark journal committed, link object refs, apply row mutations (W2).
     * IDB-only atomic; OPFS bytes must already be durable.
     */
    async commit(journalId: string): Promise<JournalEntry> {
        const existing = await this.get(journalId);
        if (!existing) {
            throw new Error(`@yydb/iris-adapter-web: journal ${journalId} not found`);
        }
        if (existing.status !== "prepared" && existing.status !== "pending") {
            throw new Error(`@yydb/iris-adapter-web: journal ${journalId} cannot commit from status ${existing.status}`);
        }
        const updated: JournalEntry = {
            ...existing,
            status: "committed",
            updatedAt: nowIso(),
        };

        const storeNames: string[] = [STORE_JOURNAL, STORE_OBJECT_PENDING];
        if (existing.mutations?.length) storeNames.push(STORE_ROWS);

        const tx = this.db.transaction(storeNames, "readwrite");
        const done = idbTransactionDone(tx);
        tx.objectStore(STORE_JOURNAL).put(updated);
        const pendingReq = tx.objectStore(STORE_OBJECT_PENDING).index("by_journal").getAll(journalId);
        const pending = await idbRequest<PendingObjectRef[]>(pendingReq);
        for (const ref of pending) {
            tx.objectStore(STORE_OBJECT_PENDING).put({
                ...ref,
                state: "linked",
                updatedAt: updated.updatedAt,
            });
        }
        if (existing.mutations?.length) {
            this.#rows.applyMutationsInTransaction(
                tx.objectStore(STORE_ROWS),
                existing.mutations,
                existing.schemaFingerprint,
                updated.updatedAt,
            );
        }
        await done;
        return updated;
    }

    /** Abort a prepared/pending journal; mark objects for GC; discard mutations. */
    async abort(journalId: string, errorMessage?: string): Promise<JournalEntry> {
        const existing = await this.get(journalId);
        if (!existing) {
            throw new Error(`@yydb/iris-adapter-web: journal ${journalId} not found`);
        }
        const updated: JournalEntry = {
            ...existing,
            status: "aborted",
            updatedAt: nowIso(),
            errorMessage,
            // Keep mutations for diagnostics; they were never applied.
        };

        const tx = this.db.transaction([STORE_JOURNAL, STORE_OBJECT_PENDING], "readwrite");
        const done = idbTransactionDone(tx);
        tx.objectStore(STORE_JOURNAL).put(updated);
        const pendingReq = tx.objectStore(STORE_OBJECT_PENDING).index("by_journal").getAll(journalId);
        const pending = await idbRequest<PendingObjectRef[]>(pendingReq);
        for (const ref of pending) {
            tx.objectStore(STORE_OBJECT_PENDING).put({
                ...ref,
                state: "gc",
                updatedAt: updated.updatedAt,
            });
        }
        await done;
        return updated;
    }

    /** Remove object_pending rows after OPFS GC succeeded or file was already missing. */
    async removePendingObjects(hashes: readonly string[]): Promise<number> {
        if (hashes.length === 0) return 0;
        const tx = this.db.transaction(STORE_OBJECT_PENDING, "readwrite");
        const done = idbTransactionDone(tx);
        const store = tx.objectStore(STORE_OBJECT_PENDING);
        for (const hash of hashes) {
            store.delete(hash);
        }
        await done;
        return hashes.length;
    }

    /**
     * Mark `intended` orphans (no durable OPFS write recorded) as `gc` when their
     * journal is aborted/failed/missing — prepares W4 GC.
     */
    async markOrphansForGc(): Promise<string[]> {
        const intended = await this.listPendingObjects("intended");
        const marked: string[] = [];
        for (const ref of intended) {
            const journal = await this.get(ref.journalId);
            if (!journal || journal.status === "aborted" || journal.status === "failed") {
                const tx = this.db.transaction(STORE_OBJECT_PENDING, "readwrite");
                tx.objectStore(STORE_OBJECT_PENDING).put({
                    ...ref,
                    state: "gc",
                    updatedAt: nowIso(),
                });
                await idbTransactionDone(tx);
                marked.push(ref.hash);
            }
        }
        return marked;
    }

    /**
     * Crash recovery (IDB half + orphan marking):
     * - `prepared`/`pending` older than `staleMs` → abort (objects → gc)
     * - intended orphans with dead journals → gc
     * - does not auto-finalize commit (Commit is explicit)
     * Caller should run OpfsCas.gc on `listPendingObjects("gc")` afterward (W4).
     */
    async recover(options?: { staleMs?: number; now?: number }): Promise<RecoveryReport> {
        const staleMs = options?.staleMs ?? 15 * 60 * 1000;
        const now = options?.now ?? Date.now();
        const prepared = await this.listByStatus("prepared");
        const pending = await this.listByStatus("pending");
        const failed = await this.listByStatus("failed");
        const actions: RecoveryAction[] = [];
        const scanned = prepared.length + pending.length;

        for (const entry of [...prepared, ...pending]) {
            const age = now - Date.parse(entry.updatedAt);
            if (Number.isFinite(age) && age > staleMs) {
                await this.abort(entry.id, "recovery: stale prepare");
                actions.push({ kind: "abort_stale_prepare", journalId: entry.id });
            } else {
                actions.push({ kind: "noop" });
            }
        }

        const orphans = await this.markOrphansForGc();
        for (const hash of orphans) {
            actions.push({ kind: "mark_orphan_object", hash });
        }

        return {
            scanned,
            actions,
            prepared: await this.listByStatus("prepared"),
            failed,
        };
    }
}
