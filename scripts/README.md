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
    publish-placeholder.mjs  0.0.0 stubs + npm trust setup
```

## npm placeholder (0.0.0)

Reserve package names before Trusted Publisher real releases:

```bash
pnpm placeholder          # status
pnpm placeholder:publish  # publish @yydb/iris + platform stubs @0.0.0
pnpm placeholder:trust    # configure Trusted Publisher (needs NPM_TOTP_SECRET in .env.placeholder.local)
```

Local secrets (gitignored): `.env.placeholder.local` at repo root.

Real versions: push tag `vX.Y.Z` or `workflow_dispatch` on `publish-npm.yml` (environment `NPM_PUBLISH`).
