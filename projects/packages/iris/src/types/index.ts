export { IrisFacadeError } from "./errors.ts";
export type {
    IrisBindingHost,
    IrisBindingProfile,
    IrisCapabilities,
    IrisCapabilityMatrix,
    IrisDurability,
    IrisHost,
    IrisRuntime,
    IrisStorageProfile,
} from "./binding.ts";
export type {
    IrisExecutionCapabilities,
    IrisLimitsCapabilities,
    IrisStorageCapabilities,
    IrisTransactionCapabilities,
} from "./capabilities.ts";
export type {
    IrisDiagnostic,
    IrisDiagnosticSeverity,
    OperationIdentity,
    OperationRequest,
    ResolvedContract,
    ResultEnvelope,
} from "./contract.ts";
export type { OperationExecutor } from "./operation-executor.ts";
export type { IrisOperation, IrisScalar, IrisWhereEq } from "./operation.ts";
export { defaultDurability, normalizeIrisHost, resolveStorageProfile } from "./profile.ts";
export type { IrisPlaceholder } from "./placeholder.ts";
export type { CheckSourceResult } from "./check-source.ts";
export type { SchemaFieldModel, SchemaIntrospection, SchemaMacroModel, SchemaTableModel } from "./schema-introspection.ts";
export type { ExecuteResult, IrisRow, IrisSession, IrisSessionProfile, OpenSessionOptions } from "./session.ts";
export type { ExecutionResult, ExecutionRow, ExecutionWireResult } from "./execution-result.ts";
export type {
    CreateIrisDbBindingOptions,
    CreateIrisExecutorOptions,
    IrisDbBinding,
    IrisExecutor,
    VosParameters,
} from "./executor.ts";
export type {
    IrisDatasourceConfig,
    IrisDatasourceKind,
    IrisGenerateConfig,
    IrisProjectDocument,
    IrisTruthMode,
    IrisUserConfig,
} from "./config.ts";
export { defineConfig, toProjectDocument } from "./config.ts";
export type { IrisTooling } from "./tooling.ts";
