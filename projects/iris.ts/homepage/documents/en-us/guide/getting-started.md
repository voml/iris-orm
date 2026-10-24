# Getting started

## Install

```bash
pnpm add @yydb/iris
```

Browser / Worker hosts also add the language layer, which loads the WASM core:

```bash
pnpm add @yydb/iris-language
```

Node apps also need the matching optional N-API platform package (resolved by the `@yydb/iris/node` loader):

```text
@yydb/iris-win32-x64
@yydb/iris-linux-x64
@yydb/iris-unknown-wasm32   # browser WASM asset, loaded by @yydb/iris-language
```

## Import paths

```ts
import type { IrisRuntime } from "@yydb/iris/types";

// browser / worker — WASM host
import { createIris, initIris } from "@yydb/iris-language";

// project config helpers — every host
import { defineConfig } from "@yydb/iris";

// node / SSR / CLI — explicit subpath required
import { createIris } from "@yydb/iris/node";
```

Do **not** branch on `typeof window` in a shared entry — bundlers may pull both N-API and WASM into one graph.

## CLI

The Node host ships the sole `iris` CLI:

```bash
pnpm exec iris --help
```

The CLI runs through `@yydb/iris/node` and the optional platform N-API packages.

## Schema

Keep `schemas/**/*.iris` in your repo and an `iris.von` project file for datasources and generate output. See the
`farm-database` crate in [vmz-circle-farm](https://github.com/voml/iris-orm) for a worked example.

## vs sql-studio-orm

`@yydb/sql-studio-orm` and Iris are **parallel products**, not stacked. Iris only speaks VOS / `.iris` and never routes
through a SQL query AST.
