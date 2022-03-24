---
name: iris-explain
description: >-
  Read Iris planner explain / capability output before applying risky migrate or
  debugging why a VOS query will not run. Do not hand-edit physical plans or
  invent SQL explains.
---

# iris-explain

Read [../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- Before applying a destructive migrate plan
- Understanding capability rejection for a VOS query
- Comparing what the planner will do vs what the agent assumed

## Do today

1. Prefer host CLI / API explain surfaces when present (`iris` help, host `plan.explain` once shipped).
2. If only diagnostics exist, **quote Iris errors** — do not invent a parallel “SQL EXPLAIN” story.
3. Optimization belongs to the Iris planner; the agent proposes VOS intent and reads the explain.

## Rules

1. Do not rewrite physical plans by hand.
2. Do not inject adapter-private commands to “make explain look green”.
3. Do not confuse with SQL Studio `query.explain`.
4. Composite plans: exactly-one Authority; respect consistency intents when documented by the host.

## Planned tools (not live)

`plan.explain` DTO is not an Agent tool yet. Use available CLI/host output only.
