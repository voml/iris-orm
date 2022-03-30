---
name: iris-migrate
description: >-
  Apply Iris schema DDL with npm iris CLI (iris push --plan / iris push).
  Local or explicit ops only — NEVER CI, Docker, or server startup. Never SQL
  or mysql2 bypass.
---

# iris-migrate

Read [../references/workflow.md](../references/workflow.md) and
[../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- `.iris` changed and a **chosen** database must catch up
- Reviewing a plan before apply

## Do today

```bash
iris push --config iris.von --source main --plan
iris push --config iris.von --source main
# or: pnpm migrate:plan / pnpm migrate
```

```text
edit .iris → iris check → iris generate (commit) → push --plan → push
```

## Hard boundary

| Allowed | Forbidden |
|---------|-----------|
| Developer / ops shell, intentional target DB | GitHub Actions, TCB build, Dockerfile `RUN`, container entrypoint |
| After human review of plan | Auto-migrate on every deploy |
| | HTTP server boot calling `managed_push` |

App demo **seed** (DML) is the same safety class: **local/ops only**, never CI/runtime — not DDL, still never automate in deploy.

## Antiforwards

| Wrong | Right |
|-------|--------|
| CI/TCB runs migrate | Human `iris push` when needed |
| Hand `ALTER TABLE` / mysql2 | Edit `.iris` + `iris push`; upstream if adapter gap |
| Path-patch iris so CI migrate “works” | Publish / pin; no sibling paths on deploy |

## Planned tools (not live)

`migration.plan` / `review` / `apply` DTOs are not Agent tools. Use CLI only.
