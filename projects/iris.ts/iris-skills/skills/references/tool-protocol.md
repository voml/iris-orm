# Tool protocol

## Correct workflow

See [workflow.md](./workflow.md) — CLI (local) vs runtime (deploy).

## Live today (CLI — use these)

| Command | Skill | Where |
|---------|--------|--------|
| `iris check --config iris.von` | `iris-schema` | local |
| `iris push … [--plan]` | `iris-migrate` | local / ops — **never CI** |
| `iris generate …` | `iris-generate` | local → **commit `generated/`** |
| Host VOS execute / generated client | `iris-operation` | **runtime** |

See [consumer-hard-rules.md](./consumer-hard-rules.md).

## Planned structured Agent tools (not live)

Until Iris tool DTOs freeze and CLI/MCP share them, **do not call these as tools**:

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

## Parallel product

`@yydb/sql-studio-skills` — SQL Studio only. No VOS↔SQL bridge for Iris apps.
