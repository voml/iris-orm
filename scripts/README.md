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

## npm placeholder (0.0.0)

Reserve package names before Trusted Publisher real releases (`@doki-land/nifty`):

```bash
pnpm placeholder          # nifty publish --placeholder --dry-run
pnpm placeholder:publish  # publish missing packages @0.0.0
pnpm placeholder:trust    # configure Trusted Publisher (needs NPM_TOTP_SECRET in .env.placeholder.local)
```

Package set: non-`private` workspace packages (`package.json`). Local secrets (gitignored): `.env.placeholder.local`.

Real versions: push tag `vX.Y.Z` or `workflow_dispatch` on `publish-npm.yml` (environment `NPM_PUBLISH`).
