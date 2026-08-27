# Getting started

## Install

```bash
pnpm add @yydb/iris
```

Node apps also need the matching optional N-API platform package (resolved by the `@yydb/iris/node` loader):

```text
@yydb/iris-win32-x64
@yydb/iris-linux-x64
@yydb/iris-unknown-wasm32   # browser WASM asset
```

## Import paths

```ts
import type { IrisRuntime } from "@yydb/iris/types";

// browser / worker — default entry
import { createIris, initIris } from "@yydb/iris";

// node / SSR / CLI — explicit subpath required
import { createIris } from "@yydb/iris/node";
```

Do **not** branch on `typeof window` in a shared entry — bundlers may pull both N-API and WASM into one graph.

## CLI

The Node host ships the `iris` CLI (same brand as Rust `iris-tools`):

```bash
pnpm exec iris --help
```

The TypeScript host CLI is still a skeleton; full semantic commands come from the Rust core via N-API.

## Schema

Keep `schemas/**/*.iris` in your repo and an `iris.von` project file for datasources and generate output. See the `farm-database` crate in [vmz-circle-farm](https://github.com/voml/iris-orm) for a worked example.

## vs sql-studio-orm

`@yydb/sql-studio-orm` and Iris are **parallel products**, not stacked. Iris only speaks VOS / `.iris` and never routes through a SQL query AST.
