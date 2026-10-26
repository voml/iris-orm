import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const pkgRoot = fileURLToPath(new URL("..", import.meta.url));

/**
 * Desktop specifier only: `@yydb/iris` or `@yydb/iris/<subpath>`.
 *
 * `@yydb/iris-language` (this package) and `@yydb/iris-unknown-wasm32` (the Web
 * semantic core) are the browser line and must stay allowed for runtime
 * values only (dynamic import + WASM function re-exports), never for types.
 */
const DESKTOP_SPECIFIER = /@yydb\/iris(?![-\w])/;

/** Static `import type` or `import` specifier that must not resolve to the
 *  WASM semantic core package — types are defined locally in `web-contract.ts`. */
const WASM_TYPE_SPECIFIER = /import\s+type\s*\{[^}]*\}\s+from\s*["']@yydb\/iris-unknown-wasm32(?:\/[^"']*)?["']/s;
const WASM_TYPE_IMPORT_LINE = /import\s+type\s+.*from\s*["']@yydb\/iris-unknown-wasm32/;

function collectFiles(dir: string): string[] {
    if (!existsSync(dir)) {
        return [];
    }
    const files: string[] = [];
    for (const name of readdirSync(dir)) {
        const full = join(dir, name);
        if (statSync(full).isDirectory()) {
            files.push(...collectFiles(full));
        } else if (/\.(?:[cm]?ts|[cm]?js)$/.test(name)) {
            files.push(full);
        }
    }
    return files;
}

test("package.json no longer depends on the desktop package", () => {
    const source = readFileSync(join(pkgRoot, "package.json"), "utf8");
    assert.doesNotMatch(source, DESKTOP_SPECIFIER);
});

test("tsconfig.json has no desktop path mapping", () => {
    const source = readFileSync(join(pkgRoot, "tsconfig.json"), "utf8");
    assert.doesNotMatch(source, /\.\.\/iris\/src/);
    assert.doesNotMatch(source, DESKTOP_SPECIFIER);
});

test("src and dist never reference the desktop package", () => {
    const srcFiles = collectFiles(join(pkgRoot, "src"));
    assert.ok(srcFiles.length > 0, "expected src files to scan");

    for (const dirName of ["src", "dist"]) {
        for (const file of collectFiles(join(pkgRoot, dirName))) {
            const source = readFileSync(file, "utf8");
            assert.doesNotMatch(
                source,
                DESKTOP_SPECIFIER,
                `${relative(pkgRoot, file)} must not reference the desktop package`,
            );
        }
    }
});

test("src never statically imports types from @yydb/iris-unknown-wasm32", () => {
    const srcFiles = collectFiles(join(pkgRoot, "src"));
    assert.ok(srcFiles.length > 0, "expected src files to scan");

    for (const file of srcFiles) {
        const source = readFileSync(file, "utf8");
        assert.doesNotMatch(
            source,
            WASM_TYPE_IMPORT_LINE,
            `${relative(pkgRoot, file)} must not statically import types from @yydb/iris-unknown-wasm32 — use local ./web-contract.ts`,
        );
    }
});

test("src/web-contract.ts defines the Web binding contract locally", () => {
    const contract = readFileSync(join(pkgRoot, "src", "web-contract.ts"), "utf8");
    assert.ok(
        contract.includes("export interface CheckSourceResult"),
        "web-contract.ts must define CheckSourceResult",
    );
    assert.ok(
        contract.includes("export interface WebIrisBindings"),
        "web-contract.ts must define WebIrisBindings",
    );
    assert.ok(
        contract.includes("export interface WebMemorySessionBinding"),
        "web-contract.ts must define WebMemorySessionBinding",
    );
    assert.ok(
        contract.includes("export interface WebCreateIrisDbBindingOptions"),
        "web-contract.ts must define WebCreateIrisDbBindingOptions",
    );
});

test("runtime WASM function re-exports still come from @yydb/iris-unknown-wasm32", () => {
    const indexSource = readFileSync(join(pkgRoot, "src", "index.ts"), "utf8");
    assert.match(
        indexSource,
        /export\s*\{[^}]*\}\s+from\s*["']@yydb\/iris-unknown-wasm32["']/s,
        "index.ts must still re-export WASM runtime values from @yydb/iris-unknown-wasm32",
    );
});
