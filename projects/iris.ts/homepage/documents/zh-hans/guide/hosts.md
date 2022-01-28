---
title: 宿主与绑定
order: 2
---

# 宿主与绑定

## 分层

```text
Rust Iris core
  ├─ iris-tools / iris-generator（Rust CLI）
  ├─ iris-connector-* / iris-adapter-*（Rust 工作区 lowering）
  ├─ Node N-API → @yydb/iris/node
  └─ browser WASM → @yydb/iris（默认入口）
```

TypeScript **不再**平行重写 parser、planner、optimizer 或 diagnostic 体系。

## npm 包面

| 包 | 用途 |
| --- | --- |
| `@yydb/iris` | 默认 Web facade（WASM 在包内） |
| `@yydb/iris/node` | Node N-API facade + `iris` CLI |
| `@yydb/iris/types` | 协议 / binding DTO（无 loader） |
| `@yydb/iris-win32-x64` 等 | optional 平台 binary |

不存在 `@yydb/iris/web`、`@yydb/iris/wasm` 或独立 `@yydb/iris-core` npm 包。

## 已退役的 TS 产品面

`projects/iris.ts/iris-adapter-{postgres,mysql,redis,sqlite,web}` 是早期 stub，**不应**再出现在安装文档或 codegen 默认路径里。Node 侧 foreign-store 执行由 Rust N-API 绑定 + Rust adapter 承担；浏览器本地存储由 Web 宿主集成（非第二套 planner）。

## WASI

当前 **冻结** WASI 公开面。`@yydb/iris-unknown-wasm32` 是 browser-safe WASM，不代表 WASI 文件系统或网络。

## Rust 原生

Rust 应用直接使用 `iris::*` crate 与 `iris-tools` CLI，不经过 npm。
