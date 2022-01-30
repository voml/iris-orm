# Iris TypeScript (`projects/iris.ts`)

Packages under this tree are **members of the repo-root pnpm workspace**
(`iris-orm/pnpm-workspace.yaml` → `projects/iris.ts/*`). Install from the repository root:

```bash
pnpm install
pnpm run typecheck:ts
pnpm run napi:build && pnpm run wasm:build
pnpm run test:ts
pnpm run iris -- --help
```

## Publish surface

| Package                     | Role                                                                |
|-----------------------------|---------------------------------------------------------------------|
| `@yydb/iris-homepage`       | VMZ official site (`projects/iris.ts/homepage`)                     |
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
| `@yydb/iris`       | Browser / Worker (default) | `initIris()` → `createIris()`; WASM via `@yydb/iris-unknown-wasm32`                   |
| `@yydb/iris/node`  | Node only                  | `createIris()`, `loadNativeBinding()`, `createIrisCli()`; non-Node → `unsupported.ts` |
| `@yydb/iris/types` | Any (no loader)            | protocol types + `version` (no semantic `checkSource`; use bindings)                  |

```ts
import type {IrisRuntime} from "@yydb/iris/types";
import {createIris, initIris} from "@yydb/iris";
import {createIris} from "@yydb/iris/node";
```

No `@yydb/iris/web`, `@yydb/iris/wasm`, `@yydb/iris-core`, or `@yydb/iris-adapter-*`.
