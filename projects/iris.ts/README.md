# Iris TypeScript (`projects/iris.ts`)

**Site:** [iris-orm.pages.dev](https://iris-orm.pages.dev/)

Packages under this tree are **members of the repo-root pnpm workspace**
(`iris-orm/pnpm-workspace.yaml` → `projects/iris.ts/*`). Install from the repository root:

```bash
pnpm install
pnpm run typecheck:ts
pnpm run napi:build && pnpm run wasm:build
pnpm run test:ts
pnpm run iris -- --help   # workspace 内 CLI（根 package.json script）
# 裸 `iris` 不在 PATH；可选：pnpm exec iris / pnpm --filter @yydb/iris iris
```

## Publish surface

| Package                     | Role                                                                |
|-----------------------------|---------------------------------------------------------------------|
| `@yydb/iris-homepage`       | VMZ official site — [iris-orm.pages.dev](https://iris-orm.pages.dev/) |
| `@yydb/iris`                | Host facade (browser default + `/node` + `/types`) + **`iris` CLI** |
| `@yydb/iris-win32-x64`      | Optional Windows x64 N-API binary                                   |
| `@yydb/iris-linux-x64`      | Optional Linux x64 N-API binary                                     |
| `@yydb/iris-unknown-wasm32` | Optional browser WASM binary                                        |
| `@yydb/iris-skills`         | Agent Skills catalog                                                |

There are **no** `@yydb/iris-adapter-*` npm packages. Foreign-store lowering and drivers live in the **Rust** workspace
(`projects/iris.rs/iris-adapter-*`) and are reached through **N-API / WASM**, not parallel TypeScript adapters.

- Rust Iris is the sole runtime semantic implementation.
- Node uses N-API (`@yydb/iris/node` + optional platform `.node` packages) and the `iris` CLI.
- Browsers use `@yydb/iris` (default) + optional `@yydb/iris-unknown-wasm32` WASM.
- TypeScript must not implement a parallel parser, planner, optimizer, consistency model, fingerprint algorithm, or
  diagnostic system.

## `@yydb/iris` entry points

| Import             | Host                       | Contents                                                                              |
|--------------------|----------------------------|---------------------------------------------------------------------------------------|
| `@yydb/iris`       | Browser / Worker (default) | `initIris()` → `createIris()` → `version()` / `checkSource()` / `openSession()` |
| `@yydb/iris/node`  | Node only                  | `createIris()`, `loadProject()`, CLI helpers; non-Node → `unsupported.ts`       |
| `@yydb/iris/types` | Any (no loader)            | protocol DTOs only (no semantic implementations)                                |

```ts
import type { IrisRuntime } from "@yydb/iris/types";
import { createIris, initIris } from "@yydb/iris";
import { createIris } from "@yydb/iris/node";

await initIris();
const iris = await createIris();
iris.checkSource(schemaSource);
```

No `@yydb/iris/web`, `@yydb/iris/wasm`, `@yydb/iris-core`, or `@yydb/iris-adapter-*`.
