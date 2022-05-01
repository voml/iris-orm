import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

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
    assert.equal(result.files.length, 10);

    assert.match(result.outputPath.replace(/\\/g, "/"), /generated\/iris$/);

    const index = await readFile(join(root, "index.ts"), "utf8");
    assert.match(index, /export \{ DbClient, createClient \}/);
    assert.match(index, /from "\.\/operations\.js"/);
    assert.match(index, /from "\.\/errors\.js"/);
    assert.doesNotMatch(index, /export \{ db \}/);
    assert.doesNotMatch(index, /synthesize/);

    const nodeEntry = await readFile(join(root, "node.ts"), "utf8");
    assert.match(nodeEntry, /createIrisDbBinding/);
    assert.match(nodeEntry, /export async function createDatabase/);
    assert.match(nodeEntry, /export async function openDatabase/);
    assert.match(nodeEntry, /export async function closeDatabase/);

    const browserEntry = await readFile(join(root, "browser.ts"), "utf8");
    assert.match(browserEntry, /@yydb\/iris\/wasm/);
    assert.match(browserEntry, /createIrisDbBinding/);
    assert.match(browserEntry, /export async function createDatabase/);
    assert.doesNotMatch(browserEntry, /createBrowserIrisDbBinding/);

    const inputs = await readFile(join(root, "inputs.ts"), "utf8");
    assert.match(inputs, /export type StringFilter/);
    assert.match(inputs, /export type BooleanFilter/);
    assert.match(inputs, /export type PatchValue</);
    assert.match(inputs, /UserPatchInput/);
    assert.doesNotMatch(inputs, /UpdateInput/);
    assert.match(inputs, /UserGetPayload</);

    const operations = await readFile(join(root, "operations.ts"), "utf8");
    assert.match(operations, /\$query<T = unknown>/);
    assert.match(operations, /findMany<const A extends UserFindManyArgs>/);
    assert.match(operations, /ReadonlyArray<UserGetPayload<A>>/);
    assert.match(operations, /synthesizeCreate/);
    assert.match(operations, /\.\/_internal\/synthesize\.js/);
    assert.doesNotMatch(operations, /\.\.\.args: unknown\[\]/);
    assert.doesNotMatch(operations, /@yydb\/iris\/node/);
    assert.doesNotMatch(operations, /include/);
    assert.doesNotMatch(operations, /::insert\(\{ \.\.\. \}\)/);

    const synthesize = await readFile(join(root, "_internal", "synthesize.ts"), "utf8");
    assert.match(synthesize, /compileWherePredicates/);
    assert.match(synthesize, /synthesizeCreate/);

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
        `export type IrisDbBinding = {
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
        `import type { CreateIrisDbBindingOptions, IrisDbBinding } from "./iris-types.js";
export async function createIrisDbBinding(_options?: CreateIrisDbBindingOptions): Promise<IrisDbBinding> {
  throw new Error("stub");
}
`,
        "utf8",
    );
    await writeFile(
        join(stubDir, "iris-wasm.ts"),
        `import type { CreateIrisDbBindingOptions, IrisDbBinding } from "./iris-types.js";
export async function createIrisDbBinding(_options?: CreateIrisDbBindingOptions): Promise<IrisDbBinding> {
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
import type { IrisDbBinding } from "@yydb/iris/types";
import type { PostFindManyArgs } from "./inputs.js";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
  const args = {
    where: {
      author: {
        user_name: { not: "" },
      },
      title: { contains: "Iris" },
    },
    select: {
      post_id: true,
      title: true,
      author: {
        select: {
          user_id: true,
          user_name: true,
        },
      },
    },
  } satisfies PostFindManyArgs;

  const posts = await db.post.findMany(args);
  const _title: string = posts[0]!.title;
  const _name: string = posts[0]!.author.user_name;
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
import type { IrisDbBinding } from "@yydb/iris/types";

declare const binding: IrisDbBinding;
const db = createClient(binding);

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

    const unit = await binding.execute("User.filter(x => x.active).collect()");
    assert.equal(unit, undefined);

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
