/**
 * W1 — IndexedDB catalog: schema fingerprint install / read / list.
 */

import { computeEnvelopeFingerprint, type SchemaCatalogRecord, type SchemaFingerprintInput } from "./fingerprint.ts";
import { getMeta, idbRequest, idbTransactionDone, putMeta, STORE_SCHEMA } from "./idb.ts";

export type InstallSchemaOptions = SchemaFingerprintInput & {
    source?: SchemaCatalogRecord["source"];
    note?: string;
    /**
     * If true, replace an existing schemaId even when fingerprint differs.
     * Default false — conflict throws.
     */
    force?: boolean;
};

export type CatalogSnapshot = {
    backendId: string;
    catalogVersion: number;
    schemas: SchemaCatalogRecord[];
};

export class WebCatalog {
    constructor(
        readonly db: IDBDatabase,
        readonly backendId: string,
    ) {}

    async ensureBootstrapped(): Promise<void> {
        const existing = await getMeta<string>(this.db, "backendId");
        if (!existing) {
            await putMeta(this.db, "backendId", this.backendId);
            await putMeta(this.db, "catalogVersion", 1);
            await putMeta(this.db, "createdAt", new Date().toISOString());
        }
    }

    async getSchema(schemaId: string): Promise<SchemaCatalogRecord | undefined> {
        const tx = this.db.transaction(STORE_SCHEMA, "readonly");
        const row = await idbRequest<SchemaCatalogRecord | undefined>(tx.objectStore(STORE_SCHEMA).get(schemaId));
        await idbTransactionDone(tx);
        return row;
    }

    async listSchemas(): Promise<SchemaCatalogRecord[]> {
        const tx = this.db.transaction(STORE_SCHEMA, "readonly");
        const rows = await idbRequest<SchemaCatalogRecord[]>(tx.objectStore(STORE_SCHEMA).getAll());
        await idbTransactionDone(tx);
        return rows;
    }

    /**
     * Install or verify a schema fingerprint.
     * Same schemaId + same fingerprint → idempotent success.
     * Same schemaId + different fingerprint → error unless `force`.
     */
    async installSchema(options: InstallSchemaOptions): Promise<SchemaCatalogRecord> {
        const fingerprint = await computeEnvelopeFingerprint(options);
        const existing = await this.getSchema(options.schemaId);

        if (existing && existing.fingerprint === fingerprint) {
            return existing;
        }
        if (existing && existing.fingerprint !== fingerprint && !options.force) {
            throw new Error(
                `@yydb/iris-adapter-web: schema fingerprint conflict for "${options.schemaId}" ` +
                    `(have ${existing.fingerprint.slice(0, 12)}…, got ${fingerprint.slice(0, 12)}…). ` +
                    `Pass force: true to replace after explicit migrate.`,
            );
        }

        const record: SchemaCatalogRecord = {
            schemaId: options.schemaId,
            fingerprint,
            contractVersion: options.contractVersion,
            mappingVersion: options.mappingVersion,
            installedAt: new Date().toISOString(),
            source: options.source ?? "install",
            note: options.note,
        };

        const tx = this.db.transaction(STORE_SCHEMA, "readwrite");
        tx.objectStore(STORE_SCHEMA).put(record);
        await idbTransactionDone(tx);
        await putMeta(this.db, "lastSchemaInstallAt", record.installedAt);
        return record;
    }

    async snapshot(): Promise<CatalogSnapshot> {
        const catalogVersion = (await getMeta<number>(this.db, "catalogVersion")) ?? 1;
        const schemas = await this.listSchemas();
        return {
            backendId: this.backendId,
            catalogVersion,
            schemas,
        };
    }

    /** Active fingerprint for a schemaId, or undefined if not installed. */
    async getFingerprint(schemaId: string): Promise<string | undefined> {
        return (await this.getSchema(schemaId))?.fingerprint;
    }
}

export type { SchemaCatalogRecord, SchemaFingerprintInput };
