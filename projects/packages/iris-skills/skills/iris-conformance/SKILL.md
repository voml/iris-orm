---
name: iris-conformance
description: >-
  Run Iris host conformance suites and collect secret-free evidence. Use when
  verifying adapters or claiming a capability is done — not for app feature work.
---

# iris-conformance

Read [../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- Changing iris-orm adapters / planner
- Claiming MySQL/Postgres/Composite behavior is complete
- Adding regression evidence for a bugfix

## Do today

1. Run the **host** conformance commands documented in the iris-orm package (e.g. Rust workspace tests / published scripts) — do not invent private pass criteria.
2. Evidence packs: reproducible, **secret-free**.
3. Foreign adapters stay private SQL/commands; conformance asserts **VOS-facing** behavior.
4. App feature work (CRUD pages, JWT, etc.) is **not** conformance — do not run this skill as a substitute for product tests.

## Rules

1. Do not claim tool-live Agent `conformance.run` until structured DTO + CLI/MCP exist.
2. Do not mark a gap “done” because an app manually applied SQL once.

## Planned tools (not live)

`conformance.run` is not an Agent tool yet.
