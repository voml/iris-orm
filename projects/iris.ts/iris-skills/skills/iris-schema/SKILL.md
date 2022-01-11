---
name: iris-schema
description: >-
  Read and check VOS schema, identity, references, and diagnostics for Iris.
  Use when editing or validating schemas — never invent SQL for Iris.
---

# iris-schema

## Planned tools

- `schema.check`

## Rules

1. Operate on VOS / `.iris` — never SQL Studio SQL or `@yydb/sql-studio-orm`.
2. Prefer structured diagnostics with spans over guessing.
3. Mutating schema files is local edits; applying migrations uses `iris-migrate`.
4. Do not invent tools not in the frozen Iris DTO list.

Authority: `决策和进度表/iris-orm-architecture.md` §1.3.
