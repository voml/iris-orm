---
title: Hosts & bindings
order: 2
---

# Hosts & bindings

## Layers

```text
Rust Iris core
  ├─ iris-tools / iris-generator (Rust CLI)
  ├─ iris-connector-* / iris-adapter-* (Rust workspace lowering)
  ├─ Node N-API → @yydb/iris/node
  └─ browser WASM → @yydb/iris (default entry)
```

TypeScript must **not** re-implement parser, planner, optimizer, or diagnostics in parallel.

## npm surface

| Package | Role |
| --- | --- |
| `@yydb/iris` | Default Web facade (WASM inside) |
| `@yydb/iris/node` | Node N-API facade + `iris` CLI |
| `@yydb/iris/types` | Protocol / binding DTOs (no loaders) |
| `@yydb/iris-win32-x64`, … | Optional platform binaries |

There is no `@yydb/iris/web`, `@yydb/iris/wasm`, or standalone `@yydb/iris-core` npm package.

## Retired TS product surface

`projects/iris.ts/iris-adapter-{postgres,mysql,redis,sqlite,web}` are early stubs and **must not** appear in install docs or codegen defaults anymore. Node foreign-store execution belongs to Rust N-API + Rust adapters; browser local storage belongs to the Web host integration layer — not a second planner.

## WASI

WASI is **frozen** as a public contract. `@yydb/iris-unknown-wasm32` is browser-safe WASM, not WASI filesystem or networking.

## Rust native

Rust apps use the `iris::*` crate and `iris-tools` CLI directly — no npm layer required.
