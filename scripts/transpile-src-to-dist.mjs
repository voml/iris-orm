#!/usr/bin/env node
/**
 * Transpile package src .ts files to dist .js (ESM).
 * Copies src .mjs files into dist unchanged.
 *
 * Usage: node scripts/transpile-src-to-dist.mjs [packageRoot]
 */
import { createRequire } from "node:module";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const ts = require("typescript");

const pkgRoot = process.argv[2] ? resolve(process.argv[2]) : process.cwd();
const srcRoot = join(pkgRoot, "src");
const distRoot = join(pkgRoot, "dist");

if (!existsSync(srcRoot)) {
    console.error(`transpile-src-to-dist: missing src/ under ${pkgRoot}`);
    process.exit(1);
}

function walk(dir, visit) {
    for (const name of readdirSync(dir)) {
        const full = join(dir, name);
        const st = statSync(full);
        if (st.isDirectory()) {
            walk(full, visit);
        } else {
            visit(full);
        }
    }
}

function rewriteModuleSpecifiers(js) {
    return String(js ?? "")
        .replace(/(from\s*['"])(\.[^'"]+)\.tsx?(['"])/g, "$1$2.js$3")
        .replace(/(import\s*\(\s*['"])(\.[^'"]+)\.tsx?(['"]\s*\))/g, "$1$2.js$3")
        .replace(/(import\s*['"])(\.[^'"]+)\.tsx?(['"])/g, "$1$2.js$3");
}

function transpileFile(absPath) {
    const source = readFileSync(absPath, "utf8");
    const out = ts.transpileModule(source, {
        fileName: absPath,
        compilerOptions: {
            module: ts.ModuleKind.ESNext,
            target: ts.ScriptTarget.ES2022,
            moduleResolution: ts.ModuleResolutionKind.Bundler,
            esModuleInterop: true,
            skipLibCheck: true,
        },
    });
    return rewriteModuleSpecifiers(out.outputText || "");
}

let tsCount = 0;
let mjsCount = 0;

walk(srcRoot, (file) => {
    const rel = relative(srcRoot, file);
    if (file.endsWith(".d.ts")) {
        return;
    }
    if (file.endsWith(".ts")) {
        const outPath = join(distRoot, rel.replace(/\.tsx?$/i, ".js"));
        mkdirSync(dirname(outPath), { recursive: true });
        writeFileSync(outPath, transpileFile(file), "utf8");
        tsCount += 1;
        return;
    }
    if (file.endsWith(".mjs")) {
        const outPath = join(distRoot, rel);
        mkdirSync(dirname(outPath), { recursive: true });
        copyFileSync(file, outPath);
        mjsCount += 1;
    }
});

const pkgName = (() => {
    try {
        return JSON.parse(readFileSync(join(pkgRoot, "package.json"), "utf8")).name;
    } catch {
        return pkgRoot;
    }
})();

console.log(`${pkgName} dist ok → ${distRoot} (${tsCount} ts, ${mjsCount} mjs)`);
