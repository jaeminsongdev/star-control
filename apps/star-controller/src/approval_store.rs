//! Durable approval scopes for pre-side-effect policy gates.

use std::{collections::BTreeMap, fs, io, path::PathBuf};

use chrono::{SecondsFormat, Utc};
use star_contracts::{
    ApprovalId, OperationId, Sha256Hash, canonical::canonical_sha256, fixed_mcp::ApprovalDecision,
    parse_no_duplicate_keys,
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
    #[serde(default)]
    pub runtime_scope: serde_json::Value,
    pub decision: Option<ApprovalDecision>,
    pub resolved_at: Option<String>,
    #[serde(default)]
    pub decision_reason: Option<String>,
    #[serde(default)]
    pub decision_conditions: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub resolved_by: Option<serde_json::Value>,
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
    pub runtime_scope: serde_json::Value,
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

fn approval_scope_hash(
    approval_id: &ApprovalId,
    scope: &ApprovalScope,
) -> Result<Sha256Hash, ApprovalStoreError> {
    canonical_sha256(&serde_json::json!({
        "approval_id": approval_id,
        "tool_id": scope.tool_id,
        "descriptor_hash": scope.descriptor_hash,
        "arguments_hash": scope.arguments_hash,
        "permission_actions": scope.permission_actions,
        "paid_limit": scope.paid_limit,
        "target_refs": scope.target_refs,
        "expected_revision": scope.expected_revision,
    }))
    .map_err(|_| ApprovalStoreError::Corrupt)
}

impl ApprovalStore {
    pub fn default_path() -> Result<PathBuf, ApprovalStoreError> {
        Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA").ok_or(ApprovalStoreError::LocalAppDataUnavailable)?,
        )
        .join("Star-Control/state/approvals.v1.json"))
    }

    pub fn load(path: PathBuf) -> Result<Self, ApprovalStoreError> {
        let file: ApprovalFile = match fs::read(&path) {
            Ok(bytes) => {
                let text = std::str::from_utf8(&bytes).map_err(|_| ApprovalStoreError::Corrupt)?;
                let value =
                    parse_no_duplicate_keys(text).map_err(|_| ApprovalStoreError::Corrupt)?;
                serde_json::from_value(value).map_err(|_| ApprovalStoreError::Corrupt)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => ApprovalFile {
                format_version: FORMAT_VERSION,
                ..Default::default()
            },
            Err(error) => return Err(error.into()),
        };
        if file.format_version != FORMAT_VERSION {
            return Err(ApprovalStoreError::Corrupt);
        }
        for (key, record) in &file.records {
            let scope = ApprovalScope {
                operation_id: record.operation_id.clone(),
                tool_id: record.tool_id.clone(),
                descriptor_hash: record.descriptor_hash.clone(),
                arguments_hash: record.arguments_hash.clone(),
                permission_actions: record.permission_actions.clone(),
                paid_limit: record.paid_limit.clone(),
                target_refs: record.target_refs.clone(),
                expected_revision: record.expected_revision,
                arguments: record.arguments.clone(),
                actor: record.actor.clone(),
                runtime_scope: record.runtime_scope.clone(),
            };
            let mut normalized_permissions = record.permission_actions.clone();
            normalized_permissions.sort();
            normalized_permissions.dedup();
            if key != record.approval_id.as_str()
                || normalized_permissions != record.permission_actions
                || approval_scope_hash(&record.approval_id, &scope)? != record.scope_hash
                || record.decision.is_some() != record.resolved_at.is_some()
            {
                return Err(ApprovalStoreError::Corrupt);
            }
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
        let scope_hash = approval_scope_hash(&approval_id, &scope)?;
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
            runtime_scope: scope.runtime_scope,
            decision: None,
            resolved_at: None,
            decision_reason: None,
            decision_conditions: None,
            resolved_by: None,
        };
        self.file
            .records
            .insert(approval_id.to_string(), record.clone());
        self.persist()?;
        Ok(record)
    }

    /// Returns the durable approval record without changing its decision.
    ///
    /// Remote coordination uses this read path immediately before a Git
    /// effect so an earlier approval response cannot be substituted for the
    /// exact, currently persisted scope.
    pub fn get(&self, approval_id: &ApprovalId) -> Option<ApprovalRecord> {
        self.file.records.get(approval_id.as_str()).cloned()
    }

    pub fn find_unresolved_exact(
        &self,
        tool_id: &str,
        arguments_hash: &Sha256Hash,
        expected_revision: Option<u64>,
    ) -> Option<ApprovalRecord> {
        self.file
            .records
            .values()
            .find(|record| {
                record.tool_id == tool_id
                    && &record.arguments_hash == arguments_hash
                    && record.expected_revision == expected_revision
                    && record.decision.is_none()
            })
            .cloned()
    }

    pub fn resolve(
        &mut self,
        approval_id: &ApprovalId,
        scope_hash: &Sha256Hash,
        decision: ApprovalDecision,
        reason: Option<String>,
        conditions: Option<serde_json::Map<String, serde_json::Value>>,
        resolved_by: serde_json::Value,
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
            record.decision_reason = reason;
            record.decision_conditions = conditions;
            record.resolved_by = Some(resolved_by);
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
    fn durable_approval_json_rejects_duplicate_keys() {
        let path = std::env::temp_dir().join(format!(
            "star-approval-duplicate-{}.json",
            star_ipc::nonce()
        ));
        fs::write(
            &path,
            br#"{"format_version":1,"format_version":1,"records":{}}"#,
        )
        .unwrap();
        assert!(matches!(
            ApprovalStore::load(path),
            Err(ApprovalStoreError::Corrupt)
        ));
    }

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
                runtime_scope: serde_json::json!({}),
            })
            .unwrap();
        let approved = store
            .resolve(
                &record.approval_id,
                &record.scope_hash,
                ApprovalDecision::Approve,
                Some("reviewed".to_owned()),
                Some(serde_json::Map::new()),
                serde_json::json!({"kind":"mcp"}),
            )
            .unwrap();
        let repeated = store
            .resolve(
                &record.approval_id,
                &record.scope_hash,
                ApprovalDecision::Approve,
                Some("different repeated reason".to_owned()),
                None,
                serde_json::json!({"kind":"replay"}),
            )
            .unwrap();
        assert_eq!(approved.resolved_at, repeated.resolved_at);
        assert_eq!(approved.decision_reason.as_deref(), Some("reviewed"));
        assert_eq!(repeated.decision_reason, approved.decision_reason);
        assert_eq!(repeated.decision_conditions, approved.decision_conditions);
        assert_eq!(repeated.resolved_by, approved.resolved_by);
        assert!(matches!(
            store.resolve(
                &record.approval_id,
                &record.scope_hash,
                ApprovalDecision::Deny,
                None,
                None,
                serde_json::Value::Null,
            ),
            Err(ApprovalStoreError::Stale)
        ));
        assert!(matches!(
            store.resolve(
                &record.approval_id,
                &Sha256Hash::digest(b"stale"),
                ApprovalDecision::Approve,
                None,
                None,
                serde_json::Value::Null,
            ),
            Err(ApprovalStoreError::Stale)
        ));
    }

    #[test]
    fn scope_hash_uses_only_the_frozen_public_scope_fields() {
        let approval_id = ApprovalId::new();
        let mut scope = ApprovalScope {
            operation_id: OperationId::new(),
            tool_id: "user.fake.paid.run".to_owned(),
            descriptor_hash: Sha256Hash::digest(b"descriptor"),
            arguments_hash: Sha256Hash::digest(b"arguments"),
            permission_actions: vec!["paid_action".to_owned()],
            paid_limit: serde_json::Value::Null,
            target_refs: vec![
                serde_json::json!({"kind":"project_root","path_hash":Sha256Hash::digest(b"project")}),
            ],
            expected_revision: Some(7),
            arguments: serde_json::json!({"private":"value"}),
            actor: serde_json::json!({"private":"actor"}),
            runtime_scope: serde_json::json!({"private":"scope"}),
        };
        let expected = approval_scope_hash(&approval_id, &scope).unwrap();
        scope.arguments = serde_json::json!({"changed":true});
        scope.actor = serde_json::json!({"changed":true});
        scope.runtime_scope = serde_json::json!({"changed":true});
        assert_eq!(approval_scope_hash(&approval_id, &scope).unwrap(), expected);
        scope.target_refs = vec![
            serde_json::json!({"kind":"project_root","path_hash":Sha256Hash::digest(b"other")}),
        ];
        assert_ne!(approval_scope_hash(&approval_id, &scope).unwrap(), expected);
    }
}
