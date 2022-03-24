---
name: iris-generate
description: >-
  Locally regenerate Iris host bindings (iris generate). Use after schema
  changes. Never run generate in deploy/CI/Docker; commit generated output when
  the app build requires it.
---

# iris-generate

Read [../references/workflow.md](../references/workflow.md) and
[../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- `.iris` / fingerprint changed and typed host client is stale
- `build.rs` or CI complains missing `generated/iris/rust/mod.rs` (or `generated/iris/typescript/index.ts`)
- Adding tables/fields that the generated client must expose

## Do today (real CLI)

```bash
iris generate --config iris.von --target rust
# other hosts when supported: follow iris CLI --help
```

Typical app script:

```bash
pnpm generate
```

Then **commit** the output if the consuming repo’s Docker/`cargo build` expects it in git (common for single-binary deploys).

## Rules

1. **Local developer / maintainer step only.** Deploy and TCB images must **not** install Node just to generate.
2. Generated code calls **that host’s Iris facade** (`iris::*`, `@yydb/iris`) — not a second ORM.
3. Do not hand-edit generated files; change schema and regenerate.
4. Generated Rust must include runtime metadata the host needs (e.g. `SCHEMA_FINGERPRINT`, `UUID_FIELDS`) — apps must **not** re-embed `.iris` source into binaries.
5. Do not invent a “Rust runtime + thin TS wrap” split; each host generates for itself.
6. If generate fails → fix schema or upstream generator; do not paste SQL types into generated trees.

## Antiforwards

| Wrong | Right |
|-------|--------|
| Dockerfile `RUN pnpm generate` | Generate locally → commit → Docker only `cargo build` |
| Server boot regenerates bindings | Boot uses committed / build-script inputs only |
| Skip commit because “gitignore generated” | Commit `generated/` for deploy builds that only run `cargo build` |
| Embed raw `.iris` into `build.rs` for runtime | Extend `iris generate` metadata (e.g. `UUID_FIELDS`); compile that |
| Path-overlay `@yydb/iris` to get a newer generator in CI | Bump npm / git dep after upstream publish |

## Planned tools (not live)

`generate.plan` / `generate.apply` are not Agent tools yet. Use CLI.

## Not this skill

| Need | Skill |
|------|--------|
| DDL to database | `iris-migrate` |
| Schema edit | `iris-schema` |
