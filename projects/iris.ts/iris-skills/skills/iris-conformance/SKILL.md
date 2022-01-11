---
name: iris-conformance
description: >-
  Run selected Iris conformance fixtures and collect evidence. Use for adapter
  or host verification against host-side conformance tests.
---

# iris-conformance

## Planned tools

- `conformance.run`

## Rules

1. Prefer host conformance suites (e.g. Rust `composite_conformance_15_6`) — do not invent private success criteria.
2. Evidence packs must be reproducible and secret-free.
3. Foreign adapters stay private SQL/commands; conformance asserts VOS-facing behavior.
4. Do not claim tool-live until structured DTO + CLI/MCP wiring exist.
