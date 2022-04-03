# Iris + TypeScript HTTP backends

Self-contained examples: each project keeps its own `schemas/user.iris`, Iris wiring in `src/iris.ts`, and HTTP routes in `src/index.ts`.

| Example   | Package                  | Default port |
|-----------|--------------------------|--------------|
| [Hono](./hono/)       | `@yydb-examples/hono`       | `8787`       |
| [Express](./express/) | `@yydb-examples/express`    | `3000`       |
| [Fastify](./fastify/) | `@yydb-examples/fastify`    | `3001`       |

Example packages use the `@yydb-examples/*` scope so an accidental publish is rejected without `@yydb` registry access.

## Layout (per example)

```text
hono/   # same shape for express/ and fastify/
  schemas/user.iris   # VOS schema beside the app
  src/iris.ts         # openIrisDb(), listUsers(), createUser()
  src/index.ts        # HTTP routes only
```

## Prerequisites

From the iris-orm repo root:

```bash
pnpm install
pnpm run build:napi
```

Examples require the Node semantic core (`@yydb/iris/node` + a platform package such as `@yydb/iris-win32-x64` on Windows).

## Run

```bash
pnpm --filter @yydb-examples/hono start
pnpm --filter @yydb-examples/express start
pnpm --filter @yydb-examples/fastify start
```

Each server exposes `GET /users` and `POST /users`.

```bash
curl http://127.0.0.1:8787/users
curl -X POST http://127.0.0.1:8787/users -H "content-type: application/json" -d "{\"user_name\":\"ada\"}"
```

## Typecheck

```bash
pnpm run examples:typecheck
```
