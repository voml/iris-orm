# `@yydb/iris-types`

TypeScript declarations for the Rust Iris binding surface.

Not a public application dependency — apps use [`@yydb/iris`](../iris/). This package describes session, plan,
capability, and diagnostic values crossing the N-API / browser-WASM boundary. It must not become a parallel runtime or
semantic source of truth.
