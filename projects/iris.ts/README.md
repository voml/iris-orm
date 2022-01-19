# Iris TypeScript (`projects/iris.ts`)

Packages under this tree are **members of the repo-root pnpm workspace**
(`iris-orm/pnpm-workspace.yaml` → `projects/iris.ts/*`). Install from the repository root:

```bash
pnpm install
pnpm run typecheck:ts
pnpm run iris -- --help
```

## Publish surface

| Package                       | Role                                                                                     |
|-------------------------------|------------------------------------------------------------------------------------------|
| `@yydb/iris`                  | User facade + **`iris` CLI** (`cac`)                                                     |
| `@yydb/iris-types`            | Internal types (pairs with Rust `iris-types`)                                            |
| `@yydb/iris-adapter-sqlite`   | SQLite via **sql.js** (WASM; not better-sqlite3)                                         |
| `@yydb/iris-adapter-postgres` | PostgreSQL (peer → prefer `@yydb/postgres` when ready)                                   |
| `@yydb/iris-adapter-mysql`    | MySQL (peer → prefer `@yydb/mysql` when ready)                                           |
| `@yydb/iris-adapter-redis`    | Redis (peer → prefer `@yydb/redis` when ready)                                           |
| `@yydb/iris-adapter-web`      | Browser **Local Web Backend** (IndexedDB + OPFS); W0 skeleton — not YYDB, not SQL/SQLite |

Drivers are **peerDependencies** — install only the adapters you need.

- Rust Iris is the sole runtime semantic implementation.
- Node uses an N-API binding behind the `@yydb/iris` facade and `iris` CLI; users do not need the standalone Rust `iris`
  executable.
- Browsers use browser-safe WebAssembly for semantic computation. IndexedDB, OPFS, Web Locks, BroadcastChannel, quota,
  and lifecycle integration remain TypeScript host responsibilities.
- WASI is not a supported target. Do not design browser APIs around WASI filesystem or networking assumptions.
- TypeScript may expose VOS types and codegen surfaces, but must not implement a parallel parser, planner, optimizer,
  consistency model, fingerprint algorithm, or diagnostic system.
- Adapter SQL/Redis commands stay **private** to adapter packages.

## `@yydb/iris` entry points (target)

Formal docs and codegen should use **explicit subpaths** (not runtime `window` checks):

```ts
import { createIris } from "@yydb/iris/node";
import { createIris, initIris } from "@yydb/iris/web";
import type { IrisRuntime } from "@yydb/iris";
```

Binding binaries ship as optional platform packages (`@yydb/iris-win32-x64`, `@yydb/iris-linux-x64`, `@yydb/iris-unknown-wasm32`). Full `exports` contract lives in the maintainer monorepo doc `决策和进度表/iris-orm-architecture.md` (not shipped inside this git tree).

Current workspace skeleton still uses a single `./src/index.ts` export until N-API / WASM loaders land.
