# Iris + Cloudflare Workers + D1 (research spike)

Deploy the same blog schema on **Cloudflare Workers** with a **D1** binding, while keeping Iris generate / `@iris/*` imports identical to the Node HTTP examples.

## What works today

| Layer | Status |
|-------|--------|
| `iris generate` + `@iris/index.ts` types | Same as other examples |
| `wrangler dev` / `wrangler deploy` | Hono Worker + D1 binding |
| **`GET/POST /users` `/posts`** | **D1** (SQL via `src/d1-blog.ts`) |
| **`GET/POST /iris/*`** | **Iris WASM** generated client (`@iris/browser.ts`, memory per isolate) |
| `@yydb/iris/node` (N-API) on Workers | **Blocked** — no native `.node` on workerd |

## Target shape (not implemented)

```ts
// Future: one generated entry, D1 as foreign SQLite session
const db = await createDatabase({ profile: "d1", binding: env.DB, source: "default" });
await db.$macros.seed_blog();
```

Until Iris ships a D1 / foreign-sqlite session, this example keeps:

1. **D1 migrations** (`migrations/`) aligned with `schemas/blog.iris`
2. A thin **`d1-blog.ts`** SQL layer (explicitly temporary)
3. **`/iris/*`** routes proving the generated WASM client runs at the edge

## Prerequisites

```bash
pnpm install
pnpm run build:wasm    # @yydb/iris-unknown-wasm32
pnpm run build:napi    # iris generate (CLI)
pnpm run examples:generate
```

**Monorepo note:** local `schemas/seed.iris` (`macro seed_blog`) needs VOS document macro parsing. Sibling checkout uses `.cargo/config.toml` (gitignored) to patch `vos` — do not add the patch to root `Cargo.toml`.

## D1 setup

```bash
cd projects/examples/cloudflare
pnpm install

# Create remote DB once (copy database_id into wrangler.toml)
wrangler d1 create iris-blog

# Apply schema + seed locally (wrangler dev uses this)
pnpm db:migrate:local

# Optional: apply to remote before deploy
pnpm db:migrate:remote
```

## Local dev

```bash
pnpm dev
# http://127.0.0.1:8787/health
# http://127.0.0.1:8787/users
# http://127.0.0.1:8787/posts?author=ada
# http://127.0.0.1:8787/iris/users   (WASM memory — resets on isolate cold start)
```

## Deploy

```bash
pnpm deploy
```

## Schema workflow

| Step | Command | Notes |
|------|---------|-------|
| Author | Edit `schemas/*.iris` | Same as Hono example |
| Generate client | `pnpm generate` | Writes `src/generated/iris` |
| Push to local SQLite | `iris push --config .` | Uses `.local/blog.sqlite` from `iris.config.ts` |
| D1 DDL | `pnpm db:migrate:local` | SQL in `migrations/` (manual parity until D1 push) |

`iris.config.ts` uses `sqlite` + `managed_push` for local `iris push` experiments. Production persistence in this spike is **D1 migrations**, not YYDB.

## API

| Route | Backend |
|-------|---------|
| `GET /health` | D1 ping + mode metadata |
| `GET /users` | D1 |
| `POST /users` | D1 |
| `GET /posts` | D1 (join on `author` → `User`) |
| `GET /posts?author=ada` | D1 filter |
| `POST /posts` | D1 |
| `/iris/*` | Iris WASM generated delegates |

Seed data (`ada` / `linus`) comes from `migrations/0002_seed.sql` on D1 and `seed_blog()` macro on WASM routes.

## Why two stacks?

```text
Worker request
  ├─ /users, /posts     → env.DB (D1)     … durable, no Iris planner yet
  └─ /iris/users, …     → WASM + @iris/*  … full generated client, memory only
```

**N-API** (`@iris/node.ts`) cannot load on workerd. **WASM** runs but only supports **memory** sessions today — no `env.DB` passthrough. The gap is an Iris **D1 session adapter** (likely wrapping D1 as a foreign SQLite execution surface), not more example glue code.

## Typecheck

```bash
pnpm generate
pnpm typecheck
```
