#!/usr/bin/env node
/**
 * `iris` CLI — plain JS entry (Node refuses --experimental-strip-types under node_modules).
 */
import { createRequire } from "node:module";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import cac from "cac";

import { resolveNativeProjectConfig } from "../src/node/config.mjs";

const require = createRequire(import.meta.url);
const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const pkg = JSON.parse(readFileSync(join(pkgRoot, "package.json"), "utf8"));

const PLATFORM_PACKAGES = {
    "win32-x64": "@yydb/iris-win32-x64",
    "linux-x64": "@yydb/iris-linux-x64",
    "linux-arm64": "@yydb/iris-linux-arm64",
    "darwin-x64": "@yydb/iris-darwin-x64",
    "darwin-arm64": "@yydb/iris-darwin-arm64",
};

function resolvePlatformPackage() {
    const override = process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
    if (override) return override;
    const key = `${process.platform}-${process.arch}`;
    const name = PLATFORM_PACKAGES[key];
    if (!name) {
        throw new Error(`@yydb/iris: no semantic core for ${key}`);
    }
    try {
        require.resolve(name);
    } catch {
        throw new Error(`@yydb/iris: install optional dependency ${name}`);
    }
    return name;
}

function loadCore() {
    const loaded = require(resolvePlatformPackage());
    return loaded?.default ?? loaded;
}

function notImplemented(name) {
    console.error(`iris ${name}: not implemented yet`);
    process.exitCode = 1;
}

async function resolveRuntimeConfig(input) {
    const core = loadCore();
    const resolved = await resolveNativeProjectConfig(input ?? process.cwd(), core);
    return resolved;
}

const cli = cac("iris");
cli.version(pkg.version || "0.0.0");
cli.help();

cli.command("check [schema]", "Validate schema")
    .option("--config <path>", "Project root or iris.config.ts")
    .action(async (schema, options) => {
        try {
            const core = loadCore();
            let source = schema;
            if (!source) {
                const project = await resolveRuntimeConfig(options?.config);
                const loaded = core.loadProject(project.runtimeConfig);
                source = core.readSchema(loaded.root, loaded.schemaGlob);
            } else {
                source = readFileSync(resolve(schema), "utf8");
            }
            const result = core.checkSource(source);
            if (!result.ok) {
                console.error(result.error || "check failed");
                process.exitCode = 1;
                return;
            }
            console.log(`ok tables=${result.tableCount} fingerprint=${result.schemaFingerprint}`);
        } catch (e) {
            console.error(e instanceof Error ? e.message : String(e));
            process.exitCode = 1;
        }
    });

cli.command("generate [schema]", "Generate client from .iris schema")
    .option("--config <path>", "Project root or iris.config.ts")
    .option("--out <dir>", "Output project root")
    .option("--target <name>", "Emitter target")
    .action(async (schema, options) => {
        try {
            const core = loadCore();
            let source;
            let target = options?.target;
            let outRoot = options?.out ? resolve(options.out) : null;
            if (schema) {
                source = readFileSync(resolve(schema), "utf8");
                target = target || "typescript";
                outRoot = outRoot || resolve(".");
            } else {
                const project = await resolveRuntimeConfig(options?.config);
                const loaded = core.loadProject(project.runtimeConfig);
                source = core.readSchema(loaded.root, loaded.schemaGlob);
                target = target || loaded.generateTarget || "typescript";
                outRoot = outRoot || resolve(loaded.root, loaded.generateOut);
            }
            const result = core.generate(source, target, outRoot);
            if (!result.ok) {
                console.error(`error: ${result.error || "generate failed"}`);
                process.exitCode = 1;
                return;
            }
            console.log(`generated ${target} client (${result.files.length} files, fingerprint=${result.schemaFingerprint})`);
            console.log(`  output: ${result.outputPath}`);
            for (const file of result.files) console.log(`  - ${file}`);
        } catch (e) {
            console.error(`error: ${e instanceof Error ? e.message : String(e)}`);
            process.exitCode = 1;
        }
    });

cli.command("push", "Push local schema to datasource")
    .option("--config <path>", "Project root or iris.config.ts")
    .option("--source <name>", "Datasource name", { default: "default" })
    .option("--out <dir>", "Plan output path or directory")
    .option("--plan", "Plan only (no apply)")
    .action(async (options) => {
        const source = options?.source || "default";
        try {
            const resolved = await resolveRuntimeConfig(options?.config);
            const config = resolved.runtimeConfig;
            const core = loadCore();
            if (options?.plan) {
                const result = core.migratePlanCmd(config, source, options?.out ?? null);
                if (!result.ok) {
                    console.error(`error: ${result.error || "push plan failed"}`);
                    process.exitCode = 1;
                    return;
                }
                console.log(`push plan written: ${result.planPath}`);
                return;
            }
            if (typeof core.migrateRunCmd !== "function") {
                notImplemented("push apply (upgrade @yydb/iris native core)");
                return;
            }
            const planOut = options?.out || null;
            const result = core.migrateRunCmd(config, source, planOut, false);
            if (!result.ok) {
                console.error(`error: ${result.error || "push failed"}`);
                process.exitCode = 1;
                return;
            }
            if (result.planOnly) {
                console.log(`push plan only: ${result.planPath}`);
                return;
            }
            const created = result.createdTables || [];
            console.log(
                created.length
                    ? `push ok — created: ${created.join(", ")}`
                    : "push ok — verify passed (no new tables)",
            );
        } catch (e) {
            console.error(`error: ${e instanceof Error ? e.message : String(e)}`);
            process.exitCode = 1;
        }
    });

cli.command("doctor", "Local diagnostics").action(() => {
    try {
        const core = loadCore();
        console.log(`@yydb/iris ${pkg.version}`);
        console.log(`semantic core: ${core.irisVersion()}`);
        console.log(`platform: ${process.platform}-${process.arch}`);
        const cfgTs = resolve("iris.config.ts");
        const cfgVon = resolve("iris.von");
        console.log(`iris.config.ts: ${existsSync(cfgTs) ? cfgTs : "(not in cwd)"}`);
        console.log(`iris.von (legacy): ${existsSync(cfgVon) ? cfgVon : "(not in cwd)"}`);
    } catch (e) {
        console.error(e instanceof Error ? e.message : String(e));
        process.exitCode = 1;
    }
});

cli.command("capabilities", "Print datasource capability summaries").action(() => {
    notImplemented("capabilities");
});

cli.parse();
