import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { defineConfig, toProjectDocument } from "../src/types/config.ts";
import { IrisConfigError } from "../src/types/errors.ts";
import { findAuthoringConfig, loadAuthoringConfig, resolveProjectRoot } from "../src/node/config.ts";

test("defineConfig requires schema data pointer", () => {
    assert.throws(() => defineConfig({ schema: "  " }), /schema/);
    const config = defineConfig({ schema: "schemas/blog.iris" });
    assert.equal(config.schema, "schemas/blog.iris");
});

test("toProjectDocument keeps schema as data pointer", () => {
    const document = toProjectDocument(
        defineConfig({
            schema: "schemas/**/*.iris",
            datasources: {
                main: { kind: "mysql", mode: "managed_push", url: "$MYSQL_URL" },
            },
        }),
    );
    assert.equal(document.format, "iris.project");
    assert.equal(document.version, 1);
    assert.equal(document.schema, "schemas/**/*.iris");
    assert.equal(document.datasources.main.kind, "mysql");
    assert.equal(document.generate.out, ".");
    assert.equal(document.generate.target, "typescript");
});

test("loadAuthoringConfig reads iris.config.ts", async () => {
    const dir = await mkdtemp(join(tmpdir(), "iris-config-"));
    const configPath = join(dir, "iris.config.ts");
    await writeFile(
        configPath,
        `export default { schema: "schemas/app.iris", datasources: { default: { kind: "sqlite", mode: "managed_push", path: ":memory:" } } };`,
        "utf8",
    );
    const loaded = await loadAuthoringConfig(configPath);
    assert.equal(loaded.schema, "schemas/app.iris");
    assert.equal(resolveProjectRoot(dir), dir);
    assert.equal(findAuthoringConfig(dir), configPath);
});

test("loadAuthoringConfig reads erasable TypeScript from iris.config.mts", async () => {
    const dir = await mkdtemp(join(tmpdir(), "iris-config-mts-"));
    const configPath = join(dir, "iris.config.mts");
    await writeFile(
        configPath,
        [
            `interface LocalConfig { schema: string }`,
            `const config: LocalConfig = { schema: "schemas/app.iris" };`,
            `export default config;`,
        ].join("\n"),
        "utf8",
    );
    const loaded = await loadAuthoringConfig(configPath);
    assert.equal(loaded.schema, "schemas/app.iris");
});

test("config execution failure keeps code, path, and cause", async () => {
    const dir = await mkdtemp(join(tmpdir(), "iris-config-throw-"));
    const configPath = join(dir, "iris.config.mts");
    await writeFile(configPath, `throw new Error("config boom");`, "utf8");

    await assert.rejects(loadAuthoringConfig(configPath), (error: unknown) => {
        assert.ok(error instanceof IrisConfigError);
        assert.equal(error.code, "config-load-failed");
        assert.equal(error.configPath, configPath);
        assert.equal(error.args.reason, "failed to execute project config");
        assert.ok(error.cause instanceof Error);
        assert.match(error.cause.message, /config boom/);
        return true;
    });
});

test("config without schema reports config-invalid", async () => {
    const dir = await mkdtemp(join(tmpdir(), "iris-config-invalid-"));
    const configPath = join(dir, "iris.config.mts");
    await writeFile(configPath, `export default {};`, "utf8");

    await assert.rejects(loadAuthoringConfig(configPath), (error: unknown) => {
        assert.ok(error instanceof IrisConfigError);
        assert.equal(error.code, "config-invalid");
        assert.equal(error.configPath, configPath);
        return true;
    });
});
