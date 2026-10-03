#!/usr/bin/env node
/**
 * `iris` CLI — published bin entry (loads TypeScript CLI via jiti).
 */
import { fileURLToPath } from "node:url";

import { createJiti } from "jiti";

const jiti = createJiti(fileURLToPath(import.meta.url), {
    interopDefault: true,
    moduleCache: false,
});

const { runCli } = await jiti.import("../src/cli/cli.ts");
const code = await runCli(process.argv);
process.exit(code);
