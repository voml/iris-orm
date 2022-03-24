//! Topology activate + version handshake (Phase 10-G).
//!
//! `plan -> verify -> activate`. Activation records the published topology
//! version so rolling upgrades can handshake; writers must not silently use
//! divergent route rules against the same authority.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::topology::{TopologyContract, TopologyError};

/// Format marker for activation documents.
pub const TOPOLOGY_ACTIVATION_FORMAT: &str = "iris.topology_activation";

/// Rolling-upgrade handshake between previously active and newly activated versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyHandshake {
    /// Lowest topology version readers must still accept during rollout.
    pub min_reader_version: i64,
    /// Version writers must use after activation.
    pub writer_version: i64,
    /// Notes (no secrets / private commands).
    #[serde(default)]
    pub notes: Vec<String>,
}

/// Durable activation record for one logical topology id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyActivation {
    /// Discriminator.
    pub format: String,
    /// Document version.
    pub version: i64,
    /// Topology id.
    pub topology_id: String,
    /// Activated topology contract version.
    pub topology_version: i64,
    /// Previous activated version, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<i64>,
    /// Activation time (unix ms).
    pub activated_unix_ms: u64,
    /// Handshake for rolling upgrade.
    pub handshake: TopologyHandshake,
    /// Preflight notes from activate.
    #[serde(default)]
    pub preflight_notes: Vec<String>,
}

/// Result of an activate attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyActivateReport {
    /// Whether activation succeeded.
    pub ok: bool,
    /// Activation record when ok (or dry-run preview).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<TopologyActivation>,
    /// Path written, when persisted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_path: Option<String>,
    /// Operator notes / errors.
    #[serde(default)]
    pub notes: Vec<String>,
}

/// Load the current activation for a topology id from `state_dir`.
pub fn load_activation(
    state_dir: impl AsRef<Path>,
    topology_id: &str,
) -> Result<Option<TopologyActivation>, TopologyError> {
    let path = activation_path(state_dir.as_ref(), topology_id);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(TopologyError::Io)?;
    let act: TopologyActivation = von::from_str(&text).map_err(TopologyError::Von)?;
    if act.format != TOPOLOGY_ACTIVATION_FORMAT {
        return Err(TopologyError::Invalid(format!(
            "unsupported activation format {}",
            act.format
        )));
    }
    Ok(Some(act))
}

/// Activate a validated topology (writes `{state_dir}/{id}.active.von`).
///
/// Handshake rules:
/// - first activation: `min_reader_version = writer_version = topology_version`
/// - upgrade (`new > old`): writers use `new`; readers still accept `old` as min
/// - same version: idempotent re-activate
/// - downgrade (`new < old`): refused unless `force`
pub fn activate_topology(
    topo: &TopologyContract,
    state_dir: impl AsRef<Path>,
    now_unix_ms: u64,
    force: bool,
) -> Result<TopologyActivateReport, TopologyError> {
    topo.validate()?;
    let state_dir = state_dir.as_ref();
    fs::create_dir_all(state_dir).map_err(TopologyError::Io)?;

    let mut preflight = verify_report_for_activate(topo)?;
    let previous = load_activation(state_dir, &topo.id)?;

    let (previous_version, handshake, mut notes) = match &previous {
        None => {
            let hs = TopologyHandshake {
                min_reader_version: topo.topology_version,
                writer_version: topo.topology_version,
                notes: vec!["first activation".into()],
            };
            (None, hs, vec!["no previous activation".into()])
        }
        Some(prev) if prev.topology_version == topo.topology_version => {
            let hs = TopologyHandshake {
                min_reader_version: prev.handshake.min_reader_version,
                writer_version: topo.topology_version,
                notes: vec!["idempotent re-activate of same topology_version".into()],
            };
            (
                Some(prev.topology_version),
                hs,
                vec!["re-activating identical topology_version".into()],
            )
        }
        Some(prev) if topo.topology_version > prev.topology_version => {
            let hs = TopologyHandshake {
                min_reader_version: prev.topology_version,
                writer_version: topo.topology_version,
                notes: vec![
                    "rolling upgrade: readers must accept previous and new versions".into(),
                    "writers must use writer_version only".into(),
                ],
            };
            (
                Some(prev.topology_version),
                hs,
                vec![format!(
                    "upgrade {} -> {}",
                    prev.topology_version, topo.topology_version
                )],
            )
        }
        Some(prev) => {
            if !force {
                return Ok(TopologyActivateReport {
                    ok: false,
                    activation: None,
                    state_path: None,
                    notes: vec![format!(
                        "refusing downgrade {} -> {} without --force (handshake would break writers)",
                        prev.topology_version, topo.topology_version
                    )],
                });
            }
            let hs = TopologyHandshake {
                min_reader_version: topo.topology_version,
                writer_version: topo.topology_version,
                notes: vec!["forced downgrade; operators must drain old writers".into()],
            };
            (
                Some(prev.topology_version),
                hs,
                vec![format!(
                    "forced downgrade {} -> {}",
                    prev.topology_version, topo.topology_version
                )],
            )
        }
    };

    preflight.append(&mut notes);
    let activation = TopologyActivation {
        format: TOPOLOGY_ACTIVATION_FORMAT.into(),
        version: 1,
        topology_id: topo.id.clone(),
        topology_version: topo.topology_version,
        previous_version,
        activated_unix_ms: now_unix_ms,
        handshake,
        preflight_notes: preflight,
    };

    let path = activation_path(state_dir, &topo.id);
    let text = von::to_string_indented(&activation).map_err(TopologyError::Von)?;
    let tmp = path.with_extension("von.tmp");
    fs::write(&tmp, &text).map_err(TopologyError::Io)?;
    fs::rename(&tmp, &path).map_err(TopologyError::Io)?;

    Ok(TopologyActivateReport {
        ok: true,
        activation: Some(activation),
        state_path: Some(path.display().to_string()),
        notes: vec!["topology activated; route rules published for this version".into()],
    })
}

/// True when a reader topology_version is within the active handshake window.
pub fn reader_version_accepted(activation: &TopologyActivation, reader_version: i64) -> bool {
    reader_version >= activation.handshake.min_reader_version
        && reader_version
            <= activation
                .handshake
                .writer_version
                .max(activation.topology_version)
}

/// True when a writer must use exactly the activated writer_version.
pub fn writer_version_ok(activation: &TopologyActivation, writer_version: i64) -> bool {
    writer_version == activation.handshake.writer_version
}

fn activation_path(state_dir: &Path, topology_id: &str) -> PathBuf {
    state_dir.join(format!("{topology_id}.active.von"))
}

fn verify_report_for_activate(topo: &TopologyContract) -> Result<Vec<String>, TopologyError> {
    let mut notes = crate::topology::verify_report(topo)?;
    // Preflight: ensure core access kinds can be planned without panicking.
    for access in [
        iris_ir::AccessKind::IdentityRead,
        iris_ir::AccessKind::Write,
        iris_ir::AccessKind::Search,
        iris_ir::AccessKind::VectorNearest,
        iris_ir::AccessKind::BytesRange,
        iris_ir::AccessKind::Effect,
    ] {
        let plan = topo.plan(access, iris_ir::ConsistencyIntent::Eventual, None)?;
        notes.push(format!(
            "preflight {access:?}: rejected={} steps={}",
            plan.rejected,
            plan.steps.len()
        ));
    }
    // Writes must never route through cache as truth.
    let write = topo.plan(
        iris_ir::AccessKind::Write,
        iris_ir::ConsistencyIntent::Authoritative,
        None,
    )?;
    let write_has_authority = write
        .steps
        .iter()
        .any(|s| matches!(s, iris_ir::CompositeStep::AuthorityStep { .. }));
    if !write_has_authority && !write.rejected {
        return Err(TopologyError::Invalid(
            "activate preflight: write plan missing AuthorityStep".into(),
        ));
    }
    notes.push("preflight: write path retains Authority (cache/search never write-truth)".into());
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{
        CachePolicy, ComponentRole, FallbackPolicy, ObjectPolicy, OutboxPolicy, ProjectionPolicy,
        RouteRule, TOPOLOGY_FORMAT, TopologyComponent,
    };
    use iris_ir::ConsistencyIntent;
    use std::collections::BTreeMap;

    fn sample(version: i64) -> TopologyContract {
        let mut components = BTreeMap::new();
        components.insert(
            "pg".into(),
            TopologyComponent {
                role: ComponentRole::Authority,
                adapter: "postgres".into(),
                adapter_version: None,
                datasource: Some("main".into()),
            },
        );
        components.insert(
            "redis".into(),
            TopologyComponent {
                role: ComponentRole::Cache,
                adapter: "redis".into(),
                adapter_version: None,
                datasource: Some("cache".into()),
            },
        );
        let mut routes = BTreeMap::new();
        routes.insert(
            "identity_read".into(),
            RouteRule {
                default_intent: ConsistencyIntent::Eventual,
                preferred_component: Some("redis".into()),
                fallback: FallbackPolicy::Authority,
            },
        );
        TopologyContract {
            format: TOPOLOGY_FORMAT.into(),
            version: 1,
            id: "commerce".into(),
            topology_version: version,
            components,
            tables: BTreeMap::new(),
            routes,
            cache: CachePolicy::default(),
            outbox: OutboxPolicy::default(),
            object: ObjectPolicy::default(),
            projection: ProjectionPolicy::default(),
        }
    }

    #[test]
    fn activate_upgrade_handshake_and_reject_downgrade() {
        let root = std::env::temp_dir().join(format!(
            "iris-act-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let r1 = activate_topology(&sample(1), &root, 1000, false).unwrap();
        assert!(r1.ok);
        let act = r1.activation.unwrap();
        assert_eq!(act.handshake.writer_version, 1);
        assert!(writer_version_ok(&act, 1));
        assert!(reader_version_accepted(&act, 1));

        let r2 = activate_topology(&sample(2), &root, 2000, false).unwrap();
        assert!(r2.ok);
        let act2 = r2.activation.unwrap();
        assert_eq!(act2.previous_version, Some(1));
        assert_eq!(act2.handshake.min_reader_version, 1);
        assert_eq!(act2.handshake.writer_version, 2);
        assert!(reader_version_accepted(&act2, 1));
        assert!(reader_version_accepted(&act2, 2));
        assert!(!reader_version_accepted(&act2, 0));
        assert!(!writer_version_ok(&act2, 1));

        let down = activate_topology(&sample(1), &root, 3000, false).unwrap();
        assert!(!down.ok);
        let forced = activate_topology(&sample(1), &root, 3001, true).unwrap();
        assert!(forced.ok);
        let _ = fs::remove_dir_all(root);
    }
}
