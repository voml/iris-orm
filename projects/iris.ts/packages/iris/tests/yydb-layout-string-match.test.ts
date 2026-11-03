import assert from "node:assert/strict";
import { copyFileSync } from "node:fs";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { test } from "node:test";

const SCHEMA = `
table Note {
    @@id: i64,
    title: utf8?,
}
`;

// Five text rows plus one `NULL` row. `NULL` is unknown for every text
// predicate, so row 6 never appears in a filtered result.
const EXPECTED: Record<string, string[]> = {
    contains: ["1", "3"],
    startsWith: ["1", "3"],
    endsWith: ["2"],
    endsWithWhole: ["1"],
    containsEmpty: ["1", "2", "3", "4", "5"],
    startsWithEmpty: ["1", "2", "3", "4", "5"],
    endsWithEmpty: ["1", "2", "3", "4", "5"],
    caseSensitive: [],
    needleLongerThanText: [],
    unicode: ["5"],
};
const ALL_IDS = ["1", "2", "3", "4", "5", "6"];

// One schema text feeds both the project file and the generator, so the
// generated client applies the same document the project session pushed.
const SCHEMA_SOURCE = `${SCHEMA.trim()}\n`;

const LAYOUTS = [
    { label: "single-file `.yydb`", path: "data/app.yydb" },
    { label: "folder `.yydx`", path: "data/app.yydx" },
];

type FaceResults = Record<string, string[]>;
type Payload = { ok: boolean; allIds: string[]; generated: FaceResults; direct: FaceResults; database: FaceResults };

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
    const node = await import(new URL("../src/node/index.ts", import.meta.url).href);
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

async function runLayout(
    core: { generate: (schema: string, target: string, out: string) => { ok: boolean; error?: string } },
    layout: { label: string; path: string },
): Promise<Payload> {
    const projectRoot = await mkdtemp(join(tmpdir(), "iris-yydb-string-match-"));
    await mkdir(join(projectRoot, "schemas"), { recursive: true });
    await writeFile(join(projectRoot, "schemas", "app.iris"), SCHEMA_SOURCE, "utf8");
    await writeFile(
        join(projectRoot, "iris.config.ts"),
        `export default {
  schema: "schemas/app.iris",
  datasources: {
    default: { kind: "yydb", mode: "native_pull", path: "${layout.path}" },
  },
  generate: { out: ".", target: "typescript" },
};`,
        "utf8",
    );

    const generatedRoot = join(projectRoot, "src", "generated", "iris");
    const result = core.generate(SCHEMA_SOURCE, "typescript", generatedRoot);
    assert.equal(result.ok, true, result.error);

    const runnerTemplate = fileURLToPath(new URL("./yydb-string-match-runner.ts", import.meta.url));
    copyFileSync(runnerTemplate, join(generatedRoot, "yydb-string-match-runner.ts"));

    const bootstrap = pathToFileURL(fileURLToPath(new URL("./generated-e2e-bootstrap.mjs", import.meta.url))).href;
    const child = spawnSync(
        process.execPath,
        ["--experimental-strip-types", "--import", bootstrap, "src/generated/iris/yydb-string-match-runner.ts"],
        {
            encoding: "utf8",
            cwd: projectRoot,
            env: { ...process.env, IRIS_TEST_PROJECT: projectRoot },
        },
    );
    assert.equal(child.status, 0, child.stdout + child.stderr);
    return JSON.parse(child.stdout.trim()) as Payload;
}

for (const layout of LAYOUTS) {
    test(`text predicates agree on every face in a ${layout.label} database`, async (t) => {
        const core = await loadCore(t);
        if (!core) {
            return;
        }
        const payload = await runLayout(core, layout);
        assert.equal(payload.ok, true);
        assert.deepEqual(payload.allIds, ALL_IDS);
        assert.deepEqual(payload.generated, EXPECTED);
        assert.deepEqual(payload.direct, EXPECTED);
        assert.deepEqual(payload.database, EXPECTED);
    });
}
