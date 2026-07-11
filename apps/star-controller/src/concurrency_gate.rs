//! Deterministic, bounded Operation concurrency gate.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationLockKey {
    pub scope_kind: String,
    pub project_id: Option<String>,
    pub tool_id: String,
    pub lock_hash: String,
}

#[derive(Clone, Debug)]
pub struct GateRequest {
    pub tool_id: String,
    pub max_parallel: u16,
    pub locks: Vec<OperationLockKey>,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("TOOL_QUEUE_TIMEOUT")]
pub struct QueueTimeout;

#[derive(Default)]
struct GateState {
    active_by_tool: BTreeMap<String, u16>,
    held_locks: BTreeSet<OperationLockKey>,
}

#[derive(Clone, Default)]
pub struct ConcurrencyGate(Arc<Mutex<GateState>>);

pub struct GateLease {
    gate: ConcurrencyGate,
    tool_id: String,
    locks: Vec<OperationLockKey>,
}

impl ConcurrencyGate {
    pub async fn acquire(
        &self,
        mut request: GateRequest,
        timeout: Duration,
    ) -> Result<GateLease, QueueTimeout> {
        request.locks.sort();
        request.locks.dedup();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            {
                let mut state = self.0.lock().expect("concurrency gate mutex poisoned");
                let active = state
                    .active_by_tool
                    .get(&request.tool_id)
                    .copied()
                    .unwrap_or(0);
                let locks_available = request
                    .locks
                    .iter()
                    .all(|lock| !state.held_locks.contains(lock));
                if active < request.max_parallel.max(1) && locks_available {
                    *state
                        .active_by_tool
                        .entry(request.tool_id.clone())
                        .or_default() += 1;
                    state.held_locks.extend(request.locks.iter().cloned());
                    return Ok(GateLease {
                        gate: self.clone(),
                        tool_id: request.tool_id,
                        locks: request.locks,
                    });
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(QueueTimeout);
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

impl Drop for GateLease {
    fn drop(&mut self) {
        let mut state = self.gate.0.lock().expect("concurrency gate mutex poisoned");
        for lock in &self.locks {
            state.held_locks.remove(lock);
        }
        if let Some(active) = state.active_by_tool.get_mut(&self.tool_id) {
            *active = active.saturating_sub(1);
            if *active == 0 {
                state.active_by_tool.remove(&self.tool_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn lock(name: &str) -> OperationLockKey {
        OperationLockKey {
            scope_kind: "custom".to_owned(),
            project_id: Some("project".to_owned()),
            tool_id: "user.fake.echo.run".to_owned(),
            lock_hash: name.to_owned(),
        }
    }

    #[tokio::test]
    // matrix: MCP-O009
    async fn opposite_lock_request_order_cannot_deadlock() {
        let gate = ConcurrencyGate::default();
        let first = gate
            .acquire(
                GateRequest {
                    tool_id: "first".to_owned(),
                    max_parallel: 1,
                    locks: vec![lock("a"), lock("b")],
                },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        let second_gate = gate.clone();
        let second = tokio::spawn(async move {
            second_gate
                .acquire(
                    GateRequest {
                        tool_id: "second".to_owned(),
                        max_parallel: 1,
                        locks: vec![lock("b"), lock("a")],
                    },
                    Duration::from_secs(1),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(25)).await;
        drop(first);
        assert!(second.await.unwrap().is_ok());
    }

    #[tokio::test]
    // matrix: MCP-O010
    async fn queue_timeout_occurs_before_any_process_start() {
        let gate = ConcurrencyGate::default();
        let held = gate
            .acquire(
                GateRequest {
                    tool_id: "held".to_owned(),
                    max_parallel: 1,
                    locks: vec![lock("exclusive")],
                },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        let process_starts = AtomicUsize::new(0);
        let result = gate
            .acquire(
                GateRequest {
                    tool_id: "waiting".to_owned(),
                    max_parallel: 1,
                    locks: vec![lock("exclusive")],
                },
                Duration::from_millis(30),
            )
            .await;
        if result.is_ok() {
            process_starts.fetch_add(1, Ordering::SeqCst);
        }
        assert_eq!(result.err(), Some(QueueTimeout));
        assert_eq!(process_starts.load(Ordering::SeqCst), 0);
        drop(held);
    }
}
