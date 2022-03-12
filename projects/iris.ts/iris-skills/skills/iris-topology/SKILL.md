---
name: iris-topology
description: >-
  Check Iris Composite topology contracts (Authority, projection, outbox,
  watermark). Use for multi-role backends — not for inventing fromRedis-style
  app APIs.
---

# iris-topology

Read [../references/consumer-hard-rules.md](../references/consumer-hard-rules.md) first.

## When to use

- App configures more than one Iris role (Authority + cache/search/…)
- Verifying topology / projection before activate
- Debugging stale-read / watermark issues in Composite setups

## Do today

1. Treat topology files / host verify commands as source of truth when present.
2. Apps speak **VOS + consistency intent** — never app-level `fromRedis` / `toPostgresAndRedis`.
3. Single-adapter apps: do not invent Composite shortcuts to paper over missing capability.

## Rules

1. Exactly one Authority per entity; Cache/Search/etc. are roles.
2. Freshness = CommitToken vs AppliedWatermark — not wall-clock alone.
3. Do not claim Composite readiness without host verify evidence.

## Planned tools (not live)

`topology.verify` / `projection.verify` are not Agent tools yet.
