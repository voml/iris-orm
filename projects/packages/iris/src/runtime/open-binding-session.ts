import type { IrisBindings, MemorySessionBinding } from "../bindings.ts";
import type { IrisHost } from "../types/binding.ts";
import type { CreateIrisDbBindingOptions } from "../types/executor.ts";

/** Node-only project wiring for generated `createDb` (`profile: "project"`). */
export type BindingSessionHooks = {
    openProjectSession?: (
        configPath: string,
        source?: string,
    ) => Promise<{ session: MemorySessionBinding; schema?: string }>;
};

/** Resolve the binding profile from generated-client options. */
export function resolveBindingProfile(options: CreateIrisDbBindingOptions = {}): "memory" | "sqlite" | "project" {
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

function pushSchemaIfPresent(session: MemorySessionBinding, schema?: string): void {
    if (schema && session.managedPush) {
        session.managedPush(schema);
    }
}

/**
 * Open a generated-client session symmetrically across Node N-API and browser WASM hosts.
 *
 * Node supports sqlite/project/memory profiles. Browser currently supports memory only and
 * accepts inline `schema` for managed-push when the binding exposes it.
 */
export async function openBindingSession(
    host: IrisHost,
    core: IrisBindings,
    options: CreateIrisDbBindingOptions = {},
    hooks: BindingSessionHooks = {},
): Promise<MemorySessionBinding> {
    const configPath = options.config ?? options.project;
    const profile = resolveBindingProfile(options);
    let schemaData = options.schema;

    if (host === "node") {
        let session: MemorySessionBinding;
        if (profile === "sqlite" && core.openSqliteSession) {
            session = core.openSqliteSession(options.sqlitePath ?? ":memory:");
        } else if (profile === "project" && configPath && hooks.openProjectSession) {
            const opened = await hooks.openProjectSession(configPath, options.source);
            session = opened.session;
            if (!schemaData) {
                schemaData = opened.schema;
            }
        } else if (profile === "project" && core.openProjectSession && configPath) {
            session = core.openProjectSession(configPath, options.source ?? "default");
        } else {
            session = core.openMemorySession();
        }
        pushSchemaIfPresent(session, schemaData);
        return session;
    }

    if (profile !== "memory") {
        throw new Error(
            `@yydb/iris: browser host only supports memory profile (got ${profile}); pass inline schema or use @yydb/iris/node`,
        );
    }

    const session = core.openMemorySession();
    pushSchemaIfPresent(session, schemaData);
    return session;
}
