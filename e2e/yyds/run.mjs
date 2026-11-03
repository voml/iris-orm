// Iris end-to-end against a live yyds node.
//
// Starts yyds through its real CLI (`init` then `start`), waits for the
// `yyds running pid=… mode=sql-runtime` line, creates the table through the
// backend driver (always outside an Iris transaction), generates both clients
// from the same schema, then runs the TypeScript and Python suites and
// compares their author `code` strings.
//
// `node e2e/yyds/run.mjs --backend mysql` or `--backend postgres`.
// `YYDS_ROOT` overrides the sibling checkout, `IRIS_PYTHON` the interpreter.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { createServer, Socket } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import mysql from "mysql2/promise";
import { Client as PgClient } from "pg";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..");
const YYDS_ROOT = process.env.YYDS_ROOT ?? resolve(REPO, "..", "yyds");
const CLI = join(YYDS_ROOT, "projects", "yyds.ts", "packages", "yyds", "bin", "yyds.mjs");
const START_TIMEOUT_MS = 90_000;
const RUNNING_LINE = /^yyds running pid=\d+ mode=(\S+)/;
const BACKEND = process.argv.includes("--backend")
    ? process.argv[process.argv.indexOf("--backend") + 1]
    : "mysql";

if (BACKEND !== "mysql" && BACKEND !== "postgres") {
    throw new Error(`unknown --backend ${BACKEND}`);
}
if (!existsSync(CLI)) {
    throw new Error(`yyds CLI not found at ${CLI} (set YYDS_ROOT)`);
}
const DIST = join(REPO, "projects", "iris.ts", "packages", "iris", "dist");
if (!existsSync(join(DIST, "node", "index.js"))) {
    throw new Error("`@yydb/iris` is not built. Run `pnpm build:ts` first.");
}

function freePort() {
    return new Promise((resolvePort, reject) => {
        const server = createServer();
        server.on("error", reject);
        server.listen(0, "127.0.0.1", () => {
            const port = server.address().port;
            server.close(() => resolvePort(port));
        });
    });
}

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

function runCli(args, timeout) {
    const result = spawnSync(process.execPath, ["--experimental-strip-types", CLI, ...args], {
        encoding: "utf8",
        timeout,
    });
    return { code: result.status ?? -1, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

const [redisPort, pgPort, mysqlPort] = await Promise.all([freePort(), freePort(), freePort()]);
const dataDir = mkdtempSync(join(tmpdir(), `iris-yyds-${BACKEND}-`));
const clusterId = `iris-cluster-${process.pid}`;
const nodeId = `iris-node-${process.pid}`;
const host = "127.0.0.1";
const generatedRoot = join(HERE, ".generated");
const stdoutLines = [];
const stderrLines = [];
let nodeProcess = null;

function nodeLogs() {
    return `--- node stdout ---\n${stdoutLines.join("\n")}\n--- node stderr ---\n${stderrLines.join("\n")}`;
}

async function stopNode() {
    if (nodeProcess === null) return;
    const stopped = runCli(["stop", "--data-dir", dataDir], 20_000);
    console.log(`[stop] exit=${stopped.code}`);
    const exited = new Promise((resolveExit) => nodeProcess.once("exit", resolveExit));
    const timeout = new Promise((resolveTimeout) => setTimeout(resolveTimeout, 10_000, "timeout"));
    if ((await Promise.race([exited, timeout])) === "timeout") {
        nodeProcess.kill();
        await exited;
    }
    nodeProcess = null;
}

async function waitUntilReady() {
    const deadline = Date.now() + START_TIMEOUT_MS;
    let announced = false;
    while (Date.now() < deadline) {
        if (nodeProcess.exitCode !== null) {
            throw new Error(`yyds start exited with code ${nodeProcess.exitCode} before readiness\n${nodeLogs()}`);
        }
        for (const line of stdoutLines) {
            const match = RUNNING_LINE.exec(line);
            if (match === null) continue;
            if (match[1] !== "sql-runtime") {
                throw new Error(`node started in '${match[1]}' mode\n${nodeLogs()}`);
            }
            announced = true;
        }
        if (announced && (await portOpen(redisPort)) && (await portOpen(pgPort)) && (await portOpen(mysqlPort))) {
            console.log(`[yyds] running pgsql=${pgPort} mysql=${mysqlPort} redis=${redisPort}`);
            return;
        }
        await sleep(50);
    }
    throw new Error(`node did not accept SQL connections within ${START_TIMEOUT_MS / 1000}s\n${nodeLogs()}`);
}

async function startNode() {
    const args = [
        "--experimental-strip-types",
        CLI,
        "start",
        "--data-dir",
        dataDir,
        "--cluster-id",
        clusterId,
        "--node-id",
        nodeId,
        "--redis-port",
        String(redisPort),
        "--pgsql-port",
        String(pgPort),
        "--mysql-port",
        String(mysqlPort),
    ];
    console.log(`[yyds] ${[process.execPath, ...args].join(" ")}`);
    nodeProcess = spawn(process.execPath, args, { stdio: ["ignore", "pipe", "pipe"] });
    nodeProcess.stdout.setEncoding("utf8");
    nodeProcess.stderr.setEncoding("utf8");
    nodeProcess.stdout.on("data", (chunk) => {
        for (const line of chunk.split(/\r?\n/)) {
            if (line.length > 0) stdoutLines.push(line);
        }
    });
    nodeProcess.stderr.on("data", (chunk) => {
        for (const line of chunk.split(/\r?\n/)) {
            if (line.length > 0) stderrLines.push(line);
        }
    });
    await waitUntilReady();
}

const CREATE_TABLE = {
    mysql: [
        `CREATE TABLE Article (
        article_id VARCHAR(191) NOT NULL PRIMARY KEY,
        slug VARCHAR(191) NOT NULL,
        body TEXT,
        UNIQUE KEY uq_article_slug (slug)
    )`,
        `CREATE TABLE Doc (
        doc_id VARCHAR(191) NOT NULL PRIMARY KEY,
        label TEXT,
        embedding TEXT
    )`,
    ],
    postgres: [
        `CREATE TABLE Article (
        article_id TEXT NOT NULL PRIMARY KEY,
        slug TEXT NOT NULL,
        body TEXT,
        CONSTRAINT uq_article_slug UNIQUE (slug)
    )`,
        `CREATE TABLE Doc (
        doc_id TEXT NOT NULL PRIMARY KEY,
        label TEXT,
        embedding TEXT
    )`,
    ],
};

const DSN = {
    mysql: `mysql://yyds@${host}:${mysqlPort}`,
    postgres: `postgres://yyds:@${host}:${pgPort}/yyds?schema=public`,
};

async function createTable() {
    if (BACKEND === "mysql") {
        const connection = await mysql.createConnection({ host, port: mysqlPort, user: "yyds", password: "", charset: "utf8mb4" });
        try {
            for (const statement of CREATE_TABLE.mysql) {
                await connection.query(statement);
            }
        } finally {
            await connection.end();
        }
        return;
    }
    const client = new PgClient({ host, port: pgPort, user: "yyds", password: "", database: "yyds" });
    await client.connect();
    try {
        for (const statement of CREATE_TABLE.postgres) {
            await client.query(statement);
        }
    } finally {
        await client.end();
    }
}

async function generateClients() {
    rmSync(generatedRoot, { recursive: true, force: true });
    const schema = (await import("node:fs/promises")).readFile(join(HERE, "schema.iris"), "utf8");
    const { loadIrisNative } = await import("@yydb/iris/node");
    const core = loadIrisNative();
    const tsDir = join(generatedRoot, "ts");
    const pyDir = join(generatedRoot, "py", "generated", "iris");
    mkdirSync(tsDir, { recursive: true });
    mkdirSync(pyDir, { recursive: true });
    for (const [target, outDir] of [
        ["typescript", tsDir],
        ["python", pyDir],
    ]) {
        const result = core.generate(await schema, target, outDir);
        if (!result.ok) throw new Error(`generate ${target} failed: ${result.message ?? "unknown"}`);
        console.log(`[generate] ${target} -> ${outDir} (${result.files.length} files)`);
    }
}

async function resetTables() {
    if (BACKEND === "mysql") {
        const connection = await mysql.createConnection({ host, port: mysqlPort, user: "yyds", password: "" });
        try {
            await connection.query("DELETE FROM Article");
            await connection.query("DELETE FROM Doc");
        } finally {
            await connection.end();
        }
        return;
    }
    const client = new PgClient({ host, port: pgPort, user: "yyds", password: "", database: "yyds" });
    await client.connect();
    try {
        await client.query("DELETE FROM Article");
        await client.query("DELETE FROM Doc");
    } finally {
        await client.end();
    }
}

const UNIQUE_DIAGNOSTIC = { mysql: "1062", postgres: "23505" };
const SUITE_ENV = { IRIS_E2E_UNIQUE_DIAGNOSTIC: UNIQUE_DIAGNOSTIC[BACKEND] };

function runSuite(label, command, args, env) {
    return new Promise((resolveRun) => {
        const child = spawn(command, args, { cwd: REPO, env: { ...process.env, ...env }, stdio: ["ignore", "pipe", "pipe"] });
        let out = "";
        child.stdout.setEncoding("utf8");
        child.stderr.setEncoding("utf8");
        child.stdout.on("data", (chunk) => {
            out += chunk;
        });
        child.stderr.on("data", (chunk) => {
            out += chunk;
        });
        child.on("exit", (code) => {
            const match = /^RESULT (.+)$/m.exec(out);
            if (code === 0 && match) {
                resolveRun({ label, ok: true, result: JSON.parse(match[1]), log: out });
                return;
            }
            resolveRun({ label, ok: false, result: null, log: out });
        });
    });
}

let exitCode = 1;
try {
    const init = runCli(["init", "--data-dir", dataDir, "--cluster-id", clusterId, "--node-id", nodeId], 30_000);
    console.log(`[yyds] init exit=${init.code}`);
    if (init.code !== 0) throw new Error(`yyds init failed: ${init.stderr.trim()}`);

    await startNode();
    await createTable();
    await generateClients();

    const python = process.env.IRIS_PYTHON ?? join(REPO, ".venv-python", "Scripts", "python.exe");
    await resetTables();
    const ts = await runSuite("ts", process.execPath, [join(REPO, "e2e", "node_modules", "tsx", "dist", "cli.mjs"), join(HERE, "suite.ts")], {
        IRIS_E2E_DSN: DSN[BACKEND],
        ...SUITE_ENV,
    });
    await resetTables();
    const py = await runSuite("py", python, [join(HERE, "suite.py")], { IRIS_E2E_DSN: DSN[BACKEND], ...SUITE_ENV });

    if (process.env.IRIS_E2E_DUMP === "1" && BACKEND === "mysql") {
        const connection = await mysql.createConnection({ host, port: mysqlPort, user: "yyds", password: "" });
        try {
            const [rows] = await connection.query("SELECT * FROM Article");
            console.log(`[dump] ${JSON.stringify(rows)}`);
        } catch (error) {
            console.log(`[dump] failed: ${error.message}`);
        } finally {
            await connection.end();
        }
    }

    for (const run of [ts, py]) {
        console.log(`[${run.label}] ${run.ok ? "PASS" : "FAIL"}`);
        if (!run.ok) console.log(run.log.trimEnd());
        else console.log(`[${run.label}] codes=${JSON.stringify(run.result.codes)}`);
    }

    let mismatches = 0;
    if (ts.ok && py.ok) {
        for (const key of Object.keys(ts.result.codes)) {
            if (ts.result.codes[key] !== py.result.codes[key]) {
                mismatches += 1;
                console.error(`[mismatch] ${key}: ts=${ts.result.codes[key]} py=${py.result.codes[key]}`);
            }
        }
        console.log(`[parity] ${Object.keys(ts.result.codes).length} codes compared, ${mismatches} mismatches`);
    }
    exitCode = ts.ok && py.ok && mismatches === 0 ? 0 : 1;
} catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    exitCode = 1;
} finally {
    await stopNode();
    if (process.env.IRIS_E2E_KEEP_DATA !== "1") {
        rmSync(dataDir, { recursive: true, force: true });
    }
    process.exitCode = exitCode;
}
