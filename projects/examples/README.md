# Iris + TypeScript backends

Self-contained examples: each project keeps **`iris.config.ts`** (project config) and **`schemas/blog.iris`** (schema **data**) beside the app, and wires Iris through `@yydb/iris/node` on the server only.

**Config vs data:** `iris.config.ts` declares datasource bindings and a `schema` pointer to on-disk `.iris` files. It does **not** embed VOS schema text.

The schema highlights **VOS references** — `author: &User` on `Post` — and route handlers query through that edge (`x.author.user_name`).

Example packages use the `@yydb-examples/*` scope so an accidental publish is rejected without `@yydb` registry access.

## Schema (`schemas/blog.iris`)

```vos
table User {
    @@user_id: uuid,
    user_name: utf8,
    active: bool,
}

table Post {
    @@post_id: uuid,
    author: &User,
    title: utf8,
    published: bool,
}
```

`&User` declares a reference field. Inserts pass the referenced primary key (`author_user_id` → `author`). Queries traverse the edge in VOS:

```vos
Post.filter(x => x.author.user_name == $name).collect()
Post.map(x => { title: x.title, author_name: x.author.user_name }).collect()
```

## HTTP servers

| Example   | Package                  | Default port |
|-----------|--------------------------|--------------|
| [Hono](./hono/)       | `@yydb-examples/hono`       | `8787`       |
| [Express](./express/) | `@yydb-examples/express`    | `3000`       |
| [Fastify](./fastify/) | `@yydb-examples/fastify`    | `3001`       |

## Full-stack frameworks

| Example       | Package                      | Default port |
|---------------|------------------------------|--------------|
| [Next.js](./next/)       | `@yydb-examples/next`       | `3002`       |
| [Nuxt](./nuxt/)          | `@yydb-examples/nuxt`        | `3003`       |
| [SvelteKit](./sveltekit/) | `@yydb-examples/sveltekit` | `3004`       |
| [Astro](./astro/)        | `@yydb-examples/astro`      | `3005`       |

## Layout (per example)

HTTP servers (Hono / Express / Fastify):

```text
hono/
  iris.config.ts            # datasources + schema pointer (`:memory:`)
  schemas/blog.iris         # schema data (User + Post with author: &User)
  src/generated/iris/       # `iris generate --config .` output (local, gitignored)
  src/db.ts                 # process singleton + seed over generated `createDb`
  src/index.ts              # HTTP routes call `db.user` / `db.post` directly
```

Full-stack (Next / Nuxt / SvelteKit / Astro) use framework-native server modules:

| Framework | DB module | Accessor |
|-----------|-----------|----------|
| Next | `lib/db.ts` | `getDb()` via `react.cache()` |
| Nuxt | `server/utils/db.ts` | `useDb()` |
| SvelteKit | `src/lib/server/db.ts` | `getDb()` |
| Astro | `src/lib/server/db.ts` | `getDb()` |

Full-stack configs use `file:.iris/dev.sqlite` so seed data survives dev HMR.

### `iris.config.ts`

Standalone server (`:memory:`):

```ts
import { defineConfig } from "@yydb/iris/types";

export default defineConfig({
    schema: "schemas/blog.iris",
    datasources: {
        default: { kind: "sqlite", mode: "managed_push", path: ":memory:" },
    },
    generate: { out: ".", target: "typescript" },
});
```

Full-stack dev (`file:.iris/dev.sqlite`):

```ts
export default defineConfig({
    schema: "schemas/blog.iris",
    datasources: {
        default: { kind: "sqlite", mode: "managed_push", path: "file:.iris/dev.sqlite" },
    },
    generate: { out: ".", target: "typescript" },
});
```

## Generate TypeScript client

The query API lives in **`src/generated/iris/`** — not hand-written. Regenerate from config + schema data:

```bash
pnpm run build:napi
pnpm run examples:generate
# or per project:
pnpm iris generate --config projects/examples/hono
```

`iris generate` reads `iris.config.ts`, loads `schemas/blog.iris`, and writes `src/generated/iris/` (`createDb`, `db.user`, `db.post`, …).

Each example keeps a small `db.ts` module (framework-native path) that opens the generated client and seeds demo data. Route handlers import the generated API directly — no parallel `iris.ts` wrapper layer.

Full-stack apps expose the same API under `/api/*` (for example `/api/posts`).

## Prerequisites

```bash
pnpm install
pnpm run build:napi
```

Requires `@yydb/iris/node` and a platform package such as `@yydb/iris-win32-x64` on Windows.

## Run

```bash
pnpm --filter @yydb-examples/hono start
pnpm --filter @yydb-examples/next dev
```

### API

| Route | Purpose |
|-------|---------|
| `GET /users` | List active users |
| `POST /users` | `{ "user_name": "grace" }` |
| `GET /posts` | Published posts with nested `author.user_name` through generated `&User` select |
| `GET /posts?author=ada` | Filter posts where `x.author.user_name == "ada"` |
| `POST /posts` | `{ "author_user_id": "<uuid>", "title": "…" }` — FK into `author: &User` |

HTTP servers use `/users` and `/posts`. Full-stack samples use `/api/users` and `/api/posts`.

```bash
curl http://127.0.0.1:8787/posts
curl "http://127.0.0.1:8787/posts?author=ada"
curl -X POST http://127.0.0.1:8787/posts \
  -H "content-type: application/json" \
  -d "{\"author_user_id\":\"<user_id from GET /users>\",\"title\":\"New post\"}"
```

Seed data creates users `ada` / `linus` and one post each so `?author=ada` returns a row immediately.

## Typecheck

```bash
pnpm run examples:typecheck
```
