# `@yydb/iris`

Primary npm entry: Node N-API facade, `iris` CLI, and project config helpers. The browser WASM host lives in
[`@yydb/iris-language`](https://www.npmjs.com/package/@yydb/iris-language). Product overview:
[repository README](https://github.com/voml/iris-orm/blob/dev/README.md) · [docs](https://iris-orm.pages.dev/d/en-us/).

## Exports

| Subpath | Host | Role |
|---------|------|------|
| `@yydb/iris` | Browser / Worker (default) | `defineConfig`, `toProjectDocument` |
| `@yydb/iris/node` | Node.js | N-API facade + CLI wiring |
| `@yydb/iris/runtime` | Any host | Host-agnostic binding helpers (no WASM) |
| `@yydb/iris/types` | Any | Protocol DTOs (no loaders) |
| `@yydb/iris/node/tooling` | Node tooling | Config / codegen helpers |

There is no `@yydb/iris/web` or `@yydb/iris/wasm` subpath. Under bundler **browser** conditions, `@yydb/iris/node`
resolves to `unsupported.js`.

Platform N-API packages install as optional dependencies — see [Related packages](#related-packages). Browser WASM is
loaded by `@yydb/iris-language` from `@yydb/iris-unknown-wasm32`.

## Runtime

Node.js **>=24.12.0**. Project config (`iris.config.ts` / `.mts` / `.mjs` / `.js`) is executed by Node itself
(`await import(pathToFileURL(configPath).href)`) — Iris ships no runtime TypeScript compiler. Use erasable TypeScript
syntax in config (type annotations and `interface` are fine; `enum` / `namespace` are not).

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
import { createIris, initIris } from "@yydb/iris-language";

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
| [`@yydb/iris-language`](https://www.npmjs.com/package/@yydb/iris-language) | Browser language layer (WASM loader, executors) |
| [`@yydb/iris-unknown-wasm32`](https://www.npmjs.com/package/@yydb/iris-unknown-wasm32) | WebAssembly artifact consumed by the language layer |
| `@yydb/iris-{platform}` | N-API binaries (`win32-x64`, `linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64`) — auto-selected |
