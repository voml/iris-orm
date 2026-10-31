import { createIrisOperationExecutor, type CreateIrisDbBindingOptions } from "@yydb/iris-language";
import { createClient, type DbClient } from "./operations.js";
import { IRIS_FIELD_WIRE_NAMES, IRIS_SCHEMA_FINGERPRINT, IRIS_VALUE_CARRIERS } from "./metadata.js";
/** Browser / Worker host: WASM operation executor (call `initIris()` or `loadIrisWeb()` before `Database.create`). */
export class Database {
    static async create(options?: CreateIrisDbBindingOptions): Promise<DbClient> {
        return createClient(
            await createIrisOperationExecutor({
                ...options,
                wireNamesByEntity: IRIS_FIELD_WIRE_NAMES,
                valueCarriersByEntity: IRIS_VALUE_CARRIERS,
                contractFingerprint: IRIS_SCHEMA_FINGERPRINT,
            }),
        );
    }
}
export { createClient, DbClient } from "./operations.js";
