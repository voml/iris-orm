/** Backend kind declared in `iris.config.ts` (no connection secrets). */
export type IrisDatasourceKind =
    | "reference"
    | "yydb"
    | "yyds"
    | "sqlite"
    | "postgres"
    | "mysql"
    | "redis";

/** How Iris treats schema truth for a datasource. */
export type IrisTruthMode = "native_pull" | "managed_push" | "adopt_existing";

/** One named datasource binding in `iris.config.ts`. */
export interface IrisDatasourceConfig {
    kind: IrisDatasourceKind;
    mode: IrisTruthMode;
    /** Filesystem path or URL template. May reference `$ENV_VAR` segments. */
    path?: string;
    /** Optional URL template (postgres/mysql/redis), env-expandable. */
    url?: string;
}

/** Optional generate defaults in `iris.config.ts`. */
export interface IrisGenerateConfig {
    /** Output directory relative to the project root. */
    out?: string;
    /** Emitter target (`typescript`, `rust`, …). */
    target?: string;
}

/**
 * Authoring config for `iris.config.ts`.
 *
 * `schema` is a pointer to schema **data** on disk (file, directory, or glob) — not inline VOS.
 */
export interface IrisUserConfig {
    /** Relative path, directory, or glob to `.iris` schema data. */
    schema: string;
    datasources?: Readonly<Record<string, IrisDatasourceConfig>>;
    generate?: IrisGenerateConfig;
}

/** Wire document materialized for the Rust core (not an authoring surface). */
export interface IrisProjectDocument {
    format: "iris.project";
    version: 1;
    schema: string;
    datasources: Record<string, IrisDatasourceConfig>;
    generate: {
        out: string;
        target: string;
    };
}

/** Identity helper for `iris.config.ts` inference. */
export function defineIrisConfig(config: IrisUserConfig): IrisUserConfig {
    if (!config.schema?.trim()) {
        throw new Error("iris.config.ts: `schema` must point at on-disk .iris data");
    }
    return config;
}

/** Convert authoring config to the Rust project document shape. */
export function toProjectDocument(config: IrisUserConfig): IrisProjectDocument {
    return {
        format: "iris.project",
        version: 1,
        schema: config.schema,
        datasources: { ...(config.datasources ?? {}) },
        generate: {
            out: config.generate?.out ?? "generated/iris",
            target: config.generate?.target ?? "typescript",
        },
    };
}
