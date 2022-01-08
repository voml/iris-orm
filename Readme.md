# Iris ORM

Iris is the **VOS data-access layer** for backend applications. It is not a
database and not a new schema language.

Applications use:

- VOS schema / operations / queries — on-disk extension **`.iris`**
- the typed Iris session API for **this language**

This repository is a **multi-language mono**. Each host implements Iris
**natively** — not a bindgen/FFI wrapper around another host — because
foreign-store adapters must follow each ecosystem’s derive/ORM/driver idioms.

| Tree | User facade | Role |
| --- | --- | --- |
| `projects/iris.rs` | `iris::*` | Rust reference runtime + host CLI / generate (Rust Dejavu) |
| `projects/iris.ts` | `@yydb/iris` | Native TypeScript full runtime + `iris` CLI (`cac`) + `@yydb/iris-adapter-*` (JS/WASM peers) |
| `projects/iris.ts/iris-skills` | `@yydb/iris-skills` | Agent Skills catalog (`npx skills`) |

Codegen shares `.dejavu` templates; each host runs generate locally so TS users
do not need the Rust `iris` binary. TypeScript Iris depends on
`@game-gpt/vos` (not `@yydb/vos`) — same rule as Rust depending on the `vos` crate.

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
  iris/                 @yydb/iris public facade + iris CLI
  iris-types/           @yydb/iris-types (pairs with Rust iris-types)
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
