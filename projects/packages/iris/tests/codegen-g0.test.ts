import assert from "node:assert/strict";
import { mkdtemp, readdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { BLOG_SCHEMA } from "./codegen-g0/schema.ts";
import {
    copyFixtureDir,
    runTsc,
    writeBindingStubs,
    writeTscConfig,
    type GenerateCore,
} from "./codegen-g0/harness.ts";
import { srcImport } from "./helpers.ts";

const FIXTURE_ROOT = fileURLToPath(new URL("./codegen-g0", import.meta.url));

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

async function loadCore(t: { skip: (msg?: string) => void }): Promise<GenerateCore | null> {
    ensureNativeOverride();
    const node = await import(srcImport("src/node/index.ts"));
    try {
        return node.loadIrisNative() as GenerateCore;
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return null;
        }
        throw error;
    }
}

async function prepareGeneratedWorkspace(
    core: GenerateCore,
    outDir: string,
): Promise<{ generatedRoot: string; stubDir: string }> {
    const generatedRoot = join(outDir, "src", "generated", "iris");
    const result = core.generate(BLOG_SCHEMA, "typescript", generatedRoot);
    assert.equal(result.ok, true, "generate must succeed for G0 fixtures");

    const stubDir = join(outDir, "stubs");
    await writeBindingStubs(stubDir);

    const positiveTarget = join(generatedRoot, "g0", "positive");
    const negativeTarget = join(generatedRoot, "g0", "negative");
    await copyFixtureDir(join(FIXTURE_ROOT, "positive"), positiveTarget);
    await copyFixtureDir(join(FIXTURE_ROOT, "negative"), negativeTarget);

    return { generatedRoot, stubDir };
}

test("G0 positive TypeScript author-surface fixtures typecheck", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const outDir = await mkdtemp(join(tmpdir(), "iris-codegen-g0-pos-"));
    const { generatedRoot, stubDir } = await prepareGeneratedWorkspace(core, outDir);

    const positiveDir = join(generatedRoot, "g0", "positive");
    const positiveFiles = (await readdir(positiveDir))
        .filter((name) => name.endsWith(".ts"))
        .map((name) => `./src/generated/iris/g0/positive/${name}`);

    const configPath = await writeTscConfig(outDir, stubDir, "tsconfig.g0-positive.json", [
        ...positiveFiles,
        "./src/generated/iris/index.ts",
        "./src/generated/iris/inputs.ts",
        "./src/generated/iris/models.ts",
        "./src/generated/iris/operations.ts",
        "./src/generated/iris/references.ts",
        "./src/generated/iris/errors.ts",
        "./src/generated/iris/metadata.ts",
        "./src/generated/iris/_internal/synthesize.ts",
    ]);

    const tsc = runTsc(configPath);
    assert.equal(tsc.status, 0, `${tsc.stdout}\n${tsc.stderr}\nroot=${generatedRoot}`);
});

test("G0 negative TypeScript author-surface fixtures fail tsc", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const outDir = await mkdtemp(join(tmpdir(), "iris-codegen-g0-neg-"));
    const { generatedRoot, stubDir } = await prepareGeneratedWorkspace(core, outDir);

    const negativeDir = join(generatedRoot, "g0", "negative");
    const tsFiles = (await readdir(negativeDir)).filter((name) => name.endsWith(".ts"));

    for (const name of tsFiles) {
        const relative = `./src/generated/iris/g0/negative/${name}`;
        const configPath = await writeTscConfig(outDir, stubDir, `tsconfig.g0-negative-${name}.json`, [relative]);
        const tsc = runTsc(configPath);
        assert.notEqual(
            tsc.status,
            0,
            `${name} must fail tsc\n${tsc.stdout}\n${tsc.stderr}\nroot=${dirname(generatedRoot)}`,
        );
    }
});
