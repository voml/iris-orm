import { execSync } from "node:child_process";
import { existsSync, unlinkSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const crateDir = join(root, "projects/iris.rs/crates/iris-wasm");
const outDir = join(root, "projects/iris.ts/platforms/iris-unknown-wasm32/src/lib");

execSync(`wasm-pack build "${crateDir}" --target web --out-dir "${outDir}" --release`, {
    cwd: root,
    stdio: "inherit",
    shell: true,
});

// wasm-pack drops its own publish scaffolding; the platform package owns these files.
for (const name of [".gitignore", "package.json", "README.md", "LICENSE"]) {
    const path = join(outDir, name);
    if (existsSync(path)) {
        unlinkSync(path);
    }
}
