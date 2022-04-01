# 快速开始

## 安装

```bash
pnpm add @yydb/iris
```

Node 应用还需要匹配平台的 optional N-API 包（由 `@yydb/iris/node` loader 解析）：

```text
@yydb/iris-win32-x64
@yydb/iris-linux-x64
@yydb/iris-unknown-wasm32   # browser WASM asset
```

## Import 路径

```ts
import type { IrisRuntime } from "@yydb/iris/types";

// 浏览器 / Worker — 默认入口
import { createIris, initIris } from "@yydb/iris";

// Node / SSR / CLI — 必须显式子路径
import { createIris } from "@yydb/iris/node";
```

**禁止**在共享入口写 `typeof window` 分支动态 import； bundler 会把 N-API 与 WASM 打进同一依赖树。

## CLI

Node 宿主提供唯一的 `iris` CLI：

```bash
pnpm exec iris --help
```

CLI 通过 `@yydb/iris/node` 与 optional 平台 N-API 包运行。

## Schema

在仓库中维护 `schemas/**/*.iris`，用 `iris.von` 声明 datasource 与 generate
输出。示例见 [circle-farm](https://github.com/voml/iris-orm) 的 `farm-database` crate。

## 与 sql-studio-orm 的关系

`@yydb/sql-studio-orm` 与 Iris 是 **平行产品**，不是上下层叠。Iris 只走 VOS / `.iris`，不路由进 SQL query AST。
