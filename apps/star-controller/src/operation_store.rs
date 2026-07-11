//! Durable, event-backed Operation state for MCP control tools.
//!
//! This store never retries external work.  A non-terminal record found after
//! Controller restart becomes `outcome_unknown`, preserving the fact that a
//! side effect may already have happened instead of guessing that it is safe
//! to run again.

use std::{collections::BTreeMap, fs, io, path::PathBuf};

use chrono::{SecondsFormat, Utc};
use star_contracts::ids::OperationId;
use thiserror::Error;

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum OperationStoreError {
    #[error("LOCALAPPDATA is not available")]
    LocalAppDataUnavailable,
    #[error("operation state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("operation state is corrupt")]
    Corrupt,
    #[error("operation state DACL failed")]
    Dacl,
    #[error("invalid Operation state transition")]
    Transition,
    #[error("idempotency key has a different invocation identity")]
    IdempotencyConflict,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationEvent {
    pub sequence: u64,
    pub timestamp: String,
    pub phase: String,
    pub detail: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationSnapshot {
    pub operation_id: OperationId,
    pub command: String,
    pub correlation_id: String,
    pub tool_id: String,
    pub descriptor_hash: String,
    pub arguments_hash: String,
    pub status: String,
    pub accepted_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub cancellable: bool,
    pub cancel_requested: bool,
    pub cancel_effective: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub latest_event_sequence: u64,
    pub events: Vec<OperationEvent>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct IdempotencyRecord {
    operation_id: OperationId,
    invocation_hash: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationFile {
    format_version: u32,
    operations: BTreeMap<String, OperationSnapshot>,
    idempotency: BTreeMap<String, IdempotencyRecord>,
}

pub struct OperationStore {
    path: PathBuf,
    file: OperationFile,
}

pub struct OperationCreate {
    pub command: String,
    pub correlation_id: String,
    pub tool_id: String,
    pub descriptor_hash: String,
    pub arguments_hash: String,
    pub cancellable: bool,
    pub idempotency_key: Option<String>,
    pub invocation_hash: String,
}

impl OperationStore {
    pub fn default_path() -> Result<PathBuf, OperationStoreError> {
        Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA").ok_or(OperationStoreError::LocalAppDataUnavailable)?,
        )
        .join("Star-Control/state/operations.v1.json"))
    }

    pub fn load(path: PathBuf) -> Result<Self, OperationStoreError> {
        let mut file = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<OperationFile>(&bytes)
                .map_err(|_| OperationStoreError::Corrupt)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => OperationFile {
                format_version: FORMAT_VERSION,
                ..Default::default()
            },
            Err(error) => return Err(OperationStoreError::Io(error)),
        };
        if file.format_version != FORMAT_VERSION {
            return Err(OperationStoreError::Corrupt);
        }
        let mut recovered = false;
        for operation in file.operations.values_mut() {
            if !terminal(&operation.status) {
                let before_process_start = matches!(
                    operation.status.as_str(),
                    "received" | "resolving" | "approval_wait" | "queued" | "starting"
                );
                operation.status = if before_process_start {
                    operation.error = Some(serde_json::json!({
                        "code":"CONTROLLER_RECOVERED_BEFORE_PROCESS_START",
                        "retryable":false
                    }));
                    "failed"
                } else {
                    "outcome_unknown"
                }
                .to_owned();
                operation.finished_at = Some(now());
                append_event(
                    operation,
                    if before_process_start {
                        "failed"
                    } else {
                        "outcome_unknown"
                    },
                    if before_process_start {
                        "controller_recovered_before_process_start"
                    } else {
                        "controller_recovered_after_process_start"
                    },
                );
                recovered = true;
            }
        }
        let store = Self { path, file };
        if recovered {
            store.persist()?;
        }
        Ok(store)
    }

    pub fn create(
        &mut self,
        request: OperationCreate,
    ) -> Result<OperationSnapshot, OperationStoreError> {
        let OperationCreate {
            command,
            correlation_id,
            tool_id,
            descriptor_hash,
            arguments_hash,
            cancellable,
            idempotency_key,
            invocation_hash,
        } = request;
        if let Some(key) = idempotency_key.as_deref() {
            if let Some(existing) = self.file.idempotency.get(key) {
                if existing.invocation_hash != invocation_hash {
                    return Err(OperationStoreError::IdempotencyConflict);
                }
                return self
                    .file
                    .operations
                    .get(existing.operation_id.as_str())
                    .cloned()
                    .ok_or(OperationStoreError::Corrupt);
            }
        }
        let operation_id = OperationId::new();
        let timestamp = now();
        let mut operation = OperationSnapshot {
            operation_id: operation_id.clone(),
            command,
            correlation_id,
            tool_id,
            descriptor_hash,
            arguments_hash,
            status: "received".to_owned(),
            accepted_at: timestamp,
            started_at: None,
            finished_at: None,
            cancellable,
            cancel_requested: false,
            cancel_effective: false,
            result: None,
            error: None,
            latest_event_sequence: 0,
            events: Vec::new(),
        };
        append_event(&mut operation, "received", "invocation_accepted");
        self.file
            .operations
            .insert(operation_id.as_str().to_owned(), operation.clone());
        if let Some(key) = idempotency_key {
            self.file.idempotency.insert(
                key.to_owned(),
                IdempotencyRecord {
                    operation_id,
                    invocation_hash,
                },
            );
        }
        self.persist()?;
        Ok(operation)
    }

    pub fn transition(
        &mut self,
        operation_id: &str,
        next: &str,
        detail: &str,
    ) -> Result<OperationSnapshot, OperationStoreError> {
        let operation = self
            .file
            .operations
            .get_mut(operation_id)
            .ok_or(OperationStoreError::Corrupt)?;
        if terminal(&operation.status) || !valid_transition(&operation.status, next) {
            return Err(OperationStoreError::Transition);
        }
        operation.status = next.to_owned();
        if next == "running" && operation.started_at.is_none() {
            operation.started_at = Some(now());
        }
        if terminal(next) {
            operation.finished_at = Some(now());
        }
        append_event(operation, next, detail);
        let snapshot = operation.clone();
        self.persist()?;
        Ok(snapshot)
    }

    pub fn complete(
        &mut self,
        operation_id: &str,
        result: Result<serde_json::Value, serde_json::Value>,
    ) -> Result<OperationSnapshot, OperationStoreError> {
        let cancellation_effective = result.as_ref().err().is_some_and(|error| {
            error.get("code").and_then(|value| value.as_str()) == Some("TOOL_CANCELLED")
        });
        let terminal_state = if cancellation_effective {
            "cancelled"
        } else if result.is_ok() {
            "succeeded"
        } else {
            "failed"
        };
        let operation = self
            .file
            .operations
            .get_mut(operation_id)
            .ok_or(OperationStoreError::Corrupt)?;
        if terminal(&operation.status) {
            return Ok(operation.clone());
        }
        operation.status = terminal_state.to_owned();
        operation.finished_at = Some(now());
        operation.cancel_effective = cancellation_effective;
        match result {
            Ok(result) => operation.result = Some(result),
            Err(error) => operation.error = Some(error),
        }
        append_event(operation, terminal_state, "backend_finished");
        let snapshot = operation.clone();
        self.persist()?;
        Ok(snapshot)
    }

    pub fn request_cancel(
        &mut self,
        operation_id: &str,
        reason: &str,
    ) -> Result<OperationSnapshot, OperationStoreError> {
        let operation = self
            .file
            .operations
            .get_mut(operation_id)
            .ok_or(OperationStoreError::Corrupt)?;
        if terminal(&operation.status) || operation.cancel_requested {
            return Ok(operation.clone());
        }
        operation.cancel_requested = true;
        if operation.cancellable {
            operation.status = "cancelling".to_owned();
        }
        append_event(operation, "cancel_requested", reason);
        let snapshot = operation.clone();
        self.persist()?;
        Ok(snapshot)
    }

    pub fn get(&self, operation_id: &str) -> Option<OperationSnapshot> {
        self.file.operations.get(operation_id).cloned()
    }

    pub fn find_by_correlation(&self, correlation_id: &str) -> Option<OperationSnapshot> {
        self.file
            .operations
            .values()
            .find(|operation| operation.correlation_id == correlation_id)
            .cloned()
    }

    pub fn events_after(
        &self,
        operation_id: &str,
        after_sequence: u64,
    ) -> Option<Vec<OperationEvent>> {
        self.file.operations.get(operation_id).map(|operation| {
            operation
                .events
                .iter()
                .filter(|event| event.sequence > after_sequence)
                .take(256)
                .cloned()
                .collect()
        })
    }

    fn persist(&self) -> Result<(), OperationStoreError> {
        let parent = self.path.parent().ok_or(OperationStoreError::Corrupt)?;
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".operations-{}.tmp", star_ipc::nonce()));
        let bytes = serde_json::to_vec(&self.file).map_err(|_| OperationStoreError::Corrupt)?;
        fs::write(&temporary, bytes)?;
        fs::OpenOptions::new()
            .write(true)
            .open(&temporary)?
            .sync_all()?;
        fs::rename(temporary, &self.path)?;
        star_ipc::key_store::apply_owner_system_dacl(&self.path)
            .map_err(|_| OperationStoreError::Dacl)
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn append_event(operation: &mut OperationSnapshot, phase: &str, detail: &str) {
    operation.latest_event_sequence += 1;
    operation.events.push(OperationEvent {
        sequence: operation.latest_event_sequence,
        timestamp: now(),
        phase: phase.to_owned(),
        detail: detail.to_owned(),
    });
}

fn terminal(status: &str) -> bool {
    matches!(
        status,
        "succeeded" | "failed" | "cancelled" | "denied" | "expired" | "outcome_unknown"
    )
}

fn valid_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("received", "resolving" | "approval_wait" | "queued")
            | ("resolving", "approval_wait" | "queued" | "failed")
            | ("approval_wait", "queued" | "denied" | "expired")
            | ("queued", "starting")
            | ("starting", "running")
            | (
                "running",
                "succeeded" | "failed" | "cancelling" | "outcome_unknown"
            )
            | (
                "cancelling",
                "cancelled" | "succeeded" | "failed" | "outcome_unknown"
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("star-operation-{name}-{}.json", star_ipc::nonce()))
    }

    fn request(invocation_hash: &str, idempotency_key: Option<&str>) -> OperationCreate {
        OperationCreate {
            command: "tool.invoke".to_owned(),
            correlation_id: "req_01K0QBFCY78G2GB5H9VBK9Q1G8".to_owned(),
            tool_id: "user.fake.echo.run".to_owned(),
            descriptor_hash:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            arguments_hash:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            cancellable: true,
            idempotency_key: idempotency_key.map(str::to_owned),
            invocation_hash: invocation_hash.to_owned(),
        }
    }

    fn create(store: &mut OperationStore) -> OperationSnapshot {
        store.create(request("invocation", Some("key"))).unwrap()
    }

    fn create_without_idempotency(store: &mut OperationStore, suffix: &str) -> OperationSnapshot {
        let mut request = request(&format!("invocation-{suffix}"), None);
        request.correlation_id = format!("correlation-{suffix}");
        store.create(request).unwrap()
    }

    #[test]
    // matrix: MCP-O003 MCP-O004
    fn repeated_cancel_keeps_one_intent_and_terminal_state_is_immutable() {
        let mut store = OperationStore::load(path("cancel")).unwrap();
        let operation = create(&mut store);
        store
            .transition(operation.operation_id.as_str(), "queued", "ready")
            .unwrap();
        let first = store
            .request_cancel(operation.operation_id.as_str(), "user")
            .unwrap();
        let second = store
            .request_cancel(operation.operation_id.as_str(), "again")
            .unwrap();
        assert!(first.cancel_requested && second.cancel_requested);
        assert_eq!(first.latest_event_sequence, second.latest_event_sequence);
        store
            .complete(
                operation.operation_id.as_str(),
                Err(serde_json::json!({"code":"TOOL_CANCELLED"})),
            )
            .unwrap();
        let terminal = store.get(operation.operation_id.as_str()).unwrap();
        assert_eq!(terminal.status, "cancelled");
        assert!(terminal.cancel_effective);
        assert!(matches!(
            store.transition(operation.operation_id.as_str(), "running", "late"),
            Err(OperationStoreError::Transition)
        ));
        let late = store
            .complete(
                operation.operation_id.as_str(),
                Ok(serde_json::json!({"late":"result"})),
            )
            .unwrap();
        assert_eq!(late.status, "cancelled");
        assert!(late.result.is_none());
    }

    #[test]
    // matrix: MCP-I010 MCP-I011
    fn idempotency_key_reuses_only_the_exact_invocation_identity() {
        let mut store = OperationStore::load(path("idempotency")).unwrap();
        let first = create(&mut store);
        let repeated = store.create(request("invocation", Some("key"))).unwrap();
        assert_eq!(repeated.operation_id, first.operation_id);
        assert!(matches!(
            store.create(request("different-invocation", Some("key"))),
            Err(OperationStoreError::IdempotencyConflict)
        ));
    }

    #[test]
    // matrix: MCP-O002
    fn operation_events_are_paged_in_order_at_256_items() {
        let mut store = OperationStore::load(path("event-page")).unwrap();
        let operation = create(&mut store);
        let record = store
            .file
            .operations
            .get_mut(operation.operation_id.as_str())
            .unwrap();
        for index in 0..300 {
            append_event(record, "progress", &format!("event-{index}"));
        }
        let first = store
            .events_after(operation.operation_id.as_str(), 0)
            .unwrap();
        assert_eq!(first.len(), 256);
        assert!(
            first
                .windows(2)
                .all(|events| events[0].sequence < events[1].sequence)
        );
        let second = store
            .events_after(
                operation.operation_id.as_str(),
                first.last().unwrap().sequence,
            )
            .unwrap();
        assert_eq!(second.len(), 45);
        assert!(
            second
                .windows(2)
                .all(|events| events[0].sequence < events[1].sequence)
        );
    }

    #[test]
    // matrix: MCP-O007
    fn restart_marks_a_nonterminal_operation_outcome_unknown_without_retrying() {
        let path = path("recovery");
        let operation_id = {
            let mut store = OperationStore::load(path.clone()).unwrap();
            let operation = create(&mut store);
            store
                .transition(operation.operation_id.as_str(), "queued", "ready")
                .unwrap();
            store
                .transition(operation.operation_id.as_str(), "starting", "spawn")
                .unwrap();
            store
                .transition(operation.operation_id.as_str(), "running", "started")
                .unwrap();
            operation.operation_id
        };
        let store = OperationStore::load(path).unwrap();
        assert_eq!(
            store.get(operation_id.as_str()).unwrap().status,
            "outcome_unknown"
        );
    }

    #[test]
    // matrix: MCP-O005 MCP-I009
    fn durable_operation_survives_client_and_gateway_lifetime() {
        let path = path("gateway-restart");
        let operation_id = {
            let mut controller_store = OperationStore::load(path.clone()).unwrap();
            let operation = create_without_idempotency(&mut controller_store, "gateway");
            controller_store
                .transition(operation.operation_id.as_str(), "queued", "accepted")
                .unwrap();
            controller_store
                .transition(operation.operation_id.as_str(), "starting", "starting")
                .unwrap();
            controller_store
                .transition(
                    operation.operation_id.as_str(),
                    "running",
                    "process_started",
                )
                .unwrap();
            controller_store
                .complete(
                    operation.operation_id.as_str(),
                    Ok(serde_json::json!({"ok":true})),
                )
                .unwrap();
            operation.operation_id
        };
        let reconnected_gateway_view = OperationStore::load(path).unwrap();
        let operation = reconnected_gateway_view.get(operation_id.as_str()).unwrap();
        assert_eq!(operation.status, "succeeded");
        assert_eq!(operation.result, Some(serde_json::json!({"ok":true})));
    }

    #[test]
    // matrix: MCP-O006
    fn crash_before_process_start_recovers_failed_without_replay() {
        let path = path("before-process-start");
        let operation_id = {
            let mut store = OperationStore::load(path.clone()).unwrap();
            let operation = create_without_idempotency(&mut store, "queued");
            store
                .transition(operation.operation_id.as_str(), "queued", "waiting")
                .unwrap();
            operation.operation_id
        };
        let recovered = OperationStore::load(path).unwrap();
        let operation = recovered.get(operation_id.as_str()).unwrap();
        assert_eq!(operation.status, "failed");
        assert_eq!(
            operation
                .error
                .as_ref()
                .and_then(|error| error["code"].as_str()),
            Some("CONTROLLER_RECOVERED_BEFORE_PROCESS_START")
        );
        assert!(
            !operation
                .events
                .iter()
                .any(|event| event.phase == "running")
        );
    }

    #[test]
    // matrix: MCP-O008
    fn approval_approve_deny_and_expire_follow_the_frozen_state_machine() {
        let mut store = OperationStore::load(path("approval")).unwrap();
        let approved = create_without_idempotency(&mut store, "approved");
        store
            .transition(approved.operation_id.as_str(), "approval_wait", "policy")
            .unwrap();
        assert_eq!(
            store
                .transition(approved.operation_id.as_str(), "queued", "approved")
                .unwrap()
                .status,
            "queued"
        );

        for (suffix, terminal_state) in [("denied", "denied"), ("expired", "expired")] {
            let operation = create_without_idempotency(&mut store, suffix);
            store
                .transition(operation.operation_id.as_str(), "approval_wait", "policy")
                .unwrap();
            assert_eq!(
                store
                    .transition(operation.operation_id.as_str(), terminal_state, suffix)
                    .unwrap()
                    .status,
                terminal_state
            );
        }
    }
}
