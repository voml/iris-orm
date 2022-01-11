---
name: iris-operation
description: >-
  Author or modify VOS operations/queries and run semantic checks. Use for Iris
  data access intent — Iris validates IR, capability, and plans; do not invent SQL.
---

# iris-operation

## Path

```text
Agent proposes VOS intent
  -> Iris semantic check
  -> capability proof / optimization
  -> explain / physical plan
  -> policy-gated execute (when tools exist)
```

## Planned tools

- `operation.check`
- `plan.explain` (see iris-explain)

## Rules

1. Edit VOS, not SQL.
2. Do not execute via `@yydb/sql-studio-orm`.
3. Do not inject private adapter commands.
4. Unsupported capability → surface Iris diagnostics; no silent SQL fallback.
