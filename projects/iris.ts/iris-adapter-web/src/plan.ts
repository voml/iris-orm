/**
 * Web-local VOS physical plan (adapter-side planner / executor).
 *
 * This is NOT a VOS language parser and NOT a substitute for `@yydb/iris` IR.
 * Iris (when present) lowers VOS → semantic IR → physical plan; this module
 * accepts already-structured web intents and plans/executes against IDB+OPFS.
 *
 * Unsupported capabilities are Rejected with notes — never silently SQL-fallback.
 */

import type { CasPutResult } from "./opfs-cas.ts";
import type { RowMutation, RowRecord } from "./rows.ts";

export type ConsistencyIntent = "Authoritative" | "ReadYourWrites" | "BoundedStale" | "Eventual" | "ProjectionRequired";

export type RealizationClass = "Native" | "Equivalent" | "Compensated" | "Rejected";

export type WebReadIntent =
    | { op: "get"; entity: string; id: string; fields?: readonly string[] }
    | {
          op: "list";
          entity: string;
          /** Equality filter on a top-level field. */
          whereEq?: { field: string; value: unknown };
          limit?: number;
          fields?: readonly string[];
      }
    | { op: "getMany"; entity: string; ids: readonly string[]; fields?: readonly string[] }
    | { op: "getObject"; hash: string };

export type WebWriteIntent =
    | {
          op: "upsert";
          entity: string;
          id: string;
          fields: Record<string, unknown>;
          objectRefs?: Record<string, string>;
      }
    | { op: "delete"; entity: string; id: string }
    | { op: "putObject"; bytes: Uint8Array };

export type WebIntentBatch = {
    schemaFingerprint: string;
    semanticHash: string;
    consistency?: ConsistencyIntent;
    reads?: readonly WebReadIntent[];
    writes?: readonly WebWriteIntent[];
    /** When true (default), writes enqueue local outbox records (W6). */
    appendOutbox?: boolean;
};

export type PlannedNode = {
    op: string;
    realization: RealizationClass;
    note?: string;
    intent: WebReadIntent | WebWriteIntent;
};

export type WebPhysicalPlan = {
    schemaFingerprint: string;
    semanticHash: string;
    consistency: ConsistencyIntent;
    appendOutbox: boolean;
    nodes: PlannedNode[];
    /** Mutations deferred to journal commit. */
    mutations: RowMutation[];
    /** Object bytes to CAS-put before prepare. */
    objectBytes: Uint8Array[];
    reads: WebReadIntent[];
    rejected: boolean;
    rejectionNotes: string[];
};

export type QueryRow = {
    entity: string;
    id: string;
    fields: Record<string, unknown>;
    objectRefs?: Record<string, string>;
};

export type ExecuteReadResult = {
    rows: QueryRow[];
    objects: { hash: string; bytes: Uint8Array }[];
};

function projectFields(row: RowRecord, fields?: readonly string[]): QueryRow {
    if (!fields || fields.length === 0) {
        return {
            entity: row.entity,
            id: row.id,
            fields: { ...row.fields },
            objectRefs: row.objectRefs ? { ...row.objectRefs } : undefined,
        };
    }
    const projected: Record<string, unknown> = {};
    for (const f of fields) {
        if (Object.prototype.hasOwnProperty.call(row.fields, f)) {
            projected[f] = row.fields[f];
        }
    }
    return {
        entity: row.entity,
        id: row.id,
        fields: projected,
        objectRefs: row.objectRefs ? { ...row.objectRefs } : undefined,
    };
}

/** Plan a batch of structured intents into a WebPhysicalPlan. */
export function planWebIntents(batch: WebIntentBatch): WebPhysicalPlan {
    const consistency = batch.consistency ?? "Authoritative";
    const appendOutbox = batch.appendOutbox !== false;
    const nodes: PlannedNode[] = [];
    const mutations: RowMutation[] = [];
    const objectBytes: Uint8Array[] = [];
    const reads: WebReadIntent[] = [];
    const rejectionNotes: string[] = [];

    for (const intent of batch.reads ?? []) {
        if (intent.op === "list" && intent.whereEq && intent.whereEq.field.includes(".")) {
            nodes.push({
                op: intent.op,
                realization: "Rejected",
                note: "nested field filters are not supported in web planner v1",
                intent,
            });
            rejectionNotes.push("nested field filters are not supported in web planner v1");
            continue;
        }
        nodes.push({ op: intent.op, realization: "Native", intent });
        reads.push(intent);
    }

    for (const intent of batch.writes ?? []) {
        if (intent.op === "putObject") {
            if (intent.bytes.byteLength === 0) {
                nodes.push({
                    op: intent.op,
                    realization: "Rejected",
                    note: "empty object bytes rejected",
                    intent,
                });
                rejectionNotes.push("empty object bytes rejected");
                continue;
            }
            nodes.push({ op: intent.op, realization: "Native", intent });
            objectBytes.push(intent.bytes);
            continue;
        }
        if (intent.op === "upsert") {
            nodes.push({ op: intent.op, realization: "Native", intent });
            mutations.push({
                kind: "upsert",
                entity: intent.entity,
                id: intent.id,
                fields: intent.fields,
                objectRefs: intent.objectRefs,
            });
            continue;
        }
        if (intent.op === "delete") {
            nodes.push({ op: intent.op, realization: "Native", intent });
            mutations.push({ kind: "delete", entity: intent.entity, id: intent.id });
        }
    }

    // Multi-write fusion note
    if (mutations.length > 1) {
        nodes.push({
            op: "fuseWrites",
            realization: "Equivalent",
            note: "multiple writes fused into one journal commit",
            intent:
                mutations[0]!.kind === "upsert"
                    ? {
                          op: "upsert",
                          entity: mutations[0]!.entity,
                          id: mutations[0]!.id,
                          fields: {},
                      }
                    : { op: "delete", entity: mutations[0]!.entity, id: mutations[0]!.id },
        });
    }

    return {
        schemaFingerprint: batch.schemaFingerprint,
        semanticHash: batch.semanticHash,
        consistency,
        appendOutbox,
        nodes,
        mutations,
        objectBytes,
        reads,
        rejected: rejectionNotes.length > 0,
        rejectionNotes,
    };
}

export type RowStore = {
    get(entity: string, id: string): Promise<RowRecord | undefined>;
    list(entity: string): Promise<RowRecord[]>;
};

export type ObjectStore = {
    get(hash: string): Promise<Uint8Array | undefined>;
    put(bytes: Uint8Array): Promise<CasPutResult>;
};

/** Execute planned read intents against row/object stores. */
export async function executeWebReads(plan: WebPhysicalPlan, rows: RowStore, objects: ObjectStore | null): Promise<ExecuteReadResult> {
    if (plan.rejected) {
        throw new Error(`@yydb/iris-adapter-web: plan rejected: ${plan.rejectionNotes.join("; ")}`);
    }
    const outRows: QueryRow[] = [];
    const outObjects: { hash: string; bytes: Uint8Array }[] = [];

    for (const intent of plan.reads) {
        if (intent.op === "get") {
            const row = await rows.get(intent.entity, intent.id);
            if (row) outRows.push(projectFields(row, intent.fields));
            continue;
        }
        if (intent.op === "getMany") {
            for (const id of intent.ids) {
                const row = await rows.get(intent.entity, id);
                if (row) outRows.push(projectFields(row, intent.fields));
            }
            continue;
        }
        if (intent.op === "list") {
            let list = await rows.list(intent.entity);
            if (intent.whereEq) {
                const { field, value } = intent.whereEq;
                list = list.filter((r) => Object.is(r.fields[field], value));
            }
            if (intent.limit != null && intent.limit >= 0) {
                list = list.slice(0, intent.limit);
            }
            for (const row of list) outRows.push(projectFields(row, intent.fields));
            continue;
        }
        if (intent.op === "getObject") {
            if (!objects) {
                throw new Error("@yydb/iris-adapter-web: OPFS CAS required for getObject");
            }
            const bytes = await objects.get(intent.hash);
            if (bytes) outObjects.push({ hash: intent.hash, bytes });
        }
    }

    return { rows: outRows, objects: outObjects };
}
