# `@yydb/iris-win32-x64`

Prebuilt N-API binary for **Windows x64**. **Not a public import target.**

Pulled automatically when [`@yydb/iris`](https://www.npmjs.com/package/@yydb/iris) installs on `win32` + `x64`.

[Product README](https://github.com/voml/iris-orm/blob/dev/README.md)

## Example

Application code on Windows x64 — this package loads when `@yydb/iris/node` starts:

```ts
import { createIris } from "@yydb/iris/node";

const runtime = await createIris();
runtime.checkSource(`table Account { @@account_id: uuid }`);
```
