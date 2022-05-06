# Iris + TypeScript backends

Self-contained examples: each project keeps **`iris.config.ts`** (project config) and **`schemas/blog.iris`** (schema **data**) beside the app, and wires Iris through `@yydb/iris/node` on the server only.

**Config vs data:** `iris.config.ts` declares datasource bindings and a `schema` pointer to on-disk `.iris` files. It does **not** embed VOS schema text.

The schema highlights **VOS references** — `author: &User` on `Post` — and route handlers query through that edge (`x.author.user_name`).

Example packages use the `@yydb-examples/*` scope so an accidental publish is rejected without `@yydb` registry access.

## Schema (`schemas/blog.iris` + `schemas/seed.iris`)

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

Demo fixtures live in `schemas/seed.iris` as the durable macro `seed_blog()` (users `ada` / `linus`, one post each). Examples call `await db.$macros.seed_blog()` on boot. `iris.config.ts` uses `schemas/**/*.iris` so `blog.iris` and `seed.iris` merge at generate and runtime.

## HTTP servers

| Example   | Package                  | Default port |
|-----------|--------------------------|--------------|
| [Hono](./hono/)       | `@yydb-examples/hono`       | `8787`       |
| [Express](./express/) | `@yydb-examples/express`    | `3000`       |
| [Fastify](./fastify/) | `@yydb-examples/fastify`    | `3001`       |

## Edge (Cloudflare Workers + D1)

| Example | Package | Notes |
|---------|---------|-------|
| [Cloudflare](./cloudflare/) | `@yydb-examples/cloudflare` | D1 for `/users` `/posts`; WASM Iris on `/iris/*`. See [cloudflare/README](./cloudflare/README.md). |

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
  iris.config.ts            # datasources + schema pointer (YYDB `:memory:`)
  schemas/blog.iris         # schema data (User + Post with author: &User)
  src/generated/iris/       # `iris generate --config .` output (local, gitignored)
  src/index.ts              # `openDatabase` from `@iris/node.ts`, then `db.user` / `db.post`
```

### `generate.out` (framework layout)

`iris.config.ts` `generate.out` is the **generated client root**. Follow each framework's source layout — never a bare `<root>/generated/` dump:

| Example | `generate.out` | Why |
|---------|----------------|-----|
| Hono / Express / Fastify | `src/generated/iris` | App code under `src/` |
| Next.js | `lib/generated/iris` | App Router at `app/`, server helpers in `lib/` |
| Nuxt | `server/generated/iris` | Nitro server under `server/` |
| SvelteKit / Astro | `src/generated/iris` | `src/` is the app root |

Legacy `generate.out: "."` still maps to `src/generated/iris`.

### `@iris/*` import alias

App code imports through **`@iris/*`**, mapped to each project's `generate.out`:

```ts
import type { UserId } from "@iris/index.ts";
import { createDatabase, openDatabase, closeDatabase } from "@iris/node.ts";
```

| Surface | Mapping |
|---------|---------|
| `tsconfig.json` | `"paths": { "@iris/*": ["<generate.out>/*"] }` |
| Node (`hono` / `express` / `fastify`) | `tsx` start (reads `tsconfig` paths) |
| Nuxt / Astro / SvelteKit / Next | framework `alias` / Vite / `tsconfig` paths on `@iris` |

Do not import `references.ts`, `inputs.ts`, or other internal stems. Domain types from `@iris/index.ts`, host entry from `@iris/node.ts` or `@iris/browser.ts`.

Full-stack (Next / Nuxt / SvelteKit / Astro) import directly from `@iris/node.ts`:

| Framework | Pattern |
|-----------|---------|
| Next | `createDatabase` per handler (no process singleton) |
| Nuxt / SvelteKit / Astro | `openDatabase({ config: process.cwd(), source: "default" })` |

HTTP servers call `openDatabase` once at startup and `closeDatabase` on shutdown. No hand-written `db.ts` wrapper.

Full-stack configs use file-backed `.iris/dev.yydb` so data survives dev HMR. Boot hooks call `seed_blog` once, or use `POST /users` and `POST /posts`.

### `iris.config.ts`

Standalone server (in-memory YYDB):

```ts
import { defineConfig } from "@yydb/iris/types";

export default defineConfig({
    schema: "schemas/**/*.iris",
    datasources: {
        default: { kind: "yydb", mode: "native_pull", path: ":memory:" },
    },
    generate: { out: "src/generated/iris", target: "typescript" },
});
```

Full-stack dev (file-backed YYDB, Next layout):

```ts
export default defineConfig({
    schema: "schemas/**/*.iris",
    datasources: {
        default: { kind: "yydb", mode: "native_pull", path: ".iris/dev.yydb" },
    },
    generate: { out: "lib/generated/iris", target: "typescript" },
});
```

Examples default to **YYDB** (`native_pull`). SQLite `managed_push` remains available for foreign-adapter experiments but is not the recommended path.

## Generate TypeScript client

The query API lives under each project's **`generate.out`** — not hand-written. Regenerate from config + schema data:

```bash
pnpm run build:napi
pnpm run examples:generate
# or per project:
pnpm iris generate --config projects/examples/hono
```

`iris generate` reads `iris.config.ts`, loads `schemas/blog.iris`, and writes `generate.out` (`createDatabase`, `openDatabase`, `closeDatabase`, `db.user`, `db.post`, …).

Route handlers import **`@iris/node.ts`** (`createDatabase` / `openDatabase` / `closeDatabase`) and **`@iris/index.ts`** (domain types). No hand-written `db.ts`, no parallel `iris.ts` wrapper, no seed logic outside routes.

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

`seed_blog` creates users `ada` / `linus` and one post each so `?author=ada` returns a row immediately.

## Typecheck

```bash
pnpm run examples:typecheck
```
