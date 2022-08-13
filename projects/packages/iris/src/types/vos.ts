/**
 * Public VOS modelling surface re-exported for Iris authors.
 *
 * `@voml/vos` is descriptors and types only — no parser, no WASM and no native
 * code — so generated clients never load a semantic core to model data. VOS
 * owns the builtin, wrapper and declaration semantics; Iris keeps owning the
 * entity-specific domain types, ids and payloads built on top of them.
 */

export { v } from "@voml/vos";
export type {
    AnyTypeDescriptor,
    Attribute,
    BuiltinKind,
    ClassDescriptor,
    FieldAccess,
    FieldDescriptor,
    FieldMap,
    Infer,
    ItemKind,
    ItemMetadataKey,
    NamedFieldMap,
    NamedItemDescriptor,
    NamedItemKind,
    ObjectDescriptor,
    RefDescriptor,
    ReferenceKey,
    ReferenceTarget,
    RowOf,
    ScalarKind,
    StructuralDescriptor,
    TableDescriptor,
    TypeDescriptor,
    VBuiltinName,
    VItemName,
    VWrapperName,
    VosFileRef,
    WrapperKind,
} from "@voml/vos";
