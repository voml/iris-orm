# Iris ORM

<p align="center">
  <a href="https://iris-orm.pages.dev/"><img src=".github/social-preview.jpg" alt="Iris ORM — generative ORM on VOS. schemas/ as source of truth, git + iris push for DB versioning." width="100%"></a>
</p>

<p align="center">
  <a href="https://iris-orm.pages.dev/"><strong>Website</strong></a> ·
  <a href="https://iris-orm.pages.dev/d/en-us/"><strong>Docs</strong></a> ·
  <a href="https://github.com/voml/iris-orm/tree/dev/projects/examples"><strong>Examples</strong></a> ·
  <a href="https://github.com/voml/iris-orm/issues"><strong>Issues</strong></a>
</p>

<p align="center">
  <img src="https://img.shields.io/github/stars/voml/iris-orm?style=social" alt="GitHub stars">
  <img src="https://img.shields.io/badge/language-VOS-0d7a62" alt="VOS">
  <img src="https://img.shields.io/badge/schema-.iris-0d7a62" alt=".iris schema">
  <img src="https://img.shields.io/badge/ORM-generative-2563eb" alt="generative ORM">
</p>

## 💡 What is Iris?

**Iris** is a **generative ORM framework** on **VOS** — **not** a database. It is the **codegen + runtime layer** that
turns schema in your local **`schemas/`** directory into typed data access and keeps backing stores aligned with that
definition.

VOS schema and operations live in `.iris` files under `schemas/` — the **single source of truth**. **`git`** tracks
schema history; **`iris push`** applies DDL to target databases. **`iris check`** / **`iris generate`** emit the typed
client (`db.account.findMany`, `db.record.create`, …) your application compiles against.

| Layer               | What it is                                                                                                    |
|---------------------|---------------------------------------------------------------------------------------------------------------|
| **`schemas/`**      | **Canonical standard** — tables, references, macros in `.iris`; this is what you review in PRs                |
| **`git`**           | Version control for schema evolution — diffs, blame, rollback of the source of truth                          |
| **`iris push`**     | Applies planned DDL to a target DB (human ops) — bridges `schemas/` to live storage                           |
| **`iris generate`** | Emits the typed `db.*` client your app compiles against                                                       |
| **Runtime**         | Plans and executes VOS against configured datasources — uses committed `generated/`, not live `.iris` parsing |

Project config (`iris.config.ts` or equivalent) only wires datasources and generate output; it does **not** replace
`schemas/` as the schema authority.

**Backends Iris talks to:** YYDB (native VOS), plus PostgreSQL, MySQL, SQLite, and Redis (keyspace-only) — all driven
by `schemas/`, not hand-written SQL migrations.

**Parallel product:** [`@yydb/sql-studio-orm`](https://www.npmjs.com/package/@yydb/sql-studio-orm) is a SQL-shaped query
and schema toolkit. Iris and SQL Studio may share drivers where useful but **never stack** — Iris routes only through
VOS.

---

## 🚀 Getting started

### 1. Agentic workflow (recommended)

Install the official Agent Skills catalog so Cursor, Codex, Claude Code, or similar tools follow the real Iris workflow
(VOS in `.iris`, not SQL bypass):

```bash
npx skills add @yydb/iris-skills
```

Then prompt your agent — for example:

```text
Add Iris to this project.
- Install @yydb/iris and add project config pointing at schemas/
- Add schemas/domain.iris with tables and references as needed
- Run iris check and iris generate, commit generated/
- Wire routes through the generated client
Follow @yydb/iris-skills: VOS only, no SQL, no CI migrate.
```

Skills cover schema authoring, generate, migrate (`iris push`), runtime operations, explain, topology, and conformance.
See [`@yydb/iris-skills`](./projects/packages/iris-skills/README.md).

**Canonical local loop** (agents and humans):

```text
edit schemas/  →  iris check  →  iris generate  →  commit generated/
              →  iris push --plan  →  iris push     # human ops only, never CI
deploy:        runtime + generated only              # no CLI on the server
```

### 2. Manual install

**Install Iris:**

```bash
pnpm add @yydb/iris
# or: npm install @yydb/iris
```

**Point config at `schemas/`** (example `iris.config.ts`):

```ts
import {defineConfig} from "@yydb/iris";

export default defineConfig({
    schema: "schemas/**/*.iris",
    datasources: {
        default: {kind: "yydb", mode: "native_pull", path: ".iris/dev.yydb"},
    },
    generate: {out: "generated/iris", target: "<your-host>"},
});
```

**Author schema** (`schemas/domain.iris`):

```vos
table Account {
    @@account_id: uuid,
    display_name: utf8,
    enabled: bool,
}

table Record {
    @@record_id: uuid,
    owner: &Account,
    label: utf8,
    archived: bool,
}
```

**Check, generate, use:**

```bash
npx iris check --config .
npx iris generate --config .
```

```text
const db = await openDatabase({ source: "default" });
const rows = await db.record.findMany({ filter: (x) => !x.archived });
```

Host-specific import paths, CLI wiring, and framework
layout: [Getting started](https://iris-orm.pages.dev/d/en-us/guide/getting-started) · [Examples](./projects/examples/README.md)

**CLI:**

```bash
npx iris doctor
npx iris check path/to/schema.iris
npx iris generate --config .
npx iris push --plan
npx iris push
```

---

## ✨ Highlights

- **`schemas/` as the only standard** — VOS in `.iris` is canonical; **git** records history, **`iris push`** rolls DDL
  forward on target databases.
- **Generative ORM on VOS** — `iris generate` emits typed `db.*` clients; runtime executes planned VOS, not ad-hoc SQL.
- **Prisma-like workflow** — schema-first `schemas/`, local generate, typed client; escape hatch `$query` / `$execute`
  for rare VOS text.
- **VOS as optimization boundary** — Iris sees schema identity, read/write sets, and consistency intent before storage
  lowering, so it can batch, push predicates, fuse commands, and specialize decoders without silent semantic drift.
- **References in the schema** — `owner: &Account` is a first-class edge; queries traverse `x.owner.display_name` in
  VOS,
  not hand-joined SQL.
- **Human-gated DDL** — `iris push` plans and applies migrations from ops shells; CI and container boot never mutate
  production schema.
- **Agent-ready** — [`@yydb/iris-skills`](./projects/packages/iris-skills) teaches the real CLI and hard rules (no SQL
  bypass, commit `generated/`, deploy runtime only).

---

## 📊 How Iris compares

|                       | **Iris**                                            | **Prisma / Drizzle (SQL-shaped)**   | **`@yydb/sql-studio-orm`**             |
|-----------------------|-----------------------------------------------------|-------------------------------------|----------------------------------------|
| Schema truth          | `schemas/` + VOS `.iris`                            | SQL / ORM schema files              | SQL / query-builder types              |
| Versioning            | **git** on `schemas/` + **`iris push`** to DB       | migration files + deploy tooling    | migration / schema tooling             |
| Query surface         | Generated VOS client + rare `$query`                | SQL or ORM builder                  | SQL AST / builder                      |
| Raw SQL as public API | **No** — storage commands stay inside Iris adapters | Yes                                 | Yes                                    |
| Relationship to Iris  | —                                                   | Different DSL; do not stack on Iris | **Parallel product** — different layer |

Iris is closest in **workflow** to Prisma (schema → generate → typed client) but the **only** DSL and planner input is
**VOS**, not SQL.

---

## 📝 VOS syntax primer

**VOS** is the language Iris is built on. `.iris` files under **`schemas/`** are the only schema standard — not SQL, not
project config. Grammar lives in [`vos-language`](https://github.com/voml/vos-language); Iris checks, generates, plans,
and executes VOS against configured storage.

### Files and layout

| Piece                | Location                              | Role                                                                                                          |
|----------------------|---------------------------------------|---------------------------------------------------------------------------------------------------------------|
| **Canonical schema** | `schemas/**/*.iris`                   | **Single source of truth** — tables, references, macros; versioned with **git**, applied with **`iris push`** |
| Project config       | `iris.config.ts` (or host equivalent) | Datasource targets + `generate.out` — configuration only                                                      |
| Generated client     | e.g. `generated/iris/`                | Typed `db.account`, `db.record`, … — **commit this** into your app repo                                       |

One domain per file when table groups differ (`accounts.iris`, `records.iris`, …). Avoid a single mega-file.

### Tables and fields

```vos
table Account {
    @@account_id: uuid,   # primary key (@@ marks the PK column)
    display_name: utf8,
    enabled: bool,
}

table Record {
    @@record_id: uuid,
    owner: &Account,      # reference — FK edge to Account
    label: utf8,
    archived: bool,
}
```

| Syntax                              | Meaning                                  |
|-------------------------------------|------------------------------------------|
| `table Name { … }`                  | Declares a persisted entity              |
| `@@field: type`                     | Primary key column                       |
| `field: utf8` / `bool` / `uuid` / … | Scalar field types                       |
| `field: &Other`                     | Reference to another table's primary key |
| PascalCase table names              | Convention (`Account`, `Record`, …)      |

New `uuid` primary keys use **UUID v7** at insert time. Do not use random v4 generators for Iris-managed keys.

### Query pipelines

VOS queries read as **pipelines** on a table root. Prefer **`.filter(x => …)`** — not SQL-style `.where`.

```vos
# rows where owner display name matches a bound parameter
Record.filter(x => x.owner.display_name == $name).collect()

# project fields, including across a reference edge
Record.map(x => { label: x.label, owner_name: x.owner.display_name }).collect()

# enabled accounts only
Account.filter(x => x.enabled).collect()
```

| Stage              | Role                           |
|--------------------|--------------------------------|
| `TableName`        | Start from a table root        |
| `.filter(x => …)`  | Keep rows matching a predicate |
| `.map(x => { … })` | Shape each row (projection)    |
| `.collect()`       | Terminate and return a row set |

In application code the **generated client** is the primary API (`db.record.findMany`, `db.account.create`). Raw VOS
text (`$query`, `$execute`) is an escape hatch for rare cases — not the everyday CRUD layer.

### Inserts and macros

Row literals insert through method chaining:

```vos
Account {
    account_id: "550e8400-e29b-41d4-a716-446655440000",
    display_name: "alpha",
    enabled: true,
}.insert()
```

Reference fields accept the **referenced primary key** value (`owner: "<account_uuid>"` for `owner: &Account`).

Reusable fixture / seed logic lives in **macros**:

```vos
macro seed_fixture() -> unit {
    Account { account_id: "…", display_name: "alpha", enabled: true }.insert()
    Record { record_id: "…", owner: "…", label: "sample", archived: false }.insert()
}
```

After `iris generate`, invoke macros through the generated client (e.g. `db.$macros.seed_fixture()`).

### Mental model

```text
schemas/*.iris  →  git (history)  →  iris check  →  iris generate  →  db.* client
                          ↓
                   iris push (DDL to target DB, human ops)
                          ↓
                   runtime + generated  →  datasource  →  storage
```

**Do not** hand-write `CREATE TABLE` / `ALTER TABLE` for Iris-managed tables. **Do not** bypass Iris with raw SQL on
the same tables. Schema changes flow: edit `schemas/` → commit to **git** → `iris check` → `iris generate` → commit
`generated/` → **`iris push`** from an ops shell.

More examples: [`projects/examples/README.md`](./projects/examples/README.md) · VOS language: [
`vos-language`](https://github.com/voml/vos-language)

---

## 🧪 Examples

Sample applications (HTTP servers, full-stack frameworks, edge workers) live under [
`projects/examples/`](./projects/examples/README.md).

| Kind         | Examples                        |
|--------------|---------------------------------|
| HTTP servers | Hono, Express, Fastify          |
| Full-stack   | Next.js, Nuxt, SvelteKit, Astro |
| Edge         | Cloudflare Workers              |

Each example keeps `schemas/` beside the app and follows the same `check → generate → run` loop. See the examples README
for per-stack commands.

---

## 📜 License

[MPL-2.0](./LICENSE)
