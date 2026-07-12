//! Win32-only launcher for a validated direct EXE.
//!
//! Tokio's process API cannot expose the initial thread handle.  The runtime
//! therefore uses this narrow launcher so a child is created suspended, put
//! into its Operation Job, and only then allowed to execute.  It receives an
//! already validated `DirectExeSpec`; it never resolves a command through a
//! shell or `PATH`.

use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{Read, Write},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::FromRawHandle,
    },
    path::PathBuf,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    thread,
    time::Instant,
};

use windows::{
    Win32::{
        Foundation::{
            CloseHandle, FILETIME, HANDLE, HANDLE_FLAG_INHERIT, HLOCAL, LocalFree,
            SetHandleInformation, WAIT_OBJECT_0,
        },
        NetworkManagement::WindowsFirewall::NetworkIsolationGetAppContainerConfig,
        Security::{
            Authorization::ConvertSidToStringSidW,
            EqualSid, FreeSid,
            Isolation::{
                CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
                GetAppContainerFolderPath,
            },
            PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES,
        },
        System::{
            Com::CoTaskMemFree,
            Memory::{GetProcessHeap, HEAP_FLAGS, HeapFree},
            Pipes::CreatePipe,
            Threading::{
                CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
                DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
                GetProcessTimes, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
                PROCESS_INFORMATION, PROCESS_NAME_WIN32, QueryFullProcessImageNameW, ResumeThread,
                STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
                WaitForSingleObject,
            },
        },
    },
    core::{HSTRING, PCWSTR, PWSTR},
};
use zeroize::Zeroize;

use super::{
    CapturedStream, DirectExeOutcome, DirectExeSpec, OperationJob, ProcessEndEvidence,
    ProcessEndObserver, ProcessStartEvidence, ProcessStartObserver, RuntimeCancellation,
    RuntimeError, StdinCancelPlan,
};

pub(super) async fn execute(
    spec: DirectExeSpec,
    cancellation: Option<RuntimeCancellation>,
    cancel_plan: Option<StdinCancelPlan>,
    process_observer: Option<ProcessStartObserver>,
    process_end_observer: Option<ProcessEndObserver>,
) -> Result<DirectExeOutcome, RuntimeError> {
    spec.validate()?;
    tokio::task::spawn_blocking(move || {
        execute_blocking(
            &spec,
            cancellation.as_ref(),
            cancel_plan.as_ref(),
            process_observer.as_ref(),
            process_end_observer.as_ref(),
        )
    })
    .await
    .map_err(|_| RuntimeError::Start)?
}

fn execute_blocking(
    spec: &DirectExeSpec,
    cancellation: Option<&RuntimeCancellation>,
    cancel_plan: Option<&StdinCancelPlan>,
    process_observer: Option<&ProcessStartObserver>,
    process_end_observer: Option<&ProcessEndObserver>,
) -> Result<DirectExeOutcome, RuntimeError> {
    if cancellation.is_some_and(RuntimeCancellation::is_cancelled) {
        return Err(RuntimeError::Cancelled);
    }
    let job = OperationJob::new_with_limits(spec.max_memory_bytes, spec.max_processes)?;
    let (stdin_read, stdin_write) = anonymous_pipe()?;
    let (stdout_read, stdout_write) = match anonymous_pipe() {
        Ok(pipe) => pipe,
        Err(error) => {
            close_many(&[stdin_read, stdin_write]);
            return Err(error);
        }
    };
    let (stderr_read, stderr_write) = match anonymous_pipe() {
        Ok(pipe) => pipe,
        Err(error) => {
            close_many(&[stdin_read, stdin_write, stdout_read, stdout_write]);
            return Err(error);
        }
    };
    let pipe_handles = [
        stdin_read,
        stdin_write,
        stdout_read,
        stdout_write,
        stderr_read,
        stderr_write,
    ];
    // Only the three child pipe ends may cross the process boundary.
    for parent_handle in [stdin_write, stdout_read, stderr_read] {
        if unsafe { SetHandleInformation(parent_handle, HANDLE_FLAG_INHERIT.0, Default::default()) }
            .is_err()
        {
            close_many(&pipe_handles);
            return Err(RuntimeError::Start);
        }
    }

    let mut handles = [stdin_read, stdout_write, stderr_write];
    let appcontainer = match spec
        .appcontainer_profile
        .as_deref()
        .map(appcontainer_capabilities)
        .transpose()
    {
        Ok(appcontainer) => appcontainer,
        Err(error) => {
            close_many(&pipe_handles);
            return Err(error);
        }
    };
    let mut attribute_bytes = 0usize;
    // This sizing call intentionally reports ERROR_INSUFFICIENT_BUFFER.
    let attribute_count = if appcontainer.is_some() { 2 } else { 1 };
    let _ = unsafe {
        InitializeProcThreadAttributeList(None, attribute_count, None, &mut attribute_bytes)
    };
    if attribute_bytes == 0 {
        close_many(&pipe_handles);
        return Err(RuntimeError::Start);
    }
    let mut attributes = vec![0u8; attribute_bytes];
    let list = LPPROC_THREAD_ATTRIBUTE_LIST(attributes.as_mut_ptr().cast::<core::ffi::c_void>());
    if unsafe {
        InitializeProcThreadAttributeList(Some(list), attribute_count, None, &mut attribute_bytes)
    }
    .is_err()
    {
        close_many(&pipe_handles);
        return Err(RuntimeError::Start);
    }
    let attribute_ready = true;
    let update = unsafe {
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(handles.as_mut_ptr().cast()),
            std::mem::size_of_val(&handles),
            None,
            None,
        )
    };
    if update.is_err() {
        if attribute_ready {
            unsafe { DeleteProcThreadAttributeList(list) };
        }
        close_many(&pipe_handles);
        return Err(RuntimeError::Start);
    }
    if let Some(appcontainer) = appcontainer.as_ref() {
        if unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                Some((&raw const appcontainer.security).cast()),
                std::mem::size_of::<SECURITY_CAPABILITIES>(),
                None,
                None,
            )
        }
        .is_err()
        {
            unsafe { DeleteProcThreadAttributeList(list) };
            close_many(&pipe_handles);
            return Err(RuntimeError::IsolationUnavailable);
        }
    }

    let prepared = (|| {
        Ok((
            wide_nul(spec.executable.as_os_str())?,
            wide_nul(spec.working_directory.as_os_str())?,
            command_line(spec)?,
            environment_block(spec)?,
        ))
    })();
    let (application, current_directory, mut command_line, mut environment) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            unsafe { DeleteProcThreadAttributeList(list) };
            close_many(&pipe_handles);
            return Err(error);
        }
    };
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin_read;
    startup.StartupInfo.hStdOutput = stdout_write;
    startup.StartupInfo.hStdError = stderr_write;
    startup.lpAttributeList = list;
    let mut process = PROCESS_INFORMATION::default();
    let created = unsafe {
        CreateProcessW(
            PCWSTR::from_raw(application.as_ptr()),
            Some(PWSTR::from_raw(command_line.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_SUSPENDED
                | CREATE_UNICODE_ENVIRONMENT
                | CREATE_NO_WINDOW
                | EXTENDED_STARTUPINFO_PRESENT,
            Some(environment.as_mut_ptr().cast()),
            PCWSTR::from_raw(current_directory.as_ptr()),
            (&raw const startup.StartupInfo),
            &mut process,
        )
    };
    environment.zeroize();
    unsafe { DeleteProcThreadAttributeList(list) };
    // Parent no longer owns the child ends. Closing them is required for EOF
    // detection in the drain threads.
    close_many(&[stdin_read, stdout_write, stderr_write]);
    if created.is_err() {
        close_many(&[stdin_write, stdout_read, stderr_read]);
        return Err(if appcontainer.is_some() {
            RuntimeError::IsolationUnavailable
        } else {
            RuntimeError::Start
        });
    }
    if process_image_path(process.hProcess)
        .is_none_or(|actual| !same_windows_path(&actual, &spec.executable))
    {
        unsafe {
            let _ = TerminateProcess(process.hProcess, 1);
            let _ = CloseHandle(process.hThread);
            let _ = WaitForSingleObject(process.hProcess, u32::MAX);
            let _ = CloseHandle(process.hProcess);
        }
        close_many(&[stdin_write, stdout_read, stderr_read]);
        return Err(RuntimeError::Start);
    }
    if job.assign_handle(process.hProcess).is_err() {
        unsafe {
            let _ = TerminateProcess(process.hProcess, 1);
        }
        unsafe {
            let _ = CloseHandle(process.hThread);
            let _ = CloseHandle(process.hProcess);
        }
        close_many(&[stdin_write, stdout_read, stderr_read]);
        return Err(RuntimeError::Start);
    }
    if let Some(observer) = process_observer {
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let observed = unsafe {
            GetProcessTimes(
                process.hProcess,
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        }
        .is_ok()
            && observer(ProcessStartEvidence {
                process_id: process.dwProcessId,
                creation_time_100ns: ((creation.dwHighDateTime as u64) << 32)
                    | creation.dwLowDateTime as u64,
                job_id: format!("job_{}", star_ipc::nonce()),
            });
        if !observed {
            let _ = job.terminate(1);
            unsafe {
                let _ = CloseHandle(process.hThread);
                let _ = WaitForSingleObject(process.hProcess, u32::MAX);
                let _ = CloseHandle(process.hProcess);
            }
            close_many(&[stdin_write, stdout_read, stderr_read]);
            return Err(RuntimeError::Start);
        }
    }

    let stdout = unsafe { File::from_raw_handle(stdout_read.0 as _) };
    let stderr = unsafe { File::from_raw_handle(stderr_read.0 as _) };
    let stdin = unsafe { File::from_raw_handle(stdin_write.0 as _) };
    let stdout_limit = spec.max_stdout_bytes;
    let stderr_limit = spec.max_stderr_bytes;
    let (final_sender, final_receiver) = std::sync::mpsc::channel();
    let observe_final = cancel_plan
        .and_then(|plan| plan.final_frame_exit_grace)
        .map(|_| final_sender);
    let stdout_line_observer =
        cancel_plan.and_then(|plan| plan.stdout_line_observer.as_ref().cloned());
    let stdout_task = thread::spawn(move || {
        drain_file(stdout, stdout_limit, observe_final, stdout_line_observer)
    });
    let stderr_task = thread::spawn(move || drain_file(stderr, stderr_limit, None, None));
    let stdin_bytes = spec.stdin.clone();
    let cancel_plan_for_stdin = cancel_plan.cloned();
    let cancel_trigger = Arc::new(AtomicU8::new(0));
    let cancel_trigger_for_stdin = Arc::clone(&cancel_trigger);
    let process_done = Arc::new(AtomicBool::new(false));
    let process_done_for_stdin = Arc::clone(&process_done);
    let stdin_task = thread::spawn(move || {
        let mut stdin = stdin;
        if let Some(mut bytes) = stdin_bytes {
            let written = stdin.write_all(&bytes);
            bytes.zeroize();
            written.map_err(|_| RuntimeError::Start)?;
        }
        stdin.flush().map_err(|_| RuntimeError::Start)?;
        if let Some(plan) = cancel_plan_for_stdin {
            while !process_done_for_stdin.load(Ordering::Acquire) {
                let trigger = cancel_trigger_for_stdin.load(Ordering::Acquire);
                let frame = match trigger {
                    1 => plan.user_frame.as_ref(),
                    2 => plan.deadline_frame.as_ref(),
                    _ => None,
                };
                if let Some(frame) = frame {
                    stdin.write_all(frame).map_err(|_| RuntimeError::Start)?;
                    stdin.flush().map_err(|_| RuntimeError::Start)?;
                    break;
                }
                if trigger != 0 {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        Ok(())
    });

    // Readers and the Job are established before the suspended primary thread
    // resumes, so the process cannot create an uncontained child first.
    if unsafe { ResumeThread(process.hThread) } == u32::MAX {
        unsafe {
            let _ = TerminateProcess(process.hProcess, 1);
        }
        unsafe {
            let _ = CloseHandle(process.hThread);
            let _ = WaitForSingleObject(process.hProcess, u32::MAX);
            let _ = CloseHandle(process.hProcess);
        }
        process_done.store(true, Ordering::Release);
        let _ = stdin_task.join();
        let _ = stdout_task.join();
        let _ = stderr_task.join();
        return Err(RuntimeError::Start);
    }
    unsafe {
        let _ = CloseHandle(process.hThread);
    }
    let started = Instant::now();
    let mut cancel_started = None;
    let mut final_frame_seen = None;
    let termination_kind = loop {
        if unsafe { WaitForSingleObject(process.hProcess, 25) } == WAIT_OBJECT_0 {
            break cancel_trigger.load(Ordering::Acquire);
        }
        if let Some(plan) = cancel_plan {
            if final_frame_seen.is_none()
                && let Ok(seen_at) = final_receiver.try_recv()
            {
                final_frame_seen = Some(seen_at);
            }
            if final_frame_seen.is_some_and(|seen| {
                plan.final_frame_exit_grace
                    .is_some_and(|grace| seen.elapsed() >= grace)
            }) {
                job.terminate(1)?;
                let _ = unsafe { WaitForSingleObject(process.hProcess, u32::MAX) };
                break 3;
            }
            let trigger = if cancellation.is_some_and(RuntimeCancellation::is_cancelled) {
                1
            } else if started.elapsed() >= spec.timeout {
                2
            } else {
                0
            };
            if trigger != 0 && cancel_trigger.load(Ordering::Acquire) == 0 {
                cancel_trigger.store(trigger, Ordering::Release);
                cancel_started = Some(Instant::now());
            }
            let effective_grace = if cancel_trigger.load(Ordering::Acquire) == 1 {
                cancellation
                    .and_then(RuntimeCancellation::force_after_ms)
                    .map_or(plan.grace, |milliseconds| {
                        std::time::Duration::from_millis(milliseconds.into())
                    })
            } else {
                plan.grace
            };
            if cancel_started.is_some_and(|started| started.elapsed() >= effective_grace) {
                let trigger = cancel_trigger.load(Ordering::Acquire);
                job.terminate(1)?;
                let _ = unsafe { WaitForSingleObject(process.hProcess, u32::MAX) };
                break trigger;
            }
        } else if cancellation.is_some_and(RuntimeCancellation::is_cancelled) {
            job.terminate(1)?;
            let _ = unsafe { WaitForSingleObject(process.hProcess, u32::MAX) };
            break 1;
        } else if started.elapsed() >= spec.timeout {
            job.terminate(1)?;
            let _ = unsafe { WaitForSingleObject(process.hProcess, u32::MAX) };
            break 2;
        }
    };
    let mut exit_code = 0u32;
    let _ = unsafe { GetExitCodeProcess(process.hProcess, &mut exit_code) };
    unsafe {
        let _ = CloseHandle(process.hProcess);
    }
    process_done.store(true, Ordering::Release);
    let stdin_result = stdin_task.join().map_err(|_| RuntimeError::Start)?;
    let stdout = stdout_task.join().map_err(|_| RuntimeError::Start)?;
    let stderr = stderr_task.join().map_err(|_| RuntimeError::Start)?;
    let termination = match termination_kind {
        1 => "cancelled",
        2 => "timeout",
        3 => "protocol_forced",
        _ => "exited",
    };
    if process_end_observer.is_some_and(|observer| {
        !observer(ProcessEndEvidence {
            exit_code: Some(exit_code),
            termination: termination.to_owned(),
            stdout_bytes: stdout.total_bytes,
            stderr_bytes: stderr.total_bytes,
            stdout_limit_exceeded: stdout.exceeded_limit,
            stderr_limit_exceeded: stderr.exceeded_limit,
        })
    }) {
        return Err(RuntimeError::Start);
    }
    if termination_kind == 2 {
        return Err(RuntimeError::Timeout);
    }
    if termination_kind == 1 {
        return Err(RuntimeError::Cancelled);
    }
    if termination_kind == 3 {
        return Err(RuntimeError::ProtocolInvalid);
    }
    stdin_result?;
    if stdout.exceeded_limit || stderr.exceeded_limit {
        return Err(RuntimeError::OutputLimit);
    }
    Ok(DirectExeOutcome {
        stdout,
        stderr,
        exit_code: Some(exit_code),
    })
}

fn anonymous_pipe() -> Result<(HANDLE, HANDLE), RuntimeError> {
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    unsafe { CreatePipe(&mut read, &mut write, Some(&raw const attributes), 0) }
        .map_err(|_| RuntimeError::Start)?;
    Ok((read, write))
}

/// Owns the AppContainer SID for the complete `CreateProcessW` call.  The
/// capability list is intentionally empty: adapter processes do not receive
/// Internet, private-network or broad-file-system capabilities.
struct AppContainerCapabilities {
    sid: PSID,
    security: SECURITY_CAPABILITIES,
}

impl Drop for AppContainerCapabilities {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeSid(self.sid);
        }
    }
}

fn appcontainer_capabilities(profile: &str) -> Result<AppContainerCapabilities, RuntimeError> {
    let profile = HSTRING::from(profile);
    let sid = create_or_open_appcontainer_sid(&profile)?;
    if loopback_exempt(sid)? {
        unsafe {
            let _ = FreeSid(sid);
        }
        return Err(RuntimeError::IsolationUnavailable);
    }
    Ok(AppContainerCapabilities {
        sid,
        security: SECURITY_CAPABILITIES {
            AppContainerSid: sid,
            Capabilities: std::ptr::null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        },
    })
}

fn create_or_open_appcontainer_sid(profile: &HSTRING) -> Result<PSID, RuntimeError> {
    let sid = match unsafe { CreateAppContainerProfile(profile, profile, profile, None) } {
        Ok(sid) => sid,
        Err(error) if error.code().0 as u32 == 0x8007_00B7 => {
            unsafe { DeriveAppContainerSidFromAppContainerName(profile) }
                .map_err(|_| RuntimeError::IsolationUnavailable)?
        }
        Err(_) => return Err(RuntimeError::IsolationUnavailable),
    };
    Ok(sid)
}

pub fn appcontainer_profile_folder(profile: &str) -> Result<PathBuf, RuntimeError> {
    let profile = HSTRING::from(profile);
    let sid = create_or_open_appcontainer_sid(&profile)?;
    let mut sid_text = PWSTR::null();
    if unsafe { ConvertSidToStringSidW(sid, &mut sid_text) }.is_err() {
        unsafe {
            let _ = FreeSid(sid);
        }
        return Err(RuntimeError::IsolationUnavailable);
    }
    let folder = match unsafe { GetAppContainerFolderPath(PCWSTR(sid_text.0)) } {
        Ok(folder) => folder,
        Err(_) => {
            unsafe {
                let _ = LocalFree(Some(HLOCAL(sid_text.0.cast())));
                let _ = FreeSid(sid);
            }
            return Err(RuntimeError::IsolationUnavailable);
        }
    };
    let path = unsafe { folder.to_string() }
        .map(PathBuf::from)
        .map_err(|_| RuntimeError::IsolationUnavailable);
    unsafe {
        CoTaskMemFree(Some(folder.0.cast_const().cast()));
        let _ = LocalFree(Some(HLOCAL(sid_text.0.cast())));
        let _ = FreeSid(sid);
    }
    path
}

pub fn appcontainer_profile_sid_string(profile: &str) -> Result<String, RuntimeError> {
    let profile = HSTRING::from(profile);
    let sid = create_or_open_appcontainer_sid(&profile)?;
    let mut sid_text = PWSTR::null();
    if unsafe { ConvertSidToStringSidW(sid, &mut sid_text) }.is_err() {
        unsafe {
            let _ = FreeSid(sid);
        }
        return Err(RuntimeError::IsolationUnavailable);
    }
    let value = unsafe { sid_text.to_string() }.map_err(|_| RuntimeError::IsolationUnavailable);
    unsafe {
        let _ = LocalFree(Some(HLOCAL(sid_text.0.cast())));
        let _ = FreeSid(sid);
    }
    value
}

fn loopback_exempt(sid: PSID) -> Result<bool, RuntimeError> {
    let heap = unsafe { GetProcessHeap() }.map_err(|_| RuntimeError::IsolationUnavailable)?;
    let mut count = 0u32;
    let mut entries: *mut SID_AND_ATTRIBUTES = std::ptr::null_mut();
    let status = unsafe { NetworkIsolationGetAppContainerConfig(&mut count, &mut entries) };
    if status != 0 {
        return Err(RuntimeError::IsolationUnavailable);
    }
    if entries.is_null() {
        return Ok(false);
    }
    let configured = unsafe { std::slice::from_raw_parts(entries, count as usize) };
    let exempt = configured
        .iter()
        .any(|entry| unsafe { EqualSid(entry.Sid, sid) }.is_ok());
    // The API contract allocates each SID and the array from the process
    // heap. Free both levels exactly as the Microsoft sample specifies.
    unsafe {
        for entry in configured {
            if !entry.Sid.is_invalid() {
                let _ = HeapFree(heap, HEAP_FLAGS(0), Some(entry.Sid.0.cast_const()));
            }
        }
        let _ = HeapFree(heap, HEAP_FLAGS(0), Some(entries.cast_const().cast()));
    }
    Ok(exempt)
}

fn drain_file(
    mut file: File,
    limit: u64,
    final_sender: Option<std::sync::mpsc::Sender<Instant>>,
    line_observer: Option<super::StdoutLineObserver>,
) -> CapturedStream {
    let mut captured = Vec::new();
    let mut total_bytes = 0u64;
    let mut exceeded_limit = false;
    let mut buffer = [0u8; 16 * 1024];
    let mut pending_line = Vec::new();
    let mut final_sent = false;
    loop {
        let read = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => break,
        };
        total_bytes += read as u64;
        let remaining = limit.saturating_sub(captured.len() as u64) as usize;
        captured.extend_from_slice(&buffer[..read.min(remaining)]);
        exceeded_limit |= total_bytes > limit;
        if (final_sender.is_some() || line_observer.is_some()) && !final_sent {
            pending_line.extend_from_slice(&buffer[..read]);
            while let Some(position) = pending_line.iter().position(|byte| *byte == b'\n') {
                let line: Vec<_> = pending_line.drain(..=position).collect();
                let line = &line[..line.len().saturating_sub(1)];
                if line.len() <= 8 * 1024 * 1024 {
                    if let Some(observer) = line_observer.as_ref() {
                        observer(line);
                    }
                }
                if let Some(final_sender) = final_sender.as_ref()
                    && line.len() <= 8 * 1024 * 1024
                    && serde_json::from_slice::<serde_json::Value>(line)
                        .ok()
                        .and_then(|value| {
                            value
                                .get("frame")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned)
                        })
                        .as_deref()
                        == Some("result")
                {
                    let _ = final_sender.send(Instant::now());
                    final_sent = true;
                    pending_line.clear();
                    break;
                }
            }
            if pending_line.len() > 8 * 1024 * 1024 {
                pending_line.clear();
            }
        }
    }
    CapturedStream {
        captured,
        total_bytes,
        exceeded_limit,
    }
}

fn command_line(spec: &DirectExeSpec) -> Result<Vec<u16>, RuntimeError> {
    let mut command_line = quote_windows(spec.executable.as_os_str())?;
    for argument in &spec.argv {
        command_line.push(' ' as u16);
        command_line.extend(quote_windows(argument.as_os_str())?);
    }
    if command_line.len() >= 32_767 {
        return Err(RuntimeError::ProtocolInvalid);
    }
    command_line.push(0);
    Ok(command_line)
}

fn quote_windows(value: &OsStr) -> Result<Vec<u16>, RuntimeError> {
    let value: Vec<u16> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let quoted = value.is_empty()
        || value
            .iter()
            .any(|value| matches!(*value, 0x20 | 0x09 | 0x22));
    if !quoted {
        return Ok(value);
    }
    let mut result = vec![0x22];
    let mut slashes = 0usize;
    for character in value {
        if character == 0x5c {
            slashes += 1;
        } else if character == 0x22 {
            result.extend(std::iter::repeat_n(0x5c, slashes * 2 + 1));
            result.push(character);
            slashes = 0;
        } else {
            result.extend(std::iter::repeat_n(0x5c, slashes));
            result.push(character);
            slashes = 0;
        }
    }
    result.extend(std::iter::repeat_n(0x5c, slashes * 2));
    result.push(0x22);
    Ok(result)
}

fn wide_nul(value: &OsStr) -> Result<Vec<u16>, RuntimeError> {
    let mut value: Vec<u16> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err(RuntimeError::ProtocolInvalid);
    }
    value.push(0);
    Ok(value)
}

fn process_image_path(process: HANDLE) -> Option<PathBuf> {
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    if length == 0 || length as usize >= buffer.len() {
        return None;
    }
    buffer.truncate(length as usize);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

fn same_windows_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches(r"\\?\")
        .eq_ignore_ascii_case(
            right
                .as_os_str()
                .to_string_lossy()
                .replace('/', "\\")
                .trim_start_matches(r"\\?\"),
        )
}

fn environment_block(spec: &DirectExeSpec) -> Result<Vec<u16>, RuntimeError> {
    let mut values = std::collections::BTreeMap::<String, OsString>::new();
    let system_root = std::env::var_os("SystemRoot").ok_or(RuntimeError::Start)?;
    values.insert("SYSTEMROOT".to_owned(), system_root.clone());
    values.insert("WINDIR".to_owned(), system_root);
    let temp = if let Some(profile) = spec.appcontainer_profile.as_deref() {
        let root = appcontainer_profile_folder(profile)?;
        let local = root.join("LocalState");
        let roaming = root.join("RoamingState");
        let temporary = root.join("TempState");
        std::fs::create_dir_all(&local).map_err(|_| RuntimeError::IsolationUnavailable)?;
        std::fs::create_dir_all(&roaming).map_err(|_| RuntimeError::IsolationUnavailable)?;
        std::fs::create_dir_all(&temporary).map_err(|_| RuntimeError::IsolationUnavailable)?;
        values.insert("USERPROFILE".to_owned(), root.into_os_string());
        values.insert("LOCALAPPDATA".to_owned(), local.into_os_string());
        values.insert("APPDATA".to_owned(), roaming.into_os_string());
        temporary.into_os_string()
    } else {
        std::env::temp_dir().into_os_string()
    };
    values.insert("TEMP".to_owned(), temp.clone());
    values.insert("TMP".to_owned(), temp);
    for (key, value) in &spec.environment {
        let key = key
            .to_str()
            .ok_or(RuntimeError::ProtocolInvalid)?
            .to_ascii_uppercase();
        let value_utf16: Vec<_> = value.encode_wide().collect();
        if key.is_empty()
            || key.len() > 128
            || !key.chars().enumerate().all(|(index, character)| {
                if index == 0 {
                    character == '_' || character.is_ascii_alphabetic()
                } else {
                    character == '_' || character.is_ascii_alphanumeric()
                }
            })
            || key.starts_with('=')
            || key.contains('\0')
            || matches!(
                key.as_str(),
                "PATH" | "PATHEXT" | "COMSPEC" | "PSMODULEPATH" | "PROMPT"
            )
            || value_utf16.contains(&0)
            || String::from_utf16(&value_utf16).is_err()
        {
            return Err(RuntimeError::ProtocolInvalid);
        }
        values.insert(key, value.clone());
    }
    let mut block = Vec::new();
    for (key, value) in values {
        block.extend(OsStr::new(&key).encode_wide());
        block.push('=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    if block.len() > 32_767 {
        block.zeroize();
        return Err(RuntimeError::ProtocolInvalid);
    }
    Ok(block)
}

fn close_many(handles: &[HANDLE]) {
    for handle in handles {
        if !handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(*handle);
            }
        }
    }
}
