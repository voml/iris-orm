# `@yydb/iris-skills`

Official Agent Skills for **Iris / VOS** (architecture §1.3).

```text
Agent -> iris-skills -> structured Iris tools -> policy -> Iris facade
```

Agents edit VOS and Iris plans — **not** SQL. Parallel package:
`@yydb/sql-studio-skills` (SQL Studio only).

## First-batch skills

| Skill              | Role                                   | Delivery  |
|--------------------|----------------------------------------|-----------|
| `iris-schema`      | schema check                           | docs-only |
| `iris-operation`   | VOS op/query + semantic check          | docs-only |
| `iris-generate`    | Dejavu generate / fingerprint          | cli-stub  |
| `iris-migrate`     | migration plan/review/apply            | docs-only |
| `iris-explain`     | capability / physical / composite plan | docs-only |
| `iris-topology`    | topology + projection verify           | docs-only |
| `iris-diagnose`    | doctor / drift                         | cli-stub  |
| `iris-conformance` | conformance.run                        | docs-only |

**Gate:** freeze shared Iris tool DTOs (CLI/MCP/HTTP), then mark skills `tool-live`. Do not prompt-fake missing
commands.

## Install

```bash
npx skills add @yydb/iris-skills
# or local checkout:
npx skills add ./projects/iris.ts/iris-skills
```
