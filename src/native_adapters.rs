//! Child-specific Windows protocols. No worker in this module evaluates fish.
#[path = "native_adapters/completion.rs"]
mod completion;
pub use completion::{argcomplete, native_completion_exit};

use crate::exec::{NativeChild, NativeLaunchGroup};
use crate::io::{IoChain, NativeEndpoint, NativeIoDirection, native_synchronous_pipe};
use crate::proc::InternalJobId;
use crate::threads::NativeRuntime;
use std::collections::BTreeMap;
use std::ffi::{OsString, c_void};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::os::windows::ffi::OsStringExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

pub const RESOURCE_LEASE_OPTION: &str = "--fish-native-resource-lease";
const MAX_TEXT: usize = 131072;
const RELEASE_DEADLINE_MS: u32 = 5000;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> RawHandle;
    fn CreateEventW(security: *const c_void, manual: i32, initial: i32, name: *const u16) -> RawHandle;
    fn SetEvent(event: RawHandle) -> i32;
    fn DuplicateHandle(source_process: RawHandle, source: RawHandle, target_process: RawHandle,
        target: *mut RawHandle, access: u32, inherit: i32, options: u32) -> i32;
    fn GetFileType(handle: RawHandle) -> u32;
    fn SetHandleInformation(handle: RawHandle, mask: u32, flags: u32) -> i32;
    fn GetHandleInformation(handle: RawHandle, flags: *mut u32) -> i32;
    fn QueryInformationJobObject(job: RawHandle, class: i32, data: *mut c_void, length: u32, returned: *mut u32) -> i32;
    fn GetProcessId(process: RawHandle) -> u32;
    fn GetProcessTimes(process: RawHandle, created: *mut u64, exited: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn QueryFullProcessImageNameW(process: RawHandle, flags: u32, name: *mut u16, length: *mut u32) -> i32;
    fn IsProcessInJob(process: RawHandle, job: RawHandle, result: *mut i32) -> i32;
    fn TerminateJobObject(job: RawHandle, status: u32) -> i32;
    fn CreateWaitableTimerW(security: *const c_void, manual: i32, name: *const u16) -> RawHandle;
    fn SetWaitableTimer(timer: RawHandle, due: *const i64, period: i32,
        callback: *const c_void, context: *const c_void, resume: i32) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationProcess(process: RawHandle, class: u32, data: *mut c_void, size: u32, returned: *mut u32) -> i32;
    fn NtQueryObject(handle: RawHandle, class: u32, data: *mut c_void, size: u32, returned: *mut u32) -> i32;
}
#[repr(C)]
struct TypeName { length: u16, maximum: u16, buffer: *const u16 }
fn validate_resource_handle(raw: RawHandle, expected: &str) -> io::Result<()> {
    let mut flags = 0;
    if unsafe { GetHandleInformation(raw, &mut flags) } == 0 { return Err(io::Error::last_os_error()); }
    if flags != 0 { return Err(invalid("resource lease handles must be unprotected and noninheritable")); }
    let mut storage = [0usize; 128];
    let mut returned = 0;
    if unsafe { NtQueryObject(raw, 2, storage.as_mut_ptr().cast(), std::mem::size_of_val(&storage) as u32, &mut returned) } < 0 {
        return Err(invalid("cannot validate resource lease kernel object type"));
    }
    let start = storage.as_ptr() as usize;
    let end = start + returned as usize;
    if returned as usize > std::mem::size_of_val(&storage) || (returned as usize) < std::mem::size_of::<TypeName>() {
        return Err(invalid("invalid resource lease kernel type response"));
    }
    let name = unsafe { &*storage.as_ptr().cast::<TypeName>() };
    let pointer = name.buffer as usize;
    let length = name.length as usize;
    if length % 2 != 0 || pointer % 2 != 0 || pointer < start + std::mem::size_of::<TypeName>()
        || pointer > end || length > end - pointer || name.maximum < name.length {
        return Err(invalid("invalid resource lease kernel type name"));
    }
    let name = unsafe { std::slice::from_raw_parts(name.buffer, length / 2) };
    if !name.iter().copied().eq(expected.encode_utf16()) {
        return Err(invalid("resource lease handle has the wrong kernel object type"));
    }
    Ok(())
}

#[repr(C)]
#[derive(Default)]
struct ProcessBasic {
    status: isize, peb: usize, affinity: usize, priority: isize, pid: usize, parent: usize,
}
fn creator(process: RawHandle) -> io::Result<usize> {
    let mut data = ProcessBasic::default();
    if unsafe { NtQueryInformationProcess(process, 0, (&mut data as *mut ProcessBasic).cast(),
        std::mem::size_of::<ProcessBasic>() as u32, std::ptr::null_mut()) } < 0 {
        return Err(invalid("cannot validate resource process creator"));
    }
    Ok(data.parent)
}
fn validate_parent(parent: &OwnedHandle) -> io::Result<()> {
    let parent_id = unsafe { GetProcessId(parent.as_raw_handle()) };
    if parent_id == 0 || creator(unsafe { GetCurrentProcess() })? != parent_id as usize {
        return Err(invalid("resource parent capability is not the actual keeper creator"));
    }
    let mut image = vec![0u16; 32768];
    let mut length = image.len() as u32;
    if unsafe { QueryFullProcessImageNameW(parent.as_raw_handle(), 0, image.as_mut_ptr(), &mut length) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let image = OsString::from_wide(&image[..length as usize]);
    if image.to_str().is_none() { return Err(invalid("resource parent image path is not strict Unicode")); }
    let image = crate::wutil::native_open(Path::new(&image), 0x80, true)?;
    if !crate::exec::is_current_native_image(&image)? {
        return Err(invalid("resource keeper parent must be the actual matching native fish image"));
    }
    Ok(())
}

fn process_creation(process: RawHandle) -> io::Result<u64> {
    let (mut created, mut exited, mut kernel, mut user) = (0, 0, 0, 0);
    if unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(created)
}

fn invalid(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, message.into()) }
fn write_endpoint(endpoint: &NativeEndpoint, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        let count = endpoint.write(bytes)?;
        if count == 0 { return Err(io::ErrorKind::WriteZero.into()); }
        bytes = &bytes[count..];
    }
    Ok(())
}
fn read_endpoint(endpoint: &NativeEndpoint, mut bytes: &mut [u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        let count = endpoint.read(bytes)?;
        if count == 0 { return Err(io::ErrorKind::UnexpectedEof.into()); }
        bytes = &mut bytes[count..];
    }
    Ok(())
}
fn response(endpoint: &NativeEndpoint) -> io::Result<Vec<u8>> {
    response_with(|bytes| read_endpoint(endpoint, bytes))
}
enum LeaseResponse {
    Data(Vec<u8>),
    Rejected(String),
}
impl LeaseResponse {
    fn into_result(self) -> io::Result<Vec<u8>> {
        match self {
            Self::Data(data) => Ok(data),
            Self::Rejected(message) => Err(io::Error::other(message)),
        }
    }
}
fn response_with(mut read_exact: impl FnMut(&mut [u8]) -> io::Result<()>) -> io::Result<Vec<u8>> {
    response_frame_with(&mut read_exact)?.into_result()
}
fn response_frame_with(mut read_exact: impl FnMut(&mut [u8]) -> io::Result<()>) -> io::Result<LeaseResponse> {
    let mut header = [0; 5];
    read_exact(&mut header)?;
    let length = u32::from_le_bytes(header[1..].try_into().unwrap()) as usize;
    if length > MAX_TEXT { return Err(invalid("oversized resource lease response")); }
    let mut data = vec![0; length];
    read_exact(&mut data)?;
    if header[0] != 0 {
        return Ok(LeaseResponse::Rejected(String::from_utf8(data)
            .map_err(|_| invalid("invalid lease diagnostic UTF-8"))?));
    }
    Ok(LeaseResponse::Data(data))
}
fn send_response(output: &mut impl Write, result: io::Result<&[u8]>) -> io::Result<()> {
    let error;
    let (status, bytes) = match result {
        Ok(bytes) => (0, bytes),
        Err(failure) => { error = failure.to_string(); (1, error.as_bytes()) }
    };
    if bytes.len() > MAX_TEXT { return Err(invalid("oversized resource lease response")); }
    output.write_all(&[status])?;
    output.write_all(&(bytes.len() as u32).to_le_bytes())?;
    output.write_all(bytes)?;
    output.flush()
}

struct LeaseControl {
    request: Option<Arc<NativeEndpoint>>,
    response: Arc<NativeEndpoint>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ResourceKind { Substitution, File, Directory }
/// The keeper owns deletion, including after normal/disowned/abrupt parent exit.
/// Its input pipe is never inherited by consumer processes.
pub(crate) struct SubstitutionLease {
    path: PathBuf,
    kind: ResourceKind,
    child: Arc<NativeChild>,
    control: Mutex<LeaseControl>,
}
static LEASES: LazyLock<Mutex<BTreeMap<InternalJobId, Vec<Arc<SubstitutionLease>>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static PERSISTENT_LEASES: LazyLock<Mutex<BTreeMap<PathBuf, Arc<SubstitutionLease>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// Also covers jobs discarded during expansion/prelaunch failure, before a
/// CallerExit event can be emitted. Registered native consumers keep their
/// acknowledged keeper Job leases even when the parser drops the caller.
#[derive(Default)]
pub(crate) struct SubstitutionCaller(pub(crate) InternalJobId);
impl Drop for SubstitutionCaller {
    fn drop(&mut self) { release_caller(self.0); }
}

impl SubstitutionLease {
    fn start(input: &mut impl Read, suffix: &str, kind: ResourceKind, cwd: &Path,
        environment: &[(OsString, OsString)], runtime: Arc<NativeRuntime>) -> io::Result<Arc<Self>> {
        crate::fs::validate_substitution_suffix(suffix)?;
        let (commands, request) = native_synchronous_pipe()?;
        let (response_endpoint, responses) = native_synchronous_pipe()?;
        let mut io = IoChain::new();
        // Do not import any caller's extra descriptors into the keeper.
        for binding in io.child_bindings()? { io.close(binding.fd); }
        io.set_endpoint(0, commands, false);
        io.set_endpoint(1, responses, false);
        let mut group = NativeLaunchGroup::new(runtime)?;
        let plan = group.prepare_resource_keeper(cwd, environment, &io)?;
        let child = group.spawn_prepared(plan)?;
        drop(io);
        // A keeper cannot create a file until accepted launch ownership is
        // installed. Parent death before this point leaves no file to orphan.
        group.accept()?;
        group.release_running()?;
        let result = (|| {
            let mut parent = std::ptr::null_mut();
            if unsafe { DuplicateHandle(GetCurrentProcess(), GetCurrentProcess(), child.handle().as_raw_handle(),
                &mut parent, 0x0010_1040, 0, 0) } == 0 { return Err(io::Error::last_os_error()); }
            write_endpoint(&request, if kind == ResourceKind::Substitution { b"PSUB2" } else { b"TEMP3" })?;
            write_endpoint(&request, &(parent as usize as u64).to_le_bytes())?;
            if kind != ResourceKind::Substitution {
                write_endpoint(&request, if kind == ResourceKind::Directory { b"D" } else { b"F" })?;
            }
            write_endpoint(&request, &(suffix.len() as u32).to_le_bytes())?;
            write_endpoint(&request, suffix.as_bytes())?;
            let mut slab = [0; 65536];
            loop {
                let count = input.read(&mut slab)?;
                write_endpoint(&request, &(count as u32).to_le_bytes())?;
                if count == 0 { break; }
                write_endpoint(&request, &slab[..count])?;
            }
            let path = String::from_utf8(response(&response_endpoint)?).map_err(|_| invalid("invalid psub native path UTF-8"))?;
            let text = crate::path::native_path_text(Path::new(&path))?;
            let path = crate::path::native_absolute_path(&text)?;
            let path = if kind == ResourceKind::Substitution { path } else { crate::path::native_tool_path(&path)? };
            Ok(Arc::new(Self { path, kind, child: Arc::clone(&child), control: Mutex::new(LeaseControl {
                request: Some(Arc::clone(&request)), response: Arc::clone(&response_endpoint),
            }) }))
        })();
        if result.is_err() {
            drop(request);
            response_endpoint.cancel_input_io();
            // EOF leaves cleanup with the private owner, even after parent
            // death. Never wait indefinitely for a paused owner during cancel.
            let waited = unsafe { WaitForSingleObject(child.handle().as_raw_handle(), RELEASE_DEADLINE_MS) };
            if waited == 258 {
                return Err(io::Error::new(io::ErrorKind::TimedOut,
                    format!("{} setup failed ({}); private owner cleanup deadline exceeded",
                        if kind == ResourceKind::Substitution { "psub" } else { "native resource" },
                        result.as_ref().err().unwrap())));
            }
            if waited != 0 { return Err(io::Error::last_os_error()); }
            child.wait(None)?;
        }
        result
    }

    pub(crate) fn register_job(&self, job: BorrowedHandle<'_>, root: BorrowedHandle<'_>) -> io::Result<usize> {
        let control = self.control.lock().unwrap();
        let request = control.request.as_ref().ok_or_else(|| invalid("psub caller already completed"))?;
        let mut packet = [0; 29];
        packet[0] = b'J';
        packet[1..9].copy_from_slice(&(job.as_raw_handle() as usize as u64).to_le_bytes());
        packet[9..17].copy_from_slice(&(root.as_raw_handle() as usize as u64).to_le_bytes());
        packet[17..21].copy_from_slice(&unsafe { GetProcessId(root.as_raw_handle()) }.to_le_bytes());
        packet[21..].copy_from_slice(&process_creation(root.as_raw_handle())?.to_le_bytes());
        // The keeper duplicates from its retained parent-process capability
        // only after receiving this complete packet. No unknown duplicate Job
        // can keep a suspended child alive after a partial write/parent death.
        write_endpoint(request, &packet)?;
        let acknowledgement = response(&control.response)?;
        let cookie = u64::from_le_bytes(acknowledgement.as_slice().try_into()
            .map_err(|_| invalid("invalid pending resource registration acknowledgement"))?);
        if cookie == 0 || cookie == u64::MAX || cookie > usize::MAX as u64 {
            return Err(invalid("invalid pending resource registration cookie"));
        }
        Ok(cookie as usize)
    }

    pub(crate) fn accept_job(&self, cookie: usize) -> io::Result<()> {
        let control = self.control.lock().unwrap();
        let request = control.request.as_ref().ok_or_else(|| invalid("psub caller already completed"))?;
        let mut packet = [0; 9];
        packet[0] = b'A';
        packet[1..].copy_from_slice(&(cookie as u64).to_le_bytes());
        write_endpoint(request, &packet)?;
        if !response(&control.response)?.is_empty() { return Err(invalid("invalid accepted resource registration acknowledgement")); }
        Ok(())
    }

    fn release(&self) -> io::Result<()> {
        self.release_control(false)
    }

    fn release_control(&self, transfer: bool) -> io::Result<()> {
        let mut control = self.control.lock().unwrap();
        let Some(request) = control.request.as_ref().cloned() else { return Ok(()); };
        let response_endpoint = Arc::clone(&control.response);
        let child = Arc::clone(&self.child);
        // A private cleanup owner ignores an already-signaled console receipt,
        // but never waits forever for a stopped/dead keeper. N2 cancellation
        // joins the actual pipe operations before any worker/buffer is dropped.
        let done = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if done.is_null() { return Err(io::Error::last_os_error()); }
        let done = unsafe { OwnedHandle::from_raw_handle(done) };
        let stop = crate::threads::NativeCancellation::new()?;
        let stage = std::sync::atomic::AtomicU8::new(0);
        let result = std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                let result = (|| {
                    write_endpoint(&request, if transfer { b"T" } else { b"R" })?;
                    stage.store(1, std::sync::atomic::Ordering::Release);
                    let state = match response_frame_with(|bytes| read_endpoint(&response_endpoint, bytes))? {
                        LeaseResponse::Data(state) => state,
                        // A rejected transfer is not a release. Preserve both
                        // the request pipe and the caller's deletion capability.
                        rejected @ LeaseResponse::Rejected(_) if transfer => return Ok(rejected),
                        LeaseResponse::Rejected(message) => return Err(io::Error::other(message)),
                    };
                    if state != b"done" && state != b"leased" {
                        return Err(invalid("invalid resource release acknowledgement"));
                    }
                    if state == b"done" {
                        stage.store(2, std::sync::atomic::Ordering::Release);
                        if child.wait(Some(&stop))? != 0 {
                            return Err(io::Error::other("resource keeper failed after cleanup acknowledgement"));
                        }
                    }
                    Ok(LeaseResponse::Data(Vec::new()))
                })();
                if unsafe { SetEvent(done.as_raw_handle()) } == 0 { std::process::abort(); }
                result
            });
            let waited = unsafe { WaitForSingleObject(done.as_raw_handle(), RELEASE_DEADLINE_MS) };
            let wait_error = (waited != 0 && waited != 258).then(io::Error::last_os_error);
            if waited != 0 {
                stop.cancel();
                request.cancel_input_io();
                response_endpoint.cancel_input_io();
            }
            let result = worker.join().map_err(|_| io::Error::other("resource cleanup worker panicked"))?;
            if waited == 258 {
                let message = match stage.load(std::sync::atomic::Ordering::Acquire) {
                    0 => "psub release command deadline exceeded",
                    1 => "psub release acknowledgement deadline exceeded after release command was sent",
                    _ => "psub keeper exit deadline exceeded after cleanup acknowledgement",
                };
                return Err(io::Error::new(io::ErrorKind::TimedOut, message));
            }
            if let Some(error) = wait_error { return Err(error); }
            result
        });
        if !matches!(&result, Ok(LeaseResponse::Rejected(_))) {
            control.request.take();
        }
        result?.into_result().map(|_| ())
    }
}

pub(crate) fn create_substitution(input: &mut impl Read, suffix: &str, caller: InternalJobId, testing: bool,
    cwd: &Path, environment: &[(OsString, OsString)], runtime: Arc<NativeRuntime>) -> io::Result<PathBuf> {
    if !testing && caller == InternalJobId::default() { return Err(invalid("psub calling job not found")); }
    let lease = SubstitutionLease::start(input, suffix, ResourceKind::Substitution, cwd, environment, runtime)?;
    let path = lease.path.clone();
    if testing { PERSISTENT_LEASES.lock().unwrap().insert(path.clone(), lease); }
    else { LEASES.lock().unwrap().entry(caller).or_default().push(lease); }
    Ok(path)
}
pub(crate) fn leases_for_caller(caller: InternalJobId) -> Vec<Arc<SubstitutionLease>> {
    LEASES.lock().unwrap().get(&caller).cloned().unwrap_or_default()
}

pub(crate) fn leases_for_launch(arguments: &[OsString], environment: &[(OsString, OsString)]) -> Vec<Arc<SubstitutionLease>> {
    let matches = |lease: &&Arc<SubstitutionLease>| {
        let value_matches = |argument: &OsString| {
            let matches_path = |path: &Path| path == lease.path
                || (lease.kind == ResourceKind::Directory && path.starts_with(&lease.path));
            matches_path(Path::new(argument))
                || argument.to_str().and_then(|argument| argument.split_once('='))
                    .is_some_and(|(_, value)| matches_path(Path::new(value)))
        };
        arguments.iter().any(value_matches) || environment.iter().any(|(_, value)| value_matches(value))
    };
    let mut result: Vec<_> = LEASES.lock().unwrap().values().flatten().filter(matches).cloned().collect();
    result.extend(PERSISTENT_LEASES.lock().unwrap().values().filter(matches).cloned());
    result
}
pub(crate) fn release_caller(caller: InternalJobId) {
    if caller == InternalJobId::default() { return; }
    let leases = LEASES.lock().unwrap().remove(&caller).unwrap_or_default();
    for lease in leases {
        if let Err(error) = lease.release() {
            let mut diagnostic = crate::io::OutputStream::stderr();
            let resource = if lease.kind == ResourceKind::Substitution { "psub" } else { "native temporary resource" };
            diagnostic.append(&format!("fish: {resource} cleanup for '{}': {error}\n", lease.path.display()));
            diagnostic.flush_and_check_error();
        }
    }
}
pub(crate) fn cleanup_test_substitution(path: &Path) -> io::Result<()> {
    let lease = PERSISTENT_LEASES.lock().unwrap().remove(path).ok_or_else(|| invalid("not an owned persistent psub test path"))?;
    lease.release()?;
    Ok(())
}

pub(crate) fn create_helper_resource(directory: bool, caller: InternalJobId, cwd: &Path,
    environment: &[(OsString, OsString)], runtime: Arc<NativeRuntime>) -> io::Result<PathBuf> {
    let kind = if directory { ResourceKind::Directory } else { ResourceKind::File };
    let lease = SubstitutionLease::start(&mut io::empty(), "", kind, cwd, environment, runtime)?;
    let path = lease.path.clone();
    if caller == InternalJobId::default() {
        PERSISTENT_LEASES.lock().unwrap().insert(path.clone(), lease);
    } else {
        LEASES.lock().unwrap().entry(caller).or_default().push(lease);
    }
    Ok(path)
}

/// Remove only a registered helper capability; arbitrary paths remain the
/// ordinary remove builtin's contract. Transfer requires an accepted child Job.
pub(crate) fn release_helper_resource(path: &Path, transfer: bool) -> io::Result<bool> {
    let eligible = |lease: &Arc<SubstitutionLease>| lease.path == path
        && lease.kind != ResourceKind::Substitution
        && (!transfer || lease.kind == ResourceKind::File);
    let persistent = PERSISTENT_LEASES.lock().unwrap().get(path).filter(|lease| eligible(lease)).cloned();
    let lease = persistent.or_else(|| {
        LEASES.lock().unwrap().values().flatten().find(|lease| eligible(lease)).cloned()
    });
    let Some(lease) = lease else { return Ok(false); };
    lease.release_control(transfer)?;
    // Do not unregister a rejected transfer: the original caller still owns
    // cleanup and may continue using the same resource.
    PERSISTENT_LEASES.lock().unwrap().retain(|_, owned| !Arc::ptr_eq(owned, &lease));
    for leases in LEASES.lock().unwrap().values_mut() {
        leases.retain(|owned| !Arc::ptr_eq(owned, &lease));
    }
    Ok(true)
}

/// File redirects need ownership too, even if the private name never appears
/// in argv or environment. Pipes/consoles are not filesystem resources.
pub(crate) fn leases_for_io(io_chain: &IoChain, cwd: &Path) -> io::Result<Vec<Arc<SubstitutionLease>>> {
    if LEASES.lock().unwrap().is_empty() && PERSISTENT_LEASES.lock().unwrap().is_empty() {
        return Ok(Vec::new());
    }
    let cwd = match crate::path::native_tool_path(cwd) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::Unsupported => cwd.to_owned(),
        Err(error) => return Err(error),
    };
    let mut paths = vec![cwd.into_os_string()];
    for binding in io_chain.child_bindings()? {
        let Some(endpoint) = binding.endpoint else { continue; };
        let handle = endpoint.child_handle()?;
        if unsafe { GetFileType(handle.as_raw_handle()) } != 1 { continue; }
        let file = File::from(handle.try_clone_to_owned()?);
        let path = crate::path::native_absolute_path(&crate::wutil::native_final_path(&file)?)?;
        let path = match crate::path::native_tool_path(&path) {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::Unsupported => path,
            Err(error) => return Err(error),
        };
        paths.push(path.into_os_string());
    }
    Ok(leases_for_launch(&paths, &[]))
}

#[repr(C)]
#[derive(Default)]
struct JobAccounting { user: i64, kernel: i64, period_user: i64, period_kernel: i64,
    faults: u32, total: u32, active: u32, terminated: u32 }
struct JobLease { job: OwnedHandle, root: OwnedHandle, accepted: bool }
impl JobLease {
    fn pending(&self) -> io::Result<bool> {
        // JOBOBJECT_BASIC_LIMIT_INFORMATION starts with two LARGE_INTEGERs,
        // then LimitFlags. Preserve the native pointer-sized alignment.
        #[repr(C)]
        #[derive(Default)]
        struct BasicLimits {
            process_time: i64, job_time: i64, flags: u32,
            minimum_working_set: usize, maximum_working_set: usize,
            active_limit: u32, affinity: usize, priority: u32, scheduling: u32,
        }
        let mut limits = BasicLimits::default();
        if unsafe { QueryInformationJobObject(self.job.as_raw_handle(), 2,
            (&mut limits as *mut BasicLimits).cast(), std::mem::size_of::<BasicLimits>() as u32,
            std::ptr::null_mut()) } == 0 { return Err(io::Error::last_os_error()); }
        Ok(limits.flags & 0x2000 != 0)
    }

    fn reconcile_acceptance(&mut self) -> io::Result<()> {
        // A parent can die after the atomic native accept commit but before A
        // is written. The Job's kill-on-close flag closes that message race.
        self.accepted = !self.pending()?;
        if !self.accepted && unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    fn empty(&self) -> io::Result<bool> {
        let mut data = JobAccounting::default();
        if unsafe { QueryInformationJobObject(self.job.as_raw_handle(), 1, (&mut data as *mut JobAccounting).cast(),
            std::mem::size_of::<JobAccounting>() as u32, std::ptr::null_mut()) } == 0 { return Err(io::Error::last_os_error()); }
        Ok(data.active == 0)
    }
    fn wait(&self) -> io::Result<()> {
        // Zero-active Job accounting can precede the root's final handle
        // rundown. Cleanup must also observe the retained process signalled,
        // otherwise an unaccepted killed reader can delay leaf deletion.
        if unsafe { WaitForSingleObject(self.root.as_raw_handle(), u32::MAX) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if self.empty()? { return Ok(()); }
        let timer = unsafe { CreateWaitableTimerW(std::ptr::null(), 0, std::ptr::null()) };
        if timer.is_null() { return Err(io::Error::last_os_error()); }
        let timer = unsafe { OwnedHandle::from_raw_handle(timer) };
        while !self.empty()? {
            // Job handles are not zero-active wait objects. This private timer
            // bounds authoritative active-process polling to 50 ms; it neither
            // associates an IOCP nor consumes any launch/runtime completion.
            let due: i64 = -500_000;
            if unsafe { SetWaitableTimer(timer.as_raw_handle(), &due, 0,
                std::ptr::null(), std::ptr::null(), 0) } == 0 { return Err(io::Error::last_os_error()); }
            if unsafe { WaitForSingleObject(timer.as_raw_handle(), u32::MAX) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

/// Invoked before option parsing, locale, environment, state or Parser startup.
/// Every creation operation allocates its own private file/directory; no user deletion path or
/// evaluator command is accepted. All resources are private to this keeper.
pub fn resource_lease_entrypoint() -> i32 {
    let result = (|| -> io::Result<()> {
        let stdin = crate::fds::standard_handle(-10i32 as u32)?;
        let stdout = crate::fds::standard_handle(-11i32 as u32)?;
        for handle in [stdin, stdout] {
            if unsafe { GetFileType(handle.as_raw_handle()) } != 3 { return Err(invalid("resource lease endpoints must be pipes")); }
            if unsafe { SetHandleInformation(handle.as_raw_handle(), 1, 0) } == 0 { return Err(io::Error::last_os_error()); }
        }
        let mut input = File::from(stdin.try_clone_to_owned()?);
        let mut output = File::from(stdout.try_clone_to_owned()?);
        keeper(&mut input, &mut output)
    })();
    if result.is_ok() { 0 } else { 1 }
}
enum KeeperResource {
    File(crate::fs::NativeSubstitutionFile),
    Directory(Option<PathBuf>),
}
impl KeeperResource {
    fn create(kind: ResourceKind, suffix: &str, input: &mut impl Read) -> io::Result<Self> {
        if kind == ResourceKind::Directory {
            let mut empty = [0; 4];
            input.read_exact(&mut empty)?;
            if empty != [0; 4] || !suffix.is_empty() {
                return Err(invalid("temporary directory setup accepts no data or suffix"));
            }
            let path = crate::path::native_absolute_path(&crate::fs::make_editor_directory()?)?;
            Ok(Self::Directory(Some(path)))
        } else {
            let mut file = crate::fs::NativeSubstitutionFile::create(suffix)?;
            file.write_frames(input)?;
            if kind == ResourceKind::File { file.allow_consumer_delete()?; }
            Ok(Self::File(file))
        }
    }
    fn path(&self) -> &Path {
        match self {
            Self::File(file) => file.path(),
            Self::Directory(path) => path.as_deref().expect("live directory capability"),
        }
    }
    fn remove(&mut self) -> io::Result<()> {
        match self {
            Self::File(file) => file.remove(),
            Self::Directory(path) => {
                if let Some(owned) = path.as_ref() { crate::fs::remove_editor_directory(owned)?; }
                path.take();
                Ok(())
            }
        }
    }
}
impl Drop for KeeperResource {
    fn drop(&mut self) {
        if let Err(error) = self.remove() {
            crate::flog!(synced_file_access, "Unable to remove native helper resource:", error);
        }
    }
}

fn keeper(input: &mut impl Read, output: &mut impl Write) -> io::Result<()> {
    let mut magic = [0; 5];
    input.read_exact(&mut magic)?;
    if &magic != b"PSUB2" && &magic != b"TEMP3" { return Err(invalid("unsupported resource lease protocol")); }
    let mut parent = [0; 8];
    input.read_exact(&mut parent)?;
    let parent = u64::from_le_bytes(parent);
    if parent == 0 || parent == u64::MAX || parent > usize::MAX as u64 { return Err(invalid("invalid parent process capability")); }
    let parent = parent as usize as RawHandle;
    validate_resource_handle(parent, "Process")?;
    let parent = unsafe { OwnedHandle::from_raw_handle(parent) };
    validate_parent(&parent)?;
    let kind = if &magic == b"TEMP3" {
        let mut kind = [0];
        input.read_exact(&mut kind)?;
        match kind[0] {
            b'D' => ResourceKind::Directory,
            b'F' => ResourceKind::File,
            _ => return Err(invalid("unsupported private temporary resource kind")),
        }
    } else {
        ResourceKind::Substitution
    };
    let mut size = [0; 4];
    input.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size > 720 { return Err(invalid("oversized psub suffix")); }
    let mut suffix = vec![0; size];
    input.read_exact(&mut suffix)?;
    let suffix = String::from_utf8(suffix).map_err(|_| invalid("invalid psub suffix UTF-8"))?;
    let mut file = match KeeperResource::create(kind, &suffix, input) {
        Ok(file) => file,
        Err(error) => {
            let _ = send_response(output, Err(io::Error::new(error.kind(), error.to_string())));
            return Err(error);
        }
    };
    let path = file.path().to_str().ok_or_else(|| invalid("psub native path is not Unicode"))?.as_bytes();
    send_response(output, Ok(path))?;
    let mut jobs: Vec<JobLease> = Vec::new();
    let mut release = false;
    let protocol = (|| -> io::Result<()> {
        loop {
            let mut command = [0];
            if input.read(&mut command)? == 0 { break; }
            match command[0] {
                b'J' => {
                    let mut packet = [0; 28];
                    input.read_exact(&mut packet)?;
                    let values = [u64::from_le_bytes(packet[..8].try_into().unwrap()), u64::from_le_bytes(packet[8..16].try_into().unwrap())];
                    let expected_pid = u32::from_le_bytes(packet[16..20].try_into().unwrap());
                    let expected_creation = u64::from_le_bytes(packet[20..].try_into().unwrap());
                    if values.iter().any(|&value| value == 0 || value == u64::MAX || value > usize::MAX as u64) {
                        return Err(invalid("invalid pending resource Job/root source handle"));
                    }
                    let duplicate = |source: u64, kind: &str| -> io::Result<OwnedHandle> {
                        let mut raw = std::ptr::null_mut();
                        if unsafe { DuplicateHandle(parent.as_raw_handle(), source as usize as RawHandle,
                            GetCurrentProcess(), &mut raw, 0, 0, 2) } == 0 { return Err(io::Error::last_os_error()); }
                        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
                        validate_resource_handle(handle.as_raw_handle(), kind)?;
                        Ok(handle)
                    };
                    let job = duplicate(values[0], "Job")?;
                    let root = duplicate(values[1], "Process")?;
                    // Parent HANDLE slots may be closed/reused after a
                    // canceled registration. PID + immutable creation time
                    // binds this delayed packet to the original process object.
                    if expected_pid == 0 || unsafe { GetProcessId(root.as_raw_handle()) } != expected_pid
                        || process_creation(root.as_raw_handle())? != expected_creation {
                        return Err(invalid("stale resource registration process identity"));
                    }
                    let mut root_member = 0;
                    let mut parent_member = 0;
                    if creator(root.as_raw_handle())? != unsafe { GetProcessId(parent.as_raw_handle()) } as usize
                        || unsafe { IsProcessInJob(root.as_raw_handle(), job.as_raw_handle(), &mut root_member) } == 0
                        || unsafe { IsProcessInJob(parent.as_raw_handle(), job.as_raw_handle(), &mut parent_member) } == 0
                        || root_member == 0 || parent_member != 0 {
                        return Err(invalid("resource Job is not a dedicated owned child group"));
                    }
                    let lease = JobLease { job, root, accepted: false };
                    if lease.empty()? { return Err(invalid("pending resource Job has no suspended root")); }
                    if !lease.pending()? { return Err(invalid("resource Job registration must precede the native accept commit")); }
                    let cookie = (lease.job.as_raw_handle() as usize as u64).to_le_bytes();
                    jobs.push(lease);
                    send_response(output, Ok(&cookie))?;
                }
                b'A' => {
                    let mut packet = [0; 8];
                    input.read_exact(&mut packet)?;
                    let cookie = u64::from_le_bytes(packet);
                    let lease = jobs.iter_mut().find(|lease| lease.job.as_raw_handle() as usize as u64 == cookie)
                        .ok_or_else(|| invalid("unknown accepted resource Job cookie"))?;
                    if lease.pending()? { return Err(invalid("resource Job accepted before native launch commit")); }
                    lease.accepted = true;
                    send_response(output, Ok(b""))?;
                }
                b'R' => { release = true; break; }
                b'T' => {
                    if !jobs.iter().any(|job| job.accepted) {
                        send_response(output, Err(invalid("temporary file transfer requires an acknowledged accepted native child Job")))?;
                        continue;
                    }
                    release = true;
                    break;
                }
                _ => return Err(invalid("unsupported resource lease command")),
            }
        }
        Ok(())
    })();
    // Parent EOF, partial setup and malformed input reconcile every pending
    // Job before waiting. Otherwise our query handle itself would delay
    // KILL_ON_JOB_CLOSE and strand a suspended, unaccepted root indefinitely.
    for job in &mut jobs { job.reconcile_acceptance()?; }
    let empty = jobs.iter().try_fold(true, |empty, job| Ok::<_, io::Error>(job.empty()? && empty))?;
    if release && !empty { let _ = send_response(output, Ok(b"leased")); }
    for job in &jobs { job.wait()?; }
    file.remove()?;
    if release && empty { send_response(output, Ok(b"done"))?; }
    protocol
}
