# `@yydb/iris-adapter-web`

Browser **local** Iris / VOS data source using **IndexedDB + OPFS**.

> Not “connect to a remote database from the browser.”  
> A local, durable, offline-capable VOS store **inside** the browser.

|            |                                                                         |
|------------|-------------------------------------------------------------------------|
| **Is**     | Iris Web Local Store / Offline Authority **candidate**                  |
| **Is not** | `@yydb/yydb`, desktop `.yydb`, SQL foreign adapter, or invisible SQLite |
| **Not**    | `@yydb/sql-studio-orm` storage backend                                  |

## Status

| Stage                                                | State                                           |
|------------------------------------------------------|-------------------------------------------------|
| W0 namespace / probe                                 | done                                            |
| **W1** catalog + fingerprint                         | **done**                                        |
| **W2** row R/W + structured plan/query/execute       | **done** (not full VOS language parser)         |
| **W3** journal + recovery                            | **done**                                        |
| **W4** OPFS CAS + GC                                 | **done**                                        |
| **W5** multi-tab (Web Locks + BroadcastChannel)      | **done**                                        |
| **W6** local outbox + CommitToken / AppliedWatermark | **done** (local durable; not remote sync)       |
| **W7** quota / writable / persistence snapshot       | **done** (probe evidence; not full soak matrix) |

```ts
import { createWebSource } from "@yydb/iris-adapter-web";

const source = createWebSource({ name: "app-main" });
await source.open();

await source.installSchema({
  schemaId: "main",
  contractVersion: "1",
  canonical: "...",
});
const fp = (await source.getSchema("main"))!.fingerprint;

// Structured intents → plan → execute (adapter-side; Iris TS planner still separate)
await source.execute({
  semanticHash: "op-1",
  schemaFingerprint: fp,
  consistency: "Authoritative",
  writes: [{ op: "upsert", entity: "User", id: "u1", fields: { name: "Ada" } }],
  reads: [{ op: "get", entity: "User", id: "u1", fields: ["name"] }],
});

const snap = await source.probe(); // W7 quota / writable / multiContext
```

## Boundaries

```text
VOS source  ->  @yydb/iris (future TS planner)
            ->  WebPhysicalPlan intents  ->  iris-adapter-web
```

This package plans/executes **structured web intents**. It does **not** parse VOS text and must not invent SQL.
See [local-commit-protocol](./documentation/local-commit-protocol.md).

Roadmap: Spark `决策和进度表/iris-orm-architecture.md` §2.1b.
