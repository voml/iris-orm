#!/usr/bin/env node
/**
 * Published `iris` CLI bin — delegates to Node-only command wiring in `src/node/`.
 */
import { createIrisCli } from "../src/node/create-cli.ts";

createIrisCli().parse();
