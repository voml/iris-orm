//! Object store lifecycle types (Phase 10-E).
//!
//! Authority holds committed object references (id / hash / length / state).
//! The object store holds bytes under a pending -> verified -> committed machine.
//! A visible committed reference must not be published if object write/verify fails.

use serde::{Deserialize, Serialize};

/// Opaque object identity (no path separators).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub String);

impl ObjectId {
    /// Validate and wrap an object id.
    pub fn new(id: impl Into<String>) -> Result<Self, ObjectError> {
        let id = id.into();
        if id.is_empty() {
            return Err(ObjectError::InvalidId("empty object id".into()));
        }
        if id.contains('/') || id.contains('\\') || id.contains("..") {
            return Err(ObjectError::InvalidId(
                "object id must not contain path separators".into(),
            ));
        }
        Ok(Self(id))
    }

    /// Borrow the raw id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Content-address hash label (hex). Algorithm is declared by topology policy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectHash(pub String);

impl ObjectHash {
    /// Wrap a hex hash string.
    pub fn new(hex: impl Into<String>) -> Self {
        Self(hex.into())
    }

    /// Borrow hex text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Object lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectLifecycleState {
    /// Slot reserved; bytes may be written.
    Pending,
    /// Bytes present and content hash verified.
    Verified,
    /// Reference is durable and may be published by authority.
    Committed,
    /// Soft-delete; bytes eligible for GC after grace.
    Deleting,
    /// Aborted before commit; GC eligible.
    Aborted,
}

impl ObjectLifecycleState {
    /// Whether a transition to `next` is allowed.
    pub fn can_transition_to(self, next: Self) -> bool {
        use ObjectLifecycleState::*;
        matches!(
            (self, next),
            (Pending, Pending)
                | (Pending, Verified)
                | (Pending, Aborted)
                | (Verified, Committed)
                | (Verified, Aborted)
                | (Committed, Deleting)
                | (Deleting, Deleting)
                | (Aborted, Aborted)
        )
    }

    /// True when a public committed reference may exist.
    pub fn is_committed(self) -> bool {
        matches!(self, Self::Committed)
    }

    /// True when bytes may still be mutated (pending writes).
    pub fn allows_write(self) -> bool {
        matches!(self, Self::Pending)
    }

    /// True when range-read of payload is allowed.
    pub fn allows_range_read(self) -> bool {
        matches!(self, Self::Committed | Self::Verified)
    }

    /// True when GC may remove the object.
    pub fn is_gc_eligible(self) -> bool {
        matches!(self, Self::Aborted | Self::Deleting)
    }
}

impl std::fmt::Display for ObjectLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Verified => write!(f, "verified"),
            Self::Committed => write!(f, "committed"),
            Self::Deleting => write!(f, "deleting"),
            Self::Aborted => write!(f, "aborted"),
        }
    }
}

/// Authority-visible object reference (only after commit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectReference {
    /// Object id.
    pub object_id: ObjectId,
    /// Content hash.
    pub content_hash: ObjectHash,
    /// Byte length.
    pub length: u64,
    /// Must be [`ObjectLifecycleState::Committed`] when published.
    pub state: ObjectLifecycleState,
}

impl ObjectReference {
    /// Build a committed reference; rejects non-committed state.
    pub fn committed(
        object_id: ObjectId,
        content_hash: ObjectHash,
        length: u64,
    ) -> Result<Self, ObjectError> {
        Ok(Self {
            object_id,
            content_hash,
            length,
            state: ObjectLifecycleState::Committed,
        })
    }
}

/// Store-side object metadata (not a middleware private command).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectMeta {
    /// Object id.
    pub object_id: ObjectId,
    /// Lifecycle state.
    pub state: ObjectLifecycleState,
    /// Hash after verify (absent while pending).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<ObjectHash>,
    /// Declared / observed length.
    pub length: u64,
    /// Created at (unix ms).
    pub created_unix_ms: u64,
    /// Last transition (unix ms).
    pub updated_unix_ms: u64,
}

/// Object lifecycle errors (backend-neutral).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ObjectError {
    /// Invalid object id.
    #[error("{0}")]
    InvalidId(String),
    /// Illegal state transition.
    #[error("illegal object transition {from} -> {to}")]
    IllegalTransition {
        /// Current state.
        from: ObjectLifecycleState,
        /// Requested state.
        to: ObjectLifecycleState,
    },
    /// Hash mismatch on verify.
    #[error("object hash mismatch")]
    HashMismatch,
    /// Missing bytes / meta.
    #[error("{0}")]
    NotFound(String),
    /// Policy / precondition failure.
    #[error("{0}")]
    Policy(String),
}

/// Result alias for object types.
pub type ObjectResult<T> = std::result::Result<T, ObjectError>;

/// Require a legal transition or return [`ObjectError::IllegalTransition`].
pub fn require_transition(
    from: ObjectLifecycleState,
    to: ObjectLifecycleState,
) -> ObjectResult<()> {
    if from.can_transition_to(to) {
        Ok(())
    } else {
        Err(ObjectError::IllegalTransition { from, to })
    }
}
