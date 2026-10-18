//! Native process launch and lifetime ownership for the Windows execution backend.
//!
//! Only explicitly duplicated child endpoints are inherited. A root cannot run
//! until its job membership and retained runtime registration have been installed.
//! The group, not a PID lookup or a cached language status, owns rollback.

use crate::io::IoChain;
use crate::native_fd_bridge::{self, BridgeHandle, PRIVATE_FD_ENV};
use crate::threads::{NativeCancellation, NativeRuntime, NativeWaitRegistration};
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
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const DUPLICATE_SAME_ACCESS: u32 = 2;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
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
    fn WaitForMultipleObjects(
        count: u32,
        handles: *const RawHandle,
        wait_all: i32,
        milliseconds: u32,
    ) -> u32;
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
    wide(absolute.as_os_str())
}

fn command_line(argv: &[OsString]) -> io::Result<Vec<u16>> {
    let Some(first) = argv.first() else {
        return Err(invalid("native argv must include argv[0]"));
    };
    let mut line = Vec::new();
    line.push(34);
    // The CRT parses argv[0] separately: backslashes are literal and a quote
    // cannot be represented in that field. Do not silently corrupt such argv[0].
    for unit in first.encode_wide() {
        if unit == 0 || unit == 34 {
            return Err(invalid("NUL or quote in native argv[0]"));
        }
        line.push(unit);
    }
    line.push(34);
    for argument in &argv[1..] {
        line.extend_from_slice(&[32, 34]);
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
        let _ = self.exit_status.set(status);
        Ok(status)
    }

    fn status_if_exited(&self) -> io::Result<Option<u32>> {
        if let Some(status) = self.exit_status.get() {
            return Ok(Some(*status));
        }
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => self.completed_status().map(Some),
            WAIT_TIMEOUT => Ok(None),
            result => Err(unexpected_wait(result)),
        }
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
            unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) }
        } else {
            unsafe { WaitForSingleObject(self.process.as_raw_handle(), INFINITE) }
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

    fn drain_registration(&self) -> io::Result<()> {
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
}

/// A rollback boundary for one native pipeline/evaluation launch group.
///
/// Successful finish requires every root to have exited, drains registrations,
/// and releases living descendants without killing them. All other destruction
/// keeps KILL_ON_JOB_CLOSE and terminates/reaps only this group's job members.
pub struct NativeLaunchGroup {
    runtime: Arc<NativeRuntime>,
    children: Vec<Arc<NativeChild>>,
    job: OwnedHandle,
    completion_port: OwnedHandle,
    finished: bool,
    terminated: bool,
}

impl NativeLaunchGroup {
    pub fn new(runtime: Arc<NativeRuntime>) -> io::Result<Self> {
        initialize_native_image_identity()?;
        let raw_job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw_job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = unsafe { OwnedHandle::from_raw_handle(raw_job) };
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
        Ok(Self {
            runtime,
            children: Vec::new(),
            job,
            completion_port,
            finished: false,
            terminated: false,
        })
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
        })
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
                CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
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
        };
        if unsafe { AssignProcessToJobObject(self.job.as_raw_handle(), information.process) } == 0 {
            return Err(io::Error::last_os_error());
        }
        pending.registration = Some(self.runtime.register_process(
            information.pid,
            Arc::clone(pending.process.as_ref().expect("pending process owner")),
        )?);
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
        self.finished = true;
        Ok(())
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
    if enabled {
        limits.basic.flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    }
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
            (&limits as *const JobExtendedLimits).cast(),
            size_of::<JobExtendedLimits>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

impl Drop for NativeLaunchGroup {
    fn drop(&mut self) {
        if !self.finished {
            // KILL_ON_JOB_CLOSE remains armed even if a native cleanup API fails.
            // The destructor never changes another group's job or uses a PID to
            // reopen/terminate an unrelated process after handle recycling.
            let _ = self.terminate_and_wait();
        }
    }
}
