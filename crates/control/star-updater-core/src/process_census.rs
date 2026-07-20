//! Exact Windows process census used before an integration restart.
//!
//! Image *names* are deliberately not an ownership proof.  A target is only
//! eligible for a later shutdown action when its canonical executable path,
//! PID and ancestry all come from the same fresh snapshot.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WPARAM},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
                TerminateProcess,
            },
        },
        UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
    },
    core::PWSTR,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub parent_pid: u32,
    pub creation_time_100ns: Option<u64>,
    pub image: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessIdentityState {
    Live,
    Exited,
    Unknown,
}

#[derive(Debug, Error)]
pub enum CensusError {
    #[error("Windows process snapshot could not be created")]
    Snapshot,
    #[error("a verified process could not be terminated")]
    Terminate,
}

/// Performs only the *fallback* termination step.  The caller must already
/// have attempted an official graceful application close and waited its grace
/// period.  Each PID is re-proven against a new snapshot immediately before
/// termination, which prevents PID reuse from widening the target set.
pub fn terminate_verified_tree(target_image: &Path) -> Result<Vec<u32>, CensusError> {
    terminate_verified_tree_excluding(target_image, None)
}

/// Same exact-image fallback termination with one protected process root and
/// its descendants excluded.  A detached updater can still be a descendant
/// of the Codex process tree which it is closing; Windows does not terminate
/// children merely because their parent exits, so excluding the updater keeps
/// the durable transaction alive without widening the target set.
pub fn terminate_verified_tree_excluding(
    target_image: &Path,
    protected_root_pid: Option<u32>,
) -> Result<Vec<u32>, CensusError> {
    let initial = snapshot()?;
    let mut targets = owned_process_tree(&initial, target_image);
    let protected = protected_root_pid
        .map(|pid| owned_process_descendants(&initial, pid))
        .unwrap_or_default();
    targets.retain(|target| !protected.contains(&target.pid));
    // Children first so a root exit cannot make a surviving child ambiguous.
    // PID values do not encode ancestry and can wrap or be reused.
    targets.sort_by_key(|process| {
        (
            std::cmp::Reverse(ancestry_depth(&initial, process.pid)),
            std::cmp::Reverse(process.pid),
        )
    });
    let mut terminated = Vec::new();
    for target in targets {
        if target.pid == std::process::id() || protected.contains(&target.pid) {
            return Err(CensusError::Terminate);
        }
        let current = snapshot()?;
        let currently_protected = protected_root_pid
            .map(|pid| owned_process_descendants(&current, pid))
            .unwrap_or_default();
        if currently_protected.contains(&target.pid)
            || !owned_process_tree(&current, target_image)
                .iter()
                .any(|process| same_process_instance(process, &target))
        {
            continue;
        }
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, target.pid) }
            .map_err(|_| CensusError::Terminate)?;
        let result = unsafe { TerminateProcess(handle, 1) };
        if result.is_err() {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(CensusError::Terminate);
        }
        unsafe {
            let _ = CloseHandle(handle);
        }
        terminated.push(target.pid);
    }
    Ok(terminated)
}

fn owned_process_descendants(snapshot: &[ProcessIdentity], root_pid: u32) -> BTreeSet<u32> {
    let mut selected = BTreeSet::new();
    if !snapshot.iter().any(|process| process.pid == root_pid) {
        return selected;
    }
    selected.insert(root_pid);
    let mut changed = true;
    while changed {
        changed = false;
        for process in snapshot {
            if selected.contains(&process.parent_pid) && selected.insert(process.pid) {
                changed = true;
            }
        }
    }
    selected
}

fn ancestry_depth(snapshot: &[ProcessIdentity], pid: u32) -> usize {
    let by_pid = snapshot
        .iter()
        .map(|process| (process.pid, process))
        .collect::<BTreeMap<_, _>>();
    let mut current = pid;
    let mut visited = BTreeSet::new();
    let mut depth = 0;
    while visited.insert(current) {
        let Some(process) = by_pid.get(&current) else {
            break;
        };
        if process.parent_pid == 0 || process.parent_pid == current {
            break;
        }
        depth += 1;
        current = process.parent_pid;
    }
    depth
}

fn same_process_instance(actual: &ProcessIdentity, expected: &ProcessIdentity) -> bool {
    actual.pid == expected.pid
        && actual.parent_pid == expected.parent_pid
        && actual
            .creation_time_100ns
            .zip(expected.creation_time_100ns)
            .is_some_and(|(left, right)| left == right)
        && match expected.image.as_deref() {
            Some(expected) => actual
                .image
                .as_deref()
                .is_some_and(|actual| paths_equal(actual, expected)),
            None => true,
        }
}

/// Requests a normal top-level-window close for every exact Codex root.  This
/// is deliberately only a request; callers wait a bounded grace period and
/// use `terminate_verified_tree` only for roots that remain proven afterward.
pub fn request_graceful_close(target_image: &Path) -> Result<Vec<u32>, CensusError> {
    let roots = exact_image_instances(&snapshot()?, target_image);
    let pids = roots
        .iter()
        .map(|process| process.pid)
        .collect::<BTreeSet<_>>();
    if pids.is_empty() {
        return Ok(Vec::new());
    }
    let mut context = WindowCloseContext {
        pids,
        signalled: BTreeSet::new(),
    };
    unsafe {
        EnumWindows(
            Some(request_window_close),
            LPARAM((&mut context as *mut WindowCloseContext).cast::<()>() as isize),
        )
        .map_err(|_| CensusError::Snapshot)?;
    }
    Ok(context.signalled.into_iter().collect())
}

struct WindowCloseContext {
    pids: BTreeSet<u32>,
    signalled: BTreeSet<u32>,
}

unsafe extern "system" fn request_window_close(
    window: HWND,
    parameter: LPARAM,
) -> windows::core::BOOL {
    let context = unsafe { &mut *(parameter.0 as *mut WindowCloseContext) };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    if context.pids.contains(&pid)
        && unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_ok()
    {
        context.signalled.insert(pid);
    }
    windows::core::BOOL(1)
}

/// Captures process IDs, parent IDs and image paths in one ToolHelp snapshot.
/// An inaccessible image is preserved as `None`; callers must not infer it is
/// a Codex process or terminate it.
pub fn snapshot() -> Result<Vec<ProcessIdentity>, CensusError> {
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|_| CensusError::Snapshot)?;
    if handle == INVALID_HANDLE_VALUE {
        return Err(CensusError::Snapshot);
    }
    let result = snapshot_from_handle(handle);
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

fn snapshot_from_handle(handle: HANDLE) -> Result<Vec<ProcessIdentity>, CensusError> {
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(handle, &mut entry) }.is_err() {
        return Err(CensusError::Snapshot);
    }
    let mut result = Vec::new();
    loop {
        let (image, creation_time_100ns) = process_details(entry.th32ProcessID);
        result.push(ProcessIdentity {
            pid: entry.th32ProcessID,
            parent_pid: entry.th32ParentProcessID,
            creation_time_100ns,
            image,
        });
        if unsafe { Process32NextW(handle, &mut entry) }.is_err() {
            break;
        }
    }
    Ok(result)
}

fn process_details(pid: u32) -> (Option<PathBuf>, Option<u64>) {
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return (None, None);
    };
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    let image_result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let time_result =
        unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
    unsafe {
        let _ = CloseHandle(process);
    }
    let image = image_result
        .ok()
        .map(|_| PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize])));
    let creation_time_100ns = time_result
        .is_ok()
        .then_some(((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64);
    (image, creation_time_100ns)
}

/// Returns only the exact target image roots and their descendants.  A stale
/// or partial snapshot cannot manufacture ownership: every selected PID must
/// be reachable from a target root in the same map.
pub fn owned_process_tree(
    snapshot: &[ProcessIdentity],
    target_image: &Path,
) -> Vec<ProcessIdentity> {
    if !target_image.is_absolute() {
        return Vec::new();
    }
    let by_pid = snapshot
        .iter()
        .map(|process| (process.pid, process))
        .collect::<BTreeMap<_, _>>();
    let mut selected = BTreeSet::new();
    for process in snapshot {
        if process
            .image
            .as_ref()
            .is_some_and(|image| paths_equal(image, target_image))
        {
            selected.insert(process.pid);
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for process in snapshot {
            if selected.contains(&process.parent_pid) && selected.insert(process.pid) {
                changed = true;
            }
        }
    }
    selected
        .into_iter()
        .filter_map(|pid| by_pid.get(&pid).copied().cloned())
        .collect()
}

/// Returns the directly observed instances whose image is exactly the chosen
/// Codex executable.  Descendants are intentionally excluded from this count.
pub fn exact_image_instances(
    snapshot: &[ProcessIdentity],
    target_image: &Path,
) -> Vec<ProcessIdentity> {
    if !target_image.is_absolute() {
        return Vec::new();
    }
    snapshot
        .iter()
        .filter(|process| {
            process
                .image
                .as_ref()
                .is_some_and(|image| paths_equal(image, target_image))
        })
        .cloned()
        .collect()
}

/// Returns the first known Codex Desktop ancestor of `child_pid` from this
/// single snapshot.  This is lifecycle attribution only: update shutdown
/// still requires the exact executable path selected by the updater.
///
/// Codex Desktop currently hosts its Windows application as `ChatGPT.exe`.
/// Keeping that narrow avoids treating the adjacent `resources\\codex.exe`
/// CLI as an application owner.
pub fn codex_desktop_ancestor_pid(snapshot: &[ProcessIdentity], child_pid: u32) -> Option<u32> {
    let by_pid = snapshot
        .iter()
        .map(|process| (process.pid, process))
        .collect::<BTreeMap<_, _>>();
    let mut current_pid = child_pid;
    let mut visited = BTreeSet::new();
    while visited.insert(current_pid) {
        let process = by_pid.get(&current_pid)?;
        if process.image.as_deref().is_some_and(is_codex_desktop_image) {
            return Some(process.pid);
        }
        if process.parent_pid == 0 || process.parent_pid == process.pid {
            return None;
        }
        current_pid = process.parent_pid;
    }
    None
}

/// Best-effort owner attribution for an installed Hook or MCP process.  A
/// failure is intentionally represented as `None`; it must never turn an
/// unobserved task into permission to shut down or update Codex.
pub fn current_codex_desktop_owner_pid() -> Option<u32> {
    current_codex_desktop_owner().map(|owner| owner.pid)
}

/// Returns the complete identity used by the MCP owner-death watchdog.  The
/// watchdog compares PID, parent and exact image path on each fresh snapshot,
/// so a reused PID cannot keep an orphaned gateway alive.
pub fn current_codex_desktop_owner() -> Option<ProcessIdentity> {
    let processes = snapshot().ok()?;
    let pid = codex_desktop_ancestor_pid(&processes, std::process::id())?;
    processes.into_iter().find(|process| process.pid == pid)
}

pub fn process_identity_state(
    snapshot: &[ProcessIdentity],
    expected: &ProcessIdentity,
) -> ProcessIdentityState {
    let Some(actual) = snapshot.iter().find(|actual| actual.pid == expected.pid) else {
        return ProcessIdentityState::Exited;
    };
    if actual.parent_pid != expected.parent_pid {
        return ProcessIdentityState::Exited;
    }
    match (actual.image.as_deref(), expected.image.as_deref()) {
        (Some(left), Some(right)) if !paths_equal(left, right) => {
            return ProcessIdentityState::Exited;
        }
        (None, _) | (_, None) => return ProcessIdentityState::Unknown,
        _ => {}
    }
    match (actual.creation_time_100ns, expected.creation_time_100ns) {
        (Some(left), Some(right)) if left == right => ProcessIdentityState::Live,
        (Some(_), Some(_)) => ProcessIdentityState::Exited,
        _ => ProcessIdentityState::Unknown,
    }
}

fn is_codex_desktop_image(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("ChatGPT.exe"))
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    normalize_windows_path(left) == normalize_windows_path(right)
}

fn normalize_windows_path(path: &Path) -> String {
    let rendered = path.as_os_str().to_string_lossy().replace('/', "\\");
    rendered
        .strip_prefix(r"\\?\")
        .unwrap_or(&rendered)
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_image_roots_and_their_children_are_selected() {
        let target = std::env::current_exe().unwrap().canonicalize().unwrap();
        let processes = vec![
            ProcessIdentity {
                pid: 10,
                parent_pid: 0,
                creation_time_100ns: Some(1010),
                image: Some(target.clone()),
            },
            ProcessIdentity {
                pid: 11,
                parent_pid: 10,
                creation_time_100ns: Some(1011),
                image: None,
            },
            ProcessIdentity {
                pid: 12,
                parent_pid: 11,
                creation_time_100ns: Some(1012),
                image: None,
            },
            ProcessIdentity {
                pid: 13,
                parent_pid: 0,
                creation_time_100ns: Some(1013),
                image: None,
            },
        ];
        assert_eq!(
            owned_process_tree(&processes, &target)
                .into_iter()
                .map(|value| value.pid)
                .collect::<Vec<_>>(),
            vec![10, 11, 12]
        );
        assert_eq!(
            exact_image_instances(&processes, &target)
                .into_iter()
                .map(|value| value.pid)
                .collect::<Vec<_>>(),
            vec![10]
        );
    }

    #[test]
    fn protected_updater_subtree_is_not_selected_for_desktop_fallback() {
        let desktop = PathBuf::from(r"C:\\Program Files\\OpenAI\\ChatGPT.exe");
        let processes = vec![
            ProcessIdentity {
                pid: 10,
                parent_pid: 1,
                creation_time_100ns: Some(1010),
                image: Some(desktop.clone()),
            },
            ProcessIdentity {
                pid: 11,
                parent_pid: 10,
                creation_time_100ns: Some(1011),
                image: Some(PathBuf::from(
                    r"C:\\Program Files\\OpenAI\\resources\\codex.exe",
                )),
            },
            ProcessIdentity {
                pid: 12,
                parent_pid: 11,
                creation_time_100ns: Some(1012),
                image: Some(PathBuf::from(r"C:\\Users\\test\\updater\\star-updater.exe")),
            },
            ProcessIdentity {
                pid: 13,
                parent_pid: 12,
                creation_time_100ns: Some(1013),
                image: None,
            },
            ProcessIdentity {
                pid: 14,
                parent_pid: 10,
                creation_time_100ns: Some(1014),
                image: None,
            },
        ];
        let selected = owned_process_tree(&processes, &desktop)
            .into_iter()
            .map(|process| process.pid)
            .filter(|pid| !owned_process_descendants(&processes, 12).contains(pid))
            .collect::<Vec<_>>();
        assert_eq!(selected, vec![10, 11, 14]);
    }

    #[test]
    fn desktop_owner_is_selected_only_from_the_callers_ancestor_chain() {
        let snapshot = vec![
            ProcessIdentity {
                pid: 1,
                parent_pid: 0,
                creation_time_100ns: Some(1001),
                image: Some(PathBuf::from(r"C:\\Windows\\explorer.exe")),
            },
            ProcessIdentity {
                pid: 2,
                parent_pid: 1,
                creation_time_100ns: Some(1002),
                image: Some(PathBuf::from(
                    r"C:\\Program Files\\WindowsApps\\OpenAI\\ChatGPT.exe",
                )),
            },
            ProcessIdentity {
                pid: 3,
                parent_pid: 2,
                creation_time_100ns: Some(1003),
                image: Some(PathBuf::from(r"D:\\Star-Control\\star-mcp.exe")),
            },
            ProcessIdentity {
                pid: 4,
                parent_pid: 1,
                creation_time_100ns: Some(1004),
                image: Some(PathBuf::from(r"C:\\Other\\ChatGPT.exe")),
            },
        ];
        assert_eq!(codex_desktop_ancestor_pid(&snapshot, 3), Some(2));
        assert_eq!(codex_desktop_ancestor_pid(&snapshot, 1), None);
        assert_eq!(
            process_identity_state(&snapshot, &snapshot[1]),
            ProcessIdentityState::Live
        );
        let reused_pid = ProcessIdentity {
            parent_pid: 99,
            ..snapshot[1].clone()
        };
        assert_eq!(
            process_identity_state(&snapshot, &reused_pid),
            ProcessIdentityState::Exited
        );
        let reused_creation = ProcessIdentity {
            creation_time_100ns: Some(2002),
            ..snapshot[1].clone()
        };
        assert_eq!(
            process_identity_state(&snapshot, &reused_creation),
            ProcessIdentityState::Exited
        );
        let mut inaccessible = snapshot.clone();
        inaccessible[1].image = None;
        assert_eq!(
            process_identity_state(&inaccessible, &snapshot[1]),
            ProcessIdentityState::Unknown
        );
    }

    #[test]
    fn ancestry_depth_orders_children_before_roots_independent_of_pid_value() {
        let processes = vec![
            ProcessIdentity {
                pid: 50,
                parent_pid: 0,
                creation_time_100ns: Some(1050),
                image: None,
            },
            ProcessIdentity {
                pid: 20,
                parent_pid: 50,
                creation_time_100ns: Some(1020),
                image: None,
            },
            ProcessIdentity {
                pid: 10,
                parent_pid: 20,
                creation_time_100ns: Some(1010),
                image: None,
            },
        ];
        assert!(ancestry_depth(&processes, 10) > ancestry_depth(&processes, 20));
        assert!(ancestry_depth(&processes, 20) > ancestry_depth(&processes, 50));
    }
}
