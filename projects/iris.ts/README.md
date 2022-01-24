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
| `@yydb/iris-homepage`         | VMZ 0.1.8 official site (`projects/iris.ts/homepage`)                                  |
| `@yydb/iris`                  | Host facade (browser default + `/node` + `/types`) + **`iris` CLI**                      |
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

## `@yydb/iris` entry points

| Import | Host | Contents |
|--------|------|----------|
| `@yydb/iris` | Browser / Worker (default) | `initIris()` → `createIris()`; WASM lives inside optional `@yydb/iris-unknown-wasm32` |
| `@yydb/iris/node` | Node only (`node` export condition) | `createIris()`, `loadProject()`, `loadNativeBinding()`, `createIrisCli()`; non-Node resolves to `unsupported.ts` |
| `@yydb/iris/types` | Any (no loader) | `IrisRuntime`, `checkSource`, `version` |

CLI: `iris` bin → `@yydb/iris/node` (`check` uses TS `checkSource`; `doctor` prints host diagnostics; `generate`/`capabilities` stub until N-API).

```ts
import type { IrisRuntime } from "@yydb/iris/types";
import { createIris, initIris } from "@yydb/iris";
import { createIris } from "@yydb/iris/node";
```

Adapters import `@yydb/iris/types` only. No `@yydb/iris/web`, `@yydb/iris/wasm`, or `@yydb/iris-core`.
