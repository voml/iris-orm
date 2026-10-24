import type { IrisSession } from "../types/session.ts";
import { buildRuntime } from "../runtime/build-runtime.ts";
import { IrisFacadeError } from "../types/errors.ts";
import { getWasmSemanticCore } from "../wasm/index.ts";

/**
 * Local Web Backend storage profile.
 *
 * IndexedDB / OPFS 持久化由 YYDB 承接，不在 Iris 内接线；公开面只允许 `memory`。
 */
export type LocalStoreBackend = "memory";

export type OpenLocalStoreOptions = {
    backend?: LocalStoreBackend;
    name: string;
};

/** Browser-local store handle (Local Web Backend; not YYDB). */
export interface LocalStore {
    readonly backend: LocalStoreBackend;
    readonly name: string;
    /** Open a WASM memory-backed Iris session (no browser persistence yet). */
    openSession(): IrisSession;
    close(): Promise<void>;
}

/** Open a browser Local Web Backend store. Only `memory` is supported until YYDB wires browser persistence. */
export async function openLocalStore(options: OpenLocalStoreOptions): Promise<LocalStore> {
    const backend = options.backend ?? "memory";
    if (backend !== "memory") {
        throw new IrisFacadeError(
            "local-store-unsupported",
            `@yydb/iris: Local Web Backend "${backend}" is not wired yet; use backend "memory"`,
        );
    }

    return {
        backend: "memory",
        name: options.name,
        openSession: () => buildRuntime("browser", getWasmSemanticCore(), "memory").openSession(),
        close: async () => {},
    };
}
