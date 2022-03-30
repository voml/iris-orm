/** One field in a schema introspection result. */
export interface SchemaFieldModel {
    name: string;
    rustTy: string;
    vosType: string;
    primary: boolean;
    optional: boolean;
    /** Target entity for `&T` reference fields. */
    referenceTarget?: string;
}

/** One table in a schema introspection result. */
export interface SchemaTableModel {
    name: string;
    rustType: string;
    fields: SchemaFieldModel[];
}

/** One macro in a schema introspection result. */
export interface SchemaMacroModel {
    name: string;
    returnType: string;
}

/** Read-only schema introspection (`GenerationModel` shape). */
export interface SchemaIntrospection {
    ok: boolean;
    generatorVersion: string;
    schemaFingerprint: string;
    tables: SchemaTableModel[];
    macros?: SchemaMacroModel[];
    error?: string | null;
}
