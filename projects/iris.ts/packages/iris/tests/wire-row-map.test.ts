import assert from "node:assert/strict";
import { test } from "node:test";

import {
    entityFromOperationId,
    mapRowsToAuthorSurface,
    wireRowToAuthorRow,
} from "../src/runtime/wire-row-map.ts";

const wireToTs = {
    userId: "user_id",
    userName: "user_name",
    active: "active",
};

test("entityFromOperationId parses generated operation ids", () => {
    assert.equal(entityFromOperationId("User.findMany"), "User");
    assert.equal(entityFromOperationId("invalid"), null);
});

test("wireRowToAuthorRow maps wire columns to TypeScript field names", () => {
    const mapped = wireRowToAuthorRow(wireToTs, {
        user_id: "u1",
        user_name: "Ada",
        active: 1,
    });
    assert.deepEqual(mapped, { userId: "u1", userName: "Ada", active: 1 });
});

test("mapRowsToAuthorSurface is a no-op without wire metadata", () => {
    const rows = [{ user_id: "u1" }];
    assert.deepEqual(mapRowsToAuthorSurface(rows, {}), rows);
});

const carrierWireToTs = { seq: "seq", payload: "payload", note: "note" };
const valueCarriers = { seq: "i64", payload: "bytes", note: "utf16" };

test("wireRowToAuthorRow keeps i64 on the bigint carrier", () => {
    const mapped = wireRowToAuthorRow(carrierWireToTs, { seq: 42, payload: null, note: "Ada" }, valueCarriers);
    assert.equal(mapped.seq, 42n);
    assert.equal(typeof mapped.seq, "bigint");
    assert.equal(mapped.note, "Ada");
});

test("wireRowToAuthorRow decodes tagged bytes onto Uint8Array", () => {
    const mapped = wireRowToAuthorRow(
        carrierWireToTs,
        { seq: 1, payload: { $yydb: "bytes", hex: "6869" }, note: null },
        valueCarriers,
    );
    assert.deepEqual(mapped.payload, new Uint8Array([0x68, 0x69]));
});

test("wireRowToAuthorRow refuses an i64 that left the exact number range", () => {
    assert.throws(
        () => wireRowToAuthorRow(carrierWireToTs, { seq: 2 ** 53, payload: null, note: null }, valueCarriers),
        /beyond the 2\^53 exact range/,
    );
});

test("wireRowToAuthorRow keeps fields without a carrier scalar", () => {
    const mapped = wireRowToAuthorRow(carrierWireToTs, { seq: "7", payload: null, note: "Ada" });
    assert.deepEqual(mapped, { seq: "7", payload: null, note: "Ada" });
});

test("wireRowToAuthorRow throws on unknown VOS label instead of falling back", () => {
    assert.throws(
        () =>
            wireRowToAuthorRow(
                carrierWireToTs,
                { seq: "x", payload: null, note: null },
                { seq: "customTypeX", payload: "bytes", note: "utf16" },
            ),
        /unknown VOS label `customTypeX`/,
    );
});
