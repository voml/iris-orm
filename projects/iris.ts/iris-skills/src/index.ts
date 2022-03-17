/**
 * Official skill catalog for `@yydb/iris-skills`.
 *
 * Skills teach Iris/VOS consumer workflows. They do not implement the planner.
 * Prefer real `@yydb/iris` CLI commands over planned Agent tool DTOs.
 */

export type SkillDelivery = "docs-only" | "cli-stub" | "cli-backed" | "tool-live";

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
        description:
            "Author/check Iris .iris schemas with iris check. Use when editing VOS schema — never invent SQL DDL.",
        skillMd: "skills/iris-schema/SKILL.md",
        delivery: "cli-backed",
    },
    {
        id: "iris-operation",
        name: "iris-operation",
        description:
            "Runtime VOS queries / generated Iris clients. Use instead of SQL or raw drivers for Iris tables.",
        skillMd: "skills/iris-operation/SKILL.md",
        delivery: "docs-only",
    },
    {
        id: "iris-generate",
        name: "iris-generate",
        description:
            "Local iris generate for host bindings; commit outputs. Never generate in deploy/CI.",
        skillMd: "skills/iris-generate/SKILL.md",
        delivery: "cli-backed",
    },
    {
        id: "iris-migrate",
        name: "iris-migrate",
        description:
            "iris push --plan / iris push for managed_push DDL. Never SQL/mysql2 or server-startup migrate.",
        skillMd: "skills/iris-migrate/SKILL.md",
        delivery: "cli-backed",
    },
    {
        id: "iris-explain",
        name: "iris-explain",
        description:
            "Read Iris planner/capability explain before risky migrate or when a VOS query is rejected.",
        skillMd: "skills/iris-explain/SKILL.md",
        delivery: "docs-only",
    },
    {
        id: "iris-topology",
        name: "iris-topology",
        description:
            "Composite topology Authority/projection/watermark checks. Do not invent fromRedis app APIs.",
        skillMd: "skills/iris-topology/SKILL.md",
        delivery: "docs-only",
    },
    {
        id: "iris-diagnose",
        name: "iris-diagnose",
        description:
            "Diagnose iris push/generate/execute failures; fix upstream Iris — no SQL Studio bypass.",
        skillMd: "skills/iris-diagnose/SKILL.md",
        delivery: "cli-backed",
    },
    {
        id: "iris-conformance",
        name: "iris-conformance",
        description:
            "Run host Iris conformance suites for adapter/capability evidence — not app feature work.",
        skillMd: "skills/iris-conformance/SKILL.md",
        delivery: "docs-only",
    },
] as const;

/** Planned Agent tool ids — not live; see skills/references/tool-protocol.md */
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

/** Live consumer CLI surface agents should teach */
export const IRIS_LIVE_CLI = [
    "iris check --config iris.von",
    "iris push --config iris.von --source main [--plan]",
    "iris generate --config iris.von --target <host>",
] as const;

export function listIrisSkills(): readonly IrisSkillMeta[] {
    return IRIS_SKILLS;
}

export function getIrisSkill(id: string): IrisSkillMeta | undefined {
    return IRIS_SKILLS.find((s) => s.id === id);
}
