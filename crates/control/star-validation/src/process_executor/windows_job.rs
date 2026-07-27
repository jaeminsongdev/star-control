//! Race-free Windows descendant containment for registered validation tools.
//!
//! `std::process::Command` owns argument quoting, the bounded environment and
//! anonymous pipes. The child starts suspended, is assigned to an
//! operation-owned Job Object, and is resumed only after assignment succeeds.

use std::{
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW,
            JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
        },
    },
};

const TERMINATED_BY_VALIDATION_TIMEOUT: u32 = 0xE000_0001;

pub(super) struct ProcessJob(HANDLE);

impl ProcessJob {
    fn new() -> Result<Self, ()> {
        let handle = unsafe { CreateJobObjectW(None, None) }.map_err(|_| ())?;
        let job = Self(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(|_| ())?;
        Ok(job)
    }

    fn assign(&self, child: &Child) -> Result<(), ()> {
        let process = HANDLE(child.as_raw_handle().cast());
        unsafe { AssignProcessToJobObject(self.0, process) }.map_err(|_| ())
    }

    pub(super) fn terminate_and_wait(&self, child: &mut Child, grace: Duration) -> bool {
        let _ = unsafe { TerminateJobObject(self.0, TERMINATED_BY_VALIDATION_TIMEOUT) };
        // If assignment failed or Windows rejected job termination, the exact
        // direct child still gets a best-effort fallback request. Waiting is
        // bounded so cleanup failure cannot hang validation forever.
        let _ = child.kill();
        let deadline = Instant::now() + grace;
        loop {
            match (child.try_wait(), self.active_processes()) {
                (Ok(Some(_)), Ok(0)) => return true,
                (Ok(_), Ok(_)) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                _ => return false,
            }
        }
    }

    pub(super) fn wait_for_empty(&self, grace: Duration) -> bool {
        let deadline = Instant::now() + grace;
        loop {
            match self.active_processes() {
                Ok(0) => return true,
                Ok(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                _ => return false,
            }
        }
    }

    fn active_processes(&self) -> Result<u32, ()> {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(self.0),
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                None,
            )
        }
        .map_err(|_| ())?;
        Ok(accounting.ActiveProcesses)
    }
}

impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct SnapshotHandle(HANDLE);

impl SnapshotHandle {
    fn threads() -> Result<Self, ()> {
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.map_err(|_| ())?;
        if handle == INVALID_HANDLE_VALUE {
            return Err(());
        }
        Ok(Self(handle))
    }
}

impl Drop for SnapshotHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn resume_initial_thread(process_id: u32) -> Result<(), ()> {
    let snapshot = SnapshotHandle::threads()?;
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    unsafe { Thread32First(snapshot.0, &mut entry) }.map_err(|_| ())?;
    loop {
        if entry.th32OwnerProcessID == process_id {
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) }
                .map_err(|_| ())?;
            let resumed = unsafe { ResumeThread(thread) };
            unsafe {
                let _ = CloseHandle(thread);
            }
            return (resumed != u32::MAX && resumed > 0).then_some(()).ok_or(());
        }
        if unsafe { Thread32Next(snapshot.0, &mut entry) }.is_err() {
            return Err(());
        }
    }
}

/// Starts a process without a window and closes the launch race before any
/// provider code can create an uncontained descendant.
pub(super) fn spawn_suspended_in_job(command: &mut Command) -> Result<(Child, ProcessJob), ()> {
    command.creation_flags(CREATE_SUSPENDED.0 | CREATE_NO_WINDOW.0);
    let job = ProcessJob::new()?;
    let mut child = command.spawn().map_err(|_| ())?;
    if job.assign(&child).is_err() || resume_initial_thread(child.id()).is_err() {
        let _ = job.terminate_and_wait(&mut child, Duration::from_secs(1));
        return Err(());
    }
    Ok((child, job))
}

#[cfg(test)]
pub(super) fn process_is_running(process_id: u32) -> bool {
    use windows::Win32::Foundation::WAIT_TIMEOUT;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    let Ok(process) = (unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            process_id,
        )
    }) else {
        return false;
    };
    let state = unsafe { WaitForSingleObject(process, 0) };
    unsafe {
        let _ = CloseHandle(process);
    }
    state == WAIT_TIMEOUT
}
