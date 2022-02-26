---
title: 概览
order: 0
---

# Iris ORM 文档

Iris 是 **VOS 数据访问层**：不是数据库，也不是新的 schema 语言。权威 schema 使用 **`.iris`**（VOS 语法）。

## 架构要点

- **Rust Iris core** 是唯一运行时语义实现（parser / planner / capability / consistency 等只实现一次）。
- **Node.js** 经 **N-API** 暴露为 `@yydb/iris/node` + optional 平台包（如 `@yydb/iris-win32-x64`）。
- **浏览器** 默认入口 `@yydb/iris` 内嵌 **WASM** 语义核；IndexedDB / OPFS 等仍由 Web 宿主集成层负责。
- **不再** 提供 TypeScript `@yydb/iris-adapter-*` npm 包；foreign-store lowering 在 Rust 工作区的 `iris-adapter-*` / `iris-connector-*` 完成，经 N-API / WASM 暴露。

## 从这里开始

- [快速开始](./guide/getting-started) — 安装与 import 路径
- [宿主与绑定](./guide/hosts) — N-API / WASM / 平台包

## 仓库

- [官方网站](https://iris-orm.pages.dev/)
- [GitHub：iris-orm](https://github.com/voml/iris-orm)
