# `@yydb/iris-skills`

Official Agent Skills for **Iris / VOS** consumers and maintainers.

```text
Agent → iris-skills (workflow rules) → npm `@yydb/iris` CLI / host Iris facade
```

Agents edit **VOS / `.iris`** and call Iris — **not** SQL. Parallel package: `@yydb/sql-studio-skills` (SQL Studio only).

## Read first

- [skills/references/consumer-hard-rules.md](./skills/references/consumer-hard-rules.md) — antiforwards (no SQL bypass, no deploy generate, no path overlay in CI)
- [skills/references/tool-protocol.md](./skills/references/tool-protocol.md) — live CLI vs planned DTOs

## Skills

| Skill | Role | Delivery |
|-------|------|----------|
| `iris-schema` | Author/check `.iris` | **CLI-backed** (`iris check`) |
| `iris-migrate` | `iris push` plan/apply | **CLI-backed** |
| `iris-generate` | Local `iris generate`; commit outputs | **CLI-backed** |
| `iris-operation` | Runtime VOS / generated client | docs + host API |
| `iris-explain` | Planner / capability explain | docs / CLI when present |
| `iris-topology` | Composite topology verify | docs / CLI when present |
| `iris-diagnose` | Failures & drift; upstream fixes | CLI-backed habits |
| `iris-conformance` | Host conformance evidence | docs / host tests |

**Gate:** structured Agent tool DTOs (`migration.apply`, …) are **not live**. Teach the real CLI. Do not prompt-fake missing MCP tools.

## Install

```bash
npx skills add @yydb/iris-skills
# or local checkout:
npx skills add ./projects/iris.ts/iris-skills --skill '*' -y --copy
```

App repos typically vendor a copy under `.agents/skills/` from the installed npm package (see app `AGENTS.md`).
