use super::*;

#[test]
fn local_process_rejects_shell_wrapper() {
    let error = execute_with_command(
        shell_wrapper_name(),
        Vec::new(),
        vec![shell_wrapper_name().to_string()],
        Vec::new(),
        10,
    )
    .expect_err("shell wrapper should be rejected");

    assert!(matches!(
        error,
        ProviderAdapterError::CommandPolicyDenied { .. }
    ));
}

#[test]
fn local_process_rejects_executable_outside_allowlist() {
    let executable = current_test_executable();
    let error = execute_with_command(
        &executable,
        Vec::new(),
        vec!["other-runner".to_string()],
        Vec::new(),
        10,
    )
    .expect_err("executable outside allowlist should be rejected");

    assert!(matches!(
        error,
        ProviderAdapterError::CommandPolicyDenied { .. }
    ));
}
