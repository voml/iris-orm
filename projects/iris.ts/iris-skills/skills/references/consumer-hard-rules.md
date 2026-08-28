# Consumer hard rules (app repos)

These rules apply to **any app** that depends on npm `@yydb/iris` / Rust `iris` crates.
Skills must not contradict them.

**Read [workflow.md](./workflow.md) first** — canonical Iris loop (CLI vs runtime, generate→commit, DDL/seed local-only).

## Product surface today

| Intent | Command / API | Where |
|--------|---------------|--------|
| Schema check | `iris check --config iris.von` | app Iris project package |
| DDL plan | `iris push --config iris.von --source main --plan` | **local / ops only — never CI** |
| DDL apply | `iris push --config iris.von --source main` | **local / ops only — never CI / Docker / server boot** |
| Host bindings | `iris generate --config iris.von --target <host>` | **local only** → **commit `generated/`** |
| Demo seed (app-owned) | app local bin (e.g. `cargo run -p farm-database --bin farm-seed`) | **local / ops only — never CI / runtime** |
| Runtime query | host facade: VOS text / generated client | app server code |
| UUID PK | `iris::uuid()` / VOS `uuid()` — **v7 only** | app + Iris |

```text
local tool:  iris check | iris push | iris generate | (app seed/admin)
commit:      generated/*   (enters language compile)
ignore:      .cache/iris/*
deploy bin:  generated + Iris *runtime* only
             — no CLI, no iris-tools, no .iris embed, no migrate/seed on boot
```

## Hard antiforwards

1. **Never invent SQL** for Iris apps (DDL or DML). Edit `.iris` / VOS; use Iris push and `execute_vos` / generated clients.
2. **Never** `mysql2` / `sqlx` / hand SQL to “finish” Iris migrate or seed. Gap → **fix upstream iris-orm**, then bump the app.
3. **Never** `link:` / sibling path / `[patch]` to local iris/vos in **deployed** app trees (TCB/`master`).
4. **`iris generate` is local-only.** Commit `generated/`. Never commit `.cache/iris/*`. Never generate in CI/Docker/server boot. Never embed raw `.iris` as a substitute for generate metadata (`UUID_FIELDS`, etc.).
5. **DDL and seed are never CI or runtime.** No migrate/seed in GitHub Actions, TCB build, Dockerfile `RUN`, or HTTP server startup.
6. **Do not invent Agent tools** (`migration.apply`, …). Use the real CLI until DTOs ship.
7. **UUID v7 only** for Iris `uuid` PKs.
8. **Separate domains → separate `.iris` files.**
9. **Bug in Iris → upstream.** No vendored adapter patches in apps.

## Docker / single-binary hosts

- Image builds **`cargo build -p <server>`** (or equivalent) only — no `pnpm migrate`, no seed, no `pnpm generate`.
- `.dockerignore` must **not** strip committed `generated/`.
- Runtime image = server binary; Iris enters as **runtime crates + generated**, not as a tool.

## Parallel product

| Product | Use for |
|---------|---------|
| `@yydb/iris` / Iris skills | VOS schema, Iris migrate/generate, VOS queries |
| `@yydb/sql-studio-*` | SQL Studio only — **not** Iris app DDL/DML |
