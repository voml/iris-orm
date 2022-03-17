---
name: iris-migrate
description: >-
  Apply Iris schema to a database with managed_push via npm iris CLI
  (iris push --plan / iris push). Use for DDL only — never SQL scripts,
  mysql2, or server-startup migrate.
---

# iris-migrate

Read [../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- Schema files changed and the **physical DB** must catch up
- Migrate plan review before apply
- Diagnose empty plan / drift after `iris push`

## Do today (real CLI)

From the app package that owns `iris.von` (needs `MYSQL_URL` / datasource env):

```bash
# Plan only — writes a plan artifact; does not apply
iris push --config iris.von --source main --plan

# Apply + verify (managed_push)
iris push --config iris.von --source main
```

Typical npm scripts in apps:

```bash
pnpm migrate:plan
pnpm migrate
```

Flow:

```text
edit .iris → iris check → iris push --plan → review → iris push
```

## Rules

1. **Iris owns DDL.** `managed_push` plans `CreateTable` / `AddField` / … via adapters. If a change cannot be planned, **fix iris-orm upstream** — do not bypass.
2. **Review destructive intent** before apply. Chat “确认” is not a substitute for reading the plan artifact.
3. **Never** run migrate from HTTP server startup or request handlers.
4. Shared DBs may report unrelated physical tables; Iris may ignore them — do not invent SQL to “clean” foreign tables unless the user explicitly owns that ops task **outside** Iris product workflow.
5. After successful push, regenerate host bindings if schema fingerprint / types changed (`iris-generate`, local only).

## Antiforwards (most common agent failures)

| Wrong | Right |
|-------|--------|
| Hand `ALTER TABLE` / SQL file | Edit `.iris` + `iris push` |
| `mysql2` / raw driver to add columns | Upstream adapter fix + npm bump |
| `cargo run` private migrate bin as product path when app standardized on npm CLI | Use `iris push` from `@yydb/iris` |
| Path-patch iris in CI so migrate “works” | Publish / pin git `dev` or npm; no sibling paths in deploy |
| Deploy pipeline runs migrate automatically without ops intent | Explicit ops; keep DDL out of app boot |

## Planned structured tools (not live)

`migration.plan` / `migration.review` / `migration.apply` DTOs are **not** Agent tools yet. Until then, **only** the CLI above is authoritative. Do not fake MCP tool calls.

## Not this skill

| Need | Skill |
|------|--------|
| Schema authoring / check | `iris-schema` |
| Binding generation | `iris-generate` |
| Drift / doctor | `iris-diagnose` |
