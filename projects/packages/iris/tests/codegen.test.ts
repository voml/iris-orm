import assert from "node:assert/strict";
import { copyFile, mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { USER_SCHEMA } from "./fixtures.ts";
import { srcImport } from "./helpers.ts";

const POST_USER_SCHEMA = `
table User {
    @@user_id: uuid,
    user_name: utf8,
}

table Post {
    @@post_id: uuid,
    author: &User,
    title: utf8,
}
`;

function ensureNativeOverride(): void {
    if (process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
        return;
    }
    const resolveScript = fileURLToPath(new URL("../../../../scripts/resolve-native-artifact.mjs", import.meta.url));
    const artifactOut = spawnSync(process.execPath, [resolveScript], {
        encoding: "utf8",
    });
    if (artifactOut.status === 0 && artifactOut.stdout.trim()) {
        process.env.NAPI_RS_NATIVE_LIBRARY_PATH = artifactOut.stdout.trim();
    }
}

async function loadCore(t: { skip: (msg?: string) => void }) {
    ensureNativeOverride();
    const node = await import(srcImport("src/node/index.ts"));
    try {
        return node.loadIrisNative();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return null;
        }
        throw error;
    }
}

test("generate writes TypeScript client via Rust iris-generator", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const outDir = await mkdtemp(join(tmpdir(), "iris-codegen-"));
    const root = join(outDir, "generated", "iris");
    const result = core.generate(USER_SCHEMA, "typescript", root);
    assert.equal(result.ok, true);
    assert.equal(result.files.length, 12);

    assert.match(result.outputPath.replace(/\\/g, "/"), /generated\/iris$/);

    const index = await readFile(join(root, "index.ts"), "utf8");
    assert.match(index, /export \{ DbClient, createClient \}/);
    assert.match(index, /from "\.\/operations\.js"/);
    assert.match(index, /from "\.\/errors\.js"/);
    assert.doesNotMatch(index, /export \{ db \}/);
    assert.doesNotMatch(index, /synthesize/);

    const nodeEntry = await readFile(join(root, "node.ts"), "utf8");
    assert.match(nodeEntry, /createIrisOperationExecutor/);
    assert.match(nodeEntry, /IRIS_SCHEMA_FINGERPRINT/);
    assert.match(nodeEntry, /contractFingerprint: IRIS_SCHEMA_FINGERPRINT/);
    assert.match(nodeEntry, /export class Database/);
    assert.match(nodeEntry, /static async create/);
    assert.match(nodeEntry, /static async open/);
    assert.match(nodeEntry, /static async close/);

    const browserEntry = await readFile(join(root, "browser.ts"), "utf8");
    assert.match(browserEntry, /@yydb\/iris\/wasm/);
    assert.match(browserEntry, /createIrisOperationExecutor/);
    assert.match(browserEntry, /IRIS_SCHEMA_FINGERPRINT/);
    assert.match(browserEntry, /contractFingerprint: IRIS_SCHEMA_FINGERPRINT/);
    assert.match(browserEntry, /export class Database/);
    assert.match(browserEntry, /static async create/);
    assert.doesNotMatch(browserEntry, /createBrowserIrisDbBinding/);

    const cloudflareEntry = await readFile(join(root, "cloudflare.ts"), "utf8");
    assert.match(cloudflareEntry, /@yydb\/iris\/cloudflare/);
    assert.match(cloudflareEntry, /IRIS_D1_PLANS/);
    assert.match(cloudflareEntry, /IRIS_FIELD_WIRE_NAMES/);
    assert.match(cloudflareEntry, /contractFingerprint: IRIS_SCHEMA_FINGERPRINT/);

    const d1Plans = await readFile(join(root, "_internal", "d1-plans.ts"), "utf8");
    assert.match(d1Plans, /IRIS_D1_PLANS/);
    assert.match(d1Plans, /User\.findMany/);
    assert.match(d1Plans, /User\.findMany@p_active/);
    assert.match(d1Plans, /User\.findMany@p_user_name/);
    assert.match(d1Plans, /User\.findMany@take/);
    assert.match(d1Plans, /User\.findMany@p_active,take/);
    assert.match(d1Plans, /User\.findUnique@p_user_id/);
    assert.match(d1Plans, /paramOrder: \["p_active"\]/);
    assert.match(d1Plans, /paramOrder: \["take"\]/);
    assert.match(d1Plans, /WHERE user_name = \?/);
    assert.match(d1Plans, /SELECT .* FROM User/);
    assert.match(d1Plans, /User\.create@data_active,data_user_id,data_user_name/);
    assert.match(d1Plans, /INSERT INTO User/);
    assert.match(d1Plans, /RETURNING user_id, user_name, active/);
    assert.match(d1Plans, /mode: "write-returning"/);
    assert.match(d1Plans, /paramOrder: \["data_user_id", "data_user_name", "data_active"\]/);
    assert.match(d1Plans, /User\.create@data_user_id/);
    assert.match(d1Plans, /User\.delete@p_user_id/);
    assert.match(d1Plans, /DELETE FROM User WHERE user_id = \?/);
    assert.match(d1Plans, /mode: "write"/);
    assert.match(d1Plans, /User\.update@p_user_id,patch_user_name/);
    assert.match(d1Plans, /UPDATE User SET user_name = \? WHERE user_id = \?/);
    assert.match(d1Plans, /paramOrder: \["patch_user_name", "p_user_id"\]/);

    const inputs = await readFile(join(root, "inputs.ts"), "utf8");
    assert.match(inputs, /export type StringFilter/);
    assert.match(inputs, /export type BooleanFilter/);
    assert.match(inputs, /export type PatchValue</);
    assert.match(inputs, /UserPatchInput/);
    assert.doesNotMatch(inputs, /UpdateInput/);
    assert.match(inputs, /UserGetPayload</);

    const operations = await readFile(join(root, "operations.ts"), "utf8");
    assert.match(operations, /OperationExecutor/);
    assert.match(operations, /buildDeclaredOperationRequest/);
    assert.match(operations, /\$query<T = unknown>/);
    assert.match(operations, /findMany<const A extends UserFindManyArgs>/);
    assert.match(operations, /ReadonlyArray<UserGetPayload<A>>/);
    assert.match(operations, /synthesizeCreate/);
    assert.match(operations, /synthesizeDelete/);
    assert.match(operations, /synthesizeUpdate/);
    assert.match(operations, /async update</);
    assert.match(operations, /async delete\(/);
    assert.match(inputs, /UserUpdateArgs/);
    assert.match(operations, /\.\/_internal\/synthesize\.js/);
    assert.doesNotMatch(operations, /\.\.\.args: unknown\[\]/);
    assert.doesNotMatch(operations, /@yydb\/iris\/node/);
    assert.doesNotMatch(operations, /include/);
    assert.doesNotMatch(operations, /::insert\(\{ \.\.\. \}\)/);

    const synthesize = await readFile(join(root, "_internal", "synthesize.ts"), "utf8");
    assert.match(synthesize, /compileWherePredicates/);
    assert.match(synthesize, /synthesizeCreate/);
    assert.match(synthesize, /synthesizeUpdate/);
    assert.match(synthesize, /patch_/);
    assert.match(synthesize, /compileSelectWireColumns/);
    assert.match(synthesize, /select_cols/);
    assert.match(synthesize, /buildDeclaredOperationRequest/);
    assert.match(synthesize, /IRIS_SCHEMA_FINGERPRINT/);

    const errors = await readFile(join(root, "errors.ts"), "utf8");
    assert.match(errors, /IrisGeneratedError/);

    const metadata = await readFile(join(root, "metadata.ts"), "utf8");
    assert.ok(metadata.includes(result.schemaFingerprint));
});

test("generated multi-table + reference client typechecks under tsc", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const outDir = await mkdtemp(join(tmpdir(), "iris-codegen-tsc-"));
    const generatedRoot = join(outDir, "src", "generated", "iris");
    const result = core.generate(POST_USER_SCHEMA, "typescript", generatedRoot);
    assert.equal(result.ok, true);

    const typesRoot = fileURLToPath(new URL("../src/types", import.meta.url));
    const stubDir = join(outDir, "stubs");
    await mkdir(stubDir, { recursive: true });
    await writeFile(
        join(stubDir, "iris-types.ts"),
        `export type IrisDiagnostic = {
  readonly code: string;
  readonly message: string;
  readonly severity: "error" | "warning";
  readonly retryable?: boolean;
  readonly unknownCommit?: boolean;
};

export type ResultEnvelope<T = unknown> =
  | { ok: true; value: T; diagnostics?: readonly IrisDiagnostic[] }
  | { ok: false; diagnostics: readonly IrisDiagnostic[] };

export type OperationIdentity = {
  readonly operationId: string;
  readonly contractFingerprint: string;
};

export type IrisOperation =
  | { kind: "find-many"; entity: string; where?: { field: string; value: unknown }; take?: number }
  | { kind: "find-unique"; entity: string; where: { field: string; value: unknown } }
  | { kind: "declared-vos"; source: string };

export type OperationRequest = {
  readonly identity: OperationIdentity;
  readonly operation: IrisOperation;
  readonly parameters?: Readonly<Record<string, unknown>>;
  readonly deadlineMs?: number;
};

export interface OperationExecutor {
  execute(request: OperationRequest): Promise<ResultEnvelope<readonly unknown[]>>;
  executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>>;
  close(): Promise<void>;
}

export type IrisDbBinding = {
  query(source: string, parameters?: Readonly<Record<string, unknown>>): Promise<unknown>;
  execute(source: string, parameters?: Readonly<Record<string, unknown>>): Promise<void>;
  close(): Promise<void>;
};
export type CreateIrisDbBindingOptions = {
  profile?: "memory" | "sqlite" | "project";
  sqlitePath?: string;
  config?: string;
  project?: string;
  source?: string;
  schema?: string;
};
`,
        "utf8",
    );
    await writeFile(
        join(stubDir, "iris-node.ts"),
        `import type { CreateIrisDbBindingOptions, IrisDbBinding, OperationExecutor } from "./iris-types.js";
export async function createIrisDbBinding(_options?: CreateIrisDbBindingOptions): Promise<IrisDbBinding> {
  throw new Error("stub");
}
export async function createIrisOperationExecutor(_options?: CreateIrisDbBindingOptions): Promise<OperationExecutor> {
  throw new Error("stub");
}
`,
        "utf8",
    );
    await writeFile(
        join(stubDir, "iris-wasm.ts"),
        `import type { CreateIrisDbBindingOptions, IrisDbBinding, OperationExecutor } from "./iris-types.js";
export async function createIrisDbBinding(_options?: CreateIrisDbBindingOptions): Promise<IrisDbBinding> {
  throw new Error("stub");
}
export async function createIrisOperationExecutor(_options?: CreateIrisDbBindingOptions): Promise<OperationExecutor> {
  throw new Error("stub");
}
`,
        "utf8",
    );
    await writeFile(
        join(stubDir, "iris-browser.ts"),
        `import type { CreateIrisDbBindingOptions, IrisDbBinding } from "./iris-types.js";
export async function createIrisDbBinding(_options?: CreateIrisDbBindingOptions): Promise<IrisDbBinding> {
  throw new Error("stub");
}
/** @deprecated */
export const createBrowserIrisDbBinding = createIrisDbBinding;
`,
        "utf8",
    );

    await writeFile(
        join(outDir, "tsconfig.generated.json"),
        JSON.stringify(
            {
                compilerOptions: {
                    target: "ES2022",
                    module: "ESNext",
                    moduleResolution: "bundler",
                    strict: true,
                    noEmit: true,
                    skipLibCheck: true,
                    paths: {
                        "@yydb/iris/types": [join(stubDir, "iris-types.ts").replace(/\\/g, "/")],
                        "@yydb/iris/node": [join(stubDir, "iris-node.ts").replace(/\\/g, "/")],
                        "@yydb/iris/wasm": [join(stubDir, "iris-wasm.ts").replace(/\\/g, "/")],
                        "@yydb/iris": [join(stubDir, "iris-browser.ts").replace(/\\/g, "/")],
                    },
                    baseUrl: ".",
                },
                include: ["./src/generated/iris/**/*.ts"],
                exclude: [
                    "./src/generated/iris/consumer-negative.ts",
                    "./src/generated/iris/node.ts",
                    "./src/generated/iris/browser.ts",
                    "./src/generated/iris/cloudflare.ts",
                ],
            },
            null,
            2,
        ),
        "utf8",
    );

    await writeFile(
        join(generatedRoot, "consumer.ts"),
        `import { createClient } from "./index.js";
import type { OperationExecutor } from "@yydb/iris/types";
import type { PostFindManyArgs } from "./inputs.js";

declare const executor: OperationExecutor;
const db = createClient(executor);

async function run() {
  const args = {
    where: {
      author: {
        is: {
          userName: { not: "" },
        },
      },
      title: { contains: "Iris" },
    },
    select: {
      postId: true,
      title: true,
      author: {
        select: {
          userId: true,
          userName: true,
        },
      },
    },
  } satisfies PostFindManyArgs;

  const posts = await db.post.findMany(args);
  const _title: string = posts[0]!.title;
  const _name: string = posts[0]!.author.userName;
  void _title;
  void _name;
}
void run;
`,
        "utf8",
    );

    await writeFile(
        join(generatedRoot, "consumer-negative.ts"),
        `import { createClient } from "./index.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = createClient(executor);

async function run() {
  await db.post.findMany({
    where: {
      title: { contains: 123 },
    },
  });
}
void run;
`,
        "utf8",
    );

    await writeFile(
        join(outDir, "tsconfig.negative.json"),
        JSON.stringify(
            {
                compilerOptions: {
                    target: "ES2022",
                    module: "ESNext",
                    moduleResolution: "bundler",
                    strict: true,
                    noEmit: true,
                    skipLibCheck: true,
                    paths: {
                        "@yydb/iris/types": [join(stubDir, "iris-types.ts").replace(/\\/g, "/")],
                    },
                    baseUrl: ".",
                },
                include: ["./src/generated/iris/consumer-negative.ts"],
            },
            null,
            2,
        ),
        "utf8",
    );

    const tsc = spawnSync(
        process.execPath,
        [
            fileURLToPath(new URL("../../../../node_modules/typescript/bin/tsc", import.meta.url)),
            "--noEmit",
            "-p",
            join(outDir, "tsconfig.generated.json"),
        ],
        { encoding: "utf8" },
    );
    if (tsc.status !== 0) {
        const tsc2 = spawnSync("pnpm", ["exec", "tsc", "--noEmit", "-p", join(outDir, "tsconfig.generated.json")], {
            encoding: "utf8",
            cwd: fileURLToPath(new URL("../../../..", import.meta.url)),
            shell: true,
        });
        assert.equal(tsc2.status, 0, tsc2.stdout + tsc2.stderr + (typesRoot ?? "") + result.schemaFingerprint);
    } else {
        assert.equal(tsc.status, 0, tsc.stdout + tsc.stderr);
    }

    const tscNegative = spawnSync(
        process.execPath,
        [
            fileURLToPath(new URL("../../../../node_modules/typescript/bin/tsc", import.meta.url)),
            "--noEmit",
            "-p",
            join(outDir, "tsconfig.negative.json"),
        ],
        { encoding: "utf8" },
    );
    const negativeStatus =
        tscNegative.status ??
        spawnSync("pnpm", ["exec", "tsc", "--noEmit", "-p", join(outDir, "tsconfig.negative.json")], {
            encoding: "utf8",
            cwd: fileURLToPath(new URL("../../../..", import.meta.url)),
            shell: true,
        }).status;
    assert.notEqual(negativeStatus, 0, "invalid filter value types must fail tsc");
});

test("generated synthesize runs findMany through OperationExecutor on Node", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const outDir = await mkdtemp(join(tmpdir(), "iris-codegen-e2e-"));
    const generatedRoot = join(outDir, "generated", "iris");
    const result = core.generate(USER_SCHEMA, "typescript", generatedRoot);
    assert.equal(result.ok, true);

    const runnerTemplate = fileURLToPath(new URL("./generated-e2e-runner.ts", import.meta.url));
    const runnerPath = join(generatedRoot, "generated-e2e-runner.ts");
    await copyFile(runnerTemplate, runnerPath);

    const bootstrap = pathToFileURL(fileURLToPath(new URL("./generated-e2e-bootstrap.mjs", import.meta.url))).href;
    const child = spawnSync(
        process.execPath,
        ["--experimental-strip-types", `--import`, bootstrap, "generated-e2e-runner.ts"],
        {
            encoding: "utf8",
            cwd: generatedRoot,
            env: { ...process.env, IRIS_TEST_SCHEMA: USER_SCHEMA },
        },
    );
    assert.equal(child.status, 0, child.stdout + child.stderr);

    const payload = JSON.parse(child.stdout.trim()) as {
        ok: boolean;
        fingerprint: string;
        directCount: number;
        clientCount: number;
        databaseCount: number;
        createdUserId: string;
        createdUserName: string;
        createdActive: boolean;
        uniqueUserName: string;
        afterCreateCount: number;
        updatedUserName: string;
        afterDeleteCount: number;
    };
    assert.equal(payload.ok, true);
    assert.equal(payload.fingerprint, result.schemaFingerprint);
    assert.equal(typeof payload.directCount, "number");
    assert.equal(typeof payload.clientCount, "number");
    assert.equal(typeof payload.databaseCount, "number");
    assert.equal(payload.createdUserId, "e2e-1");
    assert.equal(payload.createdUserName, "Ada");
    assert.equal(payload.createdActive, true);
    assert.equal(payload.uniqueUserName, "Ada");
    assert.equal(payload.afterCreateCount, 1);
    assert.equal(payload.updatedUserName, "Grace");
    assert.equal(payload.afterDeleteCount, 0);
});

test("generated cloudflare Database.create runs findMany through D1 read plans", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const outDir = await mkdtemp(join(tmpdir(), "iris-codegen-cf-e2e-"));
    const generatedRoot = join(outDir, "generated", "iris");
    const result = core.generate(USER_SCHEMA, "typescript", generatedRoot);
    assert.equal(result.ok, true);

    const runnerTemplate = fileURLToPath(new URL("./generated-e2e-cloudflare-runner.ts", import.meta.url));
    const runnerPath = join(generatedRoot, "generated-e2e-cloudflare-runner.ts");
    await copyFile(runnerTemplate, runnerPath);

    const bootstrap = pathToFileURL(fileURLToPath(new URL("./generated-e2e-bootstrap.mjs", import.meta.url))).href;
    const child = spawnSync(
        process.execPath,
        ["--experimental-strip-types", `--import`, bootstrap, "generated-e2e-cloudflare-runner.ts"],
        {
            encoding: "utf8",
            cwd: generatedRoot,
        },
    );
    assert.equal(child.status, 0, child.stdout + child.stderr);

    const payload = JSON.parse(child.stdout.trim()) as {
        ok: boolean;
        unfilteredCount: number;
        filteredCount: number;
        limitedCount: number;
        filteredLimitedCount: number;
        uniqueUserId: string;
        createdUserId: string;
        createdUserName: string;
        createdHasActive: boolean;
        updatedUserName: string;
        filteredTrace: { sql: string; bind: unknown[] };
        limitedTrace: { sql: string; bind: unknown[] };
        filteredLimitedTrace: { sql: string; bind: unknown[] };
        uniqueTrace: { sql: string; bind: unknown[] };
        createTrace: { sql: string; bind: unknown[] };
        updateTrace: { sql: string; bind: unknown[] };
        deleteTrace: { sql: string; bind: unknown[] };
    };
    assert.equal(payload.ok, true);
    assert.equal(payload.unfilteredCount, 1);
    assert.equal(payload.filteredCount, 1);
    assert.equal(payload.limitedCount, 1);
    assert.equal(payload.filteredLimitedCount, 1);
    assert.equal(payload.uniqueUserId, "u1");
    assert.equal(payload.createdUserId, "u1");
    assert.equal(payload.createdUserName, "Ada");
    assert.equal(payload.createdHasActive, false);
    assert.equal(payload.updatedUserName, "Ada");
    assert.match(payload.filteredTrace.sql, /WHERE active = \?/);
    assert.deepEqual(payload.filteredTrace.bind, [1]);
    assert.match(payload.limitedTrace.sql, /LIMIT \?/);
    assert.deepEqual(payload.limitedTrace.bind, [2]);
    assert.match(payload.filteredLimitedTrace.sql, /WHERE active = \?/);
    assert.match(payload.filteredLimitedTrace.sql, /LIMIT \?/);
    assert.deepEqual(payload.filteredLimitedTrace.bind, [1, 2]);
    assert.match(payload.uniqueTrace.sql, /WHERE user_id = \?/);
    assert.deepEqual(payload.uniqueTrace.bind, ["u1"]);
    assert.match(payload.uniqueTrace.sql, /LIMIT 1/);
    assert.match(payload.createTrace.sql, /INSERT INTO User/);
    assert.match(payload.createTrace.sql, /RETURNING user_id,user_name/);
    assert.deepEqual(payload.createTrace.bind, ["u2", "Bob", 0]);
    assert.match(payload.updateTrace.sql, /UPDATE User SET user_name = \?/);
    assert.deepEqual(payload.updateTrace.bind, ["Carol", "u2"]);
    assert.match(payload.deleteTrace.sql, /DELETE FROM User/);
    assert.deepEqual(payload.deleteTrace.bind, ["u2"]);
});

test("createIrisOperationExecutor returns ResultEnvelope for declared-vos", async (t) => {
    const node = await import(srcImport("src/node/index.ts"));
    const { buildOperationRequest, declaredVosOperation } = await import(
        srcImport("src/runtime/build-operation-request.ts")
    );

    let executor;
    try {
        executor = await node.createIrisOperationExecutor({
            profile: "sqlite",
            sqlitePath: ":memory:",
            schema: USER_SCHEMA,
            contractFingerprint: "integration-fp",
        });
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return;
        }
        throw error;
    }

    const request = buildOperationRequest(
        { operationId: "user.findMany", contractFingerprint: "integration-fp" },
        declaredVosOperation("User.filter(x => x.active).collect()"),
    );
    const result = await executor.execute(request);
    assert.equal(result.ok, true);
    if (result.ok) {
        assert.equal(Array.isArray(result.value), true);
    }

    const mismatch = await executor.execute(
        buildOperationRequest(
            { operationId: "user.findMany", contractFingerprint: "wrong-fp" },
            declaredVosOperation("User.filter(x => x.active).collect()"),
        ),
    );
    assert.equal(mismatch.ok, false);
    if (!mismatch.ok) {
        assert.equal(mismatch.diagnostics[0]?.code, "IRIS-CONTRACT-MISMATCH");
    }

    await executor.close();
});

test("createIrisDbBinding binds parameters through Rust", async (t) => {
    const node = await import(srcImport("src/node/index.ts"));
    let binding;
    try {
        binding = await node.createIrisDbBinding({
            profile: "sqlite",
            sqlitePath: ":memory:",
            schema: USER_SCHEMA,
        });
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return;
        }
        throw error;
    }

    const rows = await binding.query("User.filter(x => x.active == $active).collect()", {
        active: true,
    });
    assert.equal(Array.isArray(rows), true);

    await assert.rejects(() => binding.query("User.filter(x => x.active == $active).collect()", {}), /unbound/i);

    await binding.close();
});

test("createIrisDbBinding splits DML query and DDL execute", async (t) => {
    const node = await import(srcImport("src/node/index.ts"));

    let binding;
    try {
        binding = await node.createIrisDbBinding({
            profile: "sqlite",
            sqlitePath: ":memory:",
            schema: USER_SCHEMA,
        });
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return;
        }
        throw error;
    }

    const rows = await binding.query("User.filter(x => x.active).collect()");
    assert.equal(Array.isArray(rows), true);

    await binding.execute("User.filter(x => x.active).collect()");

    await binding.close();
});

test("createBrowserIrisDbBinding binds parameters through Rust", async (t) => {
    const browser = await import(srcImport("src/browser/index.ts"));
    const wasm = await import(srcImport("src/wasm/index.ts"));
    wasm.resetInitStateForTests();

    try {
        await wasm.initIris();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "wasm-package-missing") {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    let binding;
    try {
        binding = await browser.createIrisDbBinding({ schema: USER_SCHEMA });
    } catch (error) {
        if (error instanceof Error && error.message.includes("iris_wasm")) {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    const rows = await binding.query("User.filter(x => x.active == $active).collect()", {
        active: true,
    });
    assert.equal(Array.isArray(rows), true);

    await assert.rejects(
        () => binding.query("User.filter(x => x.active == $active).collect()", {}),
        /unbound/i,
    );

    await binding.close();
});

test("@yydb/iris/wasm createIrisDbBinding matches node binding surface", async (t) => {
    const wasm = await import(srcImport("src/wasm/index.ts"));
    wasm.resetInitStateForTests();

    try {
        await wasm.initIris();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "wasm-package-missing") {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    let binding;
    try {
        binding = await wasm.createIrisDbBinding({ schema: USER_SCHEMA });
    } catch (error) {
        if (error instanceof Error && error.message.includes("iris_wasm")) {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    assert.equal(typeof binding.query, "function");
    assert.equal(typeof binding.execute, "function");
    assert.equal(typeof binding.close, "function");
    await binding.close();
});
