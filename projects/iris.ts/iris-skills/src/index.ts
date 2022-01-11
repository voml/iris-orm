/**
 * Official skill catalog for `@yydb/iris-skills`.
 * Authority: Spark `决策和进度表/iris-orm-architecture.md` §1.3.
 *
 * Skills teach Iris/VOS workflows. They do not implement the planner/runtime.
 * Until structured tool DTOs freeze, delivery stays docs-only / CLI-stub.
 */

export type SkillDelivery = "docs-only" | "cli-stub" | "tool-live";

export type IrisSkillMeta = {
    readonly id: string;
    readonly name: string;
    readonly description: string;
    readonly skillMd: string;
    readonly delivery: SkillDelivery;
};

export const IRIS_SKILLS: readonly IrisSkillMeta[] = [
    {
        id: "iris-schema",
        name: "iris-schema",
        description: "Read/check VOS schema, identity, references, and diagnostics. Use when editing or validating Iris schemas.",
        skillMd: "skills/iris-schema/SKILL.md",
        delivery: "docs-only",
    },
    {
        id: "iris-operation",
        name: "iris-operation",
        description: "Author or modify VOS operations/queries and run semantic checks. Use instead of inventing SQL for Iris apps.",
        skillMd: "skills/iris-operation/SKILL.md",
        delivery: "docs-only",
    },
    {
        id: "iris-generate",
        name: "iris-generate",
        description: "Host Dejavu generate with fingerprint/drift checks. Use when regenerating Iris bindings.",
        skillMd: "skills/iris-generate/SKILL.md",
        delivery: "cli-stub",
    },
    {
        id: "iris-migrate",
        name: "iris-migrate",
        description: "VOS logical migration plan/review/apply with plan-hash binding. Use for Iris schema migrations.",
        skillMd: "skills/iris-migrate/SKILL.md",
        delivery: "docs-only",
    },
    {
        id: "iris-explain",
        name: "iris-explain",
        description: "Inspect capability proof, routing, and physical/composite plans. Use before authorized apply/execute.",
        skillMd: "skills/iris-explain/SKILL.md",
        delivery: "docs-only",
    },
    {
        id: "iris-topology",
        name: "iris-topology",
        description: "Verify Authority/projection/outbox/watermark contracts for Composite Backend. Use for topology checks.",
        skillMd: "skills/iris-topology/SKILL.md",
        delivery: "docs-only",
    },
    {
        id: "iris-diagnose",
        name: "iris-diagnose",
        description: "Doctor, adapter errors, drift, and consistency diagnostics for Iris. Use when Iris operations fail.",
        skillMd: "skills/iris-diagnose/SKILL.md",
        delivery: "cli-stub",
    },
    {
        id: "iris-conformance",
        name: "iris-conformance",
        description: "Run selected Iris conformance fixtures and collect evidence. Use for adapter/host verification.",
        skillMd: "skills/iris-conformance/SKILL.md",
        delivery: "docs-only",
    },
] as const;

export const IRIS_PLANNED_TOOLS = [
    "schema.check",
    "operation.check",
    "generate.plan",
    "generate.apply",
    "migration.plan",
    "migration.review",
    "migration.apply",
    "plan.explain",
    "topology.verify",
    "projection.verify",
    "doctor",
    "conformance.run",
] as const;

export function listIrisSkills(): readonly IrisSkillMeta[] {
    return IRIS_SKILLS;
}

export function getIrisSkill(id: string): IrisSkillMeta | undefined {
    return IRIS_SKILLS.find((s) => s.id === id);
}
