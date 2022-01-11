---
name: iris-generate
description: >-
  Host Dejavu generate with fingerprint and drift checks. Use when regenerating
  Iris language bindings for the current host.
---

# iris-generate

## Planned tools

- `generate.plan`
- `generate.apply`

Apply binds plan hash + schema fingerprint + host fingerprint + short grant.

## Available today (CLI stub / host)

- Host `iris generate` paths where implemented (Rust/TS hosts differ)
- Treat missing structured JSON output as not-yet-tool-live

## Rules

1. Generate is per-host — never emit “Rust runtime + thin TS wrap”.
2. Bindings call that host’s Iris facade (`iris::*` or `@yydb/iris`).
3. plan → apply; drift invalidates apply.
