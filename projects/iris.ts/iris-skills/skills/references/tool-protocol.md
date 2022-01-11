# Tool protocol (planned)

Authority: Spark `决策和进度表/iris-orm-architecture.md` §1.3.

Until Iris tool DTOs freeze and CLI/MCP share them, **these are not live Agent tools**:

```text
schema.check
operation.check
generate.plan / generate.apply
migration.plan / migration.review / migration.apply
plan.explain
topology.verify
projection.verify
doctor
conformance.run
```

Mutating ops: `plan -> review -> apply` with plan hash + fingerprints + short grant.

Parallel: `@yydb/sql-studio-skills` — SQL only; no VOS↔SQL translation.
