# `@yydb/iris-skills`

Agent Skills catalog for Iris / VOS workflows. Product context:
[repository README](https://github.com/voml/iris-orm/blob/dev/README.md).

## Example

```bash
npx skills add @yydb/iris-skills
```

```text
Add Iris to this repo: schemas/domain.iris with Account and Record,
iris.config.ts, iris check, iris generate, commit generated/.
VOS only — follow @yydb/iris-skills workflow.
```

Mandatory reading: [skills/references/workflow.md](./skills/references/workflow.md) ·
[consumer-hard-rules.md](./skills/references/consumer-hard-rules.md) ·
[tool-protocol.md](./skills/references/tool-protocol.md).

## Skills

| Skill | Role |
|-------|------|
| `iris-schema` | Author / check `.iris` |
| `iris-migrate` | `iris push` plan / apply (ops only) |
| `iris-generate` | `iris generate`; commit `generated/` |
| `iris-operation` | Runtime generated client |
| `iris-explain` | Planner / capability explain |
| `iris-topology` | Composite topology |
| `iris-diagnose` | Failures and drift |
| `iris-conformance` | Host conformance evidence |
