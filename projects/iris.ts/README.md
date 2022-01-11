# Iris TypeScript (`projects/iris.ts`)

Packages under this tree are **members of the repo-root pnpm workspace**
(`iris-orm/pnpm-workspace.yaml` → `projects/iris.ts/*`). Install from the
repository root:

```bash
pnpm install
pnpm run typecheck:ts
pnpm run iris -- --help
```

## Publish surface

| Package | Role |
| --- | --- |
| `@yydb/iris` | User facade + **`iris` CLI** (`cac`) |
| `@yydb/iris-types` | Internal types (pairs with Rust `iris-types`) |
| `@yydb/iris-adapter-sqlite` | SQLite via **sql.js** (WASM; not better-sqlite3) |
| `@yydb/iris-adapter-postgres` | PostgreSQL (peer → prefer `@yydb/postgres` when ready) |
| `@yydb/iris-adapter-mysql` | MySQL (peer → prefer `@yydb/mysql` when ready) |
| `@yydb/iris-adapter-redis` | Redis (peer → prefer `@yydb/redis` when ready) |
| `@yydb/iris-adapter-web` | Browser **Local Web Backend** (IndexedDB + OPFS); W0 skeleton — not YYDB, not SQL/SQLite |

Drivers are **peerDependencies** — install only the adapters you need. Prefer
JS/TS/WASM clients; avoid native Node addons.

- Runtime is native TypeScript — no N-API/FFI of Rust Iris.
- Generate / CLI live in this tree (TS Dejavu + `cac`); no Rust `iris` bin required.
- VOS language: `@game-gpt/vos-parser` / `@game-gpt/vos-ast` (not `@yydb/vos`).
- Adapter SQL/Redis commands stay **private** to adapter packages.
