//! Star-Control MCP contract v1.
//!
//! This crate is the only owner of MCP, IPC, registry, manifest, trust, cache,
//! and external-process wire types.  It deliberately has no filesystem,
//! process, or transport dependency.

pub mod canonical;
pub mod coordination_v2;
pub mod development;
pub mod development_effect;
pub mod development_v2;
pub mod evidence;
pub mod evidence_v2;
pub mod fixed_mcp;
pub mod ids;
pub mod index;
pub mod installation;
pub mod ipc;
pub mod maintenance_v2;
pub mod managed_registry;
pub mod management;
pub mod manifest;
pub mod migration_v2;
pub mod orchestration;
pub mod patch_v2;
pub mod planning;
pub mod profile;
pub mod recovery;
pub mod registry;
pub mod release_v2;
pub mod runtime;
pub mod rust_style;
pub mod schema;
pub mod strict_json;
pub mod trust;
pub mod validator_guard;

pub use canonical::{Sha256Hash, canonical_sha256, jcs_bytes};
pub use ids::{
    ApprovalId, ArtifactId, BackupSetId, ChangeSetId, CheckoutId, DiagnosticId, EvaluationRunId,
    EvidenceBundleId, GateId, GoalId, ImpactAnalysisId, InstallationId, LocalStateBundleId,
    ManagedRegistrySnapshotId, OperationId, PatchApplicationId, ProjectId, RecipeExecutionId,
    RecoveryPlanId, RegistryConsistencyRecordId, ReleaseManifestId, RequestId, RunId,
    ScopeRevisionId, StageId, TaskInvocationId, TaskSpecId, ToolCacheId, ToolTrustId,
    ValidationPlanId, ValidationRunId, ValidatorGuardEvidenceId, WaiverId, WorktreeDecisionId,
};
pub use management::{MANAGEMENT_STORE_VERSION, REDACTION_CONTRACT_VERSION};
pub use manifest::{ToolPackageManifest, parse_manifest_v1};
pub use strict_json::parse_no_duplicate_keys;

/// Frozen MCP contract version from `mcp-implementation-contract.md`.
pub const MCP_CONTRACT_VERSION: u32 = 1;
/// The product supports this protocol and the stated compatibility floor.
pub const MCP_PROTOCOL_CURRENT: &str = "2025-11-25";
pub const MCP_PROTOCOL_COMPATIBILITY_FLOOR: &str = "2025-06-18";
