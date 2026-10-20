//! Native Windows workers, completion queues and retained wait registrations.
//!
//! Kernel callbacks only append owned records. Queue membership, removal and the
//! manual-reset notification are serialized by one mutex; HANDLE values are
//! never identities. Resource owners drain their callbacks before freeing them.

use std::any::Any;
use std::collections::{HashSet, VecDeque};
use std::ffi::c_void;
use std::fmt;
use std::io;
use std::num::NonZeroU64;
use std::os::windows::io::{
    AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle,
};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

#[path = "windows_workers.rs"]
mod workers;
pub use workers::{Debounce, MainThread, ThreadPool, spawn};

const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;
const WAIT_FAILED: u32 = u32::MAX;
const INFINITE: u32 = u32::MAX;
const SYNCHRONIZE: u32 = 0x0010_0000;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
    fn GetCurrentProcess() -> RawHandle;
    fn CreateEventW(
        attributes: *const c_void,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> RawHandle;
    fn SetEvent(event: RawHandle) -> i32;
    fn ResetEvent(event: RawHandle) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
    fn DuplicateHandle(
        source_process: RawHandle,
        source: RawHandle,
        target_process: RawHandle,
        target: *mut RawHandle,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
    fn GetProcessId(process: RawHandle) -> u32;
    fn GetExitCodeProcess(process: RawHandle, status: *mut u32) -> i32;
    fn CreateThreadpoolWait(
        callback: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, u32),
        context: *mut c_void,
        environment: *const c_void,
    ) -> *mut c_void;
    fn SetThreadpoolWait(wait: *mut c_void, handle: RawHandle, timeout: *const c_void);
    fn WaitForThreadpoolWaitCallbacks(wait: *mut c_void, cancel_pending: i32);
    fn CloseThreadpoolWait(wait: *mut c_void);
}

// Keep this ABI identical to the existing NtQueryObject declaration in the
// descriptor bridge. ObjectTypeInformation writes another layout into the
// supplied, sufficiently aligned byte storage; the pointer is only an ABI type.
#[repr(C)]
struct ObjectBasicInformation {
    _attributes: u32,
    _granted_access: u32,
    _handle_count: u32,
    _pointer_count: u32,
    _reserved: [u32; 10],
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryObject(
        handle: RawHandle,
        information_class: u32,
        information: *mut ObjectBasicInformation,
        length: u32,
        returned_length: *mut u32,
    ) -> i32;
    fn RtlNtStatusToDosError(status: i32) -> u32;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ObjectTypeName {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[repr(C, align(16))]
struct ObjectTypeStorage([u8; 1024]);

fn validate_wait_type(handle: BorrowedHandle<'_>) -> io::Result<()> {
    // A fixed bound suffices for supported kernel type names. Reject a response
    // needing more storage rather than trusting a partial header or allocating
    // according to a kernel-returned size. No wait consumes the object's signal.
    let mut storage = ObjectTypeStorage([0; 1024]);
    let mut returned_length = 0;
    let status = unsafe {
        NtQueryObject(
            handle.as_raw_handle(),
            2, // ObjectTypeInformation; its first member is UNICODE_STRING.
            storage.0.as_mut_ptr().cast(),
            storage.0.len() as u32,
            &mut returned_length,
        )
    };
    if status < 0 {
        return Err(io::Error::from_raw_os_error(unsafe {
            RtlNtStatusToDosError(status) as i32
        }));
    }
    let invalid_response = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid native wait object type information",
        )
    };
    let returned_length = returned_length as usize;
    if returned_length < std::mem::size_of::<ObjectTypeName>() || returned_length > storage.0.len()
    {
        return Err(invalid_response());
    }
    let name = unsafe { ptr::read(storage.0.as_ptr().cast::<ObjectTypeName>()) };
    let start = storage.0.as_ptr() as usize;
    let name_start = name.buffer as usize;
    let length = usize::from(name.length);
    if length == 0
        || length % 2 != 0
        || name.maximum_length < name.length
        || name.maximum_length % 2 != 0
        || name_start % std::mem::align_of::<u16>() != 0
        || name_start < start + std::mem::size_of::<ObjectTypeName>()
        || name_start > start + returned_length
        || length > start + returned_length - name_start
    {
        return Err(invalid_response());
    }
    // The complete UTF-16 slice is now proven to lie inside initialized,
    // correctly aligned query storage, which remains live through comparison.
    let name = unsafe { std::slice::from_raw_parts(name.buffer, length / 2) };
    if char::decode_utf16(name.iter().copied()).any(|value| value.is_err()) {
        return Err(invalid_response());
    }
    if ["Event", "Process", "Thread", "Semaphore", "Timer"]
        .iter()
        .any(|supported| name.iter().copied().eq(supported.encode_utf16()))
    {
        Ok(())
    } else {
        // In particular, NT Mutant (mutex) must never reach SetThreadpoolWait:
        // that API raises STATUS_THREADPOOL_HANDLE_EXCEPTION instead of failing.
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native threadpool waits require an Event, Process, Thread, Semaphore or Timer handle",
        ))
    }
}

static MAIN_THREAD_ID: OnceLock<u32> = OnceLock::new();

fn thread_id() -> u32 {
    // This API has no failure return and does not allocate or create a handle.
    unsafe { GetCurrentThreadId() }
}

/// Register the startup thread before creating any worker threads.
pub fn init() {
    MAIN_THREAD_ID
        .set(thread_id())
        .expect("threads::init() must only be called once (at startup)!");
}

/// Low-level IO can be used before an evaluator is registered. Such callers
/// must not inherit a different owner's process-wide console cancellation.
pub(crate) fn is_registered_main_thread() -> bool {
    MAIN_THREAD_ID.get().is_some_and(|id| *id == thread_id())
}

pub fn is_main_thread() -> bool {
    thread_id()
        == *MAIN_THREAD_ID
            .get()
            .expect("threads::init() was not called at startup!")
}

pub fn assert_is_main_thread() {
    assert!(
        is_main_thread(),
        "Function is not running on the main thread!"
    );
}

pub fn assert_is_background_thread() {
    assert!(
        !is_main_thread(),
        "Function is not allowed to be called on the main thread!"
    );
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panicking consumer must not make the queue unusable by kernel callbacks.
    // No user code or user-owned destructor executes while the queue is locked.
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// One non-inheritable, manual-reset event. All users retain its owner while waiting.
struct NativeEvent(OwnedHandle);

impl NativeEvent {
    fn new() -> io::Result<Self> {
        let raw = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(raw) }))
    }

    fn handle(&self) -> BorrowedHandle<'_> {
        self.0.as_handle()
    }

    fn set(&self) {
        // Neither the event nor its ownership is exposed for mutation. Failure
        // would mean a violated handle-ownership invariant, not a dropped wakeup.
        if unsafe { SetEvent(self.0.as_raw_handle()) } == 0 {
            std::process::abort();
        }
    }

    fn reset(&self) {
        if unsafe { ResetEvent(self.0.as_raw_handle()) } == 0 {
            std::process::abort();
        }
    }

    fn is_set(&self) -> bool {
        match unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => true,
            WAIT_TIMEOUT => false,
            _ => std::process::abort(),
        }
    }
}

/// An owned, shareable cancellation token. Cancellation is permanent.
#[derive(Clone)]
pub struct NativeCancellation {
    event: Arc<NativeEvent>,
}

impl NativeCancellation {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            event: Arc::new(NativeEvent::new()?),
        })
    }

    pub fn cancel(&self) {
        self.event.set();
    }

    pub fn is_cancelled(&self) -> bool {
        self.event.is_set()
    }

    pub fn handle(&self) -> BorrowedHandle<'_> {
        self.event.handle()
    }
}

/// Process-wide monotonic identity, independent of recyclable native handles.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RegistrationId(NonZeroU64);

impl RegistrationId {
    fn next() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let value = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| io::Error::other("native registration identity space exhausted"))?;
        Ok(Self(
            NonZeroU64::new(value).expect("registration identity is nonzero"),
        ))
    }
}

pub struct NativeCompletion {
    pub registration: RegistrationId,
    pub payload: NativeCompletionPayload,
}

impl fmt::Debug for NativeCompletion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeCompletion")
            .field("registration", &self.registration)
            .field("payload", &self.payload)
            .finish()
    }
}

pub enum NativeCompletionPayload {
    Ready,
    /// Captured only after the real retained process handle signals. 259 is a
    /// valid immutable exit DWORD here, not a request to poll the PID again.
    ProcessExit {
        pid: u32,
        raw_status: u32,
    },
    User(Box<dyn Any + Send>),
    Error(io::Error),
}

impl fmt::Debug for NativeCompletionPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => f.write_str("Ready"),
            Self::ProcessExit { pid, raw_status } => f
                .debug_struct("ProcessExit")
                .field("pid", pid)
                .field("raw_status", raw_status)
                .finish(),
            Self::User(_) => f.write_str("User(..)"),
            Self::Error(error) => f.debug_tuple("Error").field(error).finish(),
        }
    }
}

#[derive(Default)]
struct QueueState {
    open: HashSet<RegistrationId>,
    records: VecDeque<NativeCompletion>,
}

/// Shared completion queue. Native callbacks never own or call evaluator state.
pub struct NativeRuntime {
    queue: Mutex<QueueState>,
    notification: NativeEvent,
}

impl NativeRuntime {
    pub fn new() -> io::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            queue: Mutex::new(QueueState::default()),
            notification: NativeEvent::new()?,
        }))
    }

    pub fn register_source(self: &Arc<Self>) -> io::Result<NativeSource> {
        let id = RegistrationId::next()?;
        lock(&self.queue).open.insert(id);
        Ok(NativeSource {
            runtime: Arc::clone(self),
            id,
            closed: false,
        })
    }

    /// Register a one-shot wait for an Event, Process, Thread, Semaphore or Timer
    /// handle. Other object types, including mutexes, files and completion ports,
    /// are rejected before arming. Each wait uses the native threadpool, not a
    /// collector thread or a fixed-size WaitForMultipleObjects registration set.
    pub fn register_wait(
        self: &Arc<Self>,
        handle: Arc<OwnedHandle>,
    ) -> io::Result<NativeWaitRegistration> {
        self.register_wait_inner(handle, None)
    }

    pub fn register_process(
        self: &Arc<Self>,
        pid: u32,
        handle: Arc<OwnedHandle>,
    ) -> io::Result<NativeWaitRegistration> {
        let actual_pid = unsafe { GetProcessId(handle.as_raw_handle()) };
        if actual_pid == 0 {
            return Err(io::Error::last_os_error());
        }
        if actual_pid != pid {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "process handle does not identify the supplied PID",
            ));
        }
        // Validate query permission before arming. This value is NOT retained as
        // a completion: a still-running process may return STILL_ACTIVE here.
        let mut status = 0;
        if unsafe { GetExitCodeProcess(handle.as_raw_handle(), &mut status) } == 0 {
            return Err(io::Error::last_os_error());
        }
        self.register_wait_inner(handle, Some(pid))
    }

    fn register_wait_inner(
        self: &Arc<Self>,
        handle: Arc<OwnedHandle>,
        pid: Option<u32>,
    ) -> io::Result<NativeWaitRegistration> {
        // Obtain a real, non-inheritable wait-rights handle without consuming a
        // signal (a zero-time wait could consume an auto-reset event or mutex).
        let process = unsafe { GetCurrentProcess() };
        let mut raw_wait_handle = ptr::null_mut();
        if unsafe {
            DuplicateHandle(
                process,
                handle.as_raw_handle(),
                process,
                &mut raw_wait_handle,
                SYNCHRONIZE,
                0,
                0,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let wait_handle = unsafe { OwnedHandle::from_raw_handle(raw_wait_handle) };
        validate_wait_type(wait_handle.as_handle())?;
        let source = self.register_source()?;
        let mut context = Box::new(WaitContext {
            poster: source.poster(),
            handle,
            pid,
        });
        let wait = unsafe {
            CreateThreadpoolWait(
                wait_callback,
                (&mut *context as *mut WaitContext).cast(),
                ptr::null(),
            )
        };
        let wait = NonNull::new(wait).ok_or_else(io::Error::last_os_error)?;
        let registration = NativeWaitRegistration {
            source,
            inner: Some(WaitRegistrationInner {
                wait,
                _context: context,
                _wait_handle: wait_handle,
            }),
        };
        // Everything reachable by the callback is pinned in its Box before arm.
        unsafe { SetThreadpoolWait(wait.as_ptr(), raw_wait_handle, ptr::null()) };
        Ok(registration)
    }

    pub fn notification_handle(&self) -> BorrowedHandle<'_> {
        self.notification.handle()
    }

    pub fn drain(&self) -> Vec<NativeCompletion> {
        let mut queue = lock(&self.queue);
        let records = queue.records.drain(..).collect();
        // This reset and every post's SetEvent share the queue lock. A post
        // racing with this drain either appears above or sets the event after it.
        self.notification.reset();
        records
    }

    /// Wait only on the coalesced queue notification and optional cancellation.
    /// An observed cancellation returns Interrupted without consuming records.
    /// Timeout returns an empty vector, never fabricated readiness.
    pub fn wait(
        &self,
        timeout: Option<Duration>,
        cancellation: Option<&NativeCancellation>,
    ) -> io::Result<Vec<NativeCompletion>> {
        let start = Instant::now();
        loop {
            if cancellation.is_some_and(NativeCancellation::is_cancelled) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "native wait cancelled",
                ));
            }
            let records = self.drain();
            if !records.is_empty() {
                return Ok(records);
            }
            let remaining = timeout.map(|limit| limit.saturating_sub(start.elapsed()));
            if remaining == Some(Duration::ZERO) {
                return Ok(Vec::new());
            }
            let handles = [
                self.notification_handle().as_raw_handle(),
                cancellation.map_or(ptr::null_mut(), |token| token.handle().as_raw_handle()),
            ];
            let count = if cancellation.is_some() { 2 } else { 1 };
            match crate::console_windows::wait_for_evaluator_handles(
                &handles[..count], wait_milliseconds(remaining),
            )? {
                WAIT_OBJECT_0 | WAIT_TIMEOUT => (),
                value if value == WAIT_OBJECT_0 + 1 && cancellation.is_some() => {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "native wait cancelled",
                    ));
                }
                WAIT_FAILED => return Err(io::Error::last_os_error()),
                value => {
                    return Err(io::Error::other(format!(
                        "unexpected native queue wait result {value}"
                    )));
                }
            }
            // Another consumer may have drained the coalesced wakeup. Recheck
            // under the lock and retain the original timeout across such wakes.
        }
    }

    /// Apply owned completion work only on the registered evaluator thread.
    pub fn service_main<F: FnMut(NativeCompletion)>(&self, mut consumer: F) -> usize {
        assert_is_main_thread();
        let records = self.drain();
        let count = records.len();
        for record in records {
            consumer(record);
        }
        count
    }

    fn close_source(&self, id: RegistrationId) {
        lock(&self.queue).open.remove(&id);
    }

    fn take_source_records(&self, id: RegistrationId, close: bool) -> Vec<NativeCompletion> {
        let mut queue = lock(&self.queue);
        if close {
            queue.open.remove(&id);
        }
        let mut result = Vec::new();
        // Rotate in place: no second allocation/copy of unrelated payloads.
        for _ in 0..queue.records.len() {
            let record = queue.records.pop_front().expect("queue length is locked");
            if record.registration == id {
                result.push(record);
            } else {
                queue.records.push_back(record);
            }
        }
        if queue.records.is_empty() {
            self.notification.reset();
        }
        result
    }
}

fn wait_milliseconds(timeout: Option<Duration>) -> u32 {
    timeout.map_or(INFINITE, |duration| {
        // Round upward so a positive sub-millisecond timeout does not busy poll.
        let millis = duration.as_millis() + u128::from(duration.subsec_nanos() % 1_000_000 != 0);
        millis.min(u128::from(INFINITE - 1)) as u32
    })
}

/// Client-owned queue identity. Before removing an I/O source its owner must
/// cancel and drain kernel I/O; this object does not own those client operations.
pub struct NativeSource {
    runtime: Arc<NativeRuntime>,
    id: RegistrationId,
    closed: bool,
}

impl NativeSource {
    pub fn id(&self) -> RegistrationId {
        self.id
    }

    pub fn poster(&self) -> NativePoster {
        NativePoster {
            runtime: Arc::clone(&self.runtime),
            id: self.id,
        }
    }

    pub fn remove_and_drain(&mut self) -> io::Result<Vec<NativeCompletion>> {
        self.closed = true;
        Ok(self.runtime.take_source_records(self.id, true))
    }
}

impl Drop for NativeSource {
    fn drop(&mut self) {
        if !self.closed {
            // Owned payloads are dropped after the queue lock is released.
            drop(self.runtime.take_source_records(self.id, true));
        }
    }
}

/// A producer retains only queue state and identity, never a resource-owner Drop.
#[derive(Clone)]
pub struct NativePoster {
    runtime: Arc<NativeRuntime>,
    id: RegistrationId,
}

impl NativePoster {
    /// Returns false after close; late records cannot reach a recycled owner.
    pub fn post(&self, payload: NativeCompletionPayload) -> bool {
        let mut queue = lock(&self.runtime.queue);
        if !queue.open.contains(&self.id) {
            // Drop a rejected User payload after releasing the queue lock.
            drop(queue);
            return false;
        }
        let was_empty = queue.records.is_empty();
        queue.records.push_back(NativeCompletion {
            registration: self.id,
            payload,
        });
        if was_empty {
            self.runtime.notification.set();
        }
        true
    }

    /// A debouncer owns only its latest result, so keep at most one queued
    /// readiness record while still setting the wakeup on every publication.
    fn post_ready_coalesced(&self) -> bool {
        let mut queue = lock(&self.runtime.queue);
        if !queue.open.contains(&self.id) {
            return false;
        }
        if !queue.records.iter().any(|record| {
            record.registration == self.id
                && matches!(&record.payload, NativeCompletionPayload::Ready)
        }) {
            queue.records.push_back(NativeCompletion {
                registration: self.id,
                payload: NativeCompletionPayload::Ready,
            });
        }
        self.runtime.notification.set();
        true
    }
}

struct WaitContext {
    poster: NativePoster,
    handle: Arc<OwnedHandle>,
    pid: Option<u32>,
}

unsafe extern "system" fn wait_callback(
    _instance: *mut c_void,
    context: *mut c_void,
    _wait: *mut c_void,
    result: u32,
) {
    // The registration disarms and joins this callback before freeing context.
    // There is no owner wrapper here to Drop or self-unregister from a callback.
    let context = unsafe { &*context.cast::<WaitContext>() };
    let payload = if result != WAIT_OBJECT_0 {
        NativeCompletionPayload::Error(io::Error::other(format!(
            "unexpected native registered wait result {result}"
        )))
    } else if let Some(pid) = context.pid {
        let mut raw_status = 0;
        if unsafe { GetExitCodeProcess(context.handle.as_raw_handle(), &mut raw_status) } == 0 {
            NativeCompletionPayload::Error(io::Error::last_os_error())
        } else {
            NativeCompletionPayload::ProcessExit { pid, raw_status }
        }
    } else {
        NativeCompletionPayload::Ready
    };
    context.poster.post(payload);
}

struct WaitRegistrationInner {
    wait: NonNull<c_void>,
    // These remain live through WaitForThreadpoolWaitCallbacks and close.
    _context: Box<WaitContext>,
    _wait_handle: OwnedHandle,
}

// The private pointer is only armed before publication and accessed thereafter
// with exclusive &mut ownership when disarming. Its callback reads only the Box.
unsafe impl Send for WaitRegistrationInner {}
unsafe impl Sync for WaitRegistrationInner {}

/// RAII, one-shot native wait with retained process/wait handle ownership.
pub struct NativeWaitRegistration {
    source: NativeSource,
    inner: Option<WaitRegistrationInner>,
}

impl NativeWaitRegistration {
    pub fn id(&self) -> RegistrationId {
        self.source.id()
    }

    pub fn remove_and_drain(&mut self) -> io::Result<Vec<NativeCompletion>> {
        // First reject all new posts; an already-entered callback may still read
        // its context, but cannot publish after this queue-close linearization.
        self.source.runtime.close_source(self.source.id);
        if let Some(inner) = self.inner.take() {
            unsafe {
                SetThreadpoolWait(inner.wait.as_ptr(), ptr::null_mut(), ptr::null());
                WaitForThreadpoolWaitCallbacks(inner.wait.as_ptr(), 1);
                CloseThreadpoolWait(inner.wait.as_ptr());
            }
            // The callback cannot own this wrapper. Therefore draining never
            // self-joins; context, original handle and wait duplicate die last.
            drop(inner);
        }
        self.source.remove_and_drain()
    }
}

impl Drop for NativeWaitRegistration {
    fn drop(&mut self) {
        drop(self.remove_and_drain());
    }
}

/// Run singleton/evaluator owner tests on an isolated registered startup thread.
#[cfg(test)]
pub(crate) fn isolated_test(name: &str) -> bool {
    use std::process::{Command, Stdio};
    let name = name
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap();
    if std::env::var("FISH_NATIVE_OWNER_UNIT_CHILD").as_deref() == Ok(name) {
        init();
        return true;
    }
    let mut child = Command::new(crate::wutil::native_current_exe_path().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("FISH_NATIVE_OWNER_UNIT_CHILD", name)
        .env_remove("RUST_BACKTRACE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    fn drain(mut stream: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            bytes
        })
    }
    let out = drain(child.stdout.take().unwrap());
    let err = drain(child.stderr.take().unwrap());
    let waited = unsafe {
        WaitForSingleObject(
            std::os::windows::io::AsRawHandle::as_raw_handle(&child),
            30_000,
        )
    };
    if waited != 0 {
        let _ = child.kill();
    }
    let status = child.wait().unwrap();
    let stdout = out.join().unwrap();
    let stderr = err.join().unwrap();
    assert_eq!(
        waited,
        0,
        "bounded native owner child: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        status.success(),
        "native owner child failed:\n{}\n{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        String::from_utf8_lossy(&stdout)
            .contains("test result: ok. 1 passed; 0 failed; 0 ignored;"),
        "isolated owner harness must execute exactly its required case"
    );
    false
}
