---
name: iris-diagnose
description: >-
  Iris doctor, adapter errors, drift, and consistency diagnostics. Use when Iris
  operations fail or drift is suspected.
---

# iris-diagnose

## Planned tools

- `doctor`

## Available today

- Host `iris doctor` where stubbed/implemented — prefer structured output when present

## Rules

1. Evidence first; redact secrets.
2. Distinguish unsupported capability vs adapter bug vs topology drift.
3. Do not “fix” by switching to SQL Studio ORM.
