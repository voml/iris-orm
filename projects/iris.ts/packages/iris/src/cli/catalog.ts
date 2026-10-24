import type { LocaleCatalog } from "@vmz/commander";

/** English CLI catalog for `iris` (`@vmz/commander` help ids). */
export const IRIS_CLI_CATALOG: LocaleCatalog = {
    "iris.intro": "Iris ORM — schema check, client generate, and managed push",
    "iris.opt.version": "Print version",
    "iris.opt.config": "Project root or iris.config.ts",
    "iris.opt.out": "Generated client root (defaults to iris.config.ts generate.out)",
    "iris.opt.target": "Emitter target (default: typescript)",
    "iris.opt.source": "Datasource name (default: default)",
    "iris.opt.plan": "Plan only (no apply)",
    "iris.cmd.check": "Validate schema and generated client drift",
    "iris.cmd.generate": "Generate Iris client from .iris schema",
    "iris.cmd.push": "Push local schema to datasource (schema to database)",
    "iris.cmd.doctor": "Local diagnostics (config and environment)",
    "iris.cmd.capabilities": "Print datasource capability summaries",
};
