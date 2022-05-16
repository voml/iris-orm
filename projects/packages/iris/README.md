# `@yydb/iris`

Primary npm entry: browser facade, Node N-API facade, `iris` CLI, and project config helpers. Product overview:
[repository README](https://github.com/voml/iris-orm/blob/dev/README.md) · [docs](https://iris-orm.pages.dev/d/en-us/).

## Exports

| Subpath | Host | Role |
|---------|------|------|
| `@yydb/iris` | Browser / Worker (default) | `initIris`, `createIris`, `defineConfig`, `openLocalStore` |
| `@yydb/iris/node` | Node.js | N-API facade + CLI wiring |
| `@yydb/iris/wasm` | Browser (advanced) | Low-level WASM loader |
| `@yydb/iris/types` | Any | Protocol DTOs (no loaders) |
| `@yydb/iris/node/tooling` | Node tooling | Config / codegen helpers |

There is no `@yydb/iris/web`. Under bundler **browser** conditions, `@yydb/iris/node` resolves to `unsupported.js`.

Platform N-API packages and `@yydb/iris-unknown-wasm32` install as optional / bundled dependencies — see [Related packages](#related-packages).

## Example

**Project config**

```ts
import { defineConfig } from "@yydb/iris";

export default defineConfig({
    schema: "schemas/**/*.iris",
    datasources: { default: { kind: "yydb", mode: "native_pull", path: ".iris/dev.yydb" } },
    generate: { out: "generated/iris", target: "typescript" },
});
```

**Browser — check schema source**

```ts
import { initIris, createIris } from "@yydb/iris";

await initIris();
const runtime = await createIris();
runtime.checkSource(`table Account { @@account_id: uuid, display_name: utf8 }`);
```

**Node — same runtime API**

```ts
import { createIris } from "@yydb/iris/node";

const runtime = await createIris();
runtime.checkSource(`table Account { @@account_id: uuid, display_name: utf8 }`);
```

**CLI**

```bash
npx iris check
npx iris generate
```

Do not branch on `typeof window` in a shared entry — bundlers may pull both N-API and WASM.

## Related packages

| Package | Role |
|---------|------|
| [`@yydb/iris-unknown-wasm32`](https://www.npmjs.com/package/@yydb/iris-unknown-wasm32) | Browser WASM artifact (via default facade) |
| `@yydb/iris-{platform}` | N-API binaries (`win32-x64`, `linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64`) — auto-selected |
