use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

static ENV_LOCK: Mutex<()> = Mutex::new(());
static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

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

pub(crate) fn temp_project() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "star-control-execution-{}-{}-{}",
        std::process::id(),
        nanos,
        counter
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}

pub(crate) fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

pub(crate) fn schema_root() -> PathBuf {
    repo_root().join("specs").join("schemas")
}

pub(crate) fn current_test_executable() -> String {
    std::env::current_exe()
        .expect("current test executable")
        .display()
        .to_string()
}
