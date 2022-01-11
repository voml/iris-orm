---
name: iris-migrate
description: >-
  VOS logical migration plan/review/apply with plan-hash binding. Use for Iris
  schema migrations — not SQL Studio migrations.
---

# iris-migrate

## Planned tools

- `migration.plan`
- `migration.review`
- `migration.apply`

```text
plan -> review -> apply
```

Apply requires plan hash, schema fingerprint, adapter/capability fingerprints,
and short-lived authorization. Input change invalidates the plan.

## Rules

1. Destructive changes never skip review.
2. Chat “确认执行” ≠ server/CLI policy.
3. Parallel track: SQL Studio migrations live under `sql-studio-migrate` only.
