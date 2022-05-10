#!/usr/bin/env node
/**
 * `iris` CLI — Node-only dev entry (`package.json` scripts / tests).
 */
import { runCli } from "../src/cli/cli.ts";

const code = await runCli(process.argv);
process.exit(code);
