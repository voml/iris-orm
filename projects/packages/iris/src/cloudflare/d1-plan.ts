import type { OperationRequest } from "../types/contract.ts";

/** Build-time D1 read plan entry (generated into client `_internal/d1-plans.ts`). */
export interface D1PhysicalPlan {
    readonly sql: string;
    readonly mode: "read";
    /** OperationRequest parameter keys bound to SQL `?` placeholders in order. */
    readonly paramOrder?: readonly string[];
}

export type D1PlanRegistry = Readonly<Record<string, D1PhysicalPlan>>;

/** Resolve a plan variant from operationId and runtime parameter keys. */
export function resolveD1Plan(
    plans: D1PlanRegistry,
    operationId: string,
    parameters?: Readonly<Record<string, unknown>>,
): D1PhysicalPlan | undefined {
    const paramKeys = parameters ? Object.keys(parameters).sort() : [];
    if (paramKeys.length > 0) {
        const variantKey = `${operationId}@${paramKeys.join(",")}`;
        const variant = plans[variantKey];
        if (variant) {
            return variant;
        }
    }
    return plans[operationId];
}

/** Coerce parameter values for D1/SQLite wire shapes. */
export function coerceD1BindValue(value: unknown): unknown {
    if (typeof value === "boolean") {
        return value ? 1 : 0;
    }
    return value;
}

/** Bind OperationRequest parameters onto a D1 prepared statement in plan order. */
export function bindD1Parameters(
    plan: D1PhysicalPlan,
    parameters?: Readonly<Record<string, unknown>>,
): readonly unknown[] {
    if (!plan.paramOrder?.length) {
        return [];
    }
    return plan.paramOrder.map((key) => coerceD1BindValue(parameters?.[key]));
}

/** @internal Test helper for plan resolution from a full request. */
export function resolveD1PlanForRequest(plans: D1PlanRegistry, request: OperationRequest): D1PhysicalPlan | undefined {
    return resolveD1Plan(plans, request.identity.operationId, request.parameters);
}
