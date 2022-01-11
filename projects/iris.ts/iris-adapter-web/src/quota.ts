/**
 * W7 — Quota / persistence / writable observability (conformance evidence).
 *
 * probe() must not equate "API exists" with "writable / persistent / multi-tab safe".
 */

import { detectCoordCapabilities, type CoordCapabilities } from "./coord.ts";

export type PersistenceState = "granted" | "prompt" | "denied" | "unknown";

export type QuotaHeadroom = "ok" | "low" | "critical" | "unknown";

export type WritableProbe = {
    ok: boolean;
    detail: string;
};

export type WebConformanceSnapshot = {
    at: string;
    indexedDb: {
        available: boolean;
        writable: WritableProbe;
        persistent: PersistenceState;
    };
    opfs: {
        available: boolean;
        writable: WritableProbe;
        syncAccessHandle: boolean | "unknown";
    };
    quota: {
        usageBytes: number | null;
        quotaBytes: number | null;
        headroom: QuotaHeadroom;
        /** Remaining bytes when both usage and quota known. */
        remainingBytes: number | null;
    };
    multiContext: CoordCapabilities;
};

const LOW_RATIO = 0.15;
const CRITICAL_RATIO = 0.05;
const LOW_BYTES = 8 * 1024 * 1024;

function nowIso(): string {
    return new Date().toISOString();
}

function classifyHeadroom(usage: number | null, quota: number | null): { headroom: QuotaHeadroom; remainingBytes: number | null } {
    if (usage == null || quota == null || quota <= 0) {
        return { headroom: "unknown", remainingBytes: null };
    }
    const remaining = Math.max(0, quota - usage);
    const ratio = remaining / quota;
    if (ratio <= CRITICAL_RATIO || remaining < 1024 * 1024) {
        return { headroom: "critical", remainingBytes: remaining };
    }
    if (ratio <= LOW_RATIO || remaining < LOW_BYTES) {
        return { headroom: "low", remainingBytes: remaining };
    }
    return { headroom: "ok", remainingBytes: remaining };
}

async function probeIdbWritable(dbName: string): Promise<WritableProbe> {
    if (typeof indexedDB === "undefined") {
        return { ok: false, detail: "indexedDB unavailable" };
    }
    const probeName = `${dbName}__probe`;
    try {
        const db = await new Promise<IDBDatabase>((resolve, reject) => {
            const req = indexedDB.open(probeName, 1);
            req.onerror = () => reject(req.error ?? new Error("idb probe open failed"));
            req.onupgradeneeded = () => {
                const d = req.result;
                if (!d.objectStoreNames.contains("p")) d.createObjectStore("p");
            };
            req.onsuccess = () => resolve(req.result);
        });
        await new Promise<void>((resolve, reject) => {
            const tx = db.transaction("p", "readwrite");
            tx.objectStore("p").put({ t: Date.now() }, "k");
            tx.oncomplete = () => resolve();
            tx.onerror = () => reject(tx.error ?? new Error("idb probe write failed"));
        });
        db.close();
        indexedDB.deleteDatabase(probeName);
        return { ok: true, detail: "write+delete ok" };
    } catch (err) {
        return {
            ok: false,
            detail: err instanceof Error ? err.message : String(err),
        };
    }
}

async function probeOpfsWritable(opfsDir: string): Promise<{
    writable: WritableProbe;
    syncAccessHandle: boolean | "unknown";
}> {
    if (typeof navigator === "undefined" || typeof navigator.storage?.getDirectory !== "function") {
        return {
            writable: { ok: false, detail: "OPFS unavailable" },
            syncAccessHandle: "unknown",
        };
    }
    let syncAccessHandle: boolean | "unknown" = "unknown";
    try {
        const root = await navigator.storage.getDirectory();
        const segments = opfsDir.split("/").filter(Boolean);
        let dir = root;
        for (const seg of segments) {
            dir = await dir.getDirectoryHandle(seg, { create: true });
        }
        const probeDir = await dir.getDirectoryHandle("__quota_probe__", { create: true });
        const file = await probeDir.getFileHandle("w7.bin", { create: true });

        // syncAccessHandle is worker-oriented; detect presence without requiring Worker.
        const anyFile = file as FileSystemFileHandle & {
            createSyncAccessHandle?: () => Promise<unknown>;
        };
        syncAccessHandle = typeof anyFile.createSyncAccessHandle === "function";

        const writable = await file.createWritable();
        await writable.write(new Uint8Array([1, 2, 3]));
        await writable.close();
        await probeDir.removeEntry("w7.bin");
        try {
            await dir.removeEntry("__quota_probe__", { recursive: true });
        } catch {
            /* ignore */
        }
        return {
            writable: { ok: true, detail: "opfs write+delete ok" },
            syncAccessHandle,
        };
    } catch (err) {
        return {
            writable: {
                ok: false,
                detail: err instanceof Error ? err.message : String(err),
            },
            syncAccessHandle,
        };
    }
}

export type CollectConformanceOptions = {
    indexedDbName: string;
    opfsDir: string;
    /** Attempt navigator.storage.persist() when not already granted. Default false. */
    requestPersist?: boolean;
    /** Skip destructive writable probes (read-only estimate). Default false. */
    skipWritableProbes?: boolean;
};

export async function collectConformanceSnapshot(options: CollectConformanceOptions): Promise<WebConformanceSnapshot> {
    const at = nowIso();
    const idbAvailable = typeof indexedDB !== "undefined";
    const opfsAvailable = typeof navigator !== "undefined" && typeof navigator.storage?.getDirectory === "function";

    let usageBytes: number | null = null;
    let quotaBytes: number | null = null;
    let persistent: PersistenceState = "unknown";

    if (typeof navigator !== "undefined" && navigator.storage?.estimate) {
        try {
            const est = await navigator.storage.estimate();
            usageBytes = typeof est.usage === "number" ? est.usage : null;
            quotaBytes = typeof est.quota === "number" ? est.quota : null;
        } catch {
            /* ignore */
        }
    }

    if (typeof navigator !== "undefined" && navigator.storage?.persisted) {
        try {
            const granted = await navigator.storage.persisted();
            persistent = granted ? "granted" : "prompt";
            if (!granted && options.requestPersist && navigator.storage.persist) {
                const ok = await navigator.storage.persist();
                persistent = ok ? "granted" : "denied";
            }
        } catch {
            persistent = "unknown";
        }
    }

    const { headroom, remainingBytes } = classifyHeadroom(usageBytes, quotaBytes);
    const multiContext = detectCoordCapabilities();

    let idbWritable: WritableProbe = {
        ok: false,
        detail: options.skipWritableProbes ? "skipped" : "not probed",
    };
    let opfsWritable: WritableProbe = {
        ok: false,
        detail: options.skipWritableProbes ? "skipped" : "not probed",
    };
    let syncAccessHandle: boolean | "unknown" = "unknown";

    if (!options.skipWritableProbes) {
        if (idbAvailable) idbWritable = await probeIdbWritable(options.indexedDbName);
        if (opfsAvailable) {
            const opfs = await probeOpfsWritable(options.opfsDir);
            opfsWritable = opfs.writable;
            syncAccessHandle = opfs.syncAccessHandle;
        } else {
            opfsWritable = { ok: false, detail: "OPFS unavailable" };
        }
    }

    return {
        at,
        indexedDb: { available: idbAvailable, writable: idbWritable, persistent },
        opfs: { available: opfsAvailable, writable: opfsWritable, syncAccessHandle },
        quota: { usageBytes, quotaBytes, headroom, remainingBytes },
        multiContext,
    };
}
