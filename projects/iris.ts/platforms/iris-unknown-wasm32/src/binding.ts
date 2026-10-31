/**
 * Web contract shared by the WASM semantic core and `@yydb/iris-language`.
 *
 * Desktop hosts bind through the N-API adapter in `@yydb/iris`, so this module
 * deliberately carries only the browser / Worker surface: the semantic core
 * bindings, the in-memory session wire, capability negotiation inputs, and the
 * generated-client execution ABI. Nothing here may import the desktop package.
 */

/** Result of validating a VOS / `.iris` schema source via the WASM core. */
export interface CheckSourceResult {
    ok: boolean;
    tableCount: number;
    schemaFingerprint: string;
    generatorVersion: string;
    error?: string | null;
}

/** Wire execute payload from a WASM memory session. */
export type WebSessionExecuteWire = string | { ok: boolean; rowsJson: string; error?: string | null };

/** In-memory VOS session exposed by the WASM semantic core. */
export interface WebMemorySessionBinding {
    /** DML entry (preferred). Aligns with generated `db.$query`. */
    query ? (source: string, parametersJson?: string | null) : WebSessionExecuteWire;
    /** DDL / unit entry (preferred). Aligns with generated `db.$execute`. */
    execute ? (source: string, parametersJson?: string | null) : WebSessionExecuteWire;
    /** Legacy DML alias kept for older native builds. */
    executeVos(source: string, parametersJson?: string | null): WebSessionExecuteWire;
    executeOperation ? (operationJson: string) : WebSessionExecuteWire;
    managedPush ? (schema: string) : void;
    close(): void;
}

/** Semantic core surface available to browser / Worker hosts. */
export interface WebIrisBindings {
    irisVersion(): string;
    checkSource(source: string): CheckSourceResult;
    introspectSchema(source: string): string;
    openMemorySession(): WebMemorySessionBinding;
}

/** Runtime that owns the WASM binding (desktop hosts are out of scope). */
export type WebIrisHost = "browser" | "cloudflare-worker";

/** Storage / durability profile (orthogonal to `WebIrisHost`). */
export type WebIrisStorageProfile = "memory" | "local-fs" | "d1";

/** Binding options profile before host negotiation. */
export type WebIrisBindingProfile = "memory" | "sqlite" | "project";

/** Durability class negotiated for the active profile. */
export type WebIrisDurability = "ephemeral" | "local-durable" | "remote-durable";

/** Execution surface negotiated for the active host. */
export interface WebIrisExecutionCapabilities {
    readonly native: boolean;
    readonly wasm: boolean;
    readonly async: boolean;
    readonly cancellation: boolean;
}

/**
 * Storage backends the Web binding can claim.
 *
 * Browser OPFS persistence is a YYDB host adapter concern. Iris must not wire
 * OPFS or other web database APIs; consumers use YYDB when persistence is needed.
 */
export interface WebIrisStorageCapabilities {
    readonly memory: boolean;
    readonly localFs: boolean;
    /** Always false: Iris does not implement OPFS. */
    readonly opfs: boolean;
    readonly d1: boolean;
}

/** Transaction semantics exposed by the active profile. */
export interface WebIrisTransactionCapabilities {
    readonly local: boolean;
    readonly batch: boolean;
    readonly crossRequest: boolean;
}

/** Host-advertised resource limits (optional until enforced). */
export interface WebIrisLimitsCapabilities {
    readonly maxBatchStatements?: number;
    readonly maxPayloadBytes?: number;
    readonly maxResultBytes?: number;
}

/** Full capability matrix for the active Web host + profile pair. */
export interface WebIrisCapabilityMatrix {
    readonly host: WebIrisHost;
    readonly profile: WebIrisStorageProfile;
    readonly durability: WebIrisDurability;
    /** Whether the semantic core binding finished loading. */
    readonly bindingReady: boolean;
    readonly execution: WebIrisExecutionCapabilities;
    readonly storage: WebIrisStorageCapabilities;
    readonly transaction: WebIrisTransactionCapabilities;
    readonly limits: WebIrisLimitsCapabilities;
}

/** One field in a schema introspection result. */
export interface WebSchemaFieldModel {
    name: string;
    rustTy: string;
    vosType: string;
    primary: boolean;
    optional: boolean;
    /** Target entity for `&T` reference fields. */
    referenceTarget?: string;
}

/** One table in a schema introspection result. */
export interface WebSchemaTableModel {
    name: string;
    rustType: string;
    fields: WebSchemaFieldModel[];
}

/** One macro in a schema introspection result. */
export interface WebSchemaMacroModel {
    name: string;
    returnType: string;
}

/** Read-only schema introspection (`GenerationModel` shape). */
export interface WebSchemaIntrospection {
    ok: boolean;
    generatorVersion: string;
    schemaFingerprint: string;
    tables: WebSchemaTableModel[];
    macros?: WebSchemaMacroModel[];
    error?: string | null;
}

/** Session profile for `openSession` on a Web host. */
export type WebIrisSessionProfile = "memory";

/** Options for opening a Web session (WASM in-memory ReferenceStore only). */
export interface WebOpenSessionOptions {
    /** Adapter profile (browser / Worker hosts support `memory` only). */
    profile?: WebIrisSessionProfile;
}

/** One logical row returned from VOS execution. */
export type WebIrisRow = Record<string, string | number | boolean | null>;

/** Result of executing VOS source on a session. */
export interface WebExecuteResult {
    ok: boolean;
    rows: WebIrisRow[];
    error?: string | null;
}

/** Symmetric session contract for the browser / Worker host. */
export interface WebIrisSession {
    /** Plan and execute VOS source on the bound adapter. */
    execute(source: string): WebExecuteResult;
    /** Structured operation ABI (generated client path). */
    executeOperation ? (request: WebOperationRequest) : WebExecuteResult;
    /** Release session resources. */
    close(): void;
    /** Debug / schema bootstrap only. */
    managedPush ? (schema: string) : void;
}

/** Binding bring-up host returned by the browser runtime facade. */
export interface WebIrisBindingHost {
    readonly host: WebIrisHost;
    readonly capabilities: WebIrisCapabilityMatrix;
    version(): string;
    /** Tooling: validate schema source (CLI / agents). */
    checkSource(source: string): CheckSourceResult;
    /** Tooling: introspect GenerationModel JSON (codegen / drift). */
    introspectSchema(source: string): WebSchemaIntrospection;
    /** Debug / VOS console session. Not the generated client query API. */
    openSession(options?: WebOpenSessionOptions): WebIrisSession;
}

/**
 * JS carrier a generated value table can promise for one field.
 *
 * `bigint` and `Uint8Array` are carriers, not conveniences: VOS `i64` must not be
 * flattened into a lossy JS `number`, and VOS `bytes` stays binary.
 * `Float32Array` carries VOS `vector` components, and the inline descriptor
 * carries VOS `file` identity without degrading to a tagged JSON string.
 */
export type WebWireCarrierValue =
    | string
    | number
    | boolean
    | bigint
    | Uint8Array
    | Float32Array
    | {
          readonly digest: string;
          readonly octets: bigint;
          readonly name?: string;
          readonly mediaType?: string;
          readonly manifest: { readonly hash: string; readonly size: bigint };
      }
    | null;

/** Row payload from a successful VOS DML execution (binding wire shape). */
export type WebExecutionRow = Record<string, WebWireCarrierValue>;

/** Binding wire result before host value mapping. */
export type WebExecutionWireResult =
    | {
          kind: "rows";
          rows: readonly WebExecutionRow[];
      }
    | {
          kind: "affected";
          affected: number;
      }
    | {
          kind: "value";
          value: unknown;
      }
    | {
          kind: "unit";
      };

/** VOS parameter bindings for direct text execution. */
export type WebVosParameters = Readonly<Record<string, unknown>>;

/** Scalar wire values for structured Iris operations. */
export type WebIrisScalar = string | number | boolean | null;

/** Single-field equality filter (current operation ABI). */
export interface WebIrisWhereEq {
    field: string;
    value: WebIrisScalar;
}

/** Stable structured operation passed from generated client to runtime ABI. */
export type WebIrisOperation =
    | {
          kind: "find-many";
          entity: string;
          where?: WebIrisWhereEq;
          take?: number;
      }
    | {
          kind: "find-unique";
          entity: string;
          where: WebIrisWhereEq;
      }
    | {
          /** Build-time synthesized VOS (parameters live on `WebOperationRequest.parameters`). */
          kind: "declared-vos";
          source: string;
      };

/** Stable operation identity within a resolved contract. */
export interface WebOperationIdentity {
    readonly operationId: string;
    readonly contractFingerprint: string;
}

/** Host-neutral operation request (generated client primary ABI). */
export interface WebOperationRequest {
    readonly identity: WebOperationIdentity;
    readonly operation: WebIrisOperation;
    readonly parameters?: Readonly<Record<string, unknown>>;
    readonly deadlineMs?: number;
}

/** Diagnostic severity carried by execution envelopes. */
export type WebIrisDiagnosticSeverity = "error" | "warning";

/** Structured diagnostic returned with results or failures. */
export interface WebIrisDiagnostic {
    readonly code: string;
    readonly message: string;
    readonly severity: WebIrisDiagnosticSeverity;
    readonly retryable?: boolean;
    readonly unknownCommit?: boolean;
}

/** Typed execution envelope for async `WebOperationExecutor`. */
export type WebResultEnvelope<T = unknown> =
    | {
          ok: true;
          value: T;
          diagnostics?: readonly WebIrisDiagnostic[];
      }
    | {
          ok: false;
          diagnostics: readonly WebIrisDiagnostic[];
      };

/** Async operation execution contract (target runtime ABI). */
export interface WebOperationExecutor {
    execute(request: WebOperationRequest): Promise<WebResultEnvelope<readonly WebExecutionRow[]>>;
    /** Unit-valued / DDL-shaped declared operations. */
    executeUnit(request: WebOperationRequest): Promise<WebResultEnvelope<void>>;
    close(): Promise<void>;
}

/**
 * Internal runtime support consumed by generated browser `db` (not an application entry).
 *
 * Generated code synthesizes VOS text; Rust owns parse, validation, planning,
 * binary encoding, and execution.
 */
export interface WebIrisDbBinding {
    /** DML entry: returns the VOS operation value (host-mapped). */
    query(source: string, parameters?: WebVosParameters): Promise<unknown>;
    /** DDL entry: returns VOS unit (mapped to void in TypeScript). */
    execute(source: string, parameters?: WebVosParameters): Promise<void>;
    close(): Promise<void>;
}

/** Generated-client wiring options for the browser / Worker host. */
export interface WebCreateIrisDbBindingOptions {
    profile?: WebIrisBindingProfile;
    sqlitePath?: string;
    /** Project root or `iris.config.ts` path. */
    config?: string;
    /** @deprecated Use `config` (project root or `iris.config.ts`). */
    project?: string;
    /** Datasource name from `iris.config.ts` (`default` when omitted). */
    source?: string;
    /** Managed-push schema **data** text. */
    schema?: string;
    /** Build-time contract fingerprint from generated metadata. */
    contractFingerprint?: string;
    /** Wire-name map from generated `metadata.ts` (`IRIS_FIELD_WIRE_NAMES`). */
    wireNamesByEntity?: Readonly<Record < string, Readonly < Record < string, string >>> >;
    /** VOS value-carrier map from generated `metadata.ts` (`IRIS_VALUE_CARRIERS`). */
    valueCarriersByEntity?: Readonly<Record < string, Readonly < Record < string, string >>> >;
}
