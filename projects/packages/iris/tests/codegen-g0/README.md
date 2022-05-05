# G0 — TypeScript generated client fixtures

Contract fixtures for `iris-generate-ux-dx.md` §8–§9 (G0 gate). Each file is copied next to a freshly generated blog schema client and checked with `tsc`.

## Positive (`positive/`)

| File | Contract |
|------|----------|
| `satisfies-nested-select.ts` | `satisfies` reuse, nested `{ select }`, `{ is }` reference filter |
| `find-first.ts` | `findFirst` cardinality and payload projection |
| `branded-id-create.ts` | `userId()` / `postId()` constructors, no `as` cast |
| `find-unique-required.ts` | `findUnique` with required unique key |

## Negative (`negative/`)

| File | Expected failure |
|------|------------------|
| `invalid-filter-type.ts` | wrong scalar operator value type |
| `empty-find-unique-where.ts` | empty `where` on `findUnique` |
| `excess-select-field.ts` | unknown select key |
| `access-unselected-field.ts` | field not included in `select` |
| `cross-entity-id.ts` | branded ID from another entity |
| `missing-create-field.ts` | required create field omitted |

Run via `pnpm --filter @yydb/iris test` (`codegen-g0.test.ts`).
