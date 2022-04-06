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
| `rust/*.dejavu` | `generated/iris/rust/` |
| `typescript/*.dejavu` | `generated/iris/typescript/` |
