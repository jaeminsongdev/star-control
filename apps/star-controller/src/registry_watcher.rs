//! Win32 directory-change watcher for the live Tool Registry.
//!
//! Events are deliberately treated as an invalidation signal, not an
//! incremental truth source.  The Controller performs a complete demand scan
//! before publishing a new snapshot, so a coalesced or overflowed event can
//! never make it trust a partial directory view.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
};

use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_NOTIFY_ENUM_DIR, GENERIC_READ, WAIT_OBJECT_0},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
            FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_CREATION,
            FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
            FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SECURITY, FILE_NOTIFY_CHANGE_SIZE,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            ReadDirectoryChangesW,
        },
        System::{
            IO::{GetOverlappedResult, OVERLAPPED},
            Threading::{CreateEventW, INFINITE, ResetEvent, WaitForSingleObject},
        },
    },
    core::{HRESULT, HSTRING},
};

use crate::registry_runtime::{RegistrySourceRoot, safe_registry_root};

#[derive(Clone, Debug, Default)]
pub struct RegistryWatchPoll {
    pub changed: bool,
    pub overflowed: bool,
    pub unavailable_roots: Vec<PathBuf>,
    pub watched_roots: usize,
}

#[derive(Clone, Debug)]
enum WatchSignal {
    Changed,
    Overflow,
    Unavailable(PathBuf),
}

/// One blocking `ReadDirectoryChangesW` loop per existing source root.  A
/// missing root is not created by the Controller; demand scan still discovers
/// it later, while status surfaces it as unavailable instead of pretending it
/// is watched.
pub struct RegistryWatcher {
    receiver: Receiver<WatchSignal>,
    sender: Sender<WatchSignal>,
    watching: BTreeSet<PathBuf>,
    watched_roots: usize,
    unavailable_roots: Vec<PathBuf>,
    max_watch_roots: usize,
}

const MAX_WATCH_ROOTS: usize = 128;

impl RegistryWatcher {
    pub fn start(roots: &[RegistrySourceRoot]) -> Self {
        Self::start_with_limit(roots, MAX_WATCH_ROOTS)
    }

    pub fn start_with_limit(roots: &[RegistrySourceRoot], max_watch_roots: usize) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = Self {
            receiver,
            sender,
            watching: BTreeSet::new(),
            watched_roots: 0,
            unavailable_roots: Vec::new(),
            max_watch_roots: max_watch_roots.min(MAX_WATCH_ROOTS),
        };
        watcher.ensure_roots(roots);
        watcher
    }

    pub fn set_max_watch_roots(&mut self, max_watch_roots: usize) {
        self.max_watch_roots = max_watch_roots.min(MAX_WATCH_ROOTS);
    }

    /// Roots may appear after Controller start (notably a first user/project
    /// `tools.d`). Demand scan calls this without creating any directory.
    pub fn ensure_roots(&mut self, roots: &[RegistrySourceRoot]) {
        self.ensure_paths(roots.iter().map(|root| root.directory.clone()), true);
    }

    pub fn ensure_directories(&mut self, directories: impl IntoIterator<Item = PathBuf>) {
        self.ensure_paths(directories, false);
    }

    fn ensure_paths(&mut self, directories: impl IntoIterator<Item = PathBuf>, subtree: bool) {
        for directory in directories {
            if !safe_registry_root(&directory) {
                if !self.unavailable_roots.contains(&directory) {
                    self.unavailable_roots.push(directory);
                }
                continue;
            }
            if !self.watching.contains(&directory) && self.watching.len() >= self.max_watch_roots {
                if !self.unavailable_roots.contains(&directory) {
                    self.unavailable_roots.push(directory);
                }
                continue;
            }
            if self.watching.insert(directory.clone()) {
                self.unavailable_roots.retain(|path| path != &directory);
                self.watched_roots += 1;
                spawn_root_watcher(directory, self.sender.clone(), subtree);
            }
        }
    }

    /// Coalesce every pending event.  A zero-byte notification buffer is an
    /// overflow/unknown state; callers must do a full scan in both cases.
    pub fn poll(&mut self) -> RegistryWatchPoll {
        let mut poll = RegistryWatchPoll {
            watched_roots: self.watched_roots,
            unavailable_roots: self.unavailable_roots.clone(),
            ..Default::default()
        };
        loop {
            match self.receiver.try_recv() {
                Ok(WatchSignal::Changed) => poll.changed = true,
                Ok(WatchSignal::Overflow) => {
                    poll.changed = true;
                    poll.overflowed = true;
                }
                Ok(WatchSignal::Unavailable(path)) => {
                    self.watching.remove(&path);
                    self.watched_roots = self.watching.len();
                    if !poll.unavailable_roots.contains(&path) {
                        poll.unavailable_roots.push(path);
                    }
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        self.unavailable_roots = poll.unavailable_roots.clone();
        poll.watched_roots = self.watched_roots;
        poll
    }
}

fn spawn_root_watcher(root: PathBuf, sender: Sender<WatchSignal>, subtree: bool) {
    thread::Builder::new()
        .name("star-registry-watch".to_owned())
        .spawn(move || watch_root(&root, &sender, subtree))
        .expect("registry watcher thread starts");
}

fn watch_root(root: &Path, sender: &Sender<WatchSignal>, subtree: bool) {
    let name = HSTRING::from(root.as_os_str().to_string_lossy().as_ref());
    let directory = unsafe {
        CreateFileW(
            &name,
            GENERIC_READ.0 | FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            None,
        )
    };
    let Ok(directory) = directory else {
        let _ = sender.send(WatchSignal::Unavailable(root.to_path_buf()));
        return;
    };
    let filter = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_ATTRIBUTES
        | FILE_NOTIFY_CHANGE_CREATION
        | FILE_NOTIFY_CHANGE_SECURITY;
    let event = match unsafe { CreateEventW(None, true, false, None) } {
        Ok(event) => event,
        Err(_) => {
            let _ = sender.send(WatchSignal::Unavailable(root.to_path_buf()));
            unsafe {
                let _ = CloseHandle(directory);
            }
            return;
        }
    };
    loop {
        let mut buffer = [0u8; 64 * 1024];
        let mut bytes = 0u32;
        let mut overlapped = OVERLAPPED {
            hEvent: event,
            ..Default::default()
        };
        let result = unsafe {
            ReadDirectoryChangesW(
                directory,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                subtree,
                filter,
                None,
                Some(&mut overlapped),
                None,
            )
        };
        if result.is_err() || unsafe { WaitForSingleObject(event, INFINITE) } != WAIT_OBJECT_0 {
            let _ = sender.send(WatchSignal::Unavailable(root.to_path_buf()));
            break;
        }
        match unsafe { GetOverlappedResult(directory, &overlapped, &mut bytes, false) } {
            Ok(()) => {}
            Err(error) if error.code() == HRESULT::from_win32(ERROR_NOTIFY_ENUM_DIR.0) => {
                let _ = unsafe { ResetEvent(event) };
                let _ = sender.send(WatchSignal::Overflow);
                continue;
            }
            Err(_) => {
                let _ = sender.send(WatchSignal::Unavailable(root.to_path_buf()));
                break;
            }
        }
        let _ = unsafe { ResetEvent(event) };
        let _ = sender.send(if bytes == 0 {
            WatchSignal::Overflow
        } else {
            WatchSignal::Changed
        });
    }
    unsafe {
        let _ = CloseHandle(event);
        let _ = CloseHandle(directory);
    }
}

#[cfg(test)]
#[allow(clippy::cloned_ref_to_slice_refs)]
mod tests {
    use super::*;
    use crate::registry_runtime::{RegistryRuntime, RegistrySourceRoot};
    use star_contracts::{canonical::Sha256Hash, manifest::ManifestSource};

    fn manifest_fixture() -> String {
        let executable = std::env::current_exe().unwrap();
        let path = executable.display().to_string().replace('\\', "\\\\");
        let hash = Sha256Hash::digest_reader(std::fs::File::open(executable).unwrap()).unwrap();
        include_str!("../../../specs/examples/valid/tool-package-manifest-v1.toml")
            .replace(r"C:\\Tools\\fake-echo.exe", &path)
            .replace(
                "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                hash.as_str(),
            )
    }

    #[test]
    // matrix: MCP-R003
    fn watcher_invalidates_an_existing_root_after_a_file_change() {
        let root = std::env::temp_dir().join(format!("star-registry-watch-{}", star_ipc::nonce()));
        std::fs::create_dir_all(&root).unwrap();
        let mut watcher = RegistryWatcher::start(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory: root.clone(),
        }]);
        std::thread::sleep(std::time::Duration::from_millis(100));
        std::fs::write(root.join("changed.toml"), "format_version = 1\n").unwrap();
        let changed = (0..30).any(|_| {
            let poll = watcher.poll();
            if poll.changed {
                true
            } else {
                std::thread::sleep(std::time::Duration::from_millis(100));
                false
            }
        });
        assert!(changed, "ReadDirectoryChangesW did not invalidate the root");
    }

    #[test]
    // matrix: MCP-R010
    fn overflow_is_an_invalidation_and_full_demand_scan_recovers_the_snapshot() {
        let directory =
            std::env::temp_dir().join(format!("star-registry-overflow-{}", star_ipc::nonce()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("fake.toml");
        std::fs::write(&path, manifest_fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let before = registry.snapshot_hash();
        std::fs::write(
            &path,
            manifest_fixture().replace("Echoes a value.", "Echoes a replacement value."),
        )
        .unwrap();

        let mut watcher = RegistryWatcher::start(&[]);
        watcher.sender.send(WatchSignal::Overflow).unwrap();
        let poll = watcher.poll();
        assert!(poll.changed && poll.overflowed);
        registry.demand_scan(&[root]);
        assert_ne!(before, registry.snapshot_hash());
    }
}
