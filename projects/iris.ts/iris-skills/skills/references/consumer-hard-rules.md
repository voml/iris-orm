# Consumer hard rules (app repos)

These rules apply to **any app** that depends on npm `@yydb/iris` / Rust `iris` crates.
Skills must not contradict them.

## Product surface today

| Intent | Command / API | Where |
|--------|---------------|--------|
| Schema check | `iris check --config iris.von` | app Iris project package |
| DDL plan | `iris push --config iris.von --source main --plan` | **local / ops only** |
| DDL apply | `iris push --config iris.von --source main` | **local / ops only** |
| Host bindings | `iris generate --config iris.von --target <host>` | **local only** → **commit `generated/`** → enters host compile |
| Runtime query | host facade: VOS text / generated client | app server code |
| UUID PK | `iris::uuid()` / VOS `uuid()` — **v7 only** | app + Iris |

**Compile contract (all hosts):** develop with `iris generate` → generated code is normal source in the language toolchain → **commit generated output**. Do **not** commit `.cache/iris/*` (plans, temp). Do **not** embed raw `.iris` into binaries; generate must emit any runtime metadata (e.g. Rust `UUID_FIELDS`).

There is **no** public `query --sql`, raw SQL migration path, or mysql client bypass in the Iris product surface.

## Hard antiforwards

1. **Never invent SQL** for Iris apps (DDL or DML). Edit `.iris` / VOS; use Iris migrate/push and `execute_vos` / generated clients.
2. **Never** `mysql2` / `sqlx` / hand SQL / DBA scripts to “finish” Iris migrate, add columns, or seed schema. If Iris cannot do it → **fix upstream iris-orm**, publish npm / push git `dev`, then bump the app.
3. **Never** `link:` / sibling path / `[patch]` to local `iris-orm` or `vos-language` in **deployed** app Cargo/npm. CI/Docker has no siblings. Local path overlays are maintainer-only and must not reach TCB/`master`.
4. **`iris generate` is local-only.** Generated output **enters the host compile stream** and **must be committed**. **Never** commit `.cache/iris/*`. **Never** run generate in deploy/TCB/server startup. **Never** embed raw `.iris` source into `build.rs` — missing runtime metadata is a **generator** gap (fix upstream emit, e.g. `UUID_FIELDS`).
5. **DDL is not server startup.** HTTP servers must not call `managed_push` / migrate. Migrate is an explicit ops command.
6. **Do not invent Agent tools** named `migration.apply`, `schema.check`, etc. Until those DTOs ship, use the **real CLI** above. Docs that say “planned tools” are not live APIs.
7. **UUID v7 only** for Iris `uuid` PKs. No v4. Random UUIDs split InnoDB pages.
8. **Separate domains → separate `.iris` files** (e.g. catalog vs cart vs gift). Do not dump unrelated tables into one blob.
9. **Bug in Iris → upstream.** App repos do not vendor patches of Iris adapters.

## Docker / single-binary hosts

- Build context must include whatever `cargo`/`build.rs` needs (`generated/`, schemas for embed, etc.).
- `.dockerignore` must **not** strip Iris `generated/` if the build script requires it.
- Runtime images that ship only a binary must **not** rely on `CARGO_MANIFEST_DIR` disk schemas unless schemas are copied or compile-time embedded.

## Parallel product (do not confuse)

| Product | Use for |
|---------|---------|
| `@yydb/iris` / Iris skills | VOS schema, Iris migrate/generate, VOS queries |
| `@yydb/sql-studio-*` | SQL Studio only — **not** Iris app DDL/DML |
