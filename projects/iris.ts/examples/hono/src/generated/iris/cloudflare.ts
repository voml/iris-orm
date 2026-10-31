/**
 * Cloudflare Worker host entry.
 *
 * D1 lowering (SQL, parameter binding and RETURNING projection) is owned by
 * the Rust adapter and reached through the WASM `d1-wasm-bridge` export of the
 * web language package. The generated edge plan artifact is not emitted yet,
 * so this entry fails closed instead of falling back to a TypeScript-side or
 * in-memory executor.
 */
export class Database {
    static async create(): Promise<never> {
        throw new Error(
            "IRIS-D1-EDGE-ARTIFACT-PENDING: the generated D1 edge artifact is not emitted yet. Bridge `env.DB` through the WASM `d1-wasm-bridge` instead of a generated `Database.create`.",
        );
    }
}
export { createClient, DbClient } from "./operations.js";
