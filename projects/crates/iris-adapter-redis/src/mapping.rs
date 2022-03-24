//! Explicit Redis keyspace mapping (never invented from SCAN).

use serde::{Deserialize, Serialize};

/// How values are encoded under a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyEncoding {
    /// UTF-8 string payload.
    Utf8String,
    /// JSON object/document as UTF-8 text.
    JsonDocument,
}

/// One VOS table -> Redis keyspace mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyspaceMapping {
    /// VOS table name.
    pub vos_table: String,
    /// Key prefix, e.g. `iris:user:`.
    pub key_prefix: String,
    /// Primary-key field identity (documentation / validation only for PK ops).
    pub primary_key_field: String,
    /// Value encoding.
    pub encoding: KeyEncoding,
    /// Optional TTL applied on put (seconds).
    pub ttl_secs: Option<u64>,
}

impl KeyspaceMapping {
    /// Build the Redis key for a primary-key value.
    pub fn redis_key(&self, primary_key: &str) -> String {
        format!("{}{primary_key}", self.key_prefix)
    }
}

/// Reviewable mapping manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingManifest {
    /// Adapter id.
    pub adapter_id: String,
    /// Adapter version.
    pub adapter_version: String,
    /// Table mappings.
    pub tables: Vec<KeyspaceMapping>,
}

impl MappingManifest {
    /// Construct a manifest stamped with this adapter's identity.
    pub fn with_tables(tables: Vec<KeyspaceMapping>) -> Self {
        Self {
            adapter_id: crate::BACKEND_ID.into(),
            adapter_version: crate::ADAPTER_VERSION.into(),
            tables,
        }
    }
}
