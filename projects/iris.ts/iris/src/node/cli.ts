#!/usr/bin/env node
/**
 * TypeScript-host `iris` CLI entry — same brand command as Rust `iris-tools`.
 */
import { createIrisCli } from "./create-cli.ts";

createIrisCli().parse();
