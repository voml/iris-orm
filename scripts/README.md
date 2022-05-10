# scripts/

Root automation for formatting, Rust checks, and npm publish.

## Layout

```text
scripts/
  format.mjs                 Biome format wrapper
  check-rs.mjs               cargo check wrapper
  test-rs.mjs                cargo test wrapper
  typecheck-ts.mjs           TypeScript typecheck wrapper
  ci/
    publish-npm.mjs          Real release (OIDC via publish-npm.yml)
```

## npm placeholder and Trusted Publisher

Package set: `nifty.config.ts` `publish.packages` (eight `@yydb/iris*` packages).

Trusted Publisher contract (must match npm registry settings per package):

| Field | Value |
|-------|-------|
| Repository | `voml/iris-orm` |
| Workflow file | `publish-npm.yml` |
| Environment | `NPM_PUBLISH` |

One-time local setup (gitignored `.env.placeholder.local`):

```bash
# NPM_TOKEN=npm_...          # from npm login or access token with publish + 2FA
# NPM_TOTP_SECRET=...        # base32 secret when the account uses 2FA
pnpm placeholder:publish     # publish missing @0.0.0 stubs
pnpm placeholder:trust -- --refresh
pnpm placeholder:trust -- --only @yydb/iris-linux-arm64   # single package
```

Dry run:

```bash
pnpm placeholder             # nifty publish --placeholder --dry-run
pnpm placeholder:trust -- --dry-run
```

Real versions: push tag `vX.Y.Z` or `workflow_dispatch` on `publish-npm.yml` (GitHub environment `NPM_PUBLISH`). CI uses OIDC only. Each `@yydb/iris*` package needs its own Trusted Publisher row pointing at the same workflow contract.
