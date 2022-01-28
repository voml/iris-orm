#!/usr/bin/env node
/**
 * Verify @yydb/iris export separation: browser default, /node (Node-only), /types, CLI bin.
 * Run from @yydb/iris package root: node --experimental-strip-types ./scripts/verify-exports.mjs
 */
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { tmpdir } from "node:os";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const pkg = JSON.parse(readFileSync(join(pkgRoot, "package.json"), "utf8"));

function ok(label, detail = "") {
    console.log(`ok: ${label}${detail ? ` — ${detail}` : ""}`);
}

function fail(label, detail = "") {
    console.error(`FAIL: ${label}${detail ? ` — ${detail}` : ""}`);
    process.exitCode = 1;
}

function resolveExport(subpath, conditions) {
    const parent = pathToFileURL(join(pkgRoot, "package.json")).href;
    const specifier =
        subpath === "." ? pkg.name : `${pkg.name}${subpath.startsWith(".") ? subpath.slice(1) : `/${subpath}`}`;
    return import.meta.resolve(specifier, { parentURL: parent, conditions });
}

/** Bundler-style resolve: honour export map keys only (no implicit Node host). */
function resolveExportMap(subpath, conditions) {
    const entry = pkg.exports[subpath];
    if (!entry || typeof entry === "string") {
        throw new Error(`missing exports entry ${subpath}`);
    }
    for (const cond of conditions) {
        if (entry[cond]) {
            return join(pkgRoot, entry[cond].replace(/^\.\//, ""));
        }
    }
    if (entry.default) {
        return join(pkgRoot, entry.default.replace(/^\.\//, ""));
    }
    throw new Error(`unresolved ${subpath} for [${conditions.join(", ")}]`);
}

function assertIncludes(resolved, fragment, label) {
    const path = fileURLToPath(resolved).replace(/\\/g, "/");
    if (path.includes(fragment.replace(/\\/g, "/"))) {
        ok(label, path);
        return true;
    }
    fail(label, `expected '${fragment}' in ${path}`);
    return false;
}

console.log(`verify @yydb/iris exports (${process.platform}-${process.arch})\n`);

// --- export map resolution (Node conditions) ---
assertIncludes(
    resolveExport(".", ["node", "import"]),
    "/src/browser/",
    'import "@yydb/iris" with node conditions → browser facade',
);
assertIncludes(
    resolveExport(".", ["browser", "import"]),
    "/src/browser/",
    'import "@yydb/iris" with browser conditions → browser facade',
);
assertIncludes(
    resolveExport("./node", ["node", "import"]),
    "/src/node/index",
    'import "@yydb/iris/node" on Node → node facade',
);
const browserNodeEntry = resolveExportMap("./node", ["browser", "import"])
    .replace(/\\/g, "/");
if (browserNodeEntry.includes("/src/node/unsupported")) {
    ok('bundler graph: "@yydb/iris/node" → unsupported stub', browserNodeEntry);
} else {
    fail('bundler graph: "@yydb/iris/node" → unsupported stub', browserNodeEntry);
}
assertIncludes(
    resolveExport("./types", ["browser", "import"]),
    "/src/types/",
    'import "@yydb/iris/types" → protocol only',
);

// --- runtime: web default must not load N-API ---
try {
    const browser = await import(pathToFileURL(join(pkgRoot, "src/browser/index.ts")).href);
    try {
        await browser.initIris();
        fail("browser initIris", "should throw before binding ready");
    } catch (error) {
        const code = error?.code ?? "";
        if (code === "wasm-not-implemented" || code === "wasm-not-initialized") {
            ok("browser initIris stays on WASM path", code);
        } else {
            fail("browser initIris", String(error));
        }
    }
} catch (error) {
    fail("browser facade import", String(error));
}

// --- runtime: /node loads platform binding when present ---
try {
    if (!process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
        const localNode = join(pkgRoot, "../iris-win32-x64/iris.win32-x64-msvc.node");
        try {
            readFileSync(localNode);
            process.env.NAPI_RS_NATIVE_LIBRARY_PATH = localNode;
        } catch {
            // optional workspace package only
        }
    }
    const native = await import(pathToFileURL(join(pkgRoot, "src/node/native.ts")).href);
    const binding = await native.loadNativeBinding();
    ok("node loadNativeBinding", binding.packageName);
    ok("node iris_version", binding.module.irisVersion());
} catch (error) {
    const code = error?.code ?? "";
    if (code === "native-package-missing") {
        console.log("skip: node N-API binding (optional platform package missing)");
    } else {
        fail("node loadNativeBinding", String(error));
    }
}

// --- runtime: unsupported stub for non-Node /node resolution ---
try {
    const unsupported = await import(
        pathToFileURL(join(pkgRoot, "src/node/unsupported.ts")).href
    );
    try {
        unsupported.createIris();
        fail("unsupported createIris", "should throw");
    } catch (error) {
        if (error?.code === "node-host-required") {
            ok("browser /node stub throws node-host-required");
        } else {
            fail("unsupported createIris", String(error));
        }
    }
} catch (error) {
    fail("unsupported stub import", String(error));
}

// --- CLI bin (Node-only) ---
const cli = join(pkgRoot, "src/node/cli.ts");
const nodeBin = process.execPath;
const require = createRequire(join(pkgRoot, "package.json"));

let hasCac = false;
try {
    require.resolve("cac");
    hasCac = true;
} catch {
    console.log("skip: CLI (install deps: pnpm install --filter @yydb/iris)");
}

if (hasCac) {
    const help = spawnSync(nodeBin, ["--experimental-strip-types", cli, "--help"], {
        encoding: "utf8",
    });
    if (help.status === 0 && help.stdout.includes("iris")) {
        ok("CLI --help");
    } else {
        fail("CLI --help", help.stderr || help.stdout);
    }

    const doctor = spawnSync(nodeBin, ["--experimental-strip-types", cli, "doctor"], {
        encoding: "utf8",
    });
    if (doctor.status === 0 && doctor.stdout.includes("@yydb/iris/node")) {
        ok("CLI doctor");
    } else {
        fail("CLI doctor", doctor.stderr || doctor.stdout);
    }

    const tmp = mkdtempSync(join(tmpdir(), "iris-verify-"));
    const schema = join(tmp, "user.iris");
    writeFileSync(
        schema,
        `table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
`,
    );
    const check = spawnSync(nodeBin, ["--experimental-strip-types", cli, "check", schema], {
        encoding: "utf8",
    });
    rmSync(tmp, { recursive: true, force: true });
    if (check.status === 0 && check.stdout.includes("iris check: ok")) {
        ok("CLI check", check.stdout.trim().split("\n")[0]);
    } else {
        fail("CLI check", check.stderr || check.stdout);
    }
}

console.log(process.exitCode ? "\nverify-exports: FAILED" : "\nverify-exports: passed");
