# Hosts & bindings

## Layers

```text
Rust Iris core
  ├─ iris-generator (Rust codegen)
  ├─ iris-connector-* / iris-adapter-* (Rust workspace lowering)
  ├─ Node N-API → @yydb/iris/node + `iris` CLI
  └─ browser WASM → @yydb/iris-language
```

TypeScript must **not** re-implement parser, planner, optimizer, or diagnostics in parallel.

## npm surface

| Package                   | Role                                      |
|---------------------------|-------------------------------------------|
| `@yydb/iris`              | Default entry: project config helpers     |
| `@yydb/iris/node`         | Node N-API facade + `iris` CLI            |
| `@yydb/iris-language`     | Browser / Worker host + WASM loader       |
| `@yydb/iris/runtime`      | Host-agnostic binding helpers (no WASM)   |
| `@yydb/iris/types`        | Protocol / binding DTOs (no loaders)      |
| `@yydb/iris-win32-x64`, … | Optional platform binaries                |

There is no `@yydb/iris/web` or `@yydb/iris/wasm` subpath, and no standalone `@yydb/iris-core` npm package.

## Retired TS product surface

`@yydb/iris-adapter-{postgres,mysql,redis,sqlite,web}` **have been removed** from the repo. Node foreign-store execution
and CLI live commands go through **Rust N-API + Rust `iris-adapter-*`** only; the public npm surface is `@yydb/iris`
plus optional platform packages.

## WASI

WASI is **frozen** as a public contract. `@yydb/iris-unknown-wasm32` is browser-safe WASM, not WASI filesystem or
networking.

## Verify separation (local)

The homepage is a **browser** host (VMZ static). It must not bundle `@yydb/iris/node` into the client graph. From the
repo root:

```bash
pnpm run verify:homepage-hosts   # browser export map + full iris verify
pnpm run verify:iris-exports     # @yydb/iris package only
```

Expected when `iris.win32-x64-msvc.node` is built on Windows:

| Check                                                             | Result                                                               |
|-------------------------------------------------------------------|----------------------------------------------------------------------|
| `import "@yydb/iris"` (browser or node resolve conditions)        | always → `src/browser/` (config helpers only)                        |
| `import "@yydb/iris/node"` under **browser** conditions (bundler) | → `unsupported.ts`                                                   |
| `import "@yydb/iris/node"` under **node** conditions              | → `src/node/index.ts`                                                |
| `loadNativeBinding()`                                             | loads optional `.node`; `irisVersion()` → `0.1.0`                    |
| `initIris()` (`@yydb/iris-language`)                              | loads the WASM core from `@yydb/iris-unknown-wasm32`                  |
| `iris` CLI                                                        | requires `pnpm install --filter @yydb/iris`; then `doctor` / `check` |

On Node, `import "@yydb/iris/node"` correctly resolves to the N-API facade. Bundlers using browser conditions get
`unsupported.ts` instead — that is intentional.

## Rust native

Rust apps use the `iris::*` crate directly. The `iris` CLI ships from `@yydb/iris` on Node hosts.
