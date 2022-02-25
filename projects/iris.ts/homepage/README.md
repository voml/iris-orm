# Iris homepage (`@yydb/iris-homepage`)

**Live:** [https://iris-orm.pages.dev/](https://iris-orm.pages.dev/)

Official Iris ORM site — [VMZ](https://github.com/vmz-framework/vmz-framework) **0.1.9** stack:

- `@vmz/core`, `@vmz/ui`, `@vmz/ui-icons`, `@vmz/vmz` (CLI); `@vmz/plugin-markdown-it` (dev — document integrate only)

## Commands

```bash
# from iris-orm repo root
pnpm install
pnpm homepage:dev          # → dist/cdn/
pnpm homepage              # release build → dist/cdn/
pnpm homepage:check
pnpm --filter @yydb/iris-homepage document:check
pnpm run verify:homepage-hosts
```

Build uses delivery profile **`static`** (`static-cdn` assembly, `vmz.config.ts`). Output folder is **`dist/cdn`** — upload to any CDN / static host (Cloudflare Pages, Netlify, …).

```text
dist/cdn/    # upload this tree to CDN / static hosting
```

Override output target: `VMZ_OUT_TARGET=cdn` (default) or `--target cdn` on `scripts/build.mjs`.

- Landing: `/`
- Documents: `/d/zh-hans/` · `/d/en-us/`

UI components are used as **bare tags** (`<Button>`, `<Card>`, `<Icon>`, …) discovered from npm dependencies — no `import { Button } from '@vmz/ui'`.

## Static host (Cloudflare Pages / Netlify / …)

| Field | Value |
|-------|--------|
| Root directory | `projects/iris.ts/homepage` |
| Build command | `pnpm build` |
| Build output directory | `dist/cdn` |
| Node version | 20 or 22 |

Do **not** enable SPA fallback — routes are pre-rendered HTML.

## Dependency discipline

| Scenario | Rule |
|----------|------|
| CI / commits | **npm registry only** — pin `@vmz/*@0.1.9`, lock resolves registry tarballs |
| Local VMZ bugfix | Temporary `pnpm link` or `file:` **on your machine only** — never commit linked `package.json` / lock |
| After local test | Restore npm deps and re-run `check` + `build` before push |

See [VMZ_ISSUES.md](./VMZ_ISSUES.md) for framework/UI gaps found while building this site.

Product messaging follows the Rust-core + N-API binding model. TypeScript `@yydb/iris-adapter-*` packages have been removed from the repo.
