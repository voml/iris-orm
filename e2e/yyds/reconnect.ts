// Iris PostgreSQL reconnect probe.
//
// One long-lived generated client is opened while the yyds node runs. The probe
// then stops the node through its real CLI, expects the next call to surface
// `IRIS-CONNECTION-FAILED`, starts the node again on the same ports and data
// directory, and keeps calling the same client until it answers. A client that
// only works after being rebuilt would hide a missing reconnect.

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { Socket } from "node:net";

import { createClient } from "./.generated/ts/node.ts";

const dsn = process.env.IRIS_E2E_DSN;
const cli = process.env.YYDS_CLI;
const dataDir = process.env.YYDS_DATA_DIR;
const clusterId = process.env.YYDS_CLUSTER_ID;
const nodeId = process.env.YYDS_NODE_ID;
const ports = JSON.parse(process.env.YYDS_PORTS ?? "null");
if (!dsn || !cli || !dataDir || !clusterId || !nodeId || !ports) {
    throw new Error("IRIS_E2E_DSN, YYDS_CLI, YYDS_DATA_DIR, YYDS_CLUSTER_ID, YYDS_NODE_ID and YYDS_PORTS are required");
}

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

function portOpen(port) {
    return new Promise((resolveProbe) => {
        const socket = new Socket();
        socket.once("connect", () => {
            socket.destroy();
            resolveProbe(true);
        });
        socket.once("error", () => resolveProbe(false));
        socket.setTimeout(500, () => {
            socket.destroy();
            resolveProbe(false);
        });
        socket.connect(port, "127.0.0.1");
    });
}

function authorCode(error) {
    return typeof error?.code === "string" ? error.code : "IRIS-INTERNAL";
}

const db = createClient({ dsn });
const codes = {};

await db.tables.article.findMany();
codes.before_stop = "usable";

const stop = spawnSync(process.execPath, ["--experimental-strip-types", cli, "stop", "--data-dir", dataDir], {
    encoding: "utf8",
    timeout: 30_000,
});
assert.equal(stop.status, 0, `yyds stop failed: ${stop.stderr ?? ""}`);

const closedBy = Date.now() + 30_000;
while (Date.now() < closedBy && (await portOpen(ports.pg))) {
    await sleep(200);
}
assert.equal(await portOpen(ports.pg), false, "the PostgreSQL port stayed open after `yyds stop`");

try {
    await db.tables.article.findMany();
    codes.after_stop = null;
} catch (error) {
    assert.equal(typeof error?.diagnosticCode, "string", "the connection failure must carry a diagnostic");
    codes.after_stop = authorCode(error);
}
assert.equal(codes.after_stop, "IRIS-CONNECTION-FAILED", "a call against a stopped node must report the connection code");

const started = spawn(
    process.execPath,
    [
        "--experimental-strip-types",
        cli,
        "start",
        "--data-dir",
        dataDir,
        "--cluster-id",
        clusterId,
        "--node-id",
        nodeId,
        "--redis-port",
        String(ports.redis),
        "--pgsql-port",
        String(ports.pg),
        "--mysql-port",
        String(ports.mysql),
    ],
    { detached: true, stdio: "ignore" },
);
started.unref();

const deadline = Date.now() + 90_000;
let last = "";
let rows = null;
while (Date.now() < deadline) {
    try {
        rows = await db.tables.article.findMany();
        break;
    } catch (error) {
        last = `${authorCode(error)}: ${error.message}`;
    }
    await sleep(1000);
}
assert.notEqual(rows, null, `the same client did not recover after the node restarted (${last})`);
codes.after_restart = "usable";

console.log(`RESULT ${JSON.stringify({ codes })}`);
