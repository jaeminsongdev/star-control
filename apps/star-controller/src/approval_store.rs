//! Durable approval scopes for pre-side-effect policy gates.

use std::{collections::BTreeMap, fs, io, path::PathBuf};

use chrono::{SecondsFormat, Utc};
use star_contracts::{
    ApprovalId, OperationId, Sha256Hash, canonical::canonical_sha256, fixed_mcp::ApprovalDecision,
};
use thiserror::Error;

const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRecord {
    pub approval_id: ApprovalId,
    pub scope_hash: Sha256Hash,
    pub operation_id: OperationId,
    pub tool_id: String,
    pub descriptor_hash: Sha256Hash,
    pub arguments_hash: Sha256Hash,
    pub permission_actions: Vec<String>,
    pub paid_limit: serde_json::Value,
    pub target_refs: Vec<serde_json::Value>,
    pub expected_revision: Option<u64>,
    /// Normalized input is retained only as a controller-private pending
    /// dispatch record. SecretRef values remain references, never secret text.
    #[serde(default)]
    pub arguments: serde_json::Value,
    #[serde(default)]
    pub actor: serde_json::Value,
    pub decision: Option<ApprovalDecision>,
    pub resolved_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ApprovalScope {
    pub operation_id: OperationId,
    pub tool_id: String,
    pub descriptor_hash: Sha256Hash,
    pub arguments_hash: Sha256Hash,
    pub permission_actions: Vec<String>,
    pub paid_limit: serde_json::Value,
    pub target_refs: Vec<serde_json::Value>,
    pub expected_revision: Option<u64>,
    pub arguments: serde_json::Value,
    pub actor: serde_json::Value,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalFile {
    format_version: u32,
    records: BTreeMap<String, ApprovalRecord>,
}

#[derive(Debug, Error)]
pub enum ApprovalStoreError {
    #[error("LOCALAPPDATA is not available")]
    LocalAppDataUnavailable,
    #[error("approval state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("approval state is corrupt")]
    Corrupt,
    #[error("approval state DACL failed")]
    Dacl,
    #[error("POLICY_APPROVAL_STALE")]
    Stale,
}

pub struct ApprovalStore {
    path: PathBuf,
    file: ApprovalFile,
}

impl ApprovalStore {
    pub fn default_path() -> Result<PathBuf, ApprovalStoreError> {
        Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA").ok_or(ApprovalStoreError::LocalAppDataUnavailable)?,
        )
        .join("Star-Control/state/approvals.v1.json"))
    }

    pub fn load(path: PathBuf) -> Result<Self, ApprovalStoreError> {
        let file = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| ApprovalStoreError::Corrupt)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => ApprovalFile {
                format_version: FORMAT_VERSION,
                ..Default::default()
            },
            Err(error) => return Err(error.into()),
        };
        if file.format_version != FORMAT_VERSION {
            return Err(ApprovalStoreError::Corrupt);
        }
        Ok(Self { path, file })
    }

    pub fn create(
        &mut self,
        mut scope: ApprovalScope,
    ) -> Result<ApprovalRecord, ApprovalStoreError> {
        scope.permission_actions.sort();
        scope.permission_actions.dedup();
        let approval_id = ApprovalId::new();
        let scope_hash = canonical_sha256(&serde_json::json!({
            "approval_id": approval_id,
            "tool_id": scope.tool_id,
            "descriptor_hash": scope.descriptor_hash,
            "arguments_hash": scope.arguments_hash,
            "permission_actions": scope.permission_actions,
            "paid_limit": scope.paid_limit,
            "target_refs": scope.target_refs,
            "expected_revision": scope.expected_revision,
        }))
        .map_err(|_| ApprovalStoreError::Corrupt)?;
        let record = ApprovalRecord {
            approval_id: approval_id.clone(),
            scope_hash,
            operation_id: scope.operation_id,
            tool_id: scope.tool_id,
            descriptor_hash: scope.descriptor_hash,
            arguments_hash: scope.arguments_hash,
            permission_actions: scope.permission_actions,
            paid_limit: scope.paid_limit,
            target_refs: scope.target_refs,
            expected_revision: scope.expected_revision,
            arguments: scope.arguments,
            actor: scope.actor,
            decision: None,
            resolved_at: None,
        };
        self.file
            .records
            .insert(approval_id.to_string(), record.clone());
        self.persist()?;
        Ok(record)
    }

    pub fn resolve(
        &mut self,
        approval_id: &ApprovalId,
        scope_hash: &Sha256Hash,
        decision: ApprovalDecision,
    ) -> Result<ApprovalRecord, ApprovalStoreError> {
        let record = self
            .file
            .records
            .get_mut(approval_id.as_str())
            .ok_or(ApprovalStoreError::Stale)?;
        if &record.scope_hash != scope_hash
            || record
                .decision
                .as_ref()
                .is_some_and(|existing| existing != &decision)
        {
            return Err(ApprovalStoreError::Stale);
        }
        if record.decision.is_none() {
            record.decision = Some(decision);
            record.resolved_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true));
            let record = record.clone();
            self.persist()?;
            return Ok(record);
        }
        Ok(record.clone())
    }

    fn persist(&self) -> Result<(), ApprovalStoreError> {
        let parent = self.path.parent().ok_or(ApprovalStoreError::Corrupt)?;
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".approvals-{}.tmp", star_ipc::nonce()));
        fs::write(
            &temporary,
            serde_json::to_vec(&self.file).map_err(|_| ApprovalStoreError::Corrupt)?,
        )?;
        fs::OpenOptions::new()
            .write(true)
            .open(&temporary)?
            .sync_all()?;
        fs::rename(temporary, &self.path)?;
        star_ipc::key_store::apply_owner_system_dacl(&self.path)
            .map_err(|_| ApprovalStoreError::Dacl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // matrix: MCP-S011
    fn approval_resolution_is_idempotent_only_for_the_exact_scope_and_decision() {
        let path =
            std::env::temp_dir().join(format!("star-approval-store-{}.json", star_ipc::nonce()));
        let mut store = ApprovalStore::load(path).unwrap();
        let record = store
            .create(ApprovalScope {
                operation_id: OperationId::new(),
                tool_id: "user.fake.paid.run".to_owned(),
                descriptor_hash: Sha256Hash::digest(b"descriptor"),
                arguments_hash: Sha256Hash::digest(b"arguments"),
                permission_actions: vec!["paid_action".to_owned()],
                paid_limit: serde_json::Value::Null,
                target_refs: vec![],
                expected_revision: Some(7),
                arguments: serde_json::json!({"value":"same"}),
                actor: serde_json::Value::Null,
            })
            .unwrap();
        let approved = store
            .resolve(
                &record.approval_id,
                &record.scope_hash,
                ApprovalDecision::Approve,
            )
            .unwrap();
        let repeated = store
            .resolve(
                &record.approval_id,
                &record.scope_hash,
                ApprovalDecision::Approve,
            )
            .unwrap();
        assert_eq!(approved.resolved_at, repeated.resolved_at);
        assert!(matches!(
            store.resolve(
                &record.approval_id,
                &record.scope_hash,
                ApprovalDecision::Deny,
            ),
            Err(ApprovalStoreError::Stale)
        ));
        assert!(matches!(
            store.resolve(
                &record.approval_id,
                &Sha256Hash::digest(b"stale"),
                ApprovalDecision::Approve,
            ),
            Err(ApprovalStoreError::Stale)
        ));
    }
}
