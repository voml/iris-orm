# Generation templates

Dejavu (`.dejavu`) sources for Iris bindings live here.

Rules:

- Templates are the generation source of truth.
- Default path is parse-AOT via `iris-generator` (`build.rs` → `render_registered`).
- Dejavu must not learn Iris/VOS/database specifics beyond the `GenerationModel` JSON context passed at render time.
- Rust/TS presentation helpers (`rust_view.rs`, `ts_view.rs`) may derive template fields from the model, but must not emit final source text.

## Layout

| Path | Target |
|------|--------|
| `rust/*.dejavu` | `src/generated/iris/rust/` |
| `typescript/*.dejavu` | `src/generated/iris/` |
| `typescript/runtime/*.dejavu` | `projects/iris.ts/packages/{iris,iris-language}/src/runtime/` |

`typescript/runtime` is the single author source for the host-neutral runtime
modules both TypeScript host packages carry. Regenerate the committed copies with
`cargo run -p iris-generator --bin iris-runtime-sync`, and keep host differences in
each package's `runtime/host.ts` adapter.
