# `@yydb/iris-unknown-wasm32`

Browser WebAssembly semantic core for [`@yydb/iris`](https://www.npmjs.com/package/@yydb/iris). **Not a typical import target** — installed as a dependency of the main package.

## Example

**Preferred** (loads this package internally):

```ts
import { initIris, createIris } from "@yydb/iris";

await initIris();
const runtime = await createIris();
```

**Direct** (low-level WASM only):

```ts
import { initWasm } from "@yydb/iris-unknown-wasm32";

await initWasm();
```

Application code should use `@yydb/iris` or `@yydb/iris/wasm` unless you own the loader. **Not WASI** — browser-safe WASM only.
