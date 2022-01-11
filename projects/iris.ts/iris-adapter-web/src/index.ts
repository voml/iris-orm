/**
 * `@yydb/iris-adapter-web` — browser local Iris / VOS store (IndexedDB + OPFS).
 *
 * Architecture §2.1b: Local Web Backend / Offline Authority candidate.
 * W0–W4: namespace, catalog, rows, journal, OPFS CAS.
 * W5: multi-tab Web Locks + BroadcastChannel.
 * W6: local outbox + CommitToken / AppliedWatermark.
 * W7: quota / writable / persistence conformance snapshot.
 * Planner: structured web intents → WebPhysicalPlan → execute (not VOS parser).
 */

import type { IrisPlaceholder } from "@yydb/iris-types";
import { BACKEND_ID } from "./backend-id.ts";
import { WebCatalog, type CatalogSnapshot, type InstallSchemaOptions } from "./catalog.ts";
import { WebCoordinator, type CoordEvent, type CoordListener } from "./coord.ts";
import {
  computeEnvelopeFingerprint,
  computeSchemaFingerprint,
  type SchemaCatalogRecord,
} from "./fingerprint.ts";
import { openCatalogDatabase } from "./idb.ts";
import {
  WebJournal,
  type JournalEntry,
  type PrepareJournalInput,
  type RecoveryReport,
} from "./journal.ts";
import { OpfsCas, casPathForHash, type CasGcReport, type CasPutResult } from "./opfs-cas.ts";
import {
  WebOutbox,
  type AppliedWatermark,
  type CommitToken,
  type OutboxAppend,
  type OutboxRecord,
} from "./outbox.ts";
import {
  executeWebReads,
  planWebIntents,
  type ExecuteReadResult,
  type WebIntentBatch,
  type WebPhysicalPlan,
} from "./plan.ts";
import { collectConformanceSnapshot, type WebConformanceSnapshot } from "./quota.ts";
import { WebRows, type RowMutation, type RowRecord } from "./rows.ts";

export { BACKEND_ID } from "./backend-id.ts";
export {
  casPathForHash,
  collectConformanceSnapshot,
  computeEnvelopeFingerprint,
  computeSchemaFingerprint,
  executeWebReads,
  OpfsCas,
  planWebIntents,
  WebCatalog,
  WebCoordinator,
  WebJournal,
  WebOutbox,
  WebRows,
};
export type {
  AppliedWatermark,
  CasGcReport,
  CasPutResult,
  CatalogSnapshot,
  CommitToken,
  CoordEvent,
  CoordListener,
  ExecuteReadResult,
  InstallSchemaOptions,
  JournalEntry,
  OutboxAppend,
  OutboxRecord,
  PrepareJournalInput,
  RecoveryReport,
  RowMutation,
  RowRecord,
  SchemaCatalogRecord,
  WebConformanceSnapshot,
  WebIntentBatch,
  WebPhysicalPlan,
};

export type WebAdapterOptions = {
  name: string;
  indexedDbName?: string;
  opfsDir?: string;
};

export type WebStorageHandles = {
  indexedDbName: string;
  opfsDir: string;
};

export type WebStorageProbe = WebConformanceSnapshot;

export type WriteThroughInput = {
  semanticHash: string;
  schemaFingerprint: string;
  consistency?: string;
  intent?: unknown;
  mutations?: readonly RowMutation[];
  objectBytes?: readonly Uint8Array[];
  /** Default true — enqueue outbox for each row mutation (W6). */
  appendOutbox?: boolean;
};

export type WriteThroughResult = {
  journal: JournalEntry;
  objects: CasPutResult[];
  commitToken?: CommitToken;
  outbox: OutboxRecord[];
};

export type RecoverWithGcReport = RecoveryReport & {
  gc: CasGcReport;
  removedPending: number;
};

export type ExecuteBatchResult = {
  plan: WebPhysicalPlan;
  reads: ExecuteReadResult;
  write?: WriteThroughResult;
};

export class WebSource {
  readonly backendId = BACKEND_ID;
  readonly storage: WebStorageHandles;
  #db: IDBDatabase | null = null;
  #catalog: WebCatalog | null = null;
  #journal: WebJournal | null = null;
  #rows: WebRows | null = null;
  #cas: OpfsCas | null = null;
  #outbox: WebOutbox | null = null;
  #coord: WebCoordinator;

  constructor(readonly options: WebAdapterOptions) {
    if (!options.name.trim()) {
      throw new Error("@yydb/iris-adapter-web: options.name is required");
    }
    this.storage = {
      indexedDbName: options.indexedDbName ?? `iris-web:${options.name}`,
      opfsDir: options.opfsDir ?? `iris-web/${options.name}`,
    };
    this.#coord = new WebCoordinator(options.name);
  }

  get indexedDbAvailable(): boolean {
    return typeof indexedDB !== "undefined";
  }

  get opfsAvailable(): boolean {
    return (
      typeof navigator !== "undefined" &&
      typeof navigator.storage?.getDirectory === "function"
    );
  }

  get isOpen(): boolean {
    return this.#db !== null;
  }

  get catalog(): WebCatalog {
    if (!this.#catalog) {
      throw new Error("@yydb/iris-adapter-web: call open() before using catalog");
    }
    return this.#catalog;
  }

  get journal(): WebJournal {
    if (!this.#journal) {
      throw new Error("@yydb/iris-adapter-web: call open() before using journal");
    }
    return this.#journal;
  }

  get rows(): WebRows {
    if (!this.#rows) {
      throw new Error("@yydb/iris-adapter-web: call open() before using rows");
    }
    return this.#rows;
  }

  get cas(): OpfsCas {
    if (!this.#cas) {
      throw new Error("@yydb/iris-adapter-web: call open() before using CAS (OPFS required)");
    }
    return this.#cas;
  }

  get outbox(): WebOutbox {
    if (!this.#outbox) {
      throw new Error("@yydb/iris-adapter-web: call open() before using outbox");
    }
    return this.#outbox;
  }

  get coordinator(): WebCoordinator {
    return this.#coord;
  }

  /** W7 — full conformance / quota / writable snapshot (may write probe keys). */
  async probe(options?: {
    requestPersist?: boolean;
    skipWritableProbes?: boolean;
  }): Promise<WebStorageProbe> {
    return collectConformanceSnapshot({
      indexedDbName: this.storage.indexedDbName,
      opfsDir: this.storage.opfsDir,
      requestPersist: options?.requestPersist,
      skipWritableProbes: options?.skipWritableProbes ?? false,
    });
  }

  /**
   * Open IndexedDB + OPFS CAS (when available) + coordinator.
   * Runs journal recovery + OPFS GC by default.
   */
  async open(options?: {
    recover?: boolean;
    staleMs?: number;
    requireOpfs?: boolean;
  }): Promise<this> {
    const p = await this.probe({ skipWritableProbes: true });
    if (!p.indexedDb.available) {
      throw new Error("@yydb/iris-adapter-web: IndexedDB is not available");
    }
    if (this.#db) return this;

    const requireOpfs = options?.requireOpfs === true;
    if (requireOpfs && !p.opfs.available) {
      throw new Error("@yydb/iris-adapter-web: OPFS is required but not available");
    }

    const db = await openCatalogDatabase(this.storage.indexedDbName);
    const catalog = new WebCatalog(db, BACKEND_ID);
    await catalog.ensureBootstrapped();
    const journal = new WebJournal(db);
    const rows = new WebRows(db);
    const outbox = new WebOutbox(db);

    let cas: OpfsCas | null = null;
    if (p.opfs.available) {
      cas = new OpfsCas(this.storage.opfsDir);
      await cas.open();
    } else if (requireOpfs) {
      db.close();
      throw new Error("@yydb/iris-adapter-web: OPFS open failed");
    }

    this.#db = db;
    this.#catalog = catalog;
    this.#journal = journal;
    this.#rows = rows;
    this.#outbox = outbox;
    this.#cas = cas;
    this.#coord.open();

    if (options?.recover !== false) {
      await this.recoverJournal({ staleMs: options?.staleMs });
    }
    return this;
  }

  async close(): Promise<void> {
    this.#coord.close();
    await this.#cas?.close();
    this.#db?.close();
    this.#db = null;
    this.#catalog = null;
    this.#journal = null;
    this.#rows = null;
    this.#cas = null;
    this.#outbox = null;
  }

  onCoordEvent(listener: CoordListener): () => void {
    return this.#coord.subscribe(listener);
  }

  installSchema(options: InstallSchemaOptions): Promise<SchemaCatalogRecord> {
    return this.#coord.withWriteLock(async () => {
      const record = await this.catalog.installSchema(options);
      this.#coord.publish({
        kind: "schema",
        schemaId: record.schemaId,
        fingerprint: record.fingerprint,
      });
      return record;
    });
  }

  getSchema(schemaId: string): Promise<SchemaCatalogRecord | undefined> {
    return this.catalog.getSchema(schemaId);
  }

  listSchemas(): Promise<SchemaCatalogRecord[]> {
    return this.catalog.listSchemas();
  }

  catalogSnapshot(): Promise<CatalogSnapshot> {
    return this.catalog.snapshot();
  }

  getRow(entity: string, id: string): Promise<RowRecord | undefined> {
    return this.rows.get(entity, id);
  }

  listRows(entity: string): Promise<RowRecord[]> {
    return this.rows.list(entity);
  }

  putObject(bytes: Uint8Array): Promise<CasPutResult> {
    return this.cas.put(bytes);
  }

  getObject(hash: string): Promise<Uint8Array | undefined> {
    return this.cas.get(hash);
  }

  /** Plan structured intents (adapter-side; not VOS source parse). */
  plan(batch: WebIntentBatch): WebPhysicalPlan {
    return planWebIntents(batch);
  }

  /** Execute planned reads only. */
  async query(batch: WebIntentBatch): Promise<{ plan: WebPhysicalPlan; reads: ExecuteReadResult }> {
    const plan = planWebIntents({
      ...batch,
      writes: [],
    });
    if (plan.rejected) {
      throw new Error(
        `@yydb/iris-adapter-web: query plan rejected: ${plan.rejectionNotes.join("; ")}`,
      );
    }
    const reads = await executeWebReads(plan, this.rows, this.#cas);
    return { plan, reads };
  }

  /**
   * Plan + execute reads; if writes present, run writeThrough under write lock.
   */
  async execute(batch: WebIntentBatch): Promise<ExecuteBatchResult> {
    const plan = planWebIntents(batch);
    if (plan.rejected) {
      throw new Error(
        `@yydb/iris-adapter-web: execute plan rejected: ${plan.rejectionNotes.join("; ")}`,
      );
    }
    const reads = await executeWebReads(
      { ...plan, mutations: [], objectBytes: [] },
      this.rows,
      this.#cas,
    );

    if (plan.mutations.length === 0 && plan.objectBytes.length === 0) {
      return { plan, reads };
    }

    const write = await this.writeThrough({
      semanticHash: plan.semanticHash,
      schemaFingerprint: plan.schemaFingerprint,
      consistency: plan.consistency,
      mutations: plan.mutations,
      objectBytes: plan.objectBytes,
      appendOutbox: plan.appendOutbox,
    });
    return { plan, reads, write };
  }

  prepareCommit(input: PrepareJournalInput): Promise<JournalEntry> {
    return this.#coord.withWriteLock(() => this.journal.prepare(input));
  }

  commitPrepared(journalId: string): Promise<JournalEntry> {
    return this.#coord.withWriteLock(async () => {
      const entry = await this.journal.commit(journalId);
      this.#coord.publish({
        kind: "commit",
        journalId: entry.id,
        schemaFingerprint: entry.schemaFingerprint,
      });
      return entry;
    });
  }

  abortPrepared(journalId: string, errorMessage?: string): Promise<JournalEntry> {
    return this.#coord.withWriteLock(() => this.journal.abort(journalId, errorMessage));
  }

  /**
   * CAS-put → prepare → commit (+ optional outbox) under W5 write lock.
   */
  async writeThrough(input: WriteThroughInput): Promise<WriteThroughResult> {
    return this.#coord.withWriteLock(async () => {
      const objects: CasPutResult[] = [];
      for (const bytes of input.objectBytes ?? []) {
        if (!this.#cas) {
          throw new Error("@yydb/iris-adapter-web: OPFS CAS required for objectBytes");
        }
        objects.push(await this.#cas.put(bytes));
      }

      const prepared = await this.journal.prepare({
        semanticHash: input.semanticHash,
        schemaFingerprint: input.schemaFingerprint,
        consistency: input.consistency,
        intent: input.intent,
        mutations: input.mutations,
        objects: objects.map((o) => ({
          hash: o.hash,
          opfsPath: o.opfsPath,
          state: "written" as const,
        })),
      });

      const journal = await this.journal.commit(prepared.id);

      let commitToken: CommitToken | undefined;
      let outboxRecords: OutboxRecord[] = [];
      const appendOutbox = input.appendOutbox !== false;
      if (appendOutbox && input.mutations?.length) {
        const appends: OutboxAppend[] = input.mutations.map((m) => ({
          entity: m.entity,
          entityId: m.id,
          effect: m.kind === "delete" ? "delete" : "upsert",
          schemaFingerprint: input.schemaFingerprint,
          journalId: journal.id,
          payload: m.kind === "upsert" ? { fields: m.fields, objectRefs: m.objectRefs } : undefined,
        }));
        const appended = await this.outbox.appendMany(appends);
        commitToken = appended.token;
        outboxRecords = appended.records;
        this.#coord.publish({ kind: "outbox", seq: commitToken.seq });
      }

      this.#coord.publish({
        kind: "commit",
        journalId: journal.id,
        schemaFingerprint: journal.schemaFingerprint,
      });

      return { journal, objects, commitToken, outbox: outboxRecords };
    });
  }

  async recoverJournal(options?: { staleMs?: number }): Promise<RecoverWithGcReport> {
    return this.#coord.withWriteLock(async () => {
      const report = await this.journal.recover({ staleMs: options?.staleMs });
      let gc: CasGcReport = { scanned: 0, deleted: [], missing: [], failed: [] };
      let removedPending = 0;

      if (this.#cas?.isOpen) {
        const pendingGc = await this.journal.listPendingObjects("gc");
        gc = await this.#cas.gc(pendingGc);
        const removable = [...gc.deleted, ...gc.missing];
        removedPending = await this.journal.removePendingObjects(removable);
      }

      this.#coord.publish({ kind: "recover" });
      return { ...report, gc, removedPending };
    });
  }

  listOutboxPending(options?: { shard?: string; limit?: number }): Promise<OutboxRecord[]> {
    return this.outbox.listPending(options);
  }

  ackOutbox(ids: readonly string[]): Promise<number> {
    return this.#coord.withWriteLock(() => this.outbox.ack(ids));
  }

  getCommitToken(shard?: string): Promise<CommitToken> {
    return this.outbox.getCommitToken(shard);
  }

  getAppliedWatermark(shard?: string): Promise<AppliedWatermark | undefined> {
    return this.outbox.getAppliedWatermark(shard);
  }

  advanceAppliedWatermark(token: CommitToken): Promise<AppliedWatermark> {
    return this.#coord.withWriteLock(() => this.outbox.advanceApplied(token));
  }

  placeholder(): IrisPlaceholder {
    return { __irisTypes: "native-ts-skeleton" };
  }
}

export function createWebSource(options: WebAdapterOptions): WebSource {
  return new WebSource(options);
}
