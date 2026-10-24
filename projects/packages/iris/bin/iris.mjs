#!/usr/bin/env node
/**
 * `iris` CLI — published bin entry (loads the compiled CLI from `dist`).
 */
const { runCli } = await import("../dist/cli/cli.js");

const code = await runCli(process.argv);
process.exit(code);
