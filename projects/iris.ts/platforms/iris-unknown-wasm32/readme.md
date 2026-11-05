# `@yydb/iris-unknown-wasm32`

Browser WebAssembly semantic core for [`@yydb/iris-language`](https://www.npmjs.com/package/@yydb/iris-language). **Not a typical import target** — installed as a dependency of the language layer.

The wasm-bindgen glue (`iris_wasm.js`) and the `iris_wasm_bg.wasm` asset are produced by the `iris` facade's `iris-wasm` binding and ship committed under `src/lib`; this repository does not carry a local Rust wasm crate. The package build copies them to `dist/lib`.

## Example

**Preferred** (loads this package internally):

```ts
import { initIris, createIris } from "@yydb/iris-language";

await initIris();
const runtime = await createIris();
```

**Direct** (low-level WASM only):

```ts
import { initWasm } from "@yydb/iris-unknown-wasm32";

await initWasm();
```

Application code should use `@yydb/iris-language` unless you own the loader. **Not WASI** — browser-safe WASM only.
