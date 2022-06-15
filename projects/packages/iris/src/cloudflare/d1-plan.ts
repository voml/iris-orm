/** Build-time D1 read plan entry (generated into client `_internal/d1-plans.ts`). */
export interface D1PhysicalPlan {
    readonly sql: string;
    readonly mode: "read";
}

export type D1PlanRegistry = Readonly<Record<string, D1PhysicalPlan>>;
