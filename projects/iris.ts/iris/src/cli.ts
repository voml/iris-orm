#!/usr/bin/env node
/**
 * TypeScript-host `iris` CLI entry — same brand command as Rust `iris-tools`.
 * Uses `cac` (same stack as `@dejavu/tools`); not hand-rolled argv/help.
 */
import { createIrisCli } from "./create-cli.ts";

createIrisCli().parse();
