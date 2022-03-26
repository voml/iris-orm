# Iris correct workflow (all hosts)

Agents **must** follow this. Wrong habits (SQL bypass, CI migrate, embed `.iris`, deploy generate) are defects.

## Two surfaces — do not confuse

| Surface | What | When |
|---------|------|------|
| **Iris CLI / tools** (`@yydb/iris`: `check` / `push` / `generate`) | Developer laptop / explicit ops window | Schema change, DDL, regenerate bindings |
| **Iris runtime** (`iris` / adapters + **committed** `generated/`) | App binary after compile | Production / Docker / TCB — **only this** |

Deployed servers **never** invoke the Iris CLI. They only link Iris **runtime** libraries and call VOS / generated APIs.

## Canonical loop (local)

```text
1. Edit schemas/**/*.iris  (+ iris.von if needed)
2. iris check --config iris.von
3. iris generate --config iris.von --target <host>
      → write generated/  →  COMMIT generated/  (enters compile)
      → do NOT commit .cache/iris/*
4. iris push --config iris.von --source main --plan   # review
5. iris push --config iris.von --source main           # apply DDL — human ops only
6. (optional) app-local seed / admin bins — human ops only
7. cargo/pnpm build app → ship binary that uses runtime + generated
```

Typical npm scripts in an app package that owns `iris.von`:

```bash
pnpm check
pnpm generate      # then git add generated/
pnpm migrate:plan  # DDL plan only
pnpm migrate       # DDL apply — NEVER in CI/Docker/server boot
```

## What enters the binary

```text
YES:  generated/*  (structs, SCHEMA_FINGERPRINT, UUID_FIELDS, …)
YES:  Iris runtime crates (iris, iris-adapter-*, iris-types, …)
NO:   Iris CLI / iris-tools / iris-generator as a deploy dependency
NO:   re-parsing or embedding raw .iris at runtime
NO:   .cache/iris/*
```

If the runtime needs schema-derived metadata (e.g. MySQL uuid column map), **`iris generate` must emit it**. Do not invent `build.rs` embeds of `.iris` source.

## DDL and seed — development / ops only

| Action | Allowed | Forbidden |
|--------|---------|-----------|
| `iris push` / migrate | Explicit local or ops shell against an intentional target DB | CI, Docker build, container entrypoint, `farm-server` boot |
| Demo / fixture seed (app bin) | Explicit local run | CI, Docker, server boot, “auto seed on deploy” |
| `iris generate` | Local before commit | CI “to fix missing generated”, Docker `RUN pnpm generate` |

**Database safety:** CI must not mutate production (or shared) schemas. DDL and seed are **human-gated**.

## Runtime data access

```text
App → generated Db / db.user.findMany  (primary)
    → escape hatch: Session::query / db.$query  (rare)
    → Iris planner + adapter
    → DB
```

- **Rust primary path:** `iris generate --target rust` → `Db` / `DbTxn` typed CRUD.
  Do **not** teach hand-written `query("….filter…")` strings as normal CRUD.
- Escape hatch: Rust `query`/`execute` ↔ TS `$query`/`$execute` (legacy `execute_vos` deprecated).
- Pipeline predicates (escape hatch only): prefer **`.filter(x => …)`**. Do not teach SQL-style `.where`.
- MySQL tests against a shared DB: `Db::with_rollback` / `MysqlSource::with_rollback` + same-connection APIs.
  Never call pool-level `insert` / `execute_plan` inside a transaction callback.
- **No** SQL / `mysql2` / `sqlx` on Iris-managed tables.
- New `uuid` PKs: **`iris::uuid()`** (v7 only).

## Bug / gap handling

1. Capability missing in adapter → **fix upstream iris-orm**, publish npm / push `dev`, bump app.
2. Do **not** path-patch siblings into TCB/`master`.
3. Do **not** “finish” migrate with hand SQL.

## Skill map

| Need | Skill |
|------|--------|
| Edit / check `.iris` | `iris-schema` |
| DDL plan/apply | `iris-migrate` |
| Bindings | `iris-generate` |
| Queries / DML via Iris | `iris-operation` |
| Failures | `iris-diagnose` |

See also [consumer-hard-rules.md](./consumer-hard-rules.md) and [tool-protocol.md](./tool-protocol.md).
