use super::evidence::ForbiddenActionEvidence;
use super::policy::LocalProcessCommandPolicy;
use crate::{ExecutionRequest, ProviderAdapterError, ProviderRunContext};
use serde_json::Value;
use star_control_state::StateStoreError;
use std::fs::File;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) enum LocalProcessRunResult {
    Exited { status: ExitStatus },
    TimedOut,
    Cancelled { phase: &'static str },
    BlockedForbiddenAction { evidence: ForbiddenActionEvidence },
    LaunchFailed { message: String },
    WaitFailed { source: std::io::Error },
}

pub(crate) fn run_process(
    policy: &LocalProcessCommandPolicy,
    request: &ExecutionRequest,
    context: &ProviderRunContext<'_>,
    stdout_file: File,
    stderr_file: File,
) -> Result<LocalProcessRunResult, ProviderAdapterError> {
    if is_cancelled(context, request.job_id())? {
        return Ok(LocalProcessRunResult::Cancelled {
            phase: "before_start",
        });
    }

    let mut command = Command::new(policy.executable());
    command
        .args(policy.args())
        .current_dir(context.state_store().project_root())
        .env_clear()
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

    for name in policy.env_allowlist() {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(source) => {
            return Ok(LocalProcessRunResult::LaunchFailed {
                message: source.to_string(),
            });
        }
    };

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(LocalProcessRunResult::Exited { status }),
            Ok(None) => {
                if is_cancelled(context, request.job_id())? {
                    return Ok(terminate_cancelled_child(&mut child));
                }
                if started_at.elapsed() >= Duration::from_secs(policy.timeout_seconds()) {
                    if let Err(source) = child.kill() {
                        return Ok(LocalProcessRunResult::WaitFailed { source });
                    }
                    if let Err(source) = child.wait() {
                        return Ok(LocalProcessRunResult::WaitFailed { source });
                    }
                    return Ok(LocalProcessRunResult::TimedOut);
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(source) => return Ok(LocalProcessRunResult::WaitFailed { source }),
        }
    }
}

fn terminate_cancelled_child(child: &mut Child) -> LocalProcessRunResult {
    if let Err(source) = child.kill() {
        match child.try_wait() {
            Ok(Some(_)) => {
                return LocalProcessRunResult::Cancelled { phase: "running" };
            }
            Ok(None) | Err(_) => return LocalProcessRunResult::WaitFailed { source },
        }
    }
    match child.wait() {
        Ok(_) => LocalProcessRunResult::Cancelled { phase: "running" },
        Err(source) => LocalProcessRunResult::WaitFailed { source },
    }
}

fn is_cancelled(
    context: &ProviderRunContext<'_>,
    job_id: &str,
) -> Result<bool, ProviderAdapterError> {
    match context.state_store().load_state(job_id) {
        Ok(state) => Ok(state.get("state").and_then(Value::as_str) == Some("CANCELLED")),
        Err(StateStoreError::ArtifactNotFound { .. }) => Ok(false),
        Err(source) => Err(ProviderAdapterError::State(source)),
    }
}
