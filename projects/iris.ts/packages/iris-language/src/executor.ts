import type {
    WebCreateIrisDbBindingOptions,
    WebIrisBindingHost,
    WebIrisDbBinding,
    WebIrisSession,
    WebOperationExecutor,
} from "@yydb/iris-unknown-wasm32";

import { IrisLanguageError } from "./errors.ts";
import {
    buildRuntime,
    createIrisDbBindingFromSession,
    createOperationExecutorFromSession,
    guardOperationExecutor,
    openBindingSession,
} from "./runtime/index.ts";
import { getWasmSemanticCore } from "./wasm.ts";

export type CreateIrisBrowserOptions = {
    /** Inline VOS / project source for browser-only hosts. */
    source?: unknown;
};

/**
 * @deprecated Use generated `IrisClient` + browser executor wiring. WASM init only.
 */
export async function createIris(_options: CreateIrisBrowserOptions = {}): Promise<WebIrisBindingHost> {
    return buildRuntime("browser", getWasmSemanticCore(), "memory");
}

/**
 * Create internal binding support for generated browser `db` (not an application entry).
 *
 * Requires prior `initIris()` / `loadIrisWeb()`. Browser host currently uses WASM in-memory ReferenceStore only.
 */
export async function createIrisDbBinding(options: WebCreateIrisDbBindingOptions = {}): Promise<WebIrisDbBinding> {
    const session = await openBindingSession("browser", getWasmSemanticCore(), options);
    return createIrisDbBindingFromSession(session);
}

/** Create the async operation executor for generated browser clients (preferred ABI). */
export async function createIrisOperationExecutor(options: WebCreateIrisDbBindingOptions = {}): Promise<WebOperationExecutor> {
    const session = await openBindingSession("browser", getWasmSemanticCore(), options);
    const executor = createOperationExecutorFromSession(session, {
        wireNamesByEntity: options.wireNamesByEntity,
        valueCarriersByEntity: options.valueCarriersByEntity,
    });
    if (options.contractFingerprint) {
        return guardOperationExecutor(executor, options.contractFingerprint);
    }
    return executor;
}

/** @deprecated Use `createIrisDbBinding`. */
export const createBrowserIrisDbBinding = createIrisDbBinding;

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
    openSession(): WebIrisSession;
    close(): Promise<void>;
}

/** Open a browser Local Web Backend store. Only `memory` is supported until YYDB wires browser persistence. */
export async function openLocalStore(options: OpenLocalStoreOptions): Promise<LocalStore> {
    const backend = options.backend ?? "memory";
    if (backend !== "memory") {
        throw new IrisLanguageError(
            "local-store-unsupported",
            `@yydb/iris-language: Local Web Backend "${backend}" is not wired yet; use backend "memory"`,
        );
    }

    return {
        backend: "memory",
        name: options.name,
        openSession: () => buildRuntime("browser", getWasmSemanticCore(), "memory").openSession(),
        close: async () => {},
    };
}
