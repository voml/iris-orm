# Iris ORM

<p align="center">
  <a href="https://iris-orm.pages.dev/"><img src=".github/social-preview.jpg" alt="Iris ORM — VOS data access layer. One Rust core, Node N-API, Browser WASM." width="100%"></a>
</p>

<p align="center">
  <a href="https://iris-orm.pages.dev/"><strong>Website</strong></a> ·
  <a href="https://iris-orm.pages.dev/d/en-us/">Docs</a> ·
  <a href="https://github.com/voml/iris-orm/issues">Issues</a>
</p>

<p align="center">
  <img src="https://img.shields.io/github/stars/voml/iris-orm?style=social" alt="GitHub stars">
  <img src="https://img.shields.io/badge/Rust-core-DEA584?logo=rust&logoColor=white" alt="Rust core">
  <img src="https://img.shields.io/badge/Node-N--API-339933?logo=nodedotjs&logoColor=white" alt="Node N-API">
  <img src="https://img.shields.io/badge/Browser-WASM-654FF0?logo=webassembly&logoColor=white" alt="Browser WASM">
  <img src="https://img.shields.io/badge/schema-.iris-0d7a62" alt=".iris schema">
</p>

**Site:** [iris-orm.pages.dev](https://iris-orm.pages.dev/) ·
**Repo:** [github.com/voml/iris-orm](https://github.com/voml/iris-orm)

Iris is the **VOS data-access layer** for backend applications. It is not a
database and not a new schema language.

Applications use:

- VOS schema / operations / queries — on-disk extension **`.iris`**
- the typed Iris session API for **this language**

This repository has one runtime semantic implementation: the **Rust Iris
core**. JavaScript hosts expose that core through host-specific bindings while
keeping ecosystem-specific driver and storage integration outside the core.
Node.js uses N-API; browsers use browser-safe WebAssembly. WASI is not currently a supported host contract.

Public binding packages use coarse host/CPU names: `@yydb/iris-win32-x64`,
`@yydb/iris-linux-x64`, `@yydb/iris-linux-arm64`, `@yydb/iris-darwin-x64`,
`@yydb/iris-darwin-arm64`, and `@yydb/iris-unknown-wasm32`. Each native package
is a thin `index.js` loader over `lib/*.node` (panduck-style). Toolchain details
such as MSVC, GNU, and musl remain internal build targets rather than public import
names. The WASM package is browser-safe WebAssembly, not WASI.

| Tree                            | User facade         | Role                                                                                  |
|---------------------------------|---------------------|---------------------------------------------------------------------------------------|
| `projects/crates`               | `iris::*`           | Sole semantic runtime + Rust facade / CLI / generate + N-API and browser-WASM exports |
| `projects/packages`             | `@yydb/iris`        | Node/browser facades, `iris` CLI, N-API/WASM loaders, platform packages               |
| `projects/packages/iris-skills` | `@yydb/iris-skills` | Agent Skills catalog (`npx skills`)                                                   |

Codegen shares `.dejavu` templates; each host facade runs generate locally so
TS users do not need the Rust `iris` executable. TypeScript must not implement
a second VOS parser, semantic planner, optimizer, consistency model, or
diagnostic system.

Backends (per host):

- **Native VOS connectors** — YYDB (ready); YYDS (readiness-gated until VOS executor ships)
- **Isolated foreign-store adapters** — SQLite, PostgreSQL, MySQL, Redis (keyspace-only)

Iris does **not** expose raw SQL, SQL query builders, SQL AST/parsers, or
SQL-shaped public APIs. Foreign commands stay inside adapter packages.

Iris and `@yydb/sql-studio-orm` are parallel products, not stacked ORMs:

- `@yydb/sql-studio-orm` is a TypeScript-first Kysely/Drizzle-style query and
  schema toolkit.
- Iris is a Prisma-like DSL-driven workflow whose only DSL and schema truth is
  VOS.
- Both may reuse `@yydb/postgres`, `@yydb/mysql`, `@yydb/sqlite`, and other
  database drivers. Iris must not depend on `@yydb/sql-studio-orm` or route VOS
  operations through its query AST.

The VOS DSL is also Iris's optimization boundary. Because Iris sees stable
schema identities, operation inputs/results, references, read/write sets,
consistency intent, capabilities, and datasource topology before driver
lowering, it can perform proven projection/predicate pushdown, batching,
command fusion, routing, invalidation, retry/outbox planning, and generated
decoder specialization. Such rewrites must preserve observable VOS semantics;
unsupported semantics are rejected before execution rather than silently
lowered to an approximate backend command.

## Workspace

```text
projects/crates/
  iris/                 public Rust facade
  iris-types/           session / planner / capability / runtime
  iris-ir/              physical plan + envelopes
  iris-generator/       Dejavu AOT for Rust host (shared templates)
  iris-connector-*      native VOS connectors
  iris-adapter-*        foreign-store adapters

projects/packages/
  iris/                 @yydb/iris — browser default + /node + /types + iris CLI
  iris-{platform}/      optional N-API platform packages (`index.js` + `lib/*.node`)
  iris-unknown-wasm32/  browser WASM artifact package (`lib/` from `iris-wasm` crate)
  iris-skills/          @yydb/iris-skills
  homepage/             official site → https://iris-orm.pages.dev/

Native builds: `projects/crates/iris-napi` + `scripts/build-napi.mjs` (not npm workspace members).
WASM builds: `projects/crates/iris-wasm` + `scripts/build-wasm.mjs`.
```

VOS language sources are **not** vendored. The Rust workspace depends on the
public `vos` facade from a sibling checkout of [`vos-language`](https://github.com/voml/vos-language)
on branch `dev` (`../../../vos-language/projects/vos.rs/vos` from `projects/crates`).
YYDB native tests also need a sibling [`yydb.rs`](https://github.com/yy-database/yydb.rs)
checkout. VON config uses sibling [`von-language`](https://github.com/voml/von-language).

## Status

Phases 0–4, 6–9, and 10-A…G landed (Composite conformance §15.6, topology
activate, projection verify). Phase 5 YYDS remains readiness-gated.

## Escape hatch naming (TS ↔ Rust)

Public VOS text entry points (not a second query dialect):

| Intent                       | TypeScript generated client         | Rust                                                              |
|------------------------------|-------------------------------------|-------------------------------------------------------------------|
| Typed CRUD (primary)         | `db.user.findMany` / `create`       | `Db::user().find_many` / `insert` (generate `--target rust`)      |
| DML escape hatch             | `db.$query(vosText, parameters?)`   | `Db::query` / `Session::query`                                    |
| DDL / unit                   | `db.$execute(vosText, parameters?)` | `Db::execute` / `Session::execute`                                |
| Plan only                    | (via binding / explain)             | `session.plan(vosText)`                                           |
| Held connection (txn / test) | (txn on client)                     | `Db::transaction` / `Db::with_rollback` → `Txn` (same CRUD names) |

Rust `iris generate --target rust` emits domain structs **and** a thin MySQL `Db`/`Txn`
CRUD shim (synthesizes `.filter` VOS). That is **not** knife-B `GeneratedCall` / identity IR.
Pooling stays inside `MysqlSource`; apps do not build a second pool.

Pipeline predicates: prefer **`.filter(x => …)`**. `.where(…)` is accepted only as a
compatibility alias of `.filter` (same physical `Filter` op); **do not document or
generate SQL-style `.where` in new examples**.

Inside `transaction` / `with_rollback`, use **`Txn`** (same method names as `Db`).
Do not call `MysqlSource::insert` / `execute_plan` from the closure — those check out
another connection and leave the transaction.

Legacy Rust names `execute_vos` / `plan_vos` / `interpret_vos` are deprecated
aliases of `query` / `plan` / `interpret`.

## Develop / clean checkout smoke

```bash
# sibling layout: vos-language/, yydb.rs/, von-language/, iris-orm/
pnpm install
pnpm run fmt:check
pnpm run check:rs
pnpm run test:rs
pnpm run typecheck:ts
pnpm run iris -- doctor   # @yydb/iris CLI
```

The `iris` CLI ships from `@yydb/iris` (`projects/packages/iris`). Use `pnpm run iris -- …` or `pnpm exec iris …` after install.

```bash
pnpm run iris -- check path/to/schema.iris
pnpm run iris -- generate path/to/schema.iris
```

Optional live backends (CI enables these when services are up):

```bash
export IRIS_TEST_POSTGRES_URL='host=127.0.0.1 user=iris password=iris dbname=iris'
export IRIS_TEST_MYSQL_URL='mysql://iris:iris@127.0.0.1:3306/iris'
export IRIS_TEST_REDIS_URL='redis://127.0.0.1:6379/'
```
