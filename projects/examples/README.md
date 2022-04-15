# Iris + TypeScript backends

Self-contained examples: each project keeps its own `schemas/user.iris`, Iris wiring beside server routes, and the same list/create users API.

Example packages use the `@yydb-examples/*` scope so an accidental publish is rejected without `@yydb` registry access.

## HTTP servers

| Example   | Package                  | Default port |
|-----------|--------------------------|--------------|
| [Hono](./hono/)       | `@yydb-examples/hono`       | `8787`       |
| [Express](./express/) | `@yydb-examples/express`    | `3000`       |
| [Fastify](./fastify/) | `@yydb-examples/fastify`    | `3001`       |

## Full-stack frameworks

| Example       | Package                      | Default port | Iris entry                         |
|---------------|------------------------------|--------------|------------------------------------|
| [Next.js](./next/)       | `@yydb-examples/next`       | `3002`       | `app/api/users/route.ts`           |
| [Nuxt](./nuxt/)          | `@yydb-examples/nuxt`        | `3003`       | `server/api/users.{get,post}.ts`   |
| [SvelteKit](./sveltekit/) | `@yydb-examples/sveltekit` | `3004`       | `src/routes/api/users/+server.ts`  |
| [Astro](./astro/)        | `@yydb-examples/astro`      | `3005`       | `src/pages/api/users.ts`           |

## Layout (per example)

```text
next/   # shape varies by framework, same pieces everywhere
  schemas/user.iris          # VOS schema beside the app
  lib/iris.ts                # or server/utils/iris.ts — openIrisDb(), listUsers(), createUser()
  app/api/users/route.ts     # framework route handler only
```

Full-stack samples set `serverExternalPackages` / Vite `ssr.external` so N-API `.node` binaries are not bundled.

## Prerequisites

From the iris-orm repo root:

```bash
pnpm install
pnpm run build:napi
```

Examples require the Node semantic core (`@yydb/iris/node` + a platform package such as `@yydb/iris-win32-x64` on Windows).

## Run

HTTP servers:

```bash
pnpm --filter @yydb-examples/hono start
pnpm --filter @yydb-examples/express start
pnpm --filter @yydb-examples/fastify start
```

Full-stack (dev servers):

```bash
pnpm --filter @yydb-examples/next dev
pnpm --filter @yydb-examples/nuxt dev
pnpm --filter @yydb-examples/sveltekit dev
pnpm --filter @yydb-examples/astro dev
```

Each app exposes list + create users:

- HTTP servers: `GET /users`, `POST /users`
- Full-stack: `GET /api/users`, `POST /api/users`

```bash
curl http://127.0.0.1:3002/api/users
curl -X POST http://127.0.0.1:3002/api/users -H "content-type: application/json" -d "{\"user_name\":\"ada\"}"
```

## Integration pattern

1. **Open once per process** — `openIrisDb()` loads `schemas/user.iris` and calls `createIrisDbBinding()` with in-memory SQLite.
2. **Call from server routes only** — handlers invoke `listUsers` / `createUser` through `@yydb/iris/node`. Do not import `@yydb/iris/node` from client components or browser bundles.
3. **Production** — replace hand-written VOS helpers with a generated `createDb()` client after `iris generate`, and point `iris.von` at a real datasource.

## Typecheck all examples

```bash
pnpm run examples:typecheck
```
