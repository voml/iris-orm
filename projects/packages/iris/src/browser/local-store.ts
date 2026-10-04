import type { IrisSession } from "../types/session.ts";

import { buildRuntime } from "../runtime/build-runtime.ts";

import { IrisFacadeError } from "../types/errors.ts";

import { getWasmSemanticCore } from "../wasm/index.ts";

import { openOpfsStore, type OpfsStore } from "../opfs/opfs-store.ts";



/**

 * Local Web Backend storage profile.

 *

 * `memory` uses the WASM ReferenceStore. `opfs` resolves an OPFS file handle for a

 * host-provided SQLite engine (`@yydb/iris/opfs`).

 */

export type LocalStoreBackend = "memory" | "opfs";



export type OpenLocalStoreOptions = {

    backend?: LocalStoreBackend;

    name: string;

};



/** Browser-local store handle (Local Web Backend; not YYDB). */

export interface LocalStore {

    readonly backend: LocalStoreBackend;

    readonly name: string;

    /** Present when `backend` is `opfs`. */

    readonly opfs?: OpfsStore;

    /** Open a WASM memory-backed Iris session (`memory` backend only). */

    openSession(): IrisSession;

    close(): Promise<void>;

}



/** Open a browser Local Web Backend store. */

export async function openLocalStore(options: OpenLocalStoreOptions): Promise<LocalStore> {

    const backend = options.backend ?? "memory";

    if (backend === "opfs") {

        const opfs = await openOpfsStore({ name: options.name });

        return {

            backend: "opfs",

            name: options.name,

            opfs,

            openSession: () => {

                throw new IrisFacadeError(

                    "opfs-session-unsupported",

                    '@yydb/iris: OPFS durable execution uses `Database.create` from generated `opfs.ts` with an injected `sqlite` handle',

                );

            },

            close: () => opfs.close(),

        };

    }

    if (backend !== "memory") {

        throw new IrisFacadeError(

            "local-store-unsupported",

            `@yydb/iris: Local Web Backend "${backend}" is not wired yet; use backend "memory" or "opfs"`,

        );

    }



    return {

        backend: "memory",

        name: options.name,

        openSession: () => buildRuntime("browser", getWasmSemanticCore(), "memory").openSession(),

        close: async () => {},

    };

}


