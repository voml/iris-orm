import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export type GenerateCore = {
    generate(schema: string, target: string, outDir: string): {
        ok: boolean;
        files: string[];
        schemaFingerprint: string;
        outputPath: string;
    };
};

export async function writeBindingStubs(stubDir: string): Promise<void> {
    await mkdir(stubDir, { recursive: true });
    await writeFile(
        join(stubDir, "iris-types.ts"),
        `import type { IrisOperation } from "./iris-operation.js";

export type IrisDiagnostic = {
  code: string;
  message: string;
  severity: "error" | "warning";
};

export type OperationIdentity = {
  operationId: string;
  contractFingerprint: string;
};

export type OperationRequest = {
  identity: OperationIdentity;
  operation: IrisOperation;
  parameters?: Readonly<Record<string, unknown>>;
  deadlineMs?: number;
};

export type ResultEnvelope<T = unknown> =
  | { ok: true; value: T; diagnostics?: readonly IrisDiagnostic[] }
  | { ok: false; diagnostics: readonly IrisDiagnostic[] };

export type OperationExecutor = {
  execute(request: OperationRequest): Promise<ResultEnvelope<readonly Record<string, unknown>[]>>;
  executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>>;
  close(): Promise<void>;
};

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
        join(stubDir, "iris-operation.ts"),
        `export type IrisOperation =
  | { kind: "declared-vos"; source: string }
  | { kind: "source"; source: string };
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
`,
        "utf8",
    );
}

export async function copyFixtureDir(sourceDir: string, targetDir: string): Promise<string[]> {
    await mkdir(targetDir, { recursive: true });
    const names = await readdir(sourceDir);
    const tsFiles = names.filter((name) => name.endsWith(".ts"));
    for (const name of tsFiles) {
        const body = await readFile(join(sourceDir, name), "utf8");
        await writeFile(join(targetDir, name), body, "utf8");
    }
    return tsFiles.map((name) => join(targetDir, name));
}

function tscBin(): string {
    return fileURLToPath(new URL("../../../../../node_modules/typescript/bin/tsc", import.meta.url));
}

function workspaceRoot(): string {
    return fileURLToPath(new URL("../../../../../", import.meta.url));
}

export function runTsc(configPath: string): { status: number | null; stdout: string; stderr: string } {
    const direct = spawnSync(process.execPath, [tscBin(), "--noEmit", "-p", configPath], {
        encoding: "utf8",
    });
    if (direct.status === 0 || direct.status === 1 || direct.status === 2) {
        return { status: direct.status, stdout: direct.stdout, stderr: direct.stderr };
    }
    const fallback = spawnSync("pnpm", ["exec", "tsc", "--noEmit", "-p", configPath], {
        encoding: "utf8",
        cwd: workspaceRoot(),
        shell: true,
    });
    return { status: fallback.status, stdout: fallback.stdout, stderr: fallback.stderr };
}

export async function writeTscConfig(
    outDir: string,
    stubDir: string,
    name: string,
    include: string[],
): Promise<string> {
    const configPath = join(outDir, name);
    await writeFile(
        configPath,
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
                include,
            },
            null,
            2,
        ),
        "utf8",
    );
    return configPath;
}
