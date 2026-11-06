# `@yydb/iris-unknown-wasm32`

Browser WebAssembly semantic core for [`@yydb/iris-language`](https://www.npmjs.com/package/@yydb/iris-language). **Not a typical import target** — installed as a dependency of the language layer.

The wasm-bindgen glue (`iris_wasm.js`) and the `iris_wasm_bg.wasm` asset are produced by the `iris` facade's `iris-wasm` binding and ship committed under `src/lib`; this repository does not carry a local Rust wasm crate. The package build copies them to `dist/lib`.

## Version snapshot

The artifacts under `src/lib` are a snapshot of the pre-restructure `database.rs` workspace, not a build of the rev pinned in the root `Cargo.toml`. At runtime `irisVersion()` and `checkSource().generatorVersion` both report `0.1.0`, while the Python binding built from the pinned rev reports `iris.version()` = `0.0.1`.

They load and expose the same binding surface (`checkSource`, `introspectSchema`, `MemorySession`, ...), so downstream usage keeps working, but this version metadata must not be read as the pinned facade version. Rebuilding is the facade's job (`projects/facades/iris-wasm`); after a rebuild, replace both files in `src/lib` and refresh the consumers' `IRIS_GENERATOR_VERSION`.

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
