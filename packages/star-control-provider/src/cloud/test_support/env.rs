use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn is_child_helper(test_name: &str) -> bool {
    std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|args| args[0] == "--exact" && args[1] == test_name)
}

pub(crate) fn current_test_executable() -> String {
    std::env::current_exe()
        .expect("current test executable")
        .display()
        .to_string()
}

pub(crate) struct EnvVarGuard<'a> {
    key: &'static str,
    _lock: MutexGuard<'a, ()>,
}

impl EnvVarGuard<'_> {
    pub(crate) fn set(key: &'static str, value: &'static str) -> Self {
        let lock = ENV_LOCK.lock().expect("env lock");
        std::env::set_var(key, value);
        Self { key, _lock: lock }
    }
}

impl Drop for EnvVarGuard<'_> {
    fn drop(&mut self) {
        std::env::remove_var(self.key);
    }
}
