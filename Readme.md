# Iris ORM

Iris is the **VOS data-access layer** for backend applications. It is not a
database and not a new schema language.

Applications use:

- VOS schema / operations / queries — on-disk extension **`.iris`**
- the typed Iris session API for **this language**

This repository has one runtime semantic implementation: the **Rust Iris
core**. JavaScript hosts expose that core through host-specific bindings while
keeping ecosystem-specific driver and storage integration outside the core.
Node.js uses N-API; browsers use browser-safe WebAssembly plus TypeScript Web
API adapters. WASI is not currently a supported host contract.

Public binding packages use coarse host/CPU names: `@yydb/iris-win32-x64`,
`@yydb/iris-linux-x64`, and `@yydb/iris-unknown-wasm32`. Toolchain details such
as MSVC, GNU, and musl remain internal build targets rather than public import
names. The WASM package is browser-safe WebAssembly, not WASI.

| Tree | User facade | Role |
| --- | --- | --- |
| `projects/iris.rs` | `iris::*` | Sole semantic runtime + Rust facade / CLI / generate + N-API and browser-WASM exports |
| `projects/iris.ts` | `@yydb/iris` | Node/Web facades, `iris` CLI, binding loaders, TypeScript codegen surface, and host adapters |
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
  iris/                 @yydb/iris — Web default + /node + /types + iris CLI
  iris-adapter-sqlite/  @yydb/iris-adapter-sqlite -> @yydb/sqlite (planned driver reuse)
  iris-adapter-postgres/@yydb/iris-adapter-postgres -> @yydb/postgres (planned driver reuse)
  iris-adapter-mysql/   @yydb/iris-adapter-mysql -> @yydb/mysql (planned driver reuse)
  iris-adapter-redis/   @yydb/iris-adapter-redis -> @yydb/redis (planned driver reuse)
  iris-adapter-web/     @yydb/iris-adapter-web — browser Local Web Backend
                        (IndexedDB+OPFS; W0 skeleton; not YYDB / not SQL)

```

VOS language sources are **not** vendored. The Rust workspace depends on the
public `vos` facade from a sibling checkout of [`vos-language`](https://github.com/voml/vos-language)
on branch `dev` (`../../../vos-language/projects/vos.rs/vos` from `projects/iris.rs`).
YYDB native tests also need a sibling [`yydb.rs`](https://github.com/yy-database/yydb.rs)
checkout. VON config uses sibling [`von-language`](https://github.com/voml/von-language).

## Status

Phases 0–4, 6–9, and 10-A…G landed (Composite conformance §15.6, topology
activate, projection verify). Phase 5 YYDS remains readiness-gated.

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
