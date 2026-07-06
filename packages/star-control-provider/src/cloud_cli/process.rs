use super::policy::CloudCliCommandPolicy;
use crate::{ExecutionRequest, ProviderAdapterError, ProviderRunContext};
use serde_json::Value;
use std::fs::File;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) enum CloudCliRunResult {
    Exited { status: ExitStatus },
    TimedOut,
    LaunchFailed { message: String },
    WaitFailed { source: std::io::Error },
}

pub(crate) fn run_cloud_cli_process(
    policy: &CloudCliCommandPolicy,
    request: &ExecutionRequest,
    context: &ProviderRunContext<'_>,
    request_ref: &Value,
    stdout_file: File,
    stderr_file: File,
) -> Result<CloudCliRunResult, ProviderAdapterError> {
    let mut command = Command::new(policy.executable());
    command
        .args(policy.rendered_args(request, request_ref))
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
            return Ok(CloudCliRunResult::LaunchFailed {
                message: source.to_string(),
            });
        }
    };

    wait_for_cloud_cli_child(&mut child, policy.timeout_seconds())
}

fn wait_for_cloud_cli_child(
    child: &mut Child,
    timeout_seconds: u64,
) -> Result<CloudCliRunResult, ProviderAdapterError> {
    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(CloudCliRunResult::Exited { status }),
            Ok(None) => {
                if started_at.elapsed() >= Duration::from_secs(timeout_seconds) {
                    if let Err(source) = child.kill() {
                        return Ok(CloudCliRunResult::WaitFailed { source });
                    }
                    if let Err(source) = child.wait() {
                        return Ok(CloudCliRunResult::WaitFailed { source });
                    }
                    return Ok(CloudCliRunResult::TimedOut);
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(source) => return Ok(CloudCliRunResult::WaitFailed { source }),
        }
    }
}
