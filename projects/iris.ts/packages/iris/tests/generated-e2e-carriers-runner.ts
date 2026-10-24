import { Database } from "./cloudflare.js";

type Trace = { sql: string; bind: unknown[] };

const PAYLOAD = new Uint8Array([0, 1, 2, 250]);
const I64_MAX = 9223372036854775807n;
const I64_MIN = -9223372036854775808n;

/**
 * Minimal D1 double that keeps the row it saw and hands the bound values back
 * as wire columns, so a decode assertion proves what the binding side sent.
 */
function createFakeD1() {
    const traces: Trace[] = [];
    let row: Record<string, unknown> = { metric_id: "m1", seq: 0, payload: PAYLOAD, note: "hello" };
    const d1 = {
        prepare(query: string) {
            return {
                bind(...values: unknown[]) {
                    traces.push({ sql: query, bind: values });
                    const insert = query.match(/INSERT INTO \w+ \(([^)]+)\)/i);
                    if (insert) {
                        const next: Record<string, unknown> = { ...row };
                        const columns = insert[1]!.split(",").map((name) => name.trim());
                        columns.forEach((column, index) => {
                            next[column] = values[index];
                        });
                        row = next;
                    } else {
                        const filter = query.match(/WHERE (\w+) = \?/i);
                        if (filter) {
                            row = { ...row, [filter[1]!]: values[0] };
                        }
                    }
                    return {
                        async all<T>() {
                            const returning = query.match(/RETURNING\s+(.+)$/i);
                            if (!returning) {
                                return { results: [row] as T[] };
                            }
                            const projected: Record<string, unknown> = {};
                            for (const column of returning[1]!.split(",").map((name) => name.trim())) {
                                projected[column] = row[column];
                            }
                            return { results: [projected] as T[] };
                        },
                        async run() {
                            return { success: true };
                        },
                    };
                },
            };
        },
    };
    return { d1, traces };
}

function describeBind(value: unknown): Record<string, unknown> {
    if (value instanceof ArrayBuffer) {
        return { kind: "ArrayBuffer", bytes: Array.from(new Uint8Array(value)) };
    }
    return { kind: typeof value, value: typeof value === "bigint" ? value.toString() : value };
}

const { d1, traces } = createFakeD1();
const db = await Database.create({ d1 });

const bySeq = await db.metric.findMany({ where: { seq: { eq: I64_MAX } } });
const byPayload = await db.metric.findMany({ where: { payload: { eq: PAYLOAD } } });
const created = await db.metric.create({
    data: { metricId: "m2", seq: I64_MIN, payload: PAYLOAD, note: "negative" },
});

let rangeError: { code?: string; message?: string } | null = null;
try {
    await db.metric.findMany({ where: { seq: { eq: I64_MAX + 1n } } });
} catch (error) {
    rangeError = error as { code?: string; message?: string };
}

await db.$close();

const seqRow = bySeq[0] as { seq?: unknown; payload?: unknown } | undefined;
const payloadRow = byPayload[0] as { seq?: unknown; payload?: unknown } | undefined;

console.log(
    JSON.stringify({
        ok: true,
        seqCount: bySeq.length,
        seqCarrier: typeof seqRow?.seq,
        seqExact: seqRow?.seq === undefined ? null : String(seqRow.seq),
        seqPayloadCarrier: seqRow?.payload instanceof Uint8Array ? "Uint8Array" : typeof seqRow?.payload,
        seqPayloadBytes: seqRow?.payload instanceof Uint8Array ? Array.from(seqRow.payload) : null,
        payloadCount: byPayload.length,
        payloadCarrier: payloadRow?.payload instanceof Uint8Array ? "Uint8Array" : typeof payloadRow?.payload,
        payloadBytes: payloadRow?.payload instanceof Uint8Array ? Array.from(payloadRow.payload) : null,
        createdSeqCarrier: typeof created.seq,
        createdSeqExact: String(created.seq),
        createdPayloadCarrier: created.payload instanceof Uint8Array ? "Uint8Array" : typeof created.payload,
        createdPayloadBytes: created.payload instanceof Uint8Array ? Array.from(created.payload) : null,
        seqFilterBind: describeBind(traces[0]?.bind[0]),
        payloadFilterBind: describeBind(traces[1]?.bind[0]),
        createBind: (traces[2]?.bind ?? []).map(describeBind),
        rangeError,
    }),
);
