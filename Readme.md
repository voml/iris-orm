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

**Site:** [iris-orm.pages.dev](https://iris-orm.pages.dev/) · **Repo:** [github.com/voml/iris-orm](https://github.com/voml/iris-orm)

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
`@yydb/iris-linux-x64`, and `@yydb/iris-unknown-wasm32`. Toolchain details such
as MSVC, GNU, and musl remain internal build targets rather than public import
names. The WASM package is browser-safe WebAssembly, not WASI.

| Tree | User facade | Role |
| --- | --- | --- |
| `projects/iris.rs` | `iris::*` | Sole semantic runtime + Rust facade / CLI / generate + N-API and browser-WASM exports |
| `projects/iris.ts` | `@yydb/iris` | Node/browser facades, `iris` CLI, N-API/WASM loaders, platform packages |
| `projects/iris.ts/iris-skills` | `@yydb/iris-skills` | Agent Skills catalog (`npx skills`) |

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
projects/iris.rs/
  iris/                 public Rust facade
  iris-types/           session / planner / capability / runtime
  iris-ir/              physical plan + envelopes
  iris-generator/       Dejavu AOT for Rust host (shared templates)
  iris-tools/           Rust-host `iris` CLI (check / generate / migrate / …)
  iris-connector-*      native VOS connectors
  iris-adapter-*        foreign-store adapters

projects/iris.ts/
  iris/                 @yydb/iris — browser default + /node + /types + iris CLI
  iris-win32-x64/       optional N-API platform package
  iris-linux-x64/       optional N-API platform package
  iris-unknown-wasm32/  optional browser WASM platform package
  iris-napi/            private napi-rs build workspace
  iris-wasm/            private wasm-pack build workspace
  iris-skills/          @yydb/iris-skills
  homepage/             official site → https://iris-orm.pages.dev/
```

VOS language sources are **not** vendored. The Rust workspace depends on the
public `vos` facade from a sibling checkout of [`vos-language`](https://github.com/voml/vos-language)
on branch `dev` (`../../../vos-language/projects/vos.rs/vos` from `projects/iris.rs`).
YYDB native tests also need a sibling [`yydb.rs`](https://github.com/yy-database/yydb.rs)
checkout. VON config uses sibling [`von-language`](https://github.com/voml/von-language).

## Status

Phases 0–4, 6–9, and 10-A…G landed (Composite conformance §15.6, topology
activate, projection verify). Phase 5 YYDS remains readiness-gated.

## Escape hatch naming (TS ↔ Rust)

Public VOS text entry points (not a second query dialect):

| Intent | TypeScript generated client | Rust `Session` (reference) |
| --- | --- | --- |
| DML (rows) | `db.$query(vosText, parameters?)` | `session.query(vosText)` |
| DDL / unit | `db.$execute(vosText, parameters?)` | `session.execute(vosText)` |
| Plan only | (via binding / explain) | `session.plan(vosText)` |

Pipeline predicates: prefer **`.filter(x => …)`**. `.where(…)` is accepted only as a
compatibility alias of `.filter` (same physical `Filter` op); **do not document or
generate SQL-style `.where` in new examples**.

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
pnpm run iris -- doctor   # TS host CLI stub
```

Or from `projects/iris.rs` directly:

```bash
cd projects/iris.rs
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p iris-tools -- doctor
```

```bash
cargo run -p iris-tools --manifest-path projects/iris.rs/Cargo.toml -- check path/to/schema.iris
cargo run -p iris-tools --manifest-path projects/iris.rs/Cargo.toml -- generate path/to/schema.iris
```

Optional live backends (CI enables these when services are up):

```bash
export IRIS_TEST_POSTGRES_URL='host=127.0.0.1 user=iris password=iris dbname=iris'
export IRIS_TEST_MYSQL_URL='mysql://iris:iris@127.0.0.1:3306/iris'
export IRIS_TEST_REDIS_URL='redis://127.0.0.1:6379/'
```
