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

`@yydb/iris-adapter-{postgres,mysql,redis,sqlite,web}` **已从仓库删除**。Node foreign-store 执行与 CLI live 命令全部由 **Rust N-API + Rust `iris-adapter-*`** 承担；公开 npm 面只有 `@yydb/iris` 与平台 optional 包。

## WASI

当前 **冻结** WASI 公开面。`@yydb/iris-unknown-wasm32` 是 browser-safe WASM，不代表 WASI 文件系统或网络。

## 验证分离（本地）

Homepage 是 **browser** 宿主（VMZ static），不会也不应把 `@yydb/iris/node` 打进客户端依赖图。本地可跑：

```bash
# 从 iris-orm 根目录
pnpm run verify:homepage-hosts   # browser export map + 完整 iris 验证
pnpm run verify:iris-exports     # 仅 @yydb/iris 包
```

预期（Windows 已构建 `iris.win32-x64-msvc.node` 时）：

| 检查 | 结果 |
| --- | --- |
| `import "@yydb/iris"`（browser / node 条件） | 始终 → `src/browser/`（**无** `@yydb/iris/web` 子路径） |
| `import "@yydb/iris/node"`（**browser** 条件，bundler 模拟） | → `unsupported.ts` |
| `import "@yydb/iris/node"`（**node** 条件） | → `src/node/index.ts` |
| `loadNativeBinding()` | 加载 optional 平台 `.node`，`irisVersion()` → `0.1.0` |
| `initIris()`（web） | `wasm-not-implemented`（WASM 未 copy 前正常） |
| `iris` CLI | 需 `pnpm install --filter @yydb/iris` 后 `doctor` / `check` 可用 |

**注意**：在 Node 进程里 `import "@yydb/iris/node"` 会正确解析到 N-API facade；这与 bundler 在 browser 条件下走 `unsupported.ts` 不矛盾。

## Rust 原生

Rust 应用直接使用 `iris::*` crate 与 `iris-tools` CLI，不经过 npm。
