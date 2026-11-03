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
    title: utf8,
}
`;

// Text rows shared by every probe: two start with `al`, one ends with `ta`,
// one is empty, one carries multi-byte text.
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

type FaceResults = Record<string, string[]>;

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

function runProfile(
    core: { generate: (schema: string, target: string, out: string) => { ok: boolean; error?: string } },
    projectRoot: string,
    sqlitePath: string,
): { generated: FaceResults; direct: FaceResults; database: FaceResults } {
    const generatedRoot = join(projectRoot, "src", "generated", "iris");
    const result = core.generate(SCHEMA, "typescript", generatedRoot);
    assert.equal(result.ok, true, result.error);

    const runnerTemplate = fileURLToPath(new URL("./string-match-filter-runner.ts", import.meta.url));
    copyFileSync(runnerTemplate, join(generatedRoot, "string-match-filter-runner.ts"));

    const bootstrap = pathToFileURL(fileURLToPath(new URL("./generated-e2e-bootstrap.mjs", import.meta.url))).href;
    const child = spawnSync(
        process.execPath,
        ["--experimental-strip-types", "--import", bootstrap, "src/generated/iris/string-match-filter-runner.ts"],
        {
            encoding: "utf8",
            cwd: projectRoot,
            env: {
                ...process.env,
                IRIS_TEST_SCHEMA: SCHEMA,
                IRIS_TEST_SQLITE: sqlitePath,
            },
        },
    );
    assert.equal(child.status, 0, child.stdout + child.stderr);
    return JSON.parse(child.stdout.trim()) as { generated: FaceResults; direct: FaceResults; database: FaceResults };
}

async function projectRoot(label: string): Promise<string> {
    const root = await mkdtemp(join(tmpdir(), `iris-${label}-`));
    await mkdir(join(root, "schemas"), { recursive: true });
    await writeFile(join(root, "schemas", "app.iris"), `${SCHEMA.trim()}\n`, "utf8");
    return root;
}

// The reference store is a query-only oracle (`WriteCaps::none`), so its text
// predicate semantics are proven against the store directly in
// `crates/iris-types/tests/string_match_semantics.rs`. This test proves the
// generated surface, the direct executor and `Database` agree on a real
// sqlite database.
test("string predicates match identically on every generated face", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }
    const root = await projectRoot("sqlite-string-match");
    const faces = runProfile(core, root, join(root, "app.sqlite"));
    assert.deepEqual(faces.generated, EXPECTED);
    assert.deepEqual(faces.direct, EXPECTED);
    assert.deepEqual(faces.database, EXPECTED);
});
