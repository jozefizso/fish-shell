//! Native process launch and lifetime ownership for the Windows execution backend.
//!
//! Only explicitly duplicated child endpoints are inherited. A root cannot run
//! until its job membership and retained runtime registration have been installed.
//! The group, not a PID lookup or a cached language status, owns rollback.

use crate::io::IoChain;
use crate::native_accounting::{self, TrackedJob};
use crate::native_fd_bridge::{self, BridgeHandle, PRIVATE_FD_ENV};
use crate::threads::{NativeCancellation, NativeRuntime, NativeWaitRegistration};
use crate::native_limits::NativeLimits;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString, c_void};
use std::fs::File;
use std::io;
use std::mem::{self, size_of};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{
    AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle,
};
use std::path::Path;
use std::ptr;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;
const WAIT_FAILED: u32 = u32::MAX;
const INFINITE: u32 = u32::MAX;
const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
const CREATE_SUSPENDED: u32 = 0x0000_0004;
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const DUPLICATE_SAME_ACCESS: u32 = 2;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const JOB_OBJECT_LIMIT_PROCESS_TIME: u32 = 0x0000_0002;
const JOB_OBJECT_LIMIT_ACTIVE_PROCESS: u32 = 0x0000_0008;
const JOB_OBJECT_LIMIT_PROCESS_MEMORY: u32 = 0x0000_0100;
const JOB_OBJECT_LIMIT_JOB_MEMORY: u32 = 0x0000_0200;
const JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION: i32 = 1;
const JOB_OBJECT_ASSOCIATE_COMPLETION_PORT_INFORMATION: i32 = 7;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
const ROLLBACK_STATUS: u32 = 1;

#[repr(C)]
struct StartupInfoW {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_count_chars: u32,
    y_count_chars: u32,
    fill_attribute: u32,
    flags: u32,
    show_window: u16,
    reserved_bytes: u16,
    reserved_data: *mut u8,
    stdin: RawHandle,
    stdout: RawHandle,
    stderr: RawHandle,
}

#[repr(C)]
struct StartupInfoExW {
    startup: StartupInfoW,
    attributes: *mut c_void,
}

#[repr(C)]
struct ProcessInformation {
    process: RawHandle,
    thread: RawHandle,
    pid: u32,
    thread_id: u32,
}

#[repr(C)]
#[derive(Default)]
struct JobBasicLimits {
    process_user_time: i64,
    job_user_time: i64,
    flags: u32,
    minimum_working_set: usize,
    maximum_working_set: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
#[derive(Default)]
struct IoCounters {
    read_operations: u64,
    write_operations: u64,
    other_operations: u64,
    read_bytes: u64,
    write_bytes: u64,
    other_bytes: u64,
}

#[repr(C)]
#[derive(Default)]
struct JobExtendedLimits {
    basic: JobBasicLimits,
    io: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory: usize,
    peak_job_memory: usize,
}

#[repr(C)]
struct JobCompletionPort {
    key: *mut c_void,
    port: RawHandle,
}

#[repr(C)]
#[derive(Default)]
struct JobAccounting {
    total_user_time: i64,
    total_kernel_time: i64,
    period_user_time: i64,
    period_kernel_time: i64,
    page_faults: u32,
    total_processes: u32,
    active_processes: u32,
    terminated_processes: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> RawHandle;
    fn DuplicateHandle(
        source_process: RawHandle,
        source: RawHandle,
        target_process: RawHandle,
        target: *mut RawHandle,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        size: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *mut c_void,
        size: usize,
        previous: *mut c_void,
        returned_size: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn CreateProcessW(
        application: *const u16,
        command_line: *mut u16,
        process_security: *const c_void,
        thread_security: *const c_void,
        inherit: i32,
        flags: u32,
        environment: *const c_void,
        cwd: *const u16,
        startup: *const StartupInfoW,
        process: *mut ProcessInformation,
    ) -> i32;
    fn ResumeThread(thread: RawHandle) -> u32;
    fn TerminateProcess(process: RawHandle, status: u32) -> i32;
    fn GetExitCodeProcess(process: RawHandle, status: *mut u32) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
    fn CreateJobObjectW(security: *const c_void, name: *const u16) -> RawHandle;
    fn SetInformationJobObject(job: RawHandle, class: i32, data: *const c_void, size: u32) -> i32;
    fn QueryInformationJobObject(
        job: RawHandle,
        class: i32,
        data: *mut c_void,
        size: u32,
        returned_size: *mut u32,
    ) -> i32;
    fn AssignProcessToJobObject(job: RawHandle, process: RawHandle) -> i32;
    fn TerminateJobObject(job: RawHandle, status: u32) -> i32;
    fn GetConsoleProcessList(processes: *mut u32, count: u32) -> u32;
    fn GenerateConsoleCtrlEvent(control: u32, process_group: u32) -> i32;
    fn CreateIoCompletionPort(
        file: RawHandle,
        existing: RawHandle,
        key: usize,
        concurrency: u32,
    ) -> RawHandle;
    fn GetQueuedCompletionStatus(
        port: RawHandle,
        bytes: *mut u32,
        key: *mut usize,
        overlapped: *mut *mut c_void,
        milliseconds: u32,
    ) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn CompareStringOrdinal(
        left: *const u16,
        left_length: i32,
        right: *const u16,
        right_length: i32,
        ignore_case: i32,
    ) -> i32;
}

/// A cooperative request is not an acknowledgement or a process completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeBreakOutcome {
    Requested,
    AlreadyExited(u32),
    UnsupportedConsole,
}

/// Request provenance remains independent of the observed native exit DWORD.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeBreakAttempt {
    Outcome(NativeBreakOutcome),
    Failed {
        kind: io::ErrorKind,
        win32: Option<i32>,
    },
}

#[derive(Default)]
struct NativeControlState {
    last_break: Option<NativeBreakAttempt>,
    break_requested: bool,
}

fn current_console_contains(pid: u32) -> io::Result<bool> {
    let mut local = [0u32; 64];
    let count = unsafe { GetConsoleProcessList(local.as_mut_ptr(), local.len() as u32) };
    if count == 0 {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(6) {
            Ok(false) // This process has no console to which it may target controls.
        } else {
            Err(error)
        };
    }
    if count as usize <= local.len() {
        return Ok(local[..count as usize].contains(&pid));
    }
    let mut processes = vec![0u32; count as usize];
    let count = unsafe { GetConsoleProcessList(processes.as_mut_ptr(), processes.len() as u32) };
    if count == 0 {
        return Err(io::Error::last_os_error());
    }
    if count as usize > processes.len() {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "console membership changed during inspection",
        ));
    }
    Ok(processes[..count as usize].contains(&pid))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn unexpected_wait(result: u32) -> io::Error {
    if result == WAIT_FAILED {
        io::Error::last_os_error()
    } else {
        io::Error::other(format!("unexpected native process wait result {result}"))
    }
}

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut units: Vec<_> = value.encode_wide().collect();
    if units.contains(&0) {
        return Err(invalid("NUL in native process argument or path"));
    }
    units.push(0);
    Ok(units)
}

fn native_path(path: &Path) -> io::Result<Vec<u16>> {
    // Reuse N1 namespace validation, rather than allowing CreateProcess's ambient
    // cwd or per-drive state to resolve a relative executable/cwd.
    let text = crate::path::native_path_text(path)?;
    let absolute = crate::path::native_absolute_path(&text)?;
    // Many native toolchains interpret the extended namespace as a UNC share.
    // Prefer an ordinary spelling only after strict validation and identity proof.
    // Extended-only names retain their original native capability, not an alias.
    let absolute = match crate::path::native_tool_path(&absolute) {
        Ok(ordinary) => ordinary,
        Err(error) if error.kind() == io::ErrorKind::Unsupported => absolute,
        Err(error) => return Err(error),
    };
    wide(absolute.as_os_str())
}

fn command_line(argv: &[OsString]) -> io::Result<Vec<u16>> {
    let Some(first) = argv.first() else {
        return Err(invalid("native argv must include argv[0]"));
    };
    let mut line = Vec::new();
    line.push(34);
    // Keep argv[0] quoted even without separators. The CRT parses it separately:
    // backslashes are literal and a quote cannot be represented in that field.
    // Do not silently corrupt such argv[0].
    for unit in first.encode_wide() {
        if unit == 0 || unit == 34 {
            return Err(invalid("NUL or quote in native argv[0]"));
        }
        line.push(unit);
    }
    line.push(34);
    for argument in &argv[1..] {
        // Only space and tab separate CRT arguments. Native metacharacters are
        // ordinary data, not cmd operators; unnecessary quotes can also change
        // how legacy native tools such as attrib interpret their switches.
        let quoted = argument.is_empty()
            || argument.as_encoded_bytes().iter().any(|byte| matches!(byte, b' ' | b'\t' | b'"'));
        line.push(32);
        if !quoted {
            for unit in argument.encode_wide() {
                if unit == 0 {
                    return Err(invalid("NUL in native process argument"));
                }
                line.push(unit);
            }
            continue;
        }
        line.push(34);
        let mut slashes = 0;
        for unit in argument.encode_wide() {
            if unit == 0 {
                return Err(invalid("NUL in native process argument"));
            }
            if unit == 92 {
                slashes += 1;
                continue;
            }
            // 2n backslashes before a quote represent n literal backslashes;
            // the additional backslash makes the quote itself literal.
            let count = if unit == 34 { slashes * 2 + 1 } else { slashes };
            line.extend(std::iter::repeat(92).take(count));
            line.push(unit);
            slashes = 0;
        }
        // Escape trailing backslashes before the closing syntactic quote.
        line.extend(std::iter::repeat(92).take(slashes * 2));
        line.push(34);
    }
    if line.len() >= 32_767 {
        return Err(invalid(
            "native process command line exceeds 32767 UTF-16 units",
        ));
    }
    line.push(0);
    Ok(line)
}

fn environment_name_cmp(left: &[u16], right: &[u16]) -> Ordering {
    // Lengths and nonempty names are checked before sorting. Match N1's
    // locale-independent, case-insensitive Windows environment ordering.
    match unsafe {
        CompareStringOrdinal(
            left.as_ptr(),
            left.len() as i32,
            right.as_ptr(),
            right.len() as i32,
            1,
        )
    } {
        1 => Ordering::Less,
        2 => Ordering::Equal,
        3 => Ordering::Greater,
        _ => unreachable!("validated CompareStringOrdinal inputs"),
    }
}

fn environment_block(environment: &[(OsString, OsString)]) -> io::Result<Vec<u16>> {
    let mut entries = Vec::with_capacity(environment.len());
    let mut capacity = 1usize;
    for (name, value) in environment {
        let name: Vec<_> = name.encode_wide().collect();
        let value: Vec<_> = value.encode_wide().collect();
        if name.is_empty()
            || name.len() > i32::MAX as usize
            || name.iter().any(|unit| *unit == 0 || *unit == 61)
            || value.contains(&0)
        {
            return Err(invalid("invalid native environment name or value"));
        }
        capacity = capacity
            .checked_add(name.len())
            .and_then(|size| size.checked_add(value.len()))
            .and_then(|size| size.checked_add(2))
            .ok_or_else(|| invalid("native environment block is too large"))?;
        entries.push((name, value));
    }
    entries.sort_unstable_by(|left, right| environment_name_cmp(&left.0, &right.0));
    if entries
        .windows(2)
        .any(|pair| environment_name_cmp(&pair[0].0, &pair[1].0) == Ordering::Equal)
    {
        return Err(invalid(
            "ambiguous case-equivalent native environment names",
        ));
    }
    let mut block = Vec::with_capacity(capacity.max(2));
    for (name, value) in entries {
        block.extend_from_slice(&name);
        block.push(61);
        block.extend_from_slice(&value);
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

/// An image file pinned against writes, deletion and leaf replacement until the
/// path-based CreateProcessW call has finished. This is not handle-based launch.
struct PinnedImage {
    file: File,
    path: Vec<u16>,
}

impl PinnedImage {
    fn open(path: &Path) -> io::Result<Self> {
        let probe = crate::wutil::native_open(path, 0x80, true)?;
        let final_path =
            crate::path::native_absolute_path(&crate::wutil::native_final_path(&probe)?)?;
        let name = wide(final_path.as_os_str())?;
        let raw = unsafe {
            // Execute access participates in sharing checks; attributes alone do not.
            CreateFileW(
                name.as_ptr(),
                0x20 | 0x80,
                1,
                ptr::null(),
                3,
                0,
                ptr::null_mut(),
            )
        };
        if raw == (-1isize) as RawHandle {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_handle(raw) };
        if crate::wutil::native_identity_for_file(&file)?
            != crate::wutil::native_identity_for_file(&probe)?
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "executable identity changed while pinning its path",
            ));
        }
        let path = native_path(&crate::path::native_absolute_path(
            &crate::wutil::native_final_path(&file)?,
        )?)?;
        Ok(Self { file, path })
    }
}

struct NativeImageIdentity {
    // A static LazyLock is not dropped at shutdown. Keep the original image
    // file pinned for the process lifetime, not merely for one bridge launch.
    _image: PinnedImage,
    identity: crate::wutil::NativeFileIdentity,
}

static NATIVE_IMAGE_IDENTITY: LazyLock<io::Result<NativeImageIdentity>> = LazyLock::new(|| {
    let image = PinnedImage::open(crate::wutil::native_current_exe_path()?)?;
    let identity = crate::wutil::native_identity_for_file(&image.file)?;
    Ok(NativeImageIdentity {
        _image: image,
        identity,
    })
});

fn native_image_identity() -> io::Result<&'static NativeImageIdentity> {
    LazyLock::force(&NATIVE_IMAGE_IDENTITY)
        .as_ref()
        .map_err(|error| match error.raw_os_error() {
            Some(code) => io::Error::from_raw_os_error(code),
            None => io::Error::new(error.kind(), error.to_string()),
        })
}

/// Pin the startup executable and its full native identity before evaluating
/// user code. Later bridge launches never reopen current_exe's mutable path.
///
/// The initial loader-to-startup path-resolution window is not a handle-based
/// image-loading guarantee. The retained file prevents ordinary running-image
/// path replacement from changing this process's trusted bridge identity.
pub fn initialize_native_image_identity() -> io::Result<()> {
    native_image_identity().map(|_| ())
}

/// The startup-pinned image, not a fresh PATH lookup or a replaceable current_exe spelling.
pub fn native_loaded_fish_path() -> io::Result<std::path::PathBuf> {
    crate::path::native_absolute_path(&crate::wutil::native_final_path(
        &native_image_identity()?._image.file,
    )?)
}

pub(crate) fn is_current_native_image(file: &File) -> io::Result<bool> {
    Ok(crate::wutil::native_identity_for_file(file)? == native_image_identity()?.identity)
}

struct ChildIo {
    // These duplicates are inheritable only within this local launch scope. The
    // explicit allowlist prevents simultaneous launches inheriting one another.
    handles: Vec<OwnedHandle>,
    allowlist: Vec<RawHandle>,
    standard: [RawHandle; 3],
    bridge: Vec<BridgeHandle>,
}

impl ChildIo {
    fn prepare(io_chain: &IoChain, bridge_allowed: bool) -> io::Result<(Self, bool)> {
        let bindings = io_chain.child_bindings()?;
        // Materialize buffered/overlapped standard endpoints into synchronous
        // child handles before deciding whether the private fish bridge is needed.
        // Extra logical descriptors still require the same-image bridge below.
        for binding in &bindings {
            if let Some(endpoint) = &binding.endpoint {
                endpoint.child_handle()?;
            }
        }
        let needs_bridge = bindings.iter().any(|binding| {
            binding
                .endpoint
                .as_ref()
                .is_some_and(|endpoint| binding.fd > 2 || endpoint.is_overlapped())
        });
        if needs_bridge && !bridge_allowed {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native child requires the private fish descriptor bridge",
            ));
        }
        let mut result = Self {
            handles: Vec::new(),
            allowlist: Vec::new(),
            standard: [ptr::null_mut(); 3],
            bridge: Vec::new(),
        };
        let mut descriptors = HashSet::with_capacity(bindings.len());
        let mut aliases = HashMap::new();
        let process = unsafe { GetCurrentProcess() };
        for binding in bindings {
            if binding.fd < 0 || !descriptors.insert(binding.fd) {
                return Err(invalid("negative or duplicate native child descriptor"));
            }
            let Some(endpoint) = binding.endpoint else {
                // In particular, a closed standard descriptor remains NULL.
                // Closed extra descriptors need no protocol or inherited HANDLE.
                continue;
            };
            let source = endpoint.child_handle()?;
            let source_raw = source.as_raw_handle();
            let inherited = if let Some(index) = aliases.get(&(source_raw as usize)) {
                result.allowlist[*index]
            } else {
                let mut raw = ptr::null_mut();
                if unsafe {
                    DuplicateHandle(
                        process,
                        source_raw,
                        process,
                        &mut raw,
                        0,
                        1,
                        DUPLICATE_SAME_ACCESS,
                    )
                } == 0
                {
                    return Err(io::Error::last_os_error());
                }
                let owner = unsafe { OwnedHandle::from_raw_handle(raw) };
                aliases.insert(source_raw as usize, result.allowlist.len());
                result.allowlist.push(raw);
                result.handles.push(owner);
                raw
            };
            if binding.fd <= 2 {
                result.standard[binding.fd as usize] = inherited;
            }
            if needs_bridge {
                result.bridge.push(BridgeHandle {
                    fd: binding.fd,
                    handle: inherited,
                    direction: endpoint.direction(),
                    overlapped: endpoint.is_overlapped(),
                });
            }
        }
        Ok((result, needs_bridge))
    }
}

/// The opaque attribute list has pointer-sized alignment, not byte alignment.
/// Its HANDLE array and all duplicate owners outlive the CreateProcessW call.
struct AttributeList {
    storage: Vec<usize>,
}

impl AttributeList {
    fn new(handles: &[RawHandle]) -> io::Result<Self> {
        let mut bytes = 0;
        let initialized =
            unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut bytes) };
        if initialized != 0 || bytes == 0 {
            return Err(io::Error::other(
                "invalid native process attribute-list sizing result",
            ));
        }
        let words = bytes
            .checked_add(size_of::<usize>() - 1)
            .ok_or_else(|| invalid("native process attribute list is too large"))?
            / size_of::<usize>();
        let mut storage = vec![0usize; words];
        if unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut bytes)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut result = Self { storage };
        if unsafe {
            UpdateProcThreadAttribute(
                result.as_mut_ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                handles.as_ptr().cast_mut().cast(),
                mem::size_of_val(handles),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(result)
    }

    fn as_mut_ptr(&mut self) -> *mut c_void {
        self.storage.as_mut_ptr().cast()
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.as_mut_ptr()) };
    }
}

/// A live process handle and its independently owned runtime registration.
/// A group may drain the registration at finish; callers retaining this child
/// still retain the real process handle and can read its immutable exit DWORD.
pub struct NativeChild {
    pid: u32,
    process: Arc<OwnedHandle>,
    registration: Mutex<Option<NativeWaitRegistration>>,
    exit_status: OnceLock<u32>,
    control: Mutex<NativeControlState>,
    provider_files: Option<Mutex<Vec<File>>>,
}

impl NativeChild {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn handle(&self) -> BorrowedHandle<'_> {
        self.process.as_handle()
    }

    fn completed_status(&self) -> io::Result<u32> {
        if let Some(status) = self.exit_status.get() {
            return Ok(*status);
        }
        let mut status = 0;
        if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut status) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // This is reached only after the process handle signals: 259 is a real
        // exit DWORD here, not STILL_ACTIVE and not a reason to look up a PID.
        // Retained process/status capabilities may outlive the job; they must not keep a
        // completed script locked against edits until an unrelated wait-cache eviction.
        if let Some(files) = &self.provider_files {
            files.lock().unwrap_or_else(|poison| poison.into_inner()).clear();
        }
        let _ = self.exit_status.set(status);
        Ok(status)
    }

    pub(crate) fn status_if_exited(&self) -> io::Result<Option<u32>> {
        if let Some(status) = self.exit_status.get() {
            return Ok(Some(*status));
        }
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => self.completed_status().map(Some),
            WAIT_TIMEOUT => Ok(None),
            result => Err(unexpected_wait(result)),
        }
    }

    /// Request Ctrl-Break only for this retained, NEW_PROCESS_GROUP root while
    /// it belongs to our current console. GUI/detached/foreign-console children
    /// are ineligible. Retaining the process object prevents PID reuse while the
    /// group-number-based Windows API is used; no PID is reopened or killed.
    pub fn request_ctrl_break(&self) -> io::Result<NativeBreakOutcome> {
        let result = self.send_ctrl_break();
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        control.last_break = Some(match &result {
            Ok(outcome) => NativeBreakAttempt::Outcome(*outcome),
            Err(error) => NativeBreakAttempt::Failed {
                kind: error.kind(),
                win32: error.raw_os_error(),
            },
        });
        control.break_requested |= matches!(result, Ok(NativeBreakOutcome::Requested));
        result
    }

    pub fn last_ctrl_break_attempt(&self) -> Option<NativeBreakAttempt> {
        self.control
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .last_break
    }

    pub(crate) fn ctrl_break_was_requested(&self) -> bool {
        self.control
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .break_requested
    }

    fn send_ctrl_break(&self) -> io::Result<NativeBreakOutcome> {
        if let Some(raw) = self.status_if_exited()? {
            return Ok(NativeBreakOutcome::AlreadyExited(raw));
        }
        if !current_console_contains(self.pid)? {
            return Ok(NativeBreakOutcome::UnsupportedConsole);
        }
        if unsafe { GenerateConsoleCtrlEvent(1, self.pid) } == 0 {
            let error = io::Error::last_os_error();
            if let Some(raw) = self.status_if_exited()? {
                return Ok(NativeBreakOutcome::AlreadyExited(raw));
            }
            return Err(error);
        }
        Ok(NativeBreakOutcome::Requested)
    }

    /// Wait on this retained process and, optionally, one cancellation event.
    /// This neither consumes nor discards unrelated runtime completion records.
    /// If completion and cancellation are both signaled, completion wins.
    pub fn wait(&self, cancellation: Option<&NativeCancellation>) -> io::Result<u32> {
        if let Some(status) = self.exit_status.get() {
            return Ok(*status);
        }
        let result = if let Some(cancellation) = cancellation {
            let handles = [
                self.process.as_raw_handle(),
                cancellation.handle().as_raw_handle(),
            ];
            crate::console_windows::wait_for_evaluator_handles(&handles, INFINITE)?
        } else {
            crate::console_windows::wait_for_evaluator_handles(&[self.process.as_raw_handle()], INFINITE)?
        };
        if result == WAIT_OBJECT_0 {
            self.completed_status()
        } else if cancellation.is_some() && result == WAIT_OBJECT_0 + 1 {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "native process wait cancelled",
            ))
        } else {
            Err(unexpected_wait(result))
        }
    }

    pub(crate) fn drain_registration(&self) -> io::Result<()> {
        let registration = self
            .registration
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
        if let Some(mut registration) = registration {
            // The registration removes only its own queue identity and joins
            // its callbacks. No late group-owned records survive this boundary.
            drop(registration.remove_and_drain()?);
        }
        Ok(())
    }
}

/// Owns every resource between CreateProcessW and publication as NativeChild.
/// This also covers assignment failure, where the process is not in our job yet.
struct PendingChild {
    pid: u32,
    process: Option<Arc<OwnedHandle>>,
    thread: Option<OwnedHandle>,
    registration: Option<NativeWaitRegistration>,
    provider_files: Vec<File>,
}

impl PendingChild {
    fn publish(mut self) -> Arc<NativeChild> {
        // Resume has already succeeded. Closing the primary thread handle does
        // not affect the running thread or the retained process handle.
        drop(self.thread.take());
        Arc::new(NativeChild {
            pid: self.pid,
            process: self.process.take().expect("pending process owner"),
            registration: Mutex::new(self.registration.take()),
            exit_status: OnceLock::new(),
            control: Mutex::new(NativeControlState::default()),
            provider_files: (!self.provider_files.is_empty())
                .then(|| Mutex::new(mem::take(&mut self.provider_files))),
        })
    }
}

impl Drop for PendingChild {
    fn drop(&mut self) {
        if let Some(process) = &self.process {
            let _ = terminate_root(process.as_raw_handle());
        }
        if let Some(mut registration) = self.registration.take() {
            drop(registration.remove_and_drain());
        }
    }
}

fn terminate_root(process: RawHandle) -> io::Result<()> {
    match unsafe { WaitForSingleObject(process, 0) } {
        WAIT_OBJECT_0 => return Ok(()),
        WAIT_TIMEOUT => {}
        result => return Err(unexpected_wait(result)),
    }
    if unsafe { TerminateProcess(process, ROLLBACK_STATUS) } == 0 {
        let error = io::Error::last_os_error();
        // TerminateProcess may race a normal exit and report access denied.
        if unsafe { WaitForSingleObject(process, 0) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        return Err(error);
    }
    match unsafe { WaitForSingleObject(process, INFINITE) } {
        WAIT_OBJECT_0 => Ok(()),
        result => Err(unexpected_wait(result)),
    }
}

/// A completely prepared, child-free native launch.
///
/// Owns image pinning, inheritable endpoint duplicates, UTF-16 arguments and
/// environment, and the explicit inheritance attribute list. Moving the plan
/// does not move the heap-backed HANDLE array referenced by its attributes.
pub struct NativeLaunchPlan {
    // Drop the opaque list before its referenced HANDLE array and owners.
    attributes: Option<AttributeList>,
    child_io: ChildIo,
    image: PinnedImage,
    cwd: Vec<u16>,
    command_line: Vec<u16>,
    environment: Vec<u16>,
    provider_files: Vec<File>,
}

/// A rollback boundary for one native pipeline/evaluation launch group.
///
/// Successful finish requires every root to have exited and releases descendants.
/// Unaccepted destruction rolls back only this group's members. Accepted work
/// releases monitoring instead, so a cooperative failure never silently escalates.
pub struct NativeLaunchGroup {
    runtime: Arc<NativeRuntime>,
    children: Vec<Arc<NativeChild>>,
    job: Arc<OwnedHandle>,
    accounting: TrackedJob,
    completion_port: OwnedHandle,
    finished: bool,
    terminated: bool,
    accepted: bool,
    substitution_leases: Vec<Arc<crate::native_adapters::SubstitutionLease>>,
    substitution_registrations: Vec<usize>,
    preparation_started: Cell<bool>,
}

impl NativeLaunchGroup {
    pub fn new(runtime: Arc<NativeRuntime>) -> io::Result<Self> {
        initialize_native_image_identity()?;
        let raw_job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw_job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Arc::new(unsafe { OwnedHandle::from_raw_handle(raw_job) });
        set_job_kill_on_close(&job, true)?;
        let raw_port =
            unsafe { CreateIoCompletionPort((-1isize) as RawHandle, ptr::null_mut(), 0, 1) };
        if raw_port.is_null() {
            return Err(io::Error::last_os_error());
        }
        let completion_port = unsafe { OwnedHandle::from_raw_handle(raw_port) };
        let association = JobCompletionPort {
            // This is a private completion port, not a pointer to callback state.
            key: 1usize as *mut c_void,
            port: completion_port.as_raw_handle(),
        };
        if unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JOB_OBJECT_ASSOCIATE_COMPLETION_PORT_INFORMATION,
                (&association as *const JobCompletionPort).cast(),
                size_of::<JobCompletionPort>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let accounting = native_accounting::register_job(Arc::clone(&job))?;
        Ok(Self {
            runtime,
            children: Vec::new(),
            job,
            accounting,
            completion_port,
            finished: false,
            terminated: false,
            accepted: false,
            preparation_started: Cell::new(false),
            substitution_leases: Vec::new(),
            substitution_registrations: Vec::new(),
        })
    }

    /// Attach command-substitution resources before preparing this caller's
    /// native stages. Acknowledgement is installed while the first root is
    /// suspended, before any consumer can run or the parent can release it.
    pub(crate) fn attach_substitutions(&mut self, caller: crate::proc::InternalJobId) {
        self.substitution_leases = crate::native_adapters::leases_for_caller(caller);
    }

    pub(crate) fn attach_substitution_inputs(&mut self, argv: &[OsString], environment: &[(OsString, OsString)]) {
        for lease in crate::native_adapters::leases_for_launch(argv, environment) {
            if !self.substitution_leases.iter().any(|owned| Arc::ptr_eq(owned, &lease)) {
                self.substitution_leases.push(lease);
            }
        }
    }

    pub(crate) fn attach_substitution_io(&mut self, io_chain: &IoChain, cwd: &Path) -> io::Result<()> {
        for lease in crate::native_adapters::leases_for_io(io_chain, cwd)? {
            if !self.substitution_leases.iter().any(|owned| Arc::ptr_eq(owned, &lease)) {
                self.substitution_leases.push(lease);
            }
        }
        Ok(())
    }

    /// Configure this isolated Job before preparation or process creation.
    /// The policy never applies to fish, the host Job, or already launched groups.
    pub fn apply_limits(&mut self, config: &NativeLimits) -> io::Result<()> {
        if self.preparation_started.get() || !self.children.is_empty() || self.accepted || self.finished {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native limits must be applied before launch preparation",
            ));
        }
        let mut limits = config.job_limits()?;
        // Preserve rollback ownership; accept/finish clear only this flag.
        limits.basic.flags |= JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        write_job_limits(&self.job, &limits)
    }

    /// Launch argv (including argv[0]) with a private native environment and the
    /// final descriptor-map snapshot. `fd_bridge` never overrides image identity.
    pub fn spawn(
        &mut self,
        executable: &Path,
        argv: &[OsString],
        cwd: &Path,
        environment: &[(OsString, OsString)],
        io_chain: &IoChain,
        fd_bridge: bool,
    ) -> io::Result<Arc<NativeChild>> {
        let plan = self.prepare(executable, argv, cwd, environment, io_chain, fd_bridge)?;
        self.spawn_prepared(plan)
    }

    /// Keep the provider's script/interpreter pins through execution, while reusing the one
    /// native image, endpoint, handle-allowlist and suspended transaction implementation.
    pub fn prepare_provider(
        &self,
        invocation: crate::path::NativeProviderInvocation,
        io_chain: &IoChain,
    ) -> io::Result<NativeLaunchPlan> {
        let mut plan = self.prepare(
            &invocation.executable,
            &invocation.argv,
            &invocation.cwd,
            &invocation.environment,
            io_chain,
            true,
        )?;
        if let Some(script) = &invocation.script {
            if crate::wutil::native_file_identity(&crate::path::native_path_text(script)?, true)?
                != crate::wutil::native_identity_for_file(&invocation.pins[0])?
            {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied,
                    "provider script identity changed before native preparation"));
            }
        }
        if let Some(interpreter) = invocation.pins.last() {
            if crate::wutil::native_identity_for_file(interpreter)?
                != crate::wutil::native_identity_for_file(&plan.image.file)?
            {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied,
                    "provider interpreter identity changed after classification"));
            }
        }
        if let Some(command_line) = invocation.command_line {
            plan.command_line = command_line;
        }
        plan.provider_files = invocation.pins;
        Ok(plan)
    }

    /// Complete every fallible image/I/O/argument/environment/bridge preparation
    /// without creating a child. The scheduler can prepare all stages before
    /// allowing even the first stage to execute.
    pub fn prepare(
        &self,
        executable: &Path,
        argv: &[OsString],
        cwd: &Path,
        environment: &[(OsString, OsString)],
        io_chain: &IoChain,
        fd_bridge: bool,
    ) -> io::Result<NativeLaunchPlan> {
        self.preparation_started.set(true);
        if self.terminated {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "native launch group has been terminated",
            ));
        }
        let image = PinnedImage::open(executable)?;
        let cwd = native_path(cwd)?;
        let command_line = command_line(argv)?;
        let (child_io, needs_bridge) = ChildIo::prepare(io_chain, fd_bridge)?;
        // Never treat a caller's bridge hint, matching basename or user-provided
        // environment marker as proof that an arbitrary executable cooperates.
        if needs_bridge
            && crate::wutil::native_identity_for_file(&image.file)?
                != native_image_identity()?.identity
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "private native descriptor bridge requires the actual current fish executable",
            ));
        }
        let mut environment = native_fd_bridge::sanitize_environment(environment);
        if needs_bridge {
            environment.push((
                OsString::from(PRIVATE_FD_ENV),
                native_fd_bridge::encode(&child_io.bridge)?,
            ));
        }
        let environment = environment_block(&environment)?;
        let attributes = if child_io.allowlist.is_empty() {
            None
        } else {
            Some(AttributeList::new(&child_io.allowlist)?)
        };
        Ok(NativeLaunchPlan {
            attributes,
            child_io,
            image,
            cwd,
            command_line,
            environment,
            provider_files: Vec::new(),
        })
    }

    /// Hidden resource owners must be the retained startup fish image, not an
    /// executable that happens to occupy current_exe's old pathname.
    pub(crate) fn prepare_resource_keeper(
        &self, cwd: &Path, environment: &[(OsString, OsString)], io_chain: &IoChain,
    ) -> io::Result<NativeLaunchPlan> {
        let image = crate::wutil::native_current_exe_path()?;
        let argv = [image.as_os_str().to_owned(), crate::native_adapters::RESOURCE_LEASE_OPTION.into()];
        let plan = self.prepare(image, &argv, cwd, environment, io_chain, false)?;
        if crate::wutil::native_identity_for_file(&plan.image.file)? != native_image_identity()?.identity {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied,
                "resource keeper requires the actual retained current fish executable"));
        }
        Ok(plan)
    }

    /// A deliberately cooperating CPython bootstrap, not a general CRT fd
    /// table. Only these two synchronous completion output handles are added.
    pub(crate) fn prepare_argcomplete(
        &self, executable: &Path, argv: &[OsString], cwd: &Path,
        environment: &[(OsString, OsString)], io_chain: &IoChain,
        completion: BorrowedHandle<'_>, diagnostics: BorrowedHandle<'_>,
    ) -> io::Result<NativeLaunchPlan> {
        let mut plan = self.prepare(executable, argv, cwd, environment, io_chain, false)?;
        let mut handles = [0usize; 2];
        for (slot, source) in handles.iter_mut().zip([completion, diagnostics]) {
            let mut raw = ptr::null_mut();
            if unsafe { DuplicateHandle(GetCurrentProcess(), source.as_raw_handle(),
                GetCurrentProcess(), &mut raw, 0, 1, DUPLICATE_SAME_ACCESS) } == 0 {
                return Err(io::Error::last_os_error());
            }
            plan.child_io.handles.push(unsafe { OwnedHandle::from_raw_handle(raw) });
            plan.child_io.allowlist.push(raw);
            *slot = raw as usize;
        }
        // Insert one reserved ASCII record into the already validated/sorted
        // UTF-16 block; do not clone/re-encode the caller's entire environment.
        let record = format!("{}=v1;{};{}\0", native_fd_bridge::ARGCOMPLETE_FD_ENV, handles[0], handles[1]);
        let mut name = [0u16; native_fd_bridge::ARGCOMPLETE_FD_ENV.len()];
        for (slot, byte) in name.iter_mut().zip(native_fd_bridge::ARGCOMPLETE_FD_ENV.bytes()) {
            *slot = u16::from(byte);
        }
        let mut insertion = 0;
        while plan.environment[insertion] != 0 {
            let end = insertion + plan.environment[insertion..].iter().position(|unit| *unit == 0).unwrap();
            let equals = insertion + plan.environment[insertion..end].iter().position(|unit| *unit == 61).unwrap();
            if environment_name_cmp(&name, &plan.environment[insertion..equals]) == Ordering::Less { break; }
            insertion = end + 1;
        }
        let replace = if plan.environment == [0, 0] { 1 } else { 0 };
        plan.environment.splice(insertion..insertion + replace, record.bytes().map(u16::from));
        plan.attributes = Some(AttributeList::new(&plan.child_io.allowlist)?);
        Ok(plan)
    }

    /// Consume one fully owned plan. Only native process creation and the
    /// suspended assign/register/resume transaction remain at this boundary.
    pub fn spawn_prepared(&mut self, mut plan: NativeLaunchPlan) -> io::Result<Arc<NativeChild>> {
        if self.terminated {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "native launch group has been terminated",
            ));
        }
        // All fields are Win32 integers/pointers, for which zero is valid.
        let mut startup: StartupInfoExW = unsafe { mem::zeroed() };
        startup.startup.cb = size_of::<StartupInfoExW>() as u32;
        startup.startup.flags = STARTF_USESTDHANDLES;
        [
            startup.startup.stdin,
            startup.startup.stdout,
            startup.startup.stderr,
        ] = plan.child_io.standard;
        startup.attributes = plan
            .attributes
            .as_mut()
            .map_or(ptr::null_mut(), AttributeList::as_mut_ptr);
        crate::io::flush_standard_streams()?;
        let mut information: ProcessInformation = unsafe { mem::zeroed() };
        if unsafe {
            CreateProcessW(
                plan.image.path.as_ptr(),
                plan.command_line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                i32::from(!plan.child_io.allowlist.is_empty()),
                CREATE_SUSPENDED
                    | CREATE_NEW_PROCESS_GROUP
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                plan.environment.as_ptr().cast(),
                plan.cwd.as_ptr(),
                &startup.startup,
                &mut information,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut pending = PendingChild {
            pid: information.pid,
            process: Some(Arc::new(unsafe {
                OwnedHandle::from_raw_handle(information.process)
            })),
            thread: Some(unsafe { OwnedHandle::from_raw_handle(information.thread) }),
            registration: None,
            provider_files: mem::take(&mut plan.provider_files),
        };
        if unsafe { AssignProcessToJobObject(self.job.as_raw_handle(), information.process) } == 0 {
            return Err(io::Error::last_os_error());
        }
        pending.registration = Some(self.runtime.register_process(
            information.pid,
            Arc::clone(pending.process.as_ref().expect("pending process owner")),
        )?);
        // Leases belong to the suspended provider too: accepted/disowned work may outlive
        // orderly or abrupt fish exit. Zero granted rights retain file-object sharing
        // reservations; bInheritHandle=false prevents propagation to grandchildren.
        // Any failure remains inside PendingChild's terminate/wait rollback boundary.
        for file in &pending.provider_files {
            let mut child_lease = ptr::null_mut();
            if unsafe {
                DuplicateHandle(GetCurrentProcess(), file.as_raw_handle(),
                    information.process, &mut child_lease, 0, 0, 0)
            } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        for lease in self.substitution_leases.iter().skip(self.substitution_registrations.len()) {
            self.substitution_registrations.push(lease.register_job(self.job.as_handle(),
                pending.process.as_ref().expect("pending process owner").as_handle())?);
        }
        if unsafe { ResumeThread(information.thread) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        let child = pending.publish();
        self.children.push(Arc::clone(&child));
        Ok(child)
    }

    /// Wait only after the scheduler has started all concurrent stages. This
    /// scales beyond 64 roots because each wait uses at most process + cancel.
    pub fn wait(&mut self, cancellation: Option<&NativeCancellation>) -> io::Result<Vec<u32>> {
        self.children
            .iter()
            .map(|child| child.wait(cancellation))
            .collect()
    }

    /// Release a normally completed group's descendants, rather than killing
    /// them merely because the shell's root processes have exited.
    pub fn finish(mut self) -> io::Result<()> {
        for child in &self.children {
            if child.status_if_exited()?.is_none() {
                return Err(invalid(
                    "cannot finish a native launch group with running root children",
                ));
            }
        }
        for child in &self.children {
            child.drain_registration()?;
        }
        set_job_kill_on_close(&self.job, false)?;
        self.accounting.finish()?;
        self.finished = true;
        Ok(())
    }

    /// Commit launch ownership without promising POSIX terminal job control.
    /// Accepted work survives normal shell exit, including after disown. Until
    /// this succeeds, every failure still rolls back the suspended/started roots.
    pub(crate) fn accept(&mut self) -> io::Result<()> {
        if self.accepted { return Ok(()); }
        set_job_kill_on_close(&self.job, false)?;
        self.accepted = true;
        // The native flag is the atomic acceptance record. A keeper whose A
        // packet is interrupted can reconcile it without deleting a live
        // accepted consumer's files or retaining an unaccepted suspended root.
        for (lease, cookie) in self.substitution_leases.iter().zip(&self.substitution_registrations) {
            lease.accept_job(*cookie)?;
        }
        Ok(())
    }

    /// Stop monitoring accepted work without terminating it. Registrations are
    /// drained before handles are released; accounting ends at this explicit
    /// ownership cutoff rather than inventing future descendant CPU usage.
    pub(crate) fn release_running(&mut self) -> io::Result<()> {
        self.accept()?;
        let mut error = None;
        for child in &self.children {
            if let Err(failure) = child.drain_registration() {
                error.get_or_insert(failure);
            }
        }
        if let Err(failure) = self.accounting.finish() {
            error.get_or_insert(failure);
        }
        self.finished = true;
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Terminate the owned job (including descendants), reap root handles, and
    /// remove/drain this group's registrations. Other runtime sources are intact.
    pub fn terminate_and_wait(&mut self) -> io::Result<()> {
        self.terminated = true;
        let mut error = None;
        let terminated =
            if unsafe { TerminateJobObject(self.job.as_raw_handle(), ROLLBACK_STATUS) } == 0 {
                error = Some(io::Error::last_os_error());
                false
            } else {
                true
            };
        for child in &self.children {
            if !terminated {
                if let Err(failure) = terminate_root(child.process.as_raw_handle()) {
                    error.get_or_insert(failure);
                    // A failed native termination must not turn cleanup into an
                    // unbounded wait on a still-running, unkillable root.
                    continue;
                }
            }
            if let Err(failure) = child.wait(None) {
                error.get_or_insert(failure);
            }
        }
        if terminated {
            if let Err(failure) = self.wait_for_empty_job() {
                error.get_or_insert(failure);
            }
        }
        for child in &self.children {
            if let Err(failure) = child.drain_registration() {
                error.get_or_insert(failure);
            }
        }
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn wait_for_empty_job(&self) -> io::Result<()> {
        loop {
            let mut accounting = JobAccounting::default();
            if unsafe {
                QueryInformationJobObject(
                    self.job.as_raw_handle(),
                    JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION,
                    (&mut accounting as *mut JobAccounting).cast(),
                    size_of::<JobAccounting>() as u32,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if accounting.active_processes == 0 {
                return Ok(());
            }
            let mut message = 0;
            let mut key = 0;
            let mut process_id = ptr::null_mut();
            // Ordinary job notifications are not guaranteed delivery. A bounded
            // wait recovers via authoritative accounting even if a notification
            // was lost; this is not a fixed-array process collector or PID poll.
            if unsafe {
                GetQueuedCompletionStatus(
                    self.completion_port.as_raw_handle(),
                    &mut message,
                    &mut key,
                    &mut process_id,
                    50,
                )
            } == 0
            {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(WAIT_TIMEOUT as i32) {
                    return Err(error);
                }
            }
        }
    }
}

fn set_job_kill_on_close(job: &OwnedHandle, enabled: bool) -> io::Result<()> {
    let mut limits = JobExtendedLimits::default();
    if unsafe {
        QueryInformationJobObject(
            job.as_raw_handle(),
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
            (&mut limits as *mut JobExtendedLimits).cast(),
            size_of::<JobExtendedLimits>() as u32,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if enabled {
        limits.basic.flags |= JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    } else {
        limits.basic.flags &= !JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    }
    write_job_limits(job, &limits)
}

fn write_job_limits(job: &OwnedHandle, limits: &JobExtendedLimits) -> io::Result<()> {
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
            (limits as *const JobExtendedLimits).cast(),
            size_of::<JobExtendedLimits>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

impl NativeLimits {
    fn job_limits(&self) -> io::Result<JobExtendedLimits> {
        fn positive(value: u64) -> io::Result<u64> {
            if value == 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "native limits must be positive; use not-configured to reset"));
            }
            Ok(value)
        }
        fn memory(kib: u64) -> io::Result<usize> {
            positive(kib)?.checked_mul(1024)
                .and_then(|bytes| usize::try_from(bytes).ok())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "native memory limit overflows SIZE_T"))
        }
        let mut limits = JobExtendedLimits::default();
        if let Some(kib) = self.process_memory_kib {
            limits.process_memory_limit = memory(kib)?;
            limits.basic.flags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        }
        if let Some(kib) = self.job_memory_kib {
            limits.job_memory_limit = memory(kib)?;
            limits.basic.flags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
        }
        if let Some(seconds) = self.process_cpu_seconds {
            limits.basic.process_user_time = positive(seconds)?.checked_mul(10_000_000)
                .and_then(|ticks| i64::try_from(ticks).ok())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "native USER CPU limit overflows signed 100-nanosecond units"))?;
            limits.basic.flags |= JOB_OBJECT_LIMIT_PROCESS_TIME;
        }
        if let Some(count) = self.active_processes {
            limits.basic.active_process_limit = u32::try_from(positive(count)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "native active process limit exceeds DWORD"))?;
            limits.basic.flags |= JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        }
        Ok(limits)
    }

    /// Ask the kernel to accept a policy on an empty private Job before saving it.
    /// No process is assigned and no existing/enclosing Job is queried or changed.
    pub fn validate(&self) -> io::Result<()> {
        let limits = self.job_limits()?;
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        write_job_limits(&job, &limits)
    }
}

impl Drop for NativeLaunchGroup {
    fn drop(&mut self) {
        if !self.finished {
            if self.accepted {
                // A monitoring error is not permission to kill accepted work.
                let _ = self.release_running();
            } else {
                // Rollback never targets an enclosing/host job or a reopened PID.
                let _ = self.terminate_and_wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::ffi::OsStringExt;

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn CommandLineToArgvW(command: *const u16, count: *mut i32) -> *mut *mut u16;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    fn assert_command_line_roundtrip(argv: &[OsString]) -> Vec<u16> {
        let line = command_line(argv).unwrap();
        let mut count = 0;
        let decoded = unsafe { CommandLineToArgvW(line.as_ptr(), &mut count) };
        assert!(!decoded.is_null(), "native argv decoder: {}", io::Error::last_os_error());
        let mut actual = Vec::new();
        for index in 0..count as usize {
            let argument = unsafe { *decoded.add(index) };
            let mut length = 0;
            while unsafe { *argument.add(length) } != 0 {
                length += 1;
            }
            actual.push(unsafe { std::slice::from_raw_parts(argument, length) }.to_vec());
        }
        assert!(unsafe { LocalFree(decoded.cast()) }.is_null());
        let expected: Vec<Vec<u16>> = argv.iter().map(|argument| argument.encode_wide().collect()).collect();
        assert_eq!(actual, expected, "real Windows decoder must preserve native argv");
        line
    }

    #[test]
    fn command_line_roundtrips_native_decoder() {
        let mut arguments: Vec<OsString> = [
            "", "+r", "-r", "/s", "ordinary", "two words", "\t", "tab\tword",
            "\"", "\"\"", "inside\"quote", "\\", "\\\\", "space and slash\\",
            "雪🐟", "雪 🐟\\", "%PATH%", "!value!", "^&|<>()", ";$`*?[]{}",
            "line\nbreak", "line\r\nbreak", "nonbreaking\u{a0}space",
        ].into_iter().map(OsString::from).collect();
        // Exercise both parity cases before literal quotes and doubled trailing
        // backslashes before a required syntactic quote, without a model decoder.
        for count in 0..=8 {
            let slashes = "\\".repeat(count);
            for argument in [
                format!("{slashes}\"tail"),
                format!("head{slashes}\""),
                format!(" {slashes}"),
                format!("\t{slashes}"),
                format!("{slashes}"),
            ] {
                arguments.push(argument.into());
            }
        }
        // The process boundary carries UTF-16, not only valid Unicode scalars.
        arguments.push(OsString::from_wide(&[0xd800, 32, 0xdc00, 92]));
        for first in ["native.exe", r"C:\Program Files\雪🐟\native.exe", r"C:\trailing\", ""] {
            let mut argv = vec![OsString::from(first)];
            argv.extend(arguments.iter().cloned());
            assert_command_line_roundtrip(&argv);
        }
    }

    #[test]
    fn command_line_rejects_unrepresentable_arguments() {
        let invalid_arguments = [
            Vec::new(),
            vec![OsString::from("bad\"image")],
            vec![OsString::from_wide(&[b'p' as u16, 0])],
            vec![OsString::from("p"), OsString::from_wide(&[0])],
            vec![OsString::from("p"), OsString::from_wide(&[32, 0, 34])],
        ];
        for argv in invalid_arguments {
            assert_eq!(command_line(&argv).unwrap_err().kind(), io::ErrorKind::InvalidInput);
        }
    }

    #[test]
    fn command_line_enforces_encoded_utf16_limit() {
        // The 32767-unit CreateProcessW limit includes the terminating NUL.
        let mut argv = vec![OsString::from("p"), OsString::from("🐟".repeat(16_381))];
        assert_eq!(assert_command_line_roundtrip(&argv).len(), 32_767);
        argv[1].push("x");
        assert_eq!(command_line(&argv).unwrap_err().kind(), io::ErrorKind::InvalidInput);

        // Required quoting expands trailing backslashes; measure the encoded
        // command rather than the unescaped argument's character count.
        argv[1] = format!(" x{}", "\\".repeat(16_379)).into();
        assert_eq!(assert_command_line_roundtrip(&argv).len(), 32_767);
        argv[1].push("\\");
        assert_eq!(command_line(&argv).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    const QUOTA_READY: &str = "FISH_NATIVE_ASSIGNMENT_QUOTA_READY";
    const QUOTA_RELEASE: &str = "FISH_NATIVE_ASSIGNMENT_QUOTA_RELEASE";
    const QUOTA_STATUS: &str = "FISH_NATIVE_ASSIGNMENT_QUOTA_STATUS";
    const JOB_OBJECT_LIMIT_ACTIVE_PROCESS: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateEventW(
            security: *const c_void,
            manual: i32,
            initial: i32,
            name: *const u16,
        ) -> RawHandle;
        fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> RawHandle;
        fn SetEvent(event: RawHandle) -> i32;
        fn ExitProcess(status: u32) -> !;
    }

    fn own(raw: RawHandle) -> OwnedHandle {
        assert!(
            !raw.is_null(),
            "native fixture handle: {}",
            io::Error::last_os_error()
        );
        unsafe { OwnedHandle::from_raw_handle(raw) }
    }

    struct NamedEvent {
        name: OsString,
        handle: OwnedHandle,
    }

    impl NamedEvent {
        fn new(label: &str) -> Self {
            let name = OsString::from(format!(
                "Local\\fish-native-assignment-quota-{}-{label}",
                std::process::id()
            ));
            let handle =
                own(unsafe { CreateEventW(ptr::null(), 1, 0, wide(&name).unwrap().as_ptr()) });
            Self { name, handle }
        }

        fn wait(&self) {
            assert_eq!(
                unsafe { WaitForSingleObject(self.handle.as_raw_handle(), INFINITE) },
                WAIT_OBJECT_0
            );
        }

        fn signal(&self) {
            assert_ne!(unsafe { SetEvent(self.handle.as_raw_handle()) }, 0);
        }
    }

    fn job_accounting(job: &OwnedHandle) -> JobAccounting {
        let mut accounting = JobAccounting::default();
        assert_ne!(
            unsafe {
                QueryInformationJobObject(
                    job.as_raw_handle(),
                    JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION,
                    (&mut accounting as *mut JobAccounting).cast(),
                    size_of::<JobAccounting>() as u32,
                    ptr::null_mut(),
                )
            },
            0,
            "private job accounting: {}",
            io::Error::last_os_error()
        );
        accounting
    }

    #[test]
    fn assignment_quota_rollback_preserves_host() {
        // The same exact libtest case is also the native sentinel image. Check
        // this mode before isolation so it neither recurses nor needs shell argv.
        if let Some(ready_name) = std::env::var_os(QUOTA_READY) {
            let ready =
                own(unsafe { OpenEventW(0x0000_0002, 0, wide(&ready_name).unwrap().as_ptr()) });
            let release = own(unsafe {
                OpenEventW(
                    0x0010_0000,
                    0,
                    wide(&std::env::var_os(QUOTA_RELEASE).unwrap())
                        .unwrap()
                        .as_ptr(),
                )
            });
            let status = std::env::var(QUOTA_STATUS).unwrap().parse::<u32>().unwrap();
            assert_ne!(unsafe { SetEvent(ready.as_raw_handle()) }, 0);
            assert_eq!(
                unsafe { WaitForSingleObject(release.as_raw_handle(), INFINITE) },
                WAIT_OBJECT_0
            );
            unsafe { ExitProcess(status) }
        }

        const CASE: &str = concat!(module_path!(), "::assignment_quota_rollback_preserves_host");
        if !crate::threads::isolated_test(CASE) {
            return;
        }

        // Only this isolated owner joins the outer job. Never terminate it or
        // enable KILL_ON_JOB_CLOSE: it contains the test root, not the CI host.
        let outer = own(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) });
        assert_ne!(
            unsafe { AssignProcessToJobObject(outer.as_raw_handle(), GetCurrentProcess()) },
            0,
            "private outer job membership: {}",
            io::Error::last_os_error()
        );
        assert_eq!(job_accounting(&outer).active_processes, 1);

        let runtime = NativeRuntime::new().unwrap();
        let exe = crate::wutil::native_current_exe_path().unwrap();
        let cwd = std::env::current_dir().unwrap();
        let case = CASE
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap();
        let argv = [
            exe.as_os_str().to_owned(),
            "--exact".into(),
            case.into(),
            "--nocapture".into(),
        ];
        // The sentinel needs only its explicit protocol and SystemRoot. An
        // inherited Windows block may contain drive pseudo-variables (`=D:`),
        // which are not valid names in the launcher's explicit environment API.
        let system_root = std::env::var_os("SystemRoot").expect("native test requires SystemRoot");
        let sentinel_environment = |ready: &NamedEvent, release: &NamedEvent, status: u32| {
            [
                ("SystemRoot".into(), system_root.clone()),
                (QUOTA_READY.into(), ready.name.clone()),
                (QUOTA_RELEASE.into(), release.name.clone()),
                (QUOTA_STATUS.into(), status.to_string().into()),
            ]
        };
        let host_ready = NamedEvent::new("host-ready");
        let host_release = NamedEvent::new("host-release");
        let target_ready = NamedEvent::new("target-ready");
        let target_release = NamedEvent::new("target-release");
        let mut host_group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
        let mut target_group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
        target_group.apply_limits(&NativeLimits {
            active_processes: Some(1),
            ..NativeLimits::default()
        }).unwrap();
        let host = host_group
            .spawn(
                &exe,
                &argv,
                &cwd,
                &sentinel_environment(&host_ready, &host_release, 37),
                &IoChain::new(),
                false,
            )
            .unwrap();
        host_ready.wait();
        let target_environment = sentinel_environment(&target_ready, &target_release, 41);
        let target = target_group
            .spawn(
                &exe,
                &argv,
                &cwd,
                &target_environment,
                &IoChain::new(),
                false,
            )
            .unwrap();
        target_ready.wait();
        assert_eq!(job_accounting(&outer).active_processes, 3);
        assert_eq!(job_accounting(&outer).total_processes, 3);
        assert_eq!(job_accounting(&target_group.job).active_processes, 1);

        // The owner is outside the target job, so CreateProcessW can still
        // create a suspended root in the outer job. Only the subsequent real
        // AssignProcessToJobObject hits the target's now-full active quota.
        let plan = target_group
            .prepare(
                &exe,
                &argv,
                &cwd,
                &target_environment,
                &IoChain::new(),
                false,
            )
            .unwrap();
        let error = match target_group.spawn_prepared(plan) {
            Err(error) => error,
            Ok(_) => panic!("full target job accepted another native root"),
        };
        assert!(
            error.raw_os_error().is_some(),
            "expected native assignment failure: {error}"
        );
        assert_eq!(target_group.children.len(), 1, "failed root was published");
        assert_eq!(job_accounting(&target_group.job).active_processes, 1);
        assert_eq!(target.status_if_exited().unwrap(), None);
        let accounting = job_accounting(&outer);
        assert_eq!(
            accounting.active_processes, 3,
            "rejected suspended root survived"
        );
        assert!(
            accounting.total_processes >= 4,
            "rejection did not reach CreateProcessW"
        );
        assert_eq!(
            unsafe { WaitForSingleObject(host.handle().as_raw_handle(), 0) },
            WAIT_TIMEOUT
        );

        // Windows itself terminates a process rejected by an active-process
        // quota (AssignProcessToJobObject's documented contract). This proves
        // PendingChild reaps that failure, not that it issued TerminateProcess.
        // Drop must then roll back only the target's still-live first root.
        drop(target_group);
        assert_eq!(target.wait(None).unwrap(), ROLLBACK_STATUS);
        assert_eq!(job_accounting(&outer).active_processes, 2);
        assert_eq!(
            unsafe { WaitForSingleObject(host.handle().as_raw_handle(), 0) },
            WAIT_TIMEOUT,
            "target rollback terminated the unrelated retained host sentinel"
        );
        host_release.signal();
        assert_eq!(host.wait(None).unwrap(), 37);
        host_group.finish().unwrap();
        assert_eq!(job_accounting(&outer).active_processes, 1);
        assert!(
            runtime.drain().is_empty(),
            "rollback left owned completion records"
        );
    }
    #[test]
    fn configured_limits_preserve_units_rollback_and_preparation_boundary() {
        const CASE: &str = concat!(module_path!(), "::configured_limits_preserve_units_rollback_and_preparation_boundary");
        if !crate::threads::isolated_test(CASE) {
            return;
        }
        let outer = own(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) });
        assert_ne!(unsafe { AssignProcessToJobObject(outer.as_raw_handle(), GetCurrentProcess()) }, 0);
        assert_eq!(job_accounting(&outer).active_processes, 1);
        let policy = NativeLimits {
            process_memory_kib: Some(65_536),
            job_memory_kib: Some(131_072),
            process_cpu_seconds: Some(2),
            active_processes: Some(3),
        };
        policy.validate().unwrap();
        let mut group = NativeLaunchGroup::new(NativeRuntime::new().unwrap()).unwrap();
        group.apply_limits(&policy).unwrap();
        assert_eq!(job_accounting(&group.job).total_processes, 0);
        assert_eq!(job_accounting(&outer).active_processes, 1);
        // The root is not subject to the configured 64 MiB child-process cap.
        let root_memory = vec![0x8au8; 80 * 1024 * 1024];
        std::hint::black_box(&root_memory);
        assert_eq!(root_memory[79 * 1024 * 1024], 0x8a);
        drop(root_memory);
        fn query(group: &NativeLaunchGroup) -> JobExtendedLimits {
            let mut limits = JobExtendedLimits::default();
            assert_ne!(unsafe {
                QueryInformationJobObject(group.job.as_raw_handle(),
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    (&mut limits as *mut JobExtendedLimits).cast(),
                    size_of::<JobExtendedLimits>() as u32, ptr::null_mut())
            }, 0);
            limits
        }
        let expected_flags = JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_JOB_MEMORY
            | JOB_OBJECT_LIMIT_PROCESS_TIME | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        for enabled in [true, false, true] {
            set_job_kill_on_close(&group.job, enabled).unwrap();
            let actual = query(&group);
            assert_eq!(actual.process_memory_limit, 65_536 * 1024);
            assert_eq!(actual.job_memory_limit, 131_072 * 1024);
            assert_eq!(actual.basic.process_user_time, 20_000_000);
            assert_eq!(actual.basic.active_process_limit, 3);
            assert_eq!(actual.basic.flags,
                expected_flags | if enabled { JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE } else { 0 });
        }
        for invalid in [
            NativeLimits { process_memory_kib: Some(0), ..policy },
            NativeLimits { process_memory_kib: Some(u64::MAX), ..policy },
            NativeLimits { job_memory_kib: Some(u64::MAX), ..policy },
            NativeLimits { process_cpu_seconds: Some(u64::MAX), ..policy },
            NativeLimits { active_processes: Some(u64::from(u32::MAX) + 1), ..policy },
        ] {
            assert_eq!(invalid.validate().unwrap_err().kind(), io::ErrorKind::InvalidInput);
            assert!(group.apply_limits(&invalid).is_err());
            assert_eq!(query(&group).basic.flags, expected_flags | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE);
            assert_eq!(query(&group).process_memory_limit, 65_536 * 1024);
        }
        let missing = std::env::temp_dir().join(format!("fish-n8-absent-{}.exe", std::process::id()));
        assert!(group.prepare(&missing, &[], &std::env::temp_dir(), &[], &IoChain::new(), false).is_err());
        assert_eq!(group.apply_limits(&NativeLimits::default()).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
}
