import { IrisFacadeError } from "../types/errors.ts";

export type OpenOpfsStoreOptions = {
    /** Logical database name; persisted as `<name>.sqlite` in the origin OPFS root. */
    name: string;
};

/** Browser OPFS file handle for a durable SQLite database. */
export interface OpfsStore {
    readonly name: string;
    /** Resolved OPFS file handle for the SQLite database file. */
    fileHandle(): Promise<FileSystemFileHandle>;
    close(): Promise<void>;
}

function assertOpfsAvailable(): void {
    if (typeof navigator === "undefined" || typeof navigator.storage?.getDirectory !== "function") {
        throw new IrisFacadeError(
            "opfs-unavailable",
            "@yydb/iris/opfs: Origin Private File System is not available in this host",
        );
    }
}

/** Open or create a durable SQLite file in the origin OPFS root. */
export async function openOpfsStore(options: OpenOpfsStoreOptions): Promise<OpfsStore> {
    assertOpfsAvailable();
    const root = await navigator.storage.getDirectory();
    const fileName = `${options.name}.sqlite`;
    const handle = await root.getFileHandle(fileName, { create: true });
    return {
        name: options.name,
        fileHandle: async () => handle,
        close: async () => {},
    };
}
