# `@yydb/iris-skills`

Official Agent Skills for **Iris / VOS**.

```text
Agent → iris-skills → npm `@yydb/iris` CLI (local) / Iris runtime (deploy)
```

## Correct workflow (mandatory)

Read **[skills/references/workflow.md](./skills/references/workflow.md)** before any Iris work.

```text
edit .iris → iris check → iris generate → COMMIT generated/
          → iris push --plan → iris push        # human ops, never CI
deploy:   runtime crates + generated only       # no CLI / no migrate / no seed
```

Also: [consumer-hard-rules.md](./skills/references/consumer-hard-rules.md) · [tool-protocol.md](./skills/references/tool-protocol.md)

Agents edit **VOS / `.iris`** — **not** SQL. Parallel: `@yydb/sql-studio-skills` (SQL Studio only).

## Skills

| Skill | Role | Delivery |
|-------|------|----------|
| `iris-schema` | Author/check `.iris` | CLI-backed (`iris check`) |
| `iris-migrate` | `iris push` plan/apply — **local/ops only** | CLI-backed |
| `iris-generate` | Local `iris generate`; **commit** outputs | CLI-backed |
| `iris-operation` | Runtime VOS / generated client | docs + host API |
| `iris-explain` | Planner / capability explain | docs / CLI when present |
| `iris-topology` | Composite topology | docs / CLI when present |
| `iris-diagnose` | Failures & drift; upstream fixes | CLI-backed habits |
| `iris-conformance` | Host conformance evidence | docs / host tests |

Structured Agent tool DTOs (`migration.apply`, …) are **not live**. Teach the real CLI.

## Install

```bash
npx skills add @yydb/iris-skills
# or local checkout:
npx skills add ./projects/iris.ts/iris-skills --skill '*' -y --copy
```
