---
name: iris-schema
description: >-
  Author and validate Iris VOS schemas (.iris). Use when creating tables,
  splitting domains, editing iris.von, or fixing schema check failures.
  Never invent SQL DDL for Iris apps.
---

# iris-schema

Read [../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- Add / rename / split tables in `schemas/**/*.iris`
- `iris check` fails
- Choosing field types (`uuid`, relations, status enums)

## Do today

1. Edit **`.iris` (VOS grammar)** under the app’s Iris project dir (often next to `iris.von`).
2. Run:

```bash
iris check --config iris.von
```

3. After schema shape changes that affect host bindings, run **local** `iris generate` (see `iris-generate`) and commit outputs if the app requires it.
4. Applying DDL to a live DB is **`iris-migrate`** (`iris push`), not this skill’s job.

## Schema rules

1. **Table names PascalCase** in VOS (e.g. `Goods`, `CartLine`).
2. **One domain per file** when domains differ (catalog ≠ cart ≠ gift ≠ order ≠ account). Avoid a single mega-`.iris`.
3. **`uuid` PKs are UUID v7** at insert time (`iris::uuid()` / VOS `uuid()`). Document fields as `uuid`; do not suggest v4 generators.
4. Prefer Iris / VOS types — do not invent parallel SQL column types in app code.
5. Prefer **structured diagnostics** from `iris check` over guessing.

## Antiforwards

- Do not write `CREATE TABLE` / `ALTER TABLE` / flyway / diesel migrations for Iris-managed tables.
- Do not “patch” missing columns with `mysql2` scripts.
- Do not point skills or README at private monorepo decision-doc paths; teach product commands only.

## Not this skill

| Need | Skill |
|------|--------|
| Push schema to DB | `iris-migrate` |
| Regenerate Rust/TS client | `iris-generate` |
| Runtime VOS queries | `iris-operation` |
