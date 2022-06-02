# `@yydb/iris-linux-arm64`

Prebuilt N-API binary for **Linux arm64**. **Not a public import target.**

Pulled automatically when [`@yydb/iris`](https://www.npmjs.com/package/@yydb/iris) installs on `linux` + `arm64`.

[Product README](https://github.com/voml/iris-orm/blob/dev/README.md)

## Example

Application code on Linux arm64 — this package loads when `@yydb/iris/node` starts:

```ts
import { createIris } from "@yydb/iris/node";

const runtime = await createIris();
runtime.checkSource(`table Account { @@account_id: uuid }`);
```
