---
title: Getting started
order: 0
---

# Iris ORM docs

Iris is the **VOS data-access layer** — not a database and not a new schema language. Authoritative schemas use **`.iris`** (VOS grammar).

## Architecture

- **Rust Iris core** owns runtime semantics (parser, planner, capability, consistency, …) — implemented once.
- **Node.js** exposes N-API via `@yydb/iris/node` plus optional platform packages (e.g. `@yydb/iris-win32-x64`).
- **Browsers** use the default `@yydb/iris` facade with an embedded **WASM** core; storage APIs stay in the Web host layer.
- **No** TypeScript `@yydb/iris-adapter-*` npm packages; foreign-store lowering lives in Rust `iris-adapter-*` / `iris-connector-*` and is exposed through N-API / WASM.

## Start here

- [Getting started](./guide/getting-started) — install and import paths
- [Hosts & bindings](./guide/hosts) — N-API, WASM, platform packages

## Repository

- [Official site](https://iris-orm.pages.dev/)
- [iris-orm on GitHub](https://github.com/voml/iris-orm)
