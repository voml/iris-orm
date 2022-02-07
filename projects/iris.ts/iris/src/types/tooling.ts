import type { CheckSourceResult } from "./check-source.ts";
import type { SchemaIntrospection } from "./schema-introspection.ts";

/** CLI / agent tooling surface (not application ORM). */
export interface IrisTooling {
    checkSchema(source: string): CheckSourceResult;
    introspectSchema(source: string): SchemaIntrospection;
}
