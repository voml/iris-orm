/**
 * IndexedDB open + object-store bootstrap for Web Local Store (W1–W7).
 */

/** v1: meta/schema/journal/object_pending; v2: + rows; v3: + outbox/watermark. */
export const IDB_SCHEMA_VERSION = 3 as const;

export const STORE_META = "meta" as const;
export const STORE_SCHEMA = "schema" as const;
export const STORE_JOURNAL = "journal" as const;
export const STORE_OBJECT_PENDING = "object_pending" as const;
export const STORE_ROWS = "rows" as const;
export const STORE_OUTBOX = "outbox" as const;
export const STORE_WATERMARK = "watermark" as const;

export type WebIdbStoreName =
  | typeof STORE_META
  | typeof STORE_SCHEMA
  | typeof STORE_JOURNAL
  | typeof STORE_OBJECT_PENDING
  | typeof STORE_ROWS
  | typeof STORE_OUTBOX
  | typeof STORE_WATERMARK;

export function requireIndexedDb(): IDBFactory {
  if (typeof indexedDB === "undefined") {
    throw new Error("@yydb/iris-adapter-web: IndexedDB is not available");
  }
  return indexedDB;
}

/** Open (or upgrade) the Iris web catalog database. */
export function openCatalogDatabase(dbName: string): Promise<IDBDatabase> {
  const factory = requireIndexedDb();
  return new Promise((resolve, reject) => {
    const req = factory.open(dbName, IDB_SCHEMA_VERSION);
    req.onerror = () => reject(req.error ?? new Error("@yydb/iris-adapter-web: IDB open failed"));
    req.onblocked = () =>
      reject(new Error("@yydb/iris-adapter-web: IDB open blocked (close other tabs?)"));
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE_META)) {
        db.createObjectStore(STORE_META, { keyPath: "key" });
      }
      if (!db.objectStoreNames.contains(STORE_SCHEMA)) {
        const schema = db.createObjectStore(STORE_SCHEMA, { keyPath: "schemaId" });
        schema.createIndex("by_fingerprint", "fingerprint", { unique: false });
      }
      if (!db.objectStoreNames.contains(STORE_JOURNAL)) {
        const journal = db.createObjectStore(STORE_JOURNAL, { keyPath: "id" });
        journal.createIndex("by_status", "status", { unique: false });
        journal.createIndex("by_updated", "updatedAt", { unique: false });
      }
      if (!db.objectStoreNames.contains(STORE_OBJECT_PENDING)) {
        const objects = db.createObjectStore(STORE_OBJECT_PENDING, { keyPath: "hash" });
        objects.createIndex("by_journal", "journalId", { unique: false });
        objects.createIndex("by_state", "state", { unique: false });
      }
      if (!db.objectStoreNames.contains(STORE_ROWS)) {
        const rows = db.createObjectStore(STORE_ROWS, { keyPath: ["entity", "id"] });
        rows.createIndex("by_entity", "entity", { unique: false });
        rows.createIndex("by_updated", "updatedAt", { unique: false });
      }
      if (!db.objectStoreNames.contains(STORE_OUTBOX)) {
        const outbox = db.createObjectStore(STORE_OUTBOX, { keyPath: "id" });
        outbox.createIndex("by_status", "status", { unique: false });
        outbox.createIndex("by_shard_seq", ["shard", "seq"], { unique: true });
      }
      if (!db.objectStoreNames.contains(STORE_WATERMARK)) {
        db.createObjectStore(STORE_WATERMARK, { keyPath: "shard" });
      }
    };
    req.onsuccess = () => {
      const db = req.result;
      db.onversionchange = () => {
        db.close();
      };
      resolve(db);
    };
  });
}

export function idbRequest<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error ?? new Error("@yydb/iris-adapter-web: IDB request failed"));
  });
}

export function idbTransactionDone(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error ?? new Error("@yydb/iris-adapter-web: IDB txn failed"));
    tx.onabort = () => reject(tx.error ?? new Error("@yydb/iris-adapter-web: IDB txn aborted"));
  });
}

export type MetaRecord = { key: string; value: unknown };

export async function putMeta(db: IDBDatabase, key: string, value: unknown): Promise<void> {
  const tx = db.transaction(STORE_META, "readwrite");
  tx.objectStore(STORE_META).put({ key, value } satisfies MetaRecord);
  await idbTransactionDone(tx);
}

export async function getMeta<T = unknown>(db: IDBDatabase, key: string): Promise<T | undefined> {
  const tx = db.transaction(STORE_META, "readonly");
  const row = await idbRequest<MetaRecord | undefined>(tx.objectStore(STORE_META).get(key));
  await idbTransactionDone(tx);
  return row?.value as T | undefined;
}
