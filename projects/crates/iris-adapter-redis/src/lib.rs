//! Limited Redis keyspace / document / cache adapter.
//!
//! Redis is **not** a relational database. This adapter only supports
//! explicit keyspace mappings: primary-key get / put / delete, optional TTL,
//! and string/JSON encodings. Arbitrary filter / sort / scan of the keyspace
//! is rejected --?Iris will not pull the whole store and fake a query engine.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cache;
mod mapping;
mod ops;

use iris_ir::{
    CommitToken, ConsistencyIntent, DEFAULT_COMMIT_SHARD, IrVersion, OutboxRecord, PhysicalOp,
    PhysicalPlan,
};
use iris_types::{
    AppliedWatermarkState, CacheWatermarkProbe, CapabilitySet, CompensationBudget, QueryCaps,
    WriteCaps,
};
use redis::Client;

pub use cache::{CacheEntry, IdentityCacheResult};
pub use mapping::{KeyEncoding, KeyspaceMapping, MappingManifest};

/// Adapter identifier.
pub const BACKEND_ID: &str = "redis";

/// Adapter crate version label.
pub const ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Redis foreign datasource (keyspace adapter).
pub struct RedisSource {
    client: Client,
    mapping: MappingManifest,
}

impl std::fmt::Debug for RedisSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisSource")
            .field("backend", &BACKEND_ID)
            .field("tables", &self.mapping.tables.len())
            .finish_non_exhaustive()
    }
}

impl RedisSource {
    /// Limited capabilities: PK get/put/delete only --?no relational filter/sort.
    pub fn capabilities() -> CapabilitySet {
        CapabilitySet {
            backend_id: BACKEND_ID.into(),
            backend_version: ADAPTER_VERSION.into(),
            ir_version_max: IrVersion::PHASE1,
            query: QueryCaps::scan_only(),
            write: WriteCaps::full(),
            budget: CompensationBudget {
                max_rows: 1,
                max_round_trips: 8,
                max_assoc_fanout: 1,
                ..CompensationBudget::default()
            },
        }
    }

    /// Connect with an explicit mapping manifest (required --?no auto catalog invent).
    pub fn connect(url: &str, mapping: MappingManifest) -> Result<Self> {
        if mapping.tables.is_empty() {
            return Err(Error::Policy(
                "Redis adapter requires at least one explicit keyspace mapping".into(),
            ));
        }
        let client = Client::open(url).map_err(Error::Redis)?;
        Ok(Self { client, mapping })
    }

    /// Ping a Redis URL without inventing a catalog (connectivity only).
    pub fn ping_url(url: &str) -> Result<()> {
        let client = Client::open(url).map_err(Error::Redis)?;
        let mut conn = client.get_connection().map_err(Error::Redis)?;
        let pong: String = redis::cmd("PING").query(&mut conn).map_err(Error::Redis)?;
        if pong.eq_ignore_ascii_case("PONG") || pong == "OK" {
            Ok(())
        } else {
            Err(Error::Policy(format!("unexpected PING reply: {pong}")))
        }
    }

    /// Draft a reviewable keyspace mapping from a VOS schema (does not SCAN Redis).
    ///
    /// Candidates use `iris:<table>:` prefixes and exact primary-key fields. Users must
    /// review/edit before runtime connect --?Redis will not invent mappings from the store.
    pub fn draft_keyspace_mapping(vos_schema: &str) -> Result<MappingManifest> {
        let document = vos::parser::parse_document(vos_schema).map_err(|d| {
            Error::Policy(format!(
                "parse schema: {}",
                d.errors
                    .first()
                    .map(|e| e.message.as_str())
                    .unwrap_or("unknown")
            ))
        })?;
        let mut tables = Vec::new();
        for item in &document.items {
            let vos::ast::Item::Table(table) = item else {
                continue;
            };
            let pk = table
                .fields
                .iter()
                .find(|f| f.is_primary())
                .ok_or_else(|| {
                    Error::Policy(format!(
                        "table `{}` has no primary key --?Redis keyspace mapping requires one",
                        table.name
                    ))
                })?;
            tables.push(KeyspaceMapping {
                vos_table: table.name.clone(),
                key_prefix: format!("iris:{}:", table.name.to_ascii_lowercase()),
                primary_key_field: pk.name.clone(),
                encoding: KeyEncoding::JsonDocument,
                ttl_secs: None,
            });
        }
        if tables.is_empty() {
            return Err(Error::Policy(
                "VOS schema has no tables to map into Redis keyspace".into(),
            ));
        }
        Ok(MappingManifest::with_tables(tables))
    }

    /// Borrow the mapping manifest.
    pub fn mapping(&self) -> &MappingManifest {
        &self.mapping
    }

    /// GET by primary key under the mapped prefix.
    pub fn get_primary(&self, table: &str, primary_key: &str) -> Result<Option<String>> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        ops::get_primary(&mut conn, map, primary_key)
    }

    /// SET (optionally with TTL) by primary key.
    pub fn put_primary(&self, table: &str, primary_key: &str, value: &str) -> Result<()> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        ops::put_primary(&mut conn, map, primary_key, value)
    }

    /// SET NX --?atomic create-if-absent.
    pub fn put_primary_nx(&self, table: &str, primary_key: &str, value: &str) -> Result<bool> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        ops::put_primary_nx(&mut conn, map, primary_key, value)
    }

    /// DELETE by primary key.
    pub fn delete_primary(&self, table: &str, primary_key: &str) -> Result<bool> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        ops::delete_primary(&mut conn, map, primary_key)
    }

    /// TTL remaining in seconds (`-1` if no expiry, `-2` if missing).
    pub fn ttl_primary(&self, table: &str, primary_key: &str) -> Result<i64> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        ops::ttl_primary(&mut conn, map, primary_key)
    }

    // --- Phase 10-C: Cache role (watermark / invalidate / 回源 / RYW / BoundedStale) ---

    /// Read the cache [`AppliedWatermarkState`] for a shard (default shard if `None`).
    pub fn cache_watermark(&self, shard: Option<&str>) -> Result<Option<AppliedWatermarkState>> {
        let shard = shard.unwrap_or(DEFAULT_COMMIT_SHARD);
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::get_watermark(&mut conn, shard)
    }

    /// Probe a Cache watermark without a keyspace mapping (Phase 10-D status).
    ///
    /// Does not invent catalog mappings; only reads `iris:wm:<shard>`.
    pub fn probe_watermark_url(
        url: &str,
        shard: Option<&str>,
    ) -> Result<Option<AppliedWatermarkState>> {
        let client = Client::open(url).map_err(Error::Redis)?;
        let mut conn = client.get_connection().map_err(Error::Redis)?;
        cache::get_watermark(&mut conn, shard.unwrap_or(DEFAULT_COMMIT_SHARD))
    }

    /// Overwrite the cache watermark (tests / rebuild). Prefer [`Self::cache_advance_watermark`].
    pub fn cache_set_watermark(&self, state: &AppliedWatermarkState) -> Result<()> {
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::set_watermark(&mut conn, state)
    }

    /// Monotonically advance the cache watermark to at least `token.seq`.
    pub fn cache_advance_watermark(
        &self,
        token: &CommitToken,
        applied_unix_ms: u64,
    ) -> Result<AppliedWatermarkState> {
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::advance_watermark(&mut conn, token, applied_unix_ms)
    }

    /// Get a versioned cache entry (projection body), distinct from raw keyspace GET.
    pub fn cache_get_entry(&self, table: &str, primary_key: &str) -> Result<Option<CacheEntry>> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::get_entry(&mut conn, &map.key_prefix, primary_key)
    }

    /// Put a versioned cache entry (may set TTL from mapping).
    pub fn cache_put_entry(
        &self,
        table: &str,
        primary_key: &str,
        entry: &CacheEntry,
    ) -> Result<()> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::put_entry(&mut conn, &map.key_prefix, primary_key, entry, map.ttl_secs)
    }

    /// Invalidate a cache entry (discard projection for one identity).
    pub fn cache_invalidate(&self, table: &str, primary_key: &str) -> Result<bool> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::invalidate_entry(&mut conn, &map.key_prefix, primary_key)
    }

    /// Apply one authority outbox record into the cache (idempotent projector step).
    pub fn cache_apply_outbox(
        &self,
        record: &OutboxRecord,
        payload: Option<&str>,
        now_unix_ms: u64,
    ) -> Result<AppliedWatermarkState> {
        let map = self.table(&record.table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::apply_outbox(
            &mut conn,
            &map.key_prefix,
            map.ttl_secs,
            record,
            payload,
            now_unix_ms,
        )
    }

    /// Identity read via cache under a consistency intent (hit / bypass / fail-closed).
    ///
    /// On [`IdentityCacheResult::Hit`] with `entry: None`, the coordinator should fill from
    /// Authority under the topology stampede budget. Redis is never treated as write truth.
    pub fn cache_identity_read(
        &self,
        table: &str,
        primary_key: &str,
        intent: &ConsistencyIntent,
        session_fence: Option<&CommitToken>,
        now_unix_ms: u64,
        shard: Option<&str>,
    ) -> Result<IdentityCacheResult> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::identity_cache_read(
            &mut conn,
            &map.key_prefix,
            primary_key,
            intent,
            session_fence,
            now_unix_ms,
            shard.unwrap_or(DEFAULT_COMMIT_SHARD),
        )
    }

    /// Fill cache from an authority-sourced value after miss/bypass (under stampede budget).
    pub fn cache_fill_from_authority(
        &self,
        table: &str,
        primary_key: &str,
        entry: &CacheEntry,
        token: &CommitToken,
        now_unix_ms: u64,
    ) -> Result<AppliedWatermarkState> {
        let map = self.table(table)?;
        let mut conn = self.client.get_connection().map_err(Error::Redis)?;
        cache::fill_from_authority(
            &mut conn,
            &map.key_prefix,
            primary_key,
            entry,
            map.ttl_secs,
            token,
            now_unix_ms,
        )
    }

    /// Reject relational physical plans that would require keyspace scans.
    pub fn execute_plan(&self, plan: &PhysicalPlan) -> Result<Vec<iris_types::Row>> {
        if plan.is_rejected() {
            return Err(Error::Unsupported(
                plan.rejection_note().unwrap_or("plan rejected").to_string(),
            ));
        }
        for node in &plan.nodes {
            match &node.op {
                PhysicalOp::Filter { .. }
                | PhysicalOp::Sort { .. }
                | PhysicalOp::Skip { .. }
                | PhysicalOp::Take { .. }
                | PhysicalOp::Project { .. } => {
                    return Err(Error::Unsupported(
                        "Redis adapter rejects filter/sort/page/project over the keyspace; \
                         use explicit primary-key get/put/delete"
                            .into(),
                    ));
                }
                PhysicalOp::Scan { .. } => {
                    return Err(Error::Unsupported(
                        "Redis adapter rejects full-table Scan; use get_primary with an explicit key"
                            .into(),
                    ));
                }
                PhysicalOp::Collect => {}
            }
        }
        Err(Error::Unsupported(
            "Redis adapter does not execute relational physical plans".into(),
        ))
    }

    fn table<'a>(&'a self, name: &str) -> Result<&'a KeyspaceMapping> {
        self.mapping
            .tables
            .iter()
            .find(|t| t.vos_table == name)
            .ok_or_else(|| {
                Error::Policy(format!(
                    "no keyspace mapping for VOS table `{name}` --?Redis will not invent one"
                ))
            })
    }
}

/// Adapter errors.
#[derive(Debug)]
pub enum Error {
    /// redis-rs failure.
    Redis(redis::RedisError),
    /// Capability / mapping policy.
    Policy(String),
    /// Operation not supported on Redis keyspace adapter.
    Unsupported(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Redis(e) => write!(f, "{e}"),
            Self::Policy(s) | Self::Unsupported(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Redis(e) => Some(e),
            _ => None,
        }
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

impl CacheWatermarkProbe for RedisSource {
    fn probe_watermark(
        &self,
        shard: Option<&str>,
    ) -> std::result::Result<Option<AppliedWatermarkState>, String> {
        self.cache_watermark(shard).map_err(|e| e.to_string())
    }
}

/// URL-only Cache watermark probe (no keyspace mapping required).
pub struct RedisWatermarkProbe {
    url: String,
}

impl RedisWatermarkProbe {
    /// Create a probe bound to a Redis URL (env-expanded by the caller).
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

impl CacheWatermarkProbe for RedisWatermarkProbe {
    fn probe_watermark(
        &self,
        shard: Option<&str>,
    ) -> std::result::Result<Option<AppliedWatermarkState>, String> {
        RedisSource::probe_watermark_url(&self.url, shard).map_err(|e| e.to_string())
    }
}
