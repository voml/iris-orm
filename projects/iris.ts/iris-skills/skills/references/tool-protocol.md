# Tool protocol

## Live today (CLI — use these)

| Command | Skill |
|---------|--------|
| `iris check --config iris.von` | `iris-schema` |
| `iris push --config iris.von --source <id> [--plan]` | `iris-migrate` |
| `iris generate --config iris.von --target <host>` | `iris-generate` |
| Host VOS execute / generated client | `iris-operation` |

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

Mutating ops (when live): `plan -> review -> apply` with plan hash + fingerprints + short grant.

## Parallel product

`@yydb/sql-studio-skills` — SQL Studio only. No VOS↔SQL translation bridge for Iris apps.
