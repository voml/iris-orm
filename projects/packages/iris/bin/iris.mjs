#!/usr/bin/env node
/**
 * `iris` CLI — published bin entry (loads the compiled CLI from `dist`).
 */
const MIN_NODE = [24, 12, 0];
const current = process.versions.node.split(".").map(Number);
const supported =
    current[0] > MIN_NODE[0] ||
    (current[0] === MIN_NODE[0] &&
        (current[1] > MIN_NODE[1] || (current[1] === MIN_NODE[1] && current[2] >= MIN_NODE[2])));
if (!supported) {
    console.error(`iris requires Node.js >=24.12.0 (found ${process.versions.node})`);
    process.exit(1);
}

const { runCli } = await import("../dist/cli/cli.js");

const code = await runCli(process.argv);
process.exit(code);
