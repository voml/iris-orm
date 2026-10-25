import type {
    WebCreateIrisDbBindingOptions,
    WebIrisBindingProfile,
    WebIrisBindings,
    WebIrisHost,
    WebMemorySessionBinding,
} from "@yydb/iris-unknown-wasm32";

import { IrisLanguageError } from "../errors.ts";

/** Resolve the binding profile from generated-client options. */
export function resolveBindingProfile(options: WebCreateIrisDbBindingOptions = {}): WebIrisBindingProfile {
    if (options.profile) {
        return options.profile;
    }
    if (options.sqlitePath) {
        return "sqlite";
    }
    if (options.config ?? options.project) {
        return "project";
    }
    return "memory";
}

function pushSchemaIfPresent(session: WebMemorySessionBinding, schema?: string): void {
    if (schema && session.managedPush) {
        session.managedPush(schema);
    }
}

/**
 * Open a generated-client session on the Web binding.
 *
 * Browser / Worker hosts support the memory profile only and accept inline
 * `schema` for managed-push when the WASM binding exposes it.
 */
export async function openBindingSession(
    host: WebIrisHost,
    core: WebIrisBindings,
    options: WebCreateIrisDbBindingOptions = {},
): Promise<WebMemorySessionBinding> {
    const profile = resolveBindingProfile(options);
    if (profile !== "memory") {
        throw new IrisLanguageError(
            "wasm-profile-unsupported",
            `@yydb/iris-language: ${host} host only supports memory profile (got ${profile}); pass inline schema`,
        );
    }

    const session = core.openMemorySession();
    pushSchemaIfPresent(session, options.schema);
    return session;
}
