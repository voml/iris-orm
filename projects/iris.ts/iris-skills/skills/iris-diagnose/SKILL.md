---
name: iris-diagnose
description: >-
  Diagnose Iris migrate/query/adapter failures and drift. Use when iris push,
  generate, or execute_vos fails. Fix upstream Iris — never switch to SQL Studio
  or raw SQL as a workaround.
---

# iris-diagnose

Read [../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- `iris push` empty plan but verify fails
- Missing columns / AddField / adapter gaps
- Runtime VOS errors, capability unsupported, connection failures
- Suspected schema fingerprint drift vs generated client

## Do today

1. **Evidence first**: full CLI stderr, plan artifact path, schema fingerprint, Iris version (`@yydb/iris` / git rev).
2. Classify:
   - unsupported capability / not-yet-implemented adapter step → **upstream iris-orm**
   - bad `.iris` / wrong config → `iris-schema` / fix `iris.von`
   - deploy missing generated/schemas → app packaging (`iris-generate`, Docker ignore, embed)
   - credentials / network → env, not schema hacks
3. Prefer `iris doctor` / host doctor when implemented; otherwise use check + push --plan + logs.
4. Redact secrets (`MYSQL_URL`, tokens) in reports.

## Antiforwards

| Wrong | Right |
|-------|--------|
| Bypass with SQL / mysql2 | Upstream fix + publish + bump |
| “Use SQL Studio ORM instead” | Stay on Iris; open Iris bug |
| Path-patch siblings to hide CI failure | Fix publish / lockfile / Dockerfile |
| Silence errors with empty catch + fake data | Surface diagnostics |

## Planned tools (not live)

`doctor` structured DTO may still be CLI-stub. Prefer real CLI output over inventing tool JSON.
