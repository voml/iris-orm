# VMZ issues found on Iris homepage

Findings while building `@yydb/iris-homepage` on **npm `@vmz/*@0.1.8`**. Confirmed defects should migrate to `规划设计/vmz/` when that tree is restored.

| ID | Area | Symptom | Expected | Blocks site? |
|----|------|---------|----------|--------------|
| I-01 | `@vmz/ui-icons` | No semantic `tool.locale` / `nav.language` in registry | Locale switcher gets a registered globe/language mark | No — using `tool.ascii` + `label` |
| I-02 | `@vmz/ui` `CodeBlock` | Published API is `caption` + `copyLabel`; no `language` / `title` props | Syntax label + optional language for docs | No — using `caption` |
| I-03 | `@vmz/ui` `Dropdown` | Items are `{ id, title, href }` only | Callable items or `onSelect` for `__vmzTransitionLocale` without URL rewrite | No — locale menu uses `Button variant="ghost"` in `<details>` |
| I-04 | `@vmz/ui` `AppShell` | Nav slot is flat `{ id, title, href }[]` | Extension slot or actions region for locale control + external CTA | No — custom header with `Link`/`Button`/`Icon` |
| I-05 | `@vmz/ui` `Link` | Underline-primary style only | `variant="nav"` / muted chrome link for header/footer | No — page SCSS targets `.site-nav .vmz-ui-link` |
| I-06 | Documents | `/d/` uses plugin markdown shell only | Optional `Prose` / `Breadcrumb` / `Callout` chrome without second runtime | No — deferred |

## How to add an issue

1. Reproduce on **registry** deps (`pnpm install` with no `file:` / link).
2. Note package (`core` / `ui` / `ui-icons` / `vmz` CLI), route, and minimal `.vmz` snippet.
3. Append a row above; link VMZ issue/PR when filed upstream.

## Local link workflow (temporary only)

```bash
# Example — do not commit resulting lock/package.json
cd vmz-framework/packages/ui/vmz-ui && pnpm link --global
cd iris-orm/projects/iris.ts/homepage && pnpm link --global @vmz/ui

# Restore before push
pnpm install   # in homepage — refreshes registry tarballs
```
