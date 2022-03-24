//! Local filesystem ObjectStore (Phase 10-E).
//!
//! State machine: pending -> verified -> committed reference (or abort / deleting -> GC).
//! Visible [`ObjectReference`] files are written only on finalize; object-store write
//! failure never yields a committed reference.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use iris_ir::{
    ObjectError, ObjectHash, ObjectId, ObjectLifecycleState, ObjectMeta, ObjectReference,
    ObjectResult, require_transition,
};
use serde::{Deserialize, Serialize};

use crate::topology::ObjectPolicy;

/// Default content-hash algorithm label.
pub const OBJECT_HASH_ALG_BLAKE3: &str = "blake3";

/// Format marker for committed reference documents.
pub const OBJECT_REF_FORMAT: &str = "iris.object_ref";

/// Local filesystem object store root.
#[derive(Debug, Clone)]
pub struct FsObjectStore {
    root: PathBuf,
    policy: ObjectPolicy,
}

impl FsObjectStore {
    /// Open (or create) a store under `root`.
    pub fn open(root: impl Into<PathBuf>, policy: ObjectPolicy) -> ObjectResult<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("meta")).map_err(io_err)?;
        fs::create_dir_all(root.join("blobs")).map_err(io_err)?;
        fs::create_dir_all(root.join("refs")).map_err(io_err)?;
        Ok(Self { root, policy })
    }

    /// Store root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Borrow object policy.
    pub fn policy(&self) -> &ObjectPolicy {
        &self.policy
    }

    /// Begin a pending object (no committed reference yet).
    pub fn begin_pending(&self, object_id: ObjectId, now_unix_ms: u64) -> ObjectResult<ObjectMeta> {
        if self.meta_path(&object_id).exists() {
            return Err(ObjectError::Policy(format!(
                "object `{}` already exists",
                object_id
            )));
        }
        let meta = ObjectMeta {
            object_id: object_id.clone(),
            state: ObjectLifecycleState::Pending,
            content_hash: None,
            length: 0,
            created_unix_ms: now_unix_ms,
            updated_unix_ms: now_unix_ms,
        };
        self.write_meta(&meta)?;
        // Touch empty blob.
        fs::File::create(self.blob_path(&object_id)).map_err(io_err)?;
        Ok(meta)
    }

    /// Overwrite pending bytes (Pending only).
    pub fn write_pending(
        &self,
        object_id: &ObjectId,
        bytes: &[u8],
        now_unix_ms: u64,
    ) -> ObjectResult<ObjectMeta> {
        let mut meta = self.load_meta(object_id)?;
        if !meta.state.allows_write() {
            return Err(ObjectError::Policy(format!(
                "object `{}` state {} does not allow writes",
                object_id, meta.state
            )));
        }
        require_transition(meta.state, ObjectLifecycleState::Pending)?;
        fs::write(self.blob_path(object_id), bytes).map_err(io_err)?;
        meta.length = bytes.len() as u64;
        meta.updated_unix_ms = now_unix_ms;
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// Append pending bytes (Pending only).
    pub fn append_pending(
        &self,
        object_id: &ObjectId,
        bytes: &[u8],
        now_unix_ms: u64,
    ) -> ObjectResult<ObjectMeta> {
        let mut meta = self.load_meta(object_id)?;
        if !meta.state.allows_write() {
            return Err(ObjectError::Policy(format!(
                "object `{}` state {} does not allow writes",
                object_id, meta.state
            )));
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.blob_path(object_id))
            .map_err(io_err)?;
        file.write_all(bytes).map_err(io_err)?;
        meta.length = meta.length.saturating_add(bytes.len() as u64);
        meta.updated_unix_ms = now_unix_ms;
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// Verify content hash and transition Pending -> Verified.
    pub fn verify(
        &self,
        object_id: &ObjectId,
        expected_hash: Option<&ObjectHash>,
        now_unix_ms: u64,
    ) -> ObjectResult<ObjectMeta> {
        let mut meta = self.load_meta(object_id)?;
        require_transition(meta.state, ObjectLifecycleState::Verified)?;
        let bytes = fs::read(self.blob_path(object_id)).map_err(io_err)?;
        if bytes.len() as u64 != meta.length {
            meta.length = bytes.len() as u64;
        }
        let actual = hash_bytes(&bytes, self.hash_alg());
        if let Some(expected) = expected_hash
            && expected.as_str() != actual.as_str()
        {
            return Err(ObjectError::HashMismatch);
        }
        meta.content_hash = Some(actual);
        meta.state = ObjectLifecycleState::Verified;
        meta.updated_unix_ms = now_unix_ms;
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// Finalize Verified -> Committed and publish authority-visible reference.
    ///
    /// This is the only path that creates a committed reference document.
    pub fn finalize_commit(
        &self,
        object_id: &ObjectId,
        now_unix_ms: u64,
    ) -> ObjectResult<ObjectReference> {
        let mut meta = self.load_meta(object_id)?;
        require_transition(meta.state, ObjectLifecycleState::Committed)?;
        let hash = meta.content_hash.clone().ok_or_else(|| {
            ObjectError::Policy("cannot finalize without verified content hash".into())
        })?;
        meta.state = ObjectLifecycleState::Committed;
        meta.updated_unix_ms = now_unix_ms;
        self.write_meta(&meta)?;

        let reference = ObjectReference::committed(object_id.clone(), hash, meta.length)?;
        self.write_reference(&reference)?;
        Ok(reference)
    }

    /// Abort Pending/Verified -> Aborted (no committed reference).
    pub fn abort(&self, object_id: &ObjectId, now_unix_ms: u64) -> ObjectResult<ObjectMeta> {
        let mut meta = self.load_meta(object_id)?;
        require_transition(meta.state, ObjectLifecycleState::Aborted)?;
        // Ensure no dangling committed ref.
        let _ = fs::remove_file(self.ref_path(object_id));
        meta.state = ObjectLifecycleState::Aborted;
        meta.updated_unix_ms = now_unix_ms;
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// Mark Committed -> Deleting (soft delete before GC).
    pub fn mark_deleting(
        &self,
        object_id: &ObjectId,
        now_unix_ms: u64,
    ) -> ObjectResult<ObjectMeta> {
        let mut meta = self.load_meta(object_id)?;
        require_transition(meta.state, ObjectLifecycleState::Deleting)?;
        meta.state = ObjectLifecycleState::Deleting;
        meta.updated_unix_ms = now_unix_ms;
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// Load store meta (not the published reference).
    pub fn meta(&self, object_id: &ObjectId) -> ObjectResult<ObjectMeta> {
        self.load_meta(object_id)
    }

    /// Load published committed reference, if any.
    pub fn committed_reference(
        &self,
        object_id: &ObjectId,
    ) -> ObjectResult<Option<ObjectReference>> {
        let path = self.ref_path(object_id);
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path).map_err(io_err)?;
        let wire: ObjectRefWire = von::from_str(&text)
            .map_err(|e| ObjectError::Policy(format!("corrupt object reference: {e}")))?;
        if wire.reference.state != ObjectLifecycleState::Committed {
            return Err(ObjectError::Policy(
                "reference document is not committed".into(),
            ));
        }
        Ok(Some(wire.reference))
    }

    /// Range-read payload. Allowed for Verified (pre-publish check) and Committed.
    pub fn range_read(
        &self,
        object_id: &ObjectId,
        offset: u64,
        len: usize,
    ) -> ObjectResult<Vec<u8>> {
        let meta = self.load_meta(object_id)?;
        if !meta.state.allows_range_read() {
            return Err(ObjectError::Policy(format!(
                "object `{}` state {} does not allow range_read",
                object_id, meta.state
            )));
        }
        // Public callers should use committed refs; Verified is for finalize/preflight only.
        let mut file = fs::File::open(self.blob_path(object_id)).map_err(io_err)?;
        file.seek(SeekFrom::Start(offset)).map_err(io_err)?;
        let mut buf = vec![0_u8; len];
        let n = file.read(&mut buf).map_err(io_err)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// List object ids with meta present.
    pub fn list_ids(&self) -> ObjectResult<Vec<ObjectId>> {
        let mut out = Vec::new();
        let dir = self.root.join("meta");
        for entry in fs::read_dir(&dir).map_err(io_err)? {
            let entry = entry.map_err(io_err)?;
            let name = entry.file_name();
            let Some(stem) = Path::new(&name).file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Ok(id) = ObjectId::new(stem) {
                out.push(id);
            }
        }
        out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        Ok(out)
    }

    /// GC aborted/deleting objects and orphan pending past TTL.
    ///
    /// Returns removed object ids. Never removes Committed objects that still
    /// have a published reference unless they are Deleting.
    pub fn gc(&self, now_unix_ms: u64) -> ObjectResult<Vec<ObjectId>> {
        let pending_ttl = self
            .policy
            .pending_ttl_secs
            .or(self.policy.orphan_ttl_secs)
            .unwrap_or(86_400)
            .saturating_mul(1000);
        let mut removed = Vec::new();
        for id in self.list_ids()? {
            let meta = match self.load_meta(&id) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let age = now_unix_ms.saturating_sub(meta.updated_unix_ms);
            let remove = match meta.state {
                ObjectLifecycleState::Aborted | ObjectLifecycleState::Deleting => true,
                ObjectLifecycleState::Pending if age >= pending_ttl => {
                    // Orphan pending: must not have a committed reference.
                    self.committed_reference(&id)?.is_none()
                }
                ObjectLifecycleState::Verified if age >= pending_ttl => {
                    // Verified but never finalized -- orphan.
                    self.committed_reference(&id)?.is_none()
                }
                _ => false,
            };
            if remove {
                self.purge(&id)?;
                removed.push(id);
            }
        }
        Ok(removed)
    }

    /// Status snapshot for operators (no secrets / private commands).
    pub fn status_report(&self, now_unix_ms: u64) -> ObjectResult<ObjectStoreStatusReport> {
        let mut by_state = ObjectStateCounts::default();
        let mut dangling_refs = 0_u64;
        let mut objects = Vec::new();
        for id in self.list_ids()? {
            let meta = self.load_meta(&id)?;
            match meta.state {
                ObjectLifecycleState::Pending => by_state.pending += 1,
                ObjectLifecycleState::Verified => by_state.verified += 1,
                ObjectLifecycleState::Committed => by_state.committed += 1,
                ObjectLifecycleState::Deleting => by_state.deleting += 1,
                ObjectLifecycleState::Aborted => by_state.aborted += 1,
            }
            let has_ref = self.committed_reference(&id)?.is_some();
            if has_ref && meta.state != ObjectLifecycleState::Committed {
                dangling_refs += 1;
            }
            if meta.state == ObjectLifecycleState::Committed && !has_ref {
                dangling_refs += 1;
            }
            objects.push(ObjectStatusRow {
                object_id: id,
                state: meta.state,
                length: meta.length,
                has_committed_reference: has_ref,
                updated_unix_ms: meta.updated_unix_ms,
            });
        }
        Ok(ObjectStoreStatusReport {
            format: "iris.object_store_status".into(),
            version: 1,
            root: self.root.display().to_string(),
            hash_alg: self.hash_alg().into(),
            observed_unix_ms: now_unix_ms,
            counts: by_state,
            dangling_refs,
            objects,
            notes: vec![
                "committed references are published only by finalize".into(),
                "pending/verified orphans are GC-eligible after TTL".into(),
            ],
        })
    }

    fn hash_alg(&self) -> &str {
        self.policy
            .hash_alg
            .as_deref()
            .unwrap_or(OBJECT_HASH_ALG_BLAKE3)
    }

    fn meta_path(&self, id: &ObjectId) -> PathBuf {
        self.root.join("meta").join(format!("{}.von", id.as_str()))
    }

    fn blob_path(&self, id: &ObjectId) -> PathBuf {
        self.root.join("blobs").join(id.as_str())
    }

    fn ref_path(&self, id: &ObjectId) -> PathBuf {
        self.root.join("refs").join(format!("{}.von", id.as_str()))
    }

    fn load_meta(&self, id: &ObjectId) -> ObjectResult<ObjectMeta> {
        let path = self.meta_path(id);
        if !path.exists() {
            return Err(ObjectError::NotFound(format!("object `{id}` not found")));
        }
        let text = fs::read_to_string(&path).map_err(io_err)?;
        von::from_str(&text).map_err(|e| ObjectError::Policy(format!("corrupt object meta: {e}")))
    }

    fn write_meta(&self, meta: &ObjectMeta) -> ObjectResult<()> {
        let text = von::to_string_indented(meta)
            .map_err(|e| ObjectError::Policy(format!("serialize object meta: {e}")))?;
        fs::write(self.meta_path(&meta.object_id), text).map_err(io_err)
    }

    fn write_reference(&self, reference: &ObjectReference) -> ObjectResult<()> {
        if reference.state != ObjectLifecycleState::Committed {
            return Err(ObjectError::Policy(
                "refusing to publish non-committed object reference".into(),
            ));
        }
        let wire = ObjectRefWire {
            format: OBJECT_REF_FORMAT.into(),
            version: 1,
            reference: reference.clone(),
        };
        let text = von::to_string_indented(&wire)
            .map_err(|e| ObjectError::Policy(format!("serialize object ref: {e}")))?;
        fs::write(self.ref_path(&reference.object_id), text).map_err(io_err)
    }

    fn purge(&self, id: &ObjectId) -> ObjectResult<()> {
        let _ = fs::remove_file(self.meta_path(id));
        let _ = fs::remove_file(self.blob_path(id));
        let _ = fs::remove_file(self.ref_path(id));
        Ok(())
    }
}

fn hash_bytes(bytes: &[u8], alg: &str) -> ObjectHash {
    if alg == OBJECT_HASH_ALG_BLAKE3 {
        let hash = blake3::hash(bytes);
        ObjectHash::new(hash.to_hex().to_string())
    } else {
        // Unknown alg: still blake3, but bake alg into digest input for distinctness.
        let mut hasher = blake3::Hasher::new();
        hasher.update(alg.as_bytes());
        hasher.update(b"\0");
        hasher.update(bytes);
        ObjectHash::new(hasher.finalize().to_hex().to_string())
    }
}

fn io_err(e: std::io::Error) -> ObjectError {
    ObjectError::Policy(format!("object store io: {e}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectRefWire {
    format: String,
    version: i64,
    reference: ObjectReference,
}

/// Counts by lifecycle state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectStateCounts {
    /// Pending objects.
    pub pending: u64,
    /// Verified objects.
    pub verified: u64,
    /// Committed objects.
    pub committed: u64,
    /// Deleting objects.
    pub deleting: u64,
    /// Aborted objects.
    pub aborted: u64,
}

/// One object row in a status report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectStatusRow {
    /// Object id.
    pub object_id: ObjectId,
    /// Lifecycle state.
    pub state: ObjectLifecycleState,
    /// Length.
    pub length: u64,
    /// Whether a committed reference document exists.
    pub has_committed_reference: bool,
    /// Last update (unix ms).
    pub updated_unix_ms: u64,
}

/// Operator-facing object store status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectStoreStatusReport {
    /// Discriminator.
    pub format: String,
    /// Document version.
    pub version: i64,
    /// Store root (path, not a URL with secrets).
    pub root: String,
    /// Hash algorithm label.
    pub hash_alg: String,
    /// Observation time.
    pub observed_unix_ms: u64,
    /// Counts by state.
    pub counts: ObjectStateCounts,
    /// Inconsistencies between meta and refs.
    pub dangling_refs: u64,
    /// Per-object rows.
    pub objects: Vec<ObjectStatusRow>,
    /// Notes.
    #[serde(default)]
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::ObjectPolicy;

    fn tmp_store() -> (FsObjectStore, PathBuf) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("iris-obj-{stamp}"));
        let store = FsObjectStore::open(
            &root,
            ObjectPolicy {
                hash_alg: Some(OBJECT_HASH_ALG_BLAKE3.into()),
                orphan_ttl_secs: Some(1),
                pending_ttl_secs: Some(1),
            },
        )
        .unwrap();
        (store, root)
    }

    #[test]
    fn pending_verify_finalize_publishes_reference() {
        let (store, root) = tmp_store();
        let id = ObjectId::new("img-1").unwrap();
        store.begin_pending(id.clone(), 1_000).unwrap();
        assert!(store.committed_reference(&id).unwrap().is_none());

        let payload = b"hello-object";
        store.write_pending(&id, payload, 1_100).unwrap();
        let expected = hash_bytes(payload, OBJECT_HASH_ALG_BLAKE3);
        store.verify(&id, Some(&expected), 1_200).unwrap();

        // Still no public reference until finalize.
        assert!(store.committed_reference(&id).unwrap().is_none());
        let reference = store.finalize_commit(&id, 1_300).unwrap();
        assert_eq!(reference.state, ObjectLifecycleState::Committed);
        assert_eq!(reference.length, payload.len() as u64);
        assert!(store.committed_reference(&id).unwrap().is_some());

        let chunk = store.range_read(&id, 0, 5).unwrap();
        assert_eq!(&chunk, b"hello");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn abort_never_publishes_reference() {
        let (store, root) = tmp_store();
        let id = ObjectId::new("img-abort").unwrap();
        store.begin_pending(id.clone(), 1).unwrap();
        store.write_pending(&id, b"x", 2).unwrap();
        store.abort(&id, 3).unwrap();
        assert!(store.committed_reference(&id).unwrap().is_none());
        assert_eq!(
            store.meta(&id).unwrap().state,
            ObjectLifecycleState::Aborted
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cannot_finalize_from_pending() {
        let (store, root) = tmp_store();
        let id = ObjectId::new("img-bad").unwrap();
        store.begin_pending(id.clone(), 1).unwrap();
        let err = store.finalize_commit(&id, 2).unwrap_err();
        assert!(matches!(
            err,
            ObjectError::IllegalTransition {
                from: ObjectLifecycleState::Pending,
                to: ObjectLifecycleState::Committed
            }
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gc_removes_aborted_and_orphan_pending() {
        let (store, root) = tmp_store();
        let aborted = ObjectId::new("a").unwrap();
        store.begin_pending(aborted.clone(), 0).unwrap();
        store.abort(&aborted, 1).unwrap();

        let orphan = ObjectId::new("o").unwrap();
        store.begin_pending(orphan.clone(), 0).unwrap();

        let committed = ObjectId::new("c").unwrap();
        store.begin_pending(committed.clone(), 0).unwrap();
        store.write_pending(&committed, b"ok", 1).unwrap();
        let h = hash_bytes(b"ok", OBJECT_HASH_ALG_BLAKE3);
        store.verify(&committed, Some(&h), 2).unwrap();
        store.finalize_commit(&committed, 3).unwrap();

        let removed = store.gc(10_000).unwrap();
        assert!(removed.iter().any(|id| id.as_str() == "a"));
        assert!(removed.iter().any(|id| id.as_str() == "o"));
        assert!(!removed.iter().any(|id| id.as_str() == "c"));
        assert!(store.committed_reference(&committed).unwrap().is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hash_mismatch_rejects_verify() {
        let (store, root) = tmp_store();
        let id = ObjectId::new("bad-hash").unwrap();
        store.begin_pending(id.clone(), 1).unwrap();
        store.write_pending(&id, b"abc", 2).unwrap();
        let err = store
            .verify(&id, Some(&ObjectHash::new("deadbeef")), 3)
            .unwrap_err();
        assert_eq!(err, ObjectError::HashMismatch);
        let _ = fs::remove_dir_all(root);
    }
}
