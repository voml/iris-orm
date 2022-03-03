# GitHub Social Preview — Iris ORM

**语言中立：** tagline 与 prompt 不得出现编程语言名、logo、后缀或典型语法片段。

File: [`social-preview.jpg`](./social-preview.jpg) — **1280×640**, target &lt; 500 KB (must be **&lt; 1 MB**).

## Upload

1. [Settings → Social preview](https://github.com/voml/iris-orm/settings#social-preview)
2. Upload **`.github/social-preview.jpg`**

## Layout brief

| Item       | Value                                       |
|------------|---------------------------------------------|
| Size       | 1280×640 (2:1，GenerateImage 后 cover 裁切) |
| Tagline    | Trustworthy, type-safe data access          |
| Primary    | `#B8860B` gold                              |
| Background | deep navy `#0F172A`                         |
| Mood       | premium fintech, dark poster                |

**构图：** 左文右图。右侧单一数据管线：`.iris schema` → **Secure Core**（hex）→ **Session API** / **Typed Access**；发光金线连接。

## Image prompt (English)

```
Ultra-wide banner, central 89% safe zone for 2:1 crop. Premium dark fintech poster, navy #0F172A, gold #B8860B glow, subtle wave texture.

LEFT: "Iris", "Trustworthy, type-safe data access".

RIGHT: architecture flow — schema document icon, gold hex "Secure Core", branches "Session API" and "Typed Access", luminous gold circuit paths, depth via glow and shadow. Structured, one clear story.

No programming language names or logos.
```

## 后处理

```bash
node path/to/postprocess-social-preview.mjs raw.png social-preview.jpg
```

## README

`![Iris ORM](.github/social-preview.jpg)`
