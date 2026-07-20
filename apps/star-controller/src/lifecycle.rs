//! Controller-owned projection of observable Codex task lifetime.
//!
//! This module deliberately separates an open stdio MCP connection from a
//! running WorkSession.  A gateway may remain connected while its Codex task
//! is open, but it must never keep the Controller alive by itself.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

pub const CONTROLLER_IDLE_GRACE: Duration = Duration::seconds(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceState {
    Active,
    Unknown,
    Exited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connected,
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceSnapshot {
    pub instance_id: String,
    /// Best-effort PID of the Codex Desktop process that owns this task.
    /// It is optional because public Hook input intentionally does not
    /// promise a process identifier.
    #[serde(default)]
    pub owner_pid: Option<u32>,
    pub state: InstanceState,
    pub observed_at: DateTime<Utc>,
    pub exited_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSnapshot {
    pub task_id: String,
    pub instance_id: String,
    pub observed_at: DateTime<Utc>,
    pub root_stop_observed: bool,
    pub tool_activity: u32,
    pub subagent_activity: u32,
    pub operation_activity: u32,
    pub work_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpConnectionSnapshot {
    pub connection_id: String,
    pub task_id: Option<String>,
    pub instance_id: String,
    pub state: ConnectionState,
    pub observed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerLifecycleDecision {
    KeepAlive,
    IdleUntil(DateTime<Utc>),
    ShutdownNow,
    BlockedByUnknownInstance,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexLifecycle {
    instances: BTreeMap<String, InstanceSnapshot>,
    tasks: BTreeMap<String, TaskSnapshot>,
    connections: BTreeMap<String, McpConnectionSnapshot>,
    #[serde(default)]
    idle_since: Option<DateTime<Utc>>,
}

impl CodexLifecycle {
    pub fn session_started(&mut self, instance_id: &str, task_id: &str, now: DateTime<Utc>) {
        self.session_started_with_owner(instance_id, task_id, None, now);
    }

    pub fn session_started_with_owner(
        &mut self,
        instance_id: &str,
        task_id: &str,
        owner_pid: Option<u32>,
        now: DateTime<Utc>,
    ) {
        self.observe_instance(instance_id, owner_pid, now);
        let task = self
            .tasks
            .entry(task_id.to_owned())
            .or_insert(TaskSnapshot {
                task_id: task_id.to_owned(),
                instance_id: instance_id.to_owned(),
                observed_at: now,
                root_stop_observed: false,
                tool_activity: 0,
                subagent_activity: 0,
                operation_activity: 0,
                work_active: false,
            });
        task.instance_id = instance_id.to_owned();
        task.observed_at = now;
    }

    pub fn user_prompt_submitted(&mut self, instance_id: &str, task_id: &str, now: DateTime<Utc>) {
        self.user_prompt_submitted_with_owner(instance_id, task_id, None, now);
    }

    pub fn user_prompt_submitted_with_owner(
        &mut self,
        instance_id: &str,
        task_id: &str,
        owner_pid: Option<u32>,
        now: DateTime<Utc>,
    ) {
        self.session_started_with_owner(instance_id, task_id, owner_pid, now);
        let task = self.tasks.get_mut(task_id).expect("session was inserted");
        task.root_stop_observed = false;
        task.work_active = true;
        task.observed_at = now;
        self.idle_since = None;
    }

    pub fn tool_started(&mut self, task_id: &str, now: DateTime<Utc>) -> bool {
        self.increment(task_id, now, Activity::Tool)
    }

    pub fn tool_finished(&mut self, task_id: &str, now: DateTime<Utc>) -> bool {
        self.decrement(task_id, now, Activity::Tool)
    }

    pub fn subagent_started(&mut self, task_id: &str, now: DateTime<Utc>) -> bool {
        self.increment(task_id, now, Activity::Subagent)
    }

    pub fn subagent_finished(&mut self, task_id: &str, now: DateTime<Utc>) -> bool {
        self.decrement(task_id, now, Activity::Subagent)
    }

    pub fn operation_started(&mut self, task_id: &str, now: DateTime<Utc>) -> bool {
        self.increment(task_id, now, Activity::Operation)
    }

    pub fn operation_finished(&mut self, task_id: &str, now: DateTime<Utc>) -> bool {
        self.decrement(task_id, now, Activity::Operation)
    }

    pub fn root_stop(&mut self, task_id: &str, now: DateTime<Utc>) -> bool {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };
        task.root_stop_observed = true;
        task.observed_at = now;
        self.finish_if_quiet(task_id, now);
        true
    }

    pub fn mcp_initialized(
        &mut self,
        connection_id: &str,
        instance_id: &str,
        task_id: Option<&str>,
        owner_pid: Option<u32>,
        now: DateTime<Utc>,
    ) {
        self.observe_instance(instance_id, owner_pid, now);
        self.connections.insert(
            connection_id.to_owned(),
            McpConnectionSnapshot {
                connection_id: connection_id.to_owned(),
                task_id: task_id.map(str::to_owned),
                instance_id: instance_id.to_owned(),
                state: ConnectionState::Connected,
                observed_at: now,
            },
        );
    }

    pub fn mcp_eof(&mut self, connection_id: &str, now: DateTime<Utc>) -> bool {
        let Some(connection) = self.connections.get_mut(connection_id) else {
            return false;
        };
        connection.state = ConnectionState::Eof;
        connection.observed_at = now;
        true
    }

    pub fn instance_exited(&mut self, instance_id: &str, now: DateTime<Utc>) -> bool {
        let Some(instance) = self.instances.get_mut(instance_id) else {
            return false;
        };
        instance.state = InstanceState::Exited;
        instance.observed_at = now;
        instance.exited_at = Some(now);
        for task in self
            .tasks
            .values_mut()
            .filter(|task| task.instance_id == instance_id)
        {
            task.work_active = false;
            task.root_stop_observed = true;
            task.tool_activity = 0;
            task.subagent_activity = 0;
            task.operation_activity = 0;
            task.observed_at = now;
        }
        true
    }

    /// A missing process-exit observation is not proof of exit.  Keeping this
    /// state unknown makes shutdown fail closed until a verified owner process
    /// observation resolves it.
    pub fn instance_became_unknown(&mut self, instance_id: &str, now: DateTime<Utc>) -> bool {
        let Some(instance) = self.instances.get_mut(instance_id) else {
            return false;
        };
        if instance.state != InstanceState::Exited {
            instance.state = InstanceState::Unknown;
            instance.observed_at = now;
        }
        true
    }

    /// Reconciles only PIDs that were previously attributed by an installed
    /// Hook or MCP process.  A missing owner is an exact terminal signal for
    /// that instance, so no stale Hook task can hold the Controller alive
    /// after Codex itself has exited.
    pub fn reconcile_owner_processes(
        &mut self,
        live_pids: &std::collections::BTreeSet<u32>,
        now: DateTime<Utc>,
    ) -> bool {
        let missing = self
            .instances
            .values()
            .filter_map(|instance| {
                (instance.state != InstanceState::Exited)
                    .then_some(instance.owner_pid)
                    .flatten()
                    .filter(|pid| !live_pids.contains(pid))
                    .map(|_| instance.instance_id.clone())
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        for instance_id in missing {
            changed |= self.instance_exited(&instance_id, now);
        }
        changed
    }

    pub fn decision(&mut self, now: DateTime<Utc>) -> ControllerLifecycleDecision {
        if self
            .instances
            .values()
            .any(|instance| instance.state == InstanceState::Unknown)
        {
            self.idle_since = None;
            return ControllerLifecycleDecision::BlockedByUnknownInstance;
        }
        if self.tasks.values().any(|task| task.work_active) {
            self.idle_since = None;
            return ControllerLifecycleDecision::KeepAlive;
        }
        if self.instances.is_empty()
            || self
                .instances
                .values()
                .all(|instance| instance.state == InstanceState::Exited)
        {
            self.idle_since = None;
            return ControllerLifecycleDecision::ShutdownNow;
        }
        let idle_since = *self.idle_since.get_or_insert(now);
        let deadline = idle_since + CONTROLLER_IDLE_GRACE;
        if now >= deadline {
            ControllerLifecycleDecision::ShutdownNow
        } else {
            ControllerLifecycleDecision::IdleUntil(deadline)
        }
    }

    pub fn instances(&self) -> impl Iterator<Item = &InstanceSnapshot> {
        self.instances.values()
    }

    pub fn has_observations(&self) -> bool {
        !self.instances.is_empty()
    }

    pub fn tasks(&self) -> impl Iterator<Item = &TaskSnapshot> {
        self.tasks.values()
    }

    pub fn connections(&self) -> impl Iterator<Item = &McpConnectionSnapshot> {
        self.connections.values()
    }

    fn observe_instance(&mut self, instance_id: &str, owner_pid: Option<u32>, now: DateTime<Utc>) {
        let instance = self
            .instances
            .entry(instance_id.to_owned())
            .or_insert(InstanceSnapshot {
                instance_id: instance_id.to_owned(),
                owner_pid,
                state: InstanceState::Active,
                observed_at: now,
                exited_at: None,
            });
        if instance.state != InstanceState::Exited {
            if owner_pid.is_some() {
                instance.owner_pid = owner_pid;
            }
            instance.state = InstanceState::Active;
            instance.observed_at = now;
        }
    }

    fn increment(&mut self, task_id: &str, now: DateTime<Utc>, activity: Activity) -> bool {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };
        match activity {
            Activity::Tool => task.tool_activity = task.tool_activity.saturating_add(1),
            Activity::Subagent => task.subagent_activity = task.subagent_activity.saturating_add(1),
            Activity::Operation => {
                task.operation_activity = task.operation_activity.saturating_add(1)
            }
        }
        task.work_active = true;
        task.observed_at = now;
        self.idle_since = None;
        true
    }

    fn decrement(&mut self, task_id: &str, now: DateTime<Utc>, activity: Activity) -> bool {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };
        let counter = match activity {
            Activity::Tool => &mut task.tool_activity,
            Activity::Subagent => &mut task.subagent_activity,
            Activity::Operation => &mut task.operation_activity,
        };
        if *counter == 0 {
            return false;
        }
        *counter -= 1;
        task.observed_at = now;
        self.finish_if_quiet(task_id, now);
        true
    }

    fn finish_if_quiet(&mut self, task_id: &str, now: DateTime<Utc>) {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return;
        };
        if task.root_stop_observed
            && task.tool_activity == 0
            && task.subagent_activity == 0
            && task.operation_activity == 0
        {
            task.work_active = false;
            task.observed_at = now;
        }
    }
}

#[derive(Clone, Copy)]
enum Activity {
    Tool,
    Subagent,
    Operation,
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 18, 0, 0, 0).unwrap()
    }

    #[test]
    fn root_stop_starts_a_cancellable_thirty_second_lease() {
        let mut lifecycle = CodexLifecycle::default();
        let start = now();
        lifecycle.user_prompt_submitted("app-1", "task-1", start);
        lifecycle.root_stop("task-1", start + Duration::seconds(1));

        assert_eq!(
            lifecycle.decision(start + Duration::seconds(1)),
            ControllerLifecycleDecision::IdleUntil(start + Duration::seconds(31))
        );
        lifecycle.user_prompt_submitted("app-1", "task-1", start + Duration::seconds(10));
        assert_eq!(
            lifecycle.decision(start + Duration::seconds(10)),
            ControllerLifecycleDecision::KeepAlive
        );
    }

    #[test]
    fn stop_does_not_finish_a_turn_while_a_tool_or_subagent_is_active() {
        let mut lifecycle = CodexLifecycle::default();
        let start = now();
        lifecycle.user_prompt_submitted("app-1", "task-1", start);
        assert!(lifecycle.tool_started("task-1", start));
        assert!(lifecycle.subagent_started("task-1", start));
        assert!(lifecycle.root_stop("task-1", start));
        assert_eq!(
            lifecycle.decision(start),
            ControllerLifecycleDecision::KeepAlive
        );

        assert!(lifecycle.tool_finished("task-1", start));
        assert_eq!(
            lifecycle.decision(start),
            ControllerLifecycleDecision::KeepAlive
        );
        assert!(lifecycle.subagent_finished("task-1", start));
        assert!(matches!(
            lifecycle.decision(start),
            ControllerLifecycleDecision::IdleUntil(_)
        ));
    }

    #[test]
    fn a_connected_mcp_does_not_keep_the_controller_alive() {
        let mut lifecycle = CodexLifecycle::default();
        let start = now();
        lifecycle.session_started("app-1", "task-1", start);
        lifecycle.mcp_initialized("mcp-1", "app-1", Some("task-1"), None, start);
        assert!(matches!(
            lifecycle.decision(start),
            ControllerLifecycleDecision::IdleUntil(_)
        ));
    }

    #[test]
    fn terminal_instance_requests_immediate_shutdown_even_if_mcp_eof_is_late() {
        let mut lifecycle = CodexLifecycle::default();
        let start = now();
        lifecycle.user_prompt_submitted("app-1", "task-1", start);
        lifecycle.mcp_initialized("mcp-1", "app-1", Some("task-1"), None, start);
        assert!(lifecycle.instance_exited("app-1", start + Duration::seconds(1)));
        assert_eq!(
            lifecycle.decision(start + Duration::seconds(1)),
            ControllerLifecycleDecision::ShutdownNow
        );
    }

    #[test]
    fn unknown_instance_fails_closed_instead_of_becoming_idle() {
        let mut lifecycle = CodexLifecycle::default();
        let start = now();
        lifecycle.session_started("app-1", "task-1", start);
        assert!(lifecycle.instance_became_unknown("app-1", start));
        assert_eq!(
            lifecycle.decision(start + Duration::minutes(10)),
            ControllerLifecycleDecision::BlockedByUnknownInstance
        );
    }

    #[test]
    fn missing_owned_desktop_exits_all_of_its_tasks_immediately() {
        let mut lifecycle = CodexLifecycle::default();
        let start = now();
        lifecycle.user_prompt_submitted_with_owner("desktop-41", "task-a", Some(41), start);
        lifecycle.user_prompt_submitted_with_owner("desktop-41", "task-b", Some(41), start);
        assert!(lifecycle.reconcile_owner_processes(&std::collections::BTreeSet::new(), start));
        assert_eq!(
            lifecycle.decision(start),
            ControllerLifecycleDecision::ShutdownNow
        );
        assert!(lifecycle.tasks().all(|task| !task.work_active));
    }
}
