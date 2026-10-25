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
 * semantic core) are the browser line and must stay allowed.
 */
const DESKTOP_SPECIFIER = /@yydb\/iris(?![-\w])/;

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
