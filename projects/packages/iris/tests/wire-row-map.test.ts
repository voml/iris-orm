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
