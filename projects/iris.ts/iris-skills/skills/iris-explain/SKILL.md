---
name: iris-explain
description: >-
  Inspect Iris capability proof, routing, and physical/composite plans. Use
  before authorized apply or execute.
---

# iris-explain

## Planned tools

- `plan.explain`

## Rules

1. Optimization decisions belong to Iris planner — Agent proposes intent and reads explain.
2. Do not rewrite physical plans by hand or inject adapter-private commands.
3. Composite plans must respect exactly-one Authority and consistency intents.
4. Do not confuse with SQL Studio `query.explain`.
