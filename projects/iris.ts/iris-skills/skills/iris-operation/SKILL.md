---
name: iris-operation
description: >-
  Write Iris runtime data access as VOS queries / generated clients
  (Session::query / db.$query, typed APIs). Use instead of SQL, query builders,
  or raw drivers for Iris-managed tables.
---

# iris-operation

Read [../references/workflow.md](../references/workflow.md) and
[../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- Implement list/get/update/insert against Iris-managed tables
- Replace ad-hoc SQL in an Iris app
- Debug empty results / plan errors from VOS execution

## Do today

```text
Agent proposes VOS intent (string or generated client)
  → Iris planner (capability + IR)
  → Adapter executes
```

In Rust apps that wrap Iris:

- Prefer **generated** table types + helpers when present.
- Or thin host wrappers with **VOS pipeline style**:
  `query("Goods.filter(x => x.sku_id == \"r50\").collect()")`.
- Prefer **`.filter(x => …)`**. Do **not** teach SQL-style `.where(…)` in new code
  (planner may accept `.where` as a compatibility alias of `.filter`, but the
  canonical surface is `.filter`).
- Escape hatch names: Rust `Session::query` / `Session::execute` ↔ TS `db.$query` / `db.$execute`
  (do **not** invent a third name; legacy `execute_vos` is deprecated).
- New primary keys: **`iris::uuid()`** (v7), never `Uuid::new_v4()`.

Escape user strings for VOS string literals with the app’s escape helper (e.g. `escape_vos_str`) — do not concatenate raw SQL.

## Rules

1. **Edit VOS / call Iris**, not SQL and not `@yydb/sql-studio-orm`.
2. Unsupported capability → surface Iris diagnostics; **no silent SQL fallback**.
3. Do not inject adapter-private SQL/commands through the app.
4. Keep DDL out of request paths (`iris-migrate` only).

## Antiforwards

| Wrong | Right |
|-------|--------|
| `sqlx::query!("SELECT …")` on Iris tables | VOS / generated Iris API |
| Invent `fromRedis` / dual-write helpers in app | Topology / Iris composite contracts |
| “Just use mysql2 for this one query” | Fix Iris / express in VOS |
| New examples using `.where(…)` like SQL | `.filter(x => …)` |

## Planned tools (not live)

`operation.check` is not an Agent tool yet. Validate by compiling, running host tests, and reading Iris errors.

## Not this skill

| Need | Skill |
|------|--------|
| Schema / check | `iris-schema` |
| Explain plans | `iris-explain` |
| Migrate | `iris-migrate` |
