---
name: iris-topology
description: >-
  Verify Authority, projection, outbox, and watermark contracts for Composite
  Backend. Use when checking Iris topologies.
---

# iris-topology

## Planned tools

- `topology.verify`
- `projection.verify`

## Rules

1. Exactly one Authority per entity; Cache/Search/etc. are roles.
2. Freshness = CommitToken vs AppliedWatermark — not wall-clock alone.
3. Apps speak VOS + consistency intent — never `fromRedis` / `toPostgresAndRedis`.
4. Do not paper over single-adapter gaps with Composite shortcuts.
