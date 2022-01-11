/**
 * VOS-aware local commit protocol (W2–W7).
 *
 * Authority: Spark `决策和进度表/iris-orm-architecture.md` §2.1b.
 *
 * ```text
 * W5: acquire exclusive Web Lock (when available)
 * Prepare
 *   1. OpfsCas.put immutable objects
 *   2. IDB journal prepared + object_pending(written) + deferred mutations
 * Commit
 *   3. IDB: committed + apply rows + link objects
 *   4. W6: append outbox + advance CommitToken
 *   5. BroadcastChannel notify other tabs
 * Abort / Recovery
 *   6. abort stale prepares; objects → gc; OpfsCas.gc
 * W7: probe() / collectConformanceSnapshot for quota/writable evidence
 * ```
 *
 * Invariants:
 * - Never claim IDB+OPFS single atomic transaction.
 * - Prefer OPFS-before-prepare.
 * - Multi-tab lock wraps writer paths; not a distributed txn.
 * - Outbox is local durable duty; remote sync consumer is later.
 * - Planner here accepts structured intents — not VOS source text.
 */
