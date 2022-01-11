/**
 * W4 — OPFS content-addressed object store (CAS).
 *
 * Layout under source `opfsDir`:
 *   objects/<hash[0:2]>/<hash>.bytes
 *
 * Prefer write-then-prepare (immutable bytes durable before journal prepare).
 * GC deletes objects whose IDB `object_pending.state === "gc"`.
 */

import { computeSchemaFingerprint } from "./fingerprint.ts";
import type { PendingObjectRef } from "./journal.ts";

export type CasPutResult = {
    hash: string;
    /** Path relative to source opfsDir (forward slashes). */
    opfsPath: string;
    byteLength: number;
};

export type CasGcReport = {
    scanned: number;
    deleted: string[];
    missing: string[];
    failed: { hash: string; error: string }[];
};

/** Relative OPFS path for a content hash (YYDB-style hash-2 sharding). */
export function casPathForHash(hash: string): string {
    const h = hash.toLowerCase();
    if (!/^[0-9a-f]{64}$/.test(h)) {
        throw new Error(`@yydb/iris-adapter-web: invalid CAS hash (want sha-256 hex): ${hash.slice(0, 16)}…`);
    }
    return `objects/${h.slice(0, 2)}/${h}.bytes`;
}

export function requireOpfsRoot(): FileSystemDirectoryHandle | Promise<FileSystemDirectoryHandle> {
    if (typeof navigator === "undefined" || typeof navigator.storage?.getDirectory !== "function") {
        throw new Error("@yydb/iris-adapter-web: OPFS (navigator.storage.getDirectory) is not available");
    }
    return navigator.storage.getDirectory();
}

async function getOrCreateDir(parent: FileSystemDirectoryHandle, name: string): Promise<FileSystemDirectoryHandle> {
    return parent.getDirectoryHandle(name, { create: true });
}

/** Resolve nested path segments under a directory handle. */
export async function resolveDirectory(
    root: FileSystemDirectoryHandle,
    segments: readonly string[],
    create: boolean,
): Promise<FileSystemDirectoryHandle> {
    let cur = root;
    for (const seg of segments) {
        if (!seg || seg === "." || seg === "..") {
            throw new Error(`@yydb/iris-adapter-web: invalid OPFS path segment "${seg}"`);
        }
        cur = create ? await getOrCreateDir(cur, seg) : await cur.getDirectoryHandle(seg, { create: false });
    }
    return cur;
}

function splitRelPath(rel: string): { dirs: string[]; file: string } {
    const parts = rel.split("/").filter(Boolean);
    if (parts.length === 0) {
        throw new Error("@yydb/iris-adapter-web: empty OPFS relative path");
    }
    const file = parts[parts.length - 1]!;
    return { dirs: parts.slice(0, -1), file };
}

export class OpfsCas {
    #root: FileSystemDirectoryHandle | null = null;
    #base: FileSystemDirectoryHandle | null = null;

    constructor(readonly opfsDir: string) {
        if (!opfsDir.trim()) {
            throw new Error("@yydb/iris-adapter-web: opfsDir is required");
        }
    }

    get isOpen(): boolean {
        return this.#base !== null;
    }

    /** Open (or create) the source OPFS directory tree. */
    async open(): Promise<this> {
        if (this.#base) return this;
        const root = await requireOpfsRoot();
        const segments = this.opfsDir.split("/").filter(Boolean);
        const base = await resolveDirectory(root, segments, true);
        this.#root = root;
        this.#base = base;
        return this;
    }

    async close(): Promise<void> {
        this.#root = null;
        this.#base = null;
    }

    #requireBase(): FileSystemDirectoryHandle {
        if (!this.#base) {
            throw new Error("@yydb/iris-adapter-web: call OpfsCas.open() first");
        }
        return this.#base;
    }

    /** SHA-256 hex of bytes (same digest as schema fingerprints). */
    async hash(bytes: Uint8Array): Promise<string> {
        return computeSchemaFingerprint(bytes);
    }

    /**
     * Write immutable object bytes. Idempotent for the same content hash.
     * Does not touch IndexedDB — caller records `object_pending` via journal.prepare.
     */
    async put(bytes: Uint8Array): Promise<CasPutResult> {
        const base = this.#requireBase();
        const hash = await this.hash(bytes);
        const opfsPath = casPathForHash(hash);
        const { dirs, file } = splitRelPath(opfsPath);
        const dir = await resolveDirectory(base, dirs, true);

        try {
            await dir.getFileHandle(file, { create: false });
            // Already present — content-addressed, treat as success.
            return { hash, opfsPath, byteLength: bytes.byteLength };
        } catch {
            /* create */
        }

        const handle = await dir.getFileHandle(file, { create: true });
        const writable = await handle.createWritable();
        try {
            await writable.write(bytes as BufferSource);
            await writable.close();
        } catch (err) {
            try {
                await writable.abort();
            } catch {
                /* ignore */
            }
            throw err;
        }
        return { hash, opfsPath, byteLength: bytes.byteLength };
    }

    async has(hash: string): Promise<boolean> {
        const data = await this.#openFile(hash, false);
        return data !== null;
    }

    async get(hash: string): Promise<Uint8Array | undefined> {
        const file = await this.#openFile(hash, false);
        if (!file) return undefined;
        const buf = await file.arrayBuffer();
        return new Uint8Array(buf);
    }

    async delete(hash: string): Promise<boolean> {
        const base = this.#requireBase();
        const opfsPath = casPathForHash(hash);
        const { dirs, file } = splitRelPath(opfsPath);
        try {
            const dir = await resolveDirectory(base, dirs, false);
            await dir.removeEntry(file);
            return true;
        } catch {
            return false;
        }
    }

    /**
     * Delete OPFS files for pending refs in `gc` state.
     * Caller removes IDB rows after successful delete / missing.
     */
    async gc(refs: readonly PendingObjectRef[]): Promise<CasGcReport> {
        const deleted: string[] = [];
        const missing: string[] = [];
        const failed: { hash: string; error: string }[] = [];
        const targets = refs.filter((r) => r.state === "gc");

        for (const ref of targets) {
            try {
                const existed = await this.has(ref.hash);
                if (!existed) {
                    missing.push(ref.hash);
                    continue;
                }
                const ok = await this.delete(ref.hash);
                if (ok) deleted.push(ref.hash);
                else missing.push(ref.hash);
            } catch (err) {
                failed.push({
                    hash: ref.hash,
                    error: err instanceof Error ? err.message : String(err),
                });
            }
        }

        return { scanned: targets.length, deleted, missing, failed };
    }

    async #openFile(hash: string, create: boolean): Promise<File | null> {
        const base = this.#requireBase();
        const opfsPath = casPathForHash(hash);
        const { dirs, file } = splitRelPath(opfsPath);
        try {
            const dir = await resolveDirectory(base, dirs, create);
            const handle = await dir.getFileHandle(file, { create });
            return await handle.getFile();
        } catch {
            return null;
        }
    }
}
