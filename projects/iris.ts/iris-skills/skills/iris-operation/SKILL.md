---
name: iris-operation
description: >-
  Write Iris runtime data access via generated clients (Rust Db / TS db.user)
  and escape-hatch query/execute. Prefer generated CRUD over hand-written VOS
  strings. Use instead of SQL, query builders, or raw drivers.
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
Agent proposes intent
  → generated client (primary) or escape-hatch VOS text
  → Iris planner (capability + IR)
  → Adapter executes
```

### Rust (generated-first)

After `iris generate --target rust` (commit `generated/`):

```rust
let db = generated::Db::new(&mysql_source);
// Pool path
let rows = db.goods().find_many(&GoodsWhere { sku_id: Some("r50".into()), ..Default::default() })?;
// Shared-DB tests: always ROLLBACK
db.with_rollback(|txn| {
    txn.goods().insert(&row)?;
    let got = txn.goods().find_unique(&GoodsWhere { sku_id: Some(id), ..Default::default() })?;
    assert!(got.is_some());
    Ok(())
})?;
```

Rules for Rust:

1. **Primary path** = generated `Db` / `DbTxn` delegates (`find_many` / `find_unique` / `insert` / `update` / `delete`).
2. Escape hatch only: `db.query("…")` / `db.execute("…")` (or `Session::query` on the reference store). **Do not** teach hand-written VOS strings as the normal CRUD API.
3. Prefer **`.filter(x => …)`** inside any escape-hatch VOS. Do **not** teach SQL-style `.where(…)`.
4. Inside `transaction` / `with_rollback`, use **`DbTxn`** (or adapter `*_on`) — never pool-level `insert` / `execute_plan` (different connection).
5. New primary keys: **`iris::uuid()`** (v7), never `Uuid::new_v4()`.

This thin CRUD is a **TS-parity shim** (synthesizes VOS). It is **not** knife-B `GeneratedCall` / identity IR.

### TypeScript

- Prefer `db.user.findMany({ where: { … } })` from generate.
- Escape hatch: `db.$query` / `db.$execute`.

## Rules

1. **Edit VOS / call Iris**, not SQL and not `@yydb/sql-studio-orm`.
2. Unsupported capability → surface Iris diagnostics; **no silent SQL fallback**.
3. Do not inject adapter-private SQL/commands through the app.
4. Keep DDL out of request paths (`iris-migrate` only).

## Antiforwards

| Wrong | Right |
|-------|--------|
| `sqlx::query!("SELECT …")` on Iris tables | VOS / generated Iris API |
| Hand-written `query("Goods.filter…")` as the app CRUD layer | Generated `db.goods().find_many` |
| Pool `insert` / `execute_plan` inside `transaction` | `DbTxn` / `insert_on` / `execute_plan_on` |
| Invent `fromRedis` / dual-write helpers in app | Topology / Iris composite contracts |
| New examples using `.where(…)` like SQL | `.filter(x => …)` (or generated where struct) |

## Planned tools (not live)

`operation.check` is not an Agent tool yet. Validate by compiling, running host tests, and reading Iris errors.

## Not this skill

| Need | Skill |
|------|--------|
| Schema / check | `iris-schema` |
| Explain plans | `iris-explain` |
| Migrate | `iris-migrate` |
