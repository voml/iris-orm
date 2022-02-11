import type { IrisSession } from "../types/session.ts";
import { buildRuntime } from "../runtime/build-runtime.ts";
import { IrisFacadeError } from "../types/errors.ts";
import { getWasmSemanticCore } from "./wasm.ts";

/** Local Web Backend storage profile. */
export type LocalStoreBackend = "memory" | "indexeddb" | "opfs";

export type OpenLocalStoreOptions = {
    backend: LocalStoreBackend;
    name: string;
};

/** Browser-local persistence handle (Local Web Backend; not YYDB). */
export interface LocalStore {
    readonly backend: LocalStoreBackend;
    readonly name: string;
    /** Open a memory-backed Iris session bound to this store profile. */
    openSession(): IrisSession;
    close(): Promise<void>;
}

const IDB_NAME = "yydb-iris-local";
const IDB_STORE = "tables";

async function openIndexedDb(name: string): Promise<IDBDatabase> {
    return await new Promise((resolve, reject) => {
        const request = indexedDB.open(`${IDB_NAME}:${name}`, 1);
        request.onupgradeneeded = () => {
            const db = request.result;
            if (!db.objectStoreNames.contains(IDB_STORE)) {
                db.createObjectStore(IDB_STORE);
            }
        };
        request.onerror = () => reject(request.error ?? new Error("indexedDB open failed"));
        request.onsuccess = () => resolve(request.result);
    });
}

async function openOpfsRoot(name: string): Promise<FileSystemDirectoryHandle> {
    const root = await navigator.storage.getDirectory();
    return await root.getDirectoryHandle(name, { create: true });
}

/** Open a browser Local Web Backend store (memory / IndexedDB / OPFS metadata). */
export async function openLocalStore(options: OpenLocalStoreOptions): Promise<LocalStore> {
    if (options.backend === "memory") {
        return {
            backend: "memory",
            name: options.name,
            openSession: () => buildRuntime("web", getWasmSemanticCore()).openSession(),
            close: async () => {},
        };
    }

    if (options.backend === "indexeddb") {
        const db = await openIndexedDb(options.name);
        return {
            backend: "indexeddb",
            name: options.name,
            openSession: () => buildRuntime("web", getWasmSemanticCore()).openSession(),
            close: async () => {
                db.close();
            },
        };
    }

    if (options.backend === "opfs") {
        if (!globalThis.navigator?.storage?.getDirectory) {
            throw new IrisFacadeError("opfs-unavailable", "@yydb/iris: OPFS is not available in this browser host");
        }
        await openOpfsRoot(options.name);
        return {
            backend: "opfs",
            name: options.name,
            openSession: () => buildRuntime("web", getWasmSemanticCore()).openSession(),
            close: async () => {},
        };
    }

    throw new IrisFacadeError("local-store-unsupported", `@yydb/iris: unsupported local store backend ${options.backend}`);
}
