//! Owned native pipe operations. OVERLAPPED storage survives kernel and callback completion.
use super::descriptors::NativeHandle;
use super::{IoBuffer, IoChain, NativeEndpoint, NativeIoDirection, SeparatedBuffer};
use crate::threads::{NativeCompletionPayload, NativePoster, NativeRuntime, NativeSource};
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::os::windows::io::{
    AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle,
};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread::JoinHandle;

const IO_PENDING: u32 = 997;
const OPERATION_ABORTED: u32 = 995;
const INFINITE: u32 = u32::MAX;
const SLAB: usize = 16384;

#[repr(C)]
#[derive(Clone, Copy)]
union Offset {
    offsets: [u32; 2],
    pointer: *mut c_void,
}
#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: Offset,
    event: RawHandle,
}
impl Overlapped {
    fn new() -> Self {
        Self {
            internal: 0,
            internal_high: 0,
            offset: Offset { offsets: [0, 0] },
            event: ptr::null_mut(),
        }
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> RawHandle;
    fn SetEvent(event: RawHandle) -> i32;
    fn ResetEvent(event: RawHandle) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
    fn ReadFile(
        handle: RawHandle,
        buffer: *mut u8,
        count: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn WriteFile(
        handle: RawHandle,
        buffer: *const u8,
        count: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CancelIoEx(handle: RawHandle, overlapped: *const Overlapped) -> i32;
    fn CancelSynchronousIo(thread: RawHandle) -> i32;
    fn CreateThreadpoolIo(
        handle: RawHandle,
        callback: unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            *mut c_void,
            u32,
            usize,
            *mut c_void,
        ),
        context: *mut c_void,
        environment: *const c_void,
    ) -> *mut c_void;
    fn StartThreadpoolIo(io: *mut c_void);
    fn CancelThreadpoolIo(io: *mut c_void);
    fn WaitForThreadpoolIoCallbacks(io: *mut c_void, cancel_pending: i32);
    fn CloseThreadpoolIo(io: *mut c_void);
    fn CreatePipe(
        read: *mut RawHandle,
        write: *mut RawHandle,
        attributes: *const c_void,
        size: u32,
    ) -> i32;
    fn CreateNamedPipeW(
        name: *const u16,
        open_mode: u32,
        pipe_mode: u32,
        maximum: u32,
        output_size: u32,
        input_size: u32,
        timeout: u32,
        attributes: *const c_void,
    ) -> RawHandle;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        attributes: *const c_void,
        creation: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn ConnectNamedPipe(pipe: RawHandle, overlapped: *mut Overlapped) -> i32;
    fn GetOverlappedResult(
        handle: RawHandle,
        overlapped: *mut Overlapped,
        transferred: *mut u32,
        wait: i32,
    ) -> i32;
    fn GetNamedPipeClientProcessId(pipe: RawHandle, pid: *mut u32) -> i32;
    fn PeekNamedPipe(
        pipe: RawHandle,
        buffer: *mut u8,
        size: u32,
        read: *mut u32,
        available: *mut u32,
        left: *mut u32,
    ) -> i32;
    fn GetFileType(handle: RawHandle) -> u32;
    fn GetCurrentProcessId() -> u32;
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}
fn eof(error: u32) -> bool {
    matches!(error, 109 | 232 | 233)
}
fn error(code: u32) -> io::Error {
    if code == OPERATION_ABORTED {
        io::ErrorKind::Interrupted.into()
    } else {
        io::Error::from_raw_os_error(code as i32)
    }
}
fn last_error() -> u32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(1) as u32
}
fn own(raw: RawHandle) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == (-1isize) as RawHandle {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
    }
}

struct Event(OwnedHandle);
impl Event {
    fn new(manual: bool, initial: bool) -> io::Result<Self> {
        own(unsafe { CreateEventW(ptr::null(), manual as i32, initial as i32, ptr::null()) })
            .map(Self)
    }
    fn set(&self) {
        if unsafe { SetEvent(self.0.as_raw_handle()) } == 0 {
            std::process::abort();
        }
    }
    fn reset(&self) {
        if unsafe { ResetEvent(self.0.as_raw_handle()) } == 0 {
            std::process::abort();
        }
    }
    fn wait(&self) -> io::Result<()> {
        if unsafe { WaitForSingleObject(self.0.as_raw_handle(), INFINITE) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

pub(super) fn is_pipe(handle: BorrowedHandle<'_>) -> bool {
    unsafe { GetFileType(handle.as_raw_handle()) == 3 }
}
pub(super) fn read_synchronous(handle: BorrowedHandle<'_>, bytes: &mut [u8]) -> io::Result<usize> {
    if bytes.is_empty() {
        return Ok(0);
    }
    let mut count = 0;
    if unsafe {
        ReadFile(
            handle.as_raw_handle(),
            bytes.as_mut_ptr(),
            bytes.len().min(u32::MAX as usize) as u32,
            &mut count,
            ptr::null_mut(),
        )
    } == 0
    {
        let code = last_error();
        if eof(code) {
            return Ok(0);
        }
        return Err(error(code));
    }
    Ok(count as usize)
}

#[derive(Clone, Copy)]
struct IoResult {
    code: u32,
    bytes: usize,
}
#[repr(C)]
struct BlockingOperation {
    overlapped: UnsafeCell<Overlapped>,
    event: Event,
    result: Mutex<Option<IoResult>>,
}
// Only the kernel writes OVERLAPPED; the submitter retains the box until callback drain.
unsafe impl Send for BlockingOperation {}
unsafe impl Sync for BlockingOperation {}
unsafe extern "system" fn blocking_callback(
    _: *mut c_void,
    _: *mut c_void,
    overlapped: *mut c_void,
    code: u32,
    bytes: usize,
    _: *mut c_void,
) {
    let operation = unsafe { &*(overlapped.cast::<BlockingOperation>()) };
    *lock(&operation.result) = Some(IoResult { code, bytes });
    operation.event.set();
}

struct OperationControl {
    cancelled: bool,
    active: bool,
}
pub(super) struct NativeOverlappedFile {
    owner: NativeHandle,
    io: *mut c_void,
    operation: Mutex<Box<BlockingOperation>>,
    control: Mutex<OperationControl>,
}
// Submission/cancellation are serialized; only one operation uses the pinned slot.
unsafe impl Send for NativeOverlappedFile {}
unsafe impl Sync for NativeOverlappedFile {}
impl NativeOverlappedFile {
    pub(super) fn new(owner: NativeHandle) -> io::Result<Self> {
        let operation = Box::new(BlockingOperation {
            overlapped: UnsafeCell::new(Overlapped::new()),
            event: Event::new(true, false)?,
            result: Mutex::new(None),
        });
        let io = unsafe {
            CreateThreadpoolIo(
                owner.as_handle().as_raw_handle(),
                blocking_callback,
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if io.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            owner,
            io,
            operation: Mutex::new(operation),
            control: Mutex::new(OperationControl {
                cancelled: false,
                active: false,
            }),
        })
    }
    fn run(&self, bytes: *mut u8, count: usize, write: bool) -> io::Result<usize> {
        if count == 0 {
            return Ok(0);
        }
        let operation = lock(&self.operation);
        {
            let mut control = lock(&self.control);
            if control.cancelled {
                return Err(io::ErrorKind::Interrupted.into());
            }
            unsafe {
                *operation.overlapped.get() = Overlapped::new();
            }
            *lock(&operation.result) = None;
            operation.event.reset();
            unsafe {
                StartThreadpoolIo(self.io);
            }
            control.active = true;
            let result = unsafe {
                if write {
                    WriteFile(
                        self.owner.as_handle().as_raw_handle(),
                        bytes.cast_const(),
                        count.min(u32::MAX as usize) as u32,
                        ptr::null_mut(),
                        operation.overlapped.get().cast(),
                    )
                } else {
                    ReadFile(
                        self.owner.as_handle().as_raw_handle(),
                        bytes,
                        count.min(u32::MAX as usize) as u32,
                        ptr::null_mut(),
                        operation.overlapped.get().cast(),
                    )
                }
            };
            if result == 0 {
                let code = last_error();
                if code != IO_PENDING {
                    unsafe {
                        CancelThreadpoolIo(self.io);
                    }
                    control.active = false;
                    return if !write && eof(code) {
                        Ok(0)
                    } else {
                        Err(error(code))
                    };
                }
            }
        }
        // Even an event-wait failure cannot permit a borrowed buffer to escape pending I/O.
        let wait_error = operation.event.wait().err();
        if wait_error.is_some() {
            self.cancel();
        }
        unsafe {
            WaitForThreadpoolIoCallbacks(self.io, 0);
        }
        lock(&self.control).active = false;
        if let Some(error) = wait_error {
            return Err(error);
        }
        let result = lock(&operation.result)
            .take()
            .expect("drained native I/O has a completion");
        if result.bytes > count {
            std::process::abort();
        }
        if result.bytes != 0 {
            return Ok(result.bytes);
        }
        if result.code == 0 || (!write && eof(result.code)) {
            Ok(0)
        } else {
            Err(error(result.code))
        }
    }
    pub(super) fn read(&self, bytes: &mut [u8]) -> io::Result<usize> {
        self.run(bytes.as_mut_ptr(), bytes.len(), false)
    }
    pub(super) fn write(&self, bytes: &[u8]) -> io::Result<usize> {
        self.run(bytes.as_ptr().cast_mut(), bytes.len(), true)
    }
    pub(super) fn cancel(&self) {
        let mut control = lock(&self.control);
        control.cancelled = true;
        if control.active {
            unsafe {
                CancelIoEx(self.owner.as_handle().as_raw_handle(), ptr::null());
            }
        }
    }
}
impl Drop for NativeOverlappedFile {
    fn drop(&mut self) {
        self.cancel();
        unsafe {
            WaitForThreadpoolIoCallbacks(self.io, 0);
            CloseThreadpoolIo(self.io);
        }
    }
}

struct SyncRequest {
    buffer: *mut u8,
    count: usize,
    requested: bool,
    active: bool,
    stopped: bool,
    cancelled: bool,
    result: Option<IoResult>,
}
struct SyncState {
    owner: NativeHandle,
    request: Event,
    complete: Event,
    state: Mutex<SyncRequest>,
}
// The reader holds its caller's mutable buffer until the worker acknowledges completion.
// State lock prevents cancellation from targeting a reused worker operation.
unsafe impl Send for SyncRequest {}
unsafe impl Sync for SyncRequest {}
pub(super) struct NativeSynchronousInput {
    state: Arc<SyncState>,
    worker: Option<JoinHandle<()>>,
    gate: Mutex<()>,
}
impl NativeSynchronousInput {
    pub(super) fn new(owner: NativeHandle) -> io::Result<Self> {
        let state = Arc::new(SyncState {
            owner,
            request: Event::new(false, false)?,
            complete: Event::new(true, false)?,
            state: Mutex::new(SyncRequest {
                buffer: ptr::null_mut(),
                count: 0,
                requested: false,
                active: false,
                stopped: false,
                cancelled: false,
                result: None,
            }),
        });
        let worker_state = Arc::clone(&state);
        let worker = std::thread::Builder::new().spawn(move || {
            loop {
                if worker_state.request.wait().is_err() {
                    std::process::abort();
                }
                let (buffer, count) = {
                    let mut request = lock(&worker_state.state);
                    if request.stopped {
                        break;
                    }
                    if !request.requested {
                        continue;
                    }
                    request.requested = false;
                    if request.cancelled {
                        request.result = Some(IoResult {
                            code: OPERATION_ABORTED,
                            bytes: 0,
                        });
                        worker_state.complete.set();
                        continue;
                    }
                    request.active = true;
                    (request.buffer, request.count)
                };
                let mut count_read = 0;
                let result = unsafe {
                    ReadFile(
                        worker_state.owner.as_handle().as_raw_handle(),
                        buffer,
                        count as u32,
                        &mut count_read,
                        ptr::null_mut(),
                    )
                };
                let code = if result == 0 { last_error() } else { 0 };
                let mut request = lock(&worker_state.state);
                request.active = false;
                request.result = Some(IoResult {
                    code,
                    bytes: count_read as usize,
                });
                worker_state.complete.set();
            }
        })?;
        Ok(Self {
            state,
            worker: Some(worker),
            gate: Mutex::new(()),
        })
    }
    pub(super) fn read(&self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let _gate = lock(&self.gate);
        {
            let mut request = lock(&self.state.state);
            if request.cancelled || request.stopped {
                return Err(io::ErrorKind::Interrupted.into());
            }
            request.buffer = bytes.as_mut_ptr();
            request.count = bytes.len().min(u32::MAX as usize);
            request.result = None;
            request.requested = true;
            self.state.complete.reset();
            self.state.request.set();
        }
        if self.state.complete.wait().is_err() {
            self.cancel();
            // A private event cannot fail normally. Returning could expose a
            // borrowed buffer while the synchronous worker still writes to it.
            std::process::abort();
        }
        let mut request = lock(&self.state.state);
        request.buffer = ptr::null_mut();
        let result = request
            .result
            .take()
            .expect("synchronous worker acknowledged its request");
        if result.bytes != 0 {
            return Ok(result.bytes);
        }
        if result.code == 0 || eof(result.code) {
            Ok(0)
        } else {
            Err(error(result.code))
        }
    }
    fn cancel(&self) {
        let mut request = lock(&self.state.state);
        request.cancelled = true;
        if request.active {
            if let Some(worker) = &self.worker {
                unsafe {
                    CancelSynchronousIo(worker.as_handle().as_raw_handle());
                }
            }
        }
    }
}
impl Drop for NativeSynchronousInput {
    fn drop(&mut self) {
        self.cancel();
        lock(&self.state.state).stopped = true;
        self.state.request.set();
        if self
            .worker
            .take()
            .is_some_and(|worker| worker.join().is_err())
        {
            std::process::abort();
        }
    }
}

static NEXT_PIPE: AtomicU64 = AtomicU64::new(1);
fn pipe_name() -> io::Result<[u16; 80]> {
    let sequence = NEXT_PIPE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| io::Error::other("native pipe identities exhausted"))?;
    let mut name = [0u16; 80];
    let mut end = 0;
    for byte in b"\\\\.\\pipe\\fish." {
        name[end] = *byte as u16;
        end += 1;
    }
    for number in [unsafe { GetCurrentProcessId() } as u64, sequence] {
        let mut digits = [0u16; 20];
        let mut start = digits.len();
        let mut number = number;
        loop {
            start -= 1;
            digits[start] = b'0' as u16 + (number % 10) as u16;
            number /= 10;
            if number == 0 {
                break;
            }
        }
        let count = digits.len() - start;
        name[end..end + count].copy_from_slice(&digits[start..]);
        end += count;
        name[end] = b'.' as u16;
        end += 1;
    }
    Ok(name)
}

fn named_pipe(server_reads: bool) -> io::Result<(File, File)> {
    let name = pipe_name()?;
    let mode = if server_reads { 1 } else { 2 };
    let server = own(unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            mode | 0x4000_0000 | 0x0008_0000,
            8,
            1,
            65536,
            65536,
            0,
            ptr::null(),
        )
    })?;
    let client = own(unsafe {
        CreateFileW(
            name.as_ptr(),
            if server_reads {
                0x4000_0000
            } else {
                0x8000_0000
            },
            0,
            ptr::null(),
            3,
            0x80,
            ptr::null_mut(),
        )
    })?;
    let mut overlapped = Overlapped::new();
    if unsafe { ConnectNamedPipe(server.as_raw_handle(), &mut overlapped) } == 0 {
        let code = last_error();
        if code == IO_PENDING {
            let mut transferred = 0;
            if unsafe {
                GetOverlappedResult(server.as_raw_handle(), &mut overlapped, &mut transferred, 1)
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
        } else if code != 535 {
            return Err(error(code));
        }
    }
    let mut client_pid = 0;
    if unsafe { GetNamedPipeClientProcessId(server.as_raw_handle(), &mut client_pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if client_pid != unsafe { GetCurrentProcessId() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native pipe connected to an unexpected process",
        ));
    }
    Ok((File::from(server), File::from(client)))
}

pub(crate) fn native_synchronous_pipe() -> io::Result<(Arc<NativeEndpoint>, Arc<NativeEndpoint>)> {
    let mut read = ptr::null_mut();
    let mut write = ptr::null_mut();
    if unsafe { CreatePipe(&mut read, &mut write, ptr::null(), 65536) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let read = own(read)?;
    let write = own(write)?;
    Ok((
        NativeEndpoint::from_file(File::from(read), NativeIoDirection::Input),
        NativeEndpoint::from_file(File::from(write), NativeIoDirection::Output),
    ))
}
pub(crate) fn native_input_pipe() -> io::Result<(Arc<NativeEndpoint>, Arc<NativeEndpoint>)> {
    let (read, write) = named_pipe(true)?;
    Ok((
        NativeEndpoint::from_overlapped(read, NativeIoDirection::Input),
        NativeEndpoint::from_file(write, NativeIoDirection::Output),
    ))
}
pub(crate) fn native_output_pipe() -> io::Result<(Arc<NativeEndpoint>, Arc<NativeEndpoint>)> {
    let (write, read) = named_pipe(false)?;
    Ok((
        NativeEndpoint::from_file(read, NativeIoDirection::Input),
        NativeEndpoint::from_overlapped(write, NativeIoDirection::Output),
    ))
}

struct CaptureControl {
    stopped: bool,
    closed: bool,
    pending: bool,
    eof: bool,
    cancelled: bool,
    last_transfer: usize,
    error: Option<io::Error>,
}
struct CaptureState {
    file: RawHandle,
    io: UnsafeCell<*mut c_void>,
    overlapped: UnsafeCell<Overlapped>,
    slab: UnsafeCell<Vec<u8>>,
    idle: Event,
    control: Mutex<CaptureControl>,
    buffer: IoBuffer,
    poster: NativePoster,
    notified: std::sync::atomic::AtomicBool,
}
// The reader owner drains callbacks before closing file/io/context. Control lock
// serializes stop, submission, slab reuse and closing; completion owns slab access.
unsafe impl Send for CaptureState {}
unsafe impl Sync for CaptureState {}
impl CaptureState {
    fn notify(&self) {
        if !self.notified.swap(true, Ordering::AcqRel) {
            self.poster.post(NativeCompletionPayload::Ready);
        }
    }
    fn start(&self, control: &mut CaptureControl, count: usize) {
        if control.closed || control.pending || control.eof {
            return;
        }
        unsafe {
            *self.overlapped.get() = Overlapped::new();
        }
        self.idle.reset();
        control.pending = true;
        let io = unsafe { *self.io.get() };
        unsafe {
            StartThreadpoolIo(io);
        }
        let result = unsafe {
            ReadFile(
                self.file,
                (&mut *self.slab.get()).as_mut_ptr(),
                count as u32,
                ptr::null_mut(),
                self.overlapped.get().cast(),
            )
        };
        if result == 0 {
            let code = last_error();
            if code != IO_PENDING {
                unsafe {
                    CancelThreadpoolIo(io);
                }
                control.pending = false;
                control.eof = eof(code);
                if !control.eof && code != OPERATION_ABORTED {
                    control.error.get_or_insert_with(|| error(code));
                }
                control.last_transfer = 0;
                self.idle.set();
                self.notify();
            }
        }
    }
    fn request_stop(&self, cancelled: bool) {
        let mut control = lock(&self.control);
        control.cancelled |= cancelled;
        control.stopped = true;
        if !control.closed && control.pending {
            if unsafe { CancelIoEx(self.file, self.overlapped.get()) } == 0 {
                let code = last_error();
                if code != 1168 {
                    control.error.get_or_insert_with(|| error(code));
                }
            }
        }
    }
    fn pause_and_drain(&self) -> io::Result<()> {
        self.request_stop(false);
        self.idle.wait()?;
        unsafe {
            WaitForThreadpoolIoCallbacks(*self.io.get(), 0);
        }
        // Snapshot once: an inherited writer may stay alive or keep producing forever.
        let mut available = 0;
        if unsafe {
            PeekNamedPipe(
                self.file,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                &mut available,
                ptr::null_mut(),
            )
        } == 0
        {
            let code = last_error();
            if eof(code) {
                return Ok(());
            }
            return Err(error(code));
        }
        let mut remaining = available as usize;
        while remaining != 0 {
            {
                let mut control = lock(&self.control);
                if control.eof {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                control.last_transfer = 0;
                self.start(&mut control, remaining.min(SLAB));
            }
            self.idle.wait()?;
            unsafe {
                WaitForThreadpoolIoCallbacks(*self.io.get(), 0);
            }
            let control = lock(&self.control);
            let count = control.last_transfer;
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "native capture snapshot ended before its available bytes",
                ));
            }
            remaining -= count;
        }
        Ok(())
    }
}
unsafe extern "system" fn capture_callback(
    _: *mut c_void,
    context: *mut c_void,
    _: *mut c_void,
    code: u32,
    bytes: usize,
    _: *mut c_void,
) {
    let weak = unsafe { &*context.cast::<Weak<CaptureState>>() };
    let Some(state) = weak.upgrade() else {
        std::process::abort()
    };
    let mut control = lock(&state.control);
    if bytes > SLAB {
        std::process::abort();
    }
    control.pending = false;
    control.last_transfer = bytes;
    if bytes != 0 {
        let slab = unsafe { &*state.slab.get() };
        state
            .buffer
            .append(&slab[..bytes], super::SeparationType::Inferred);
    }
    if eof(code) || (code == 0 && bytes == 0) {
        control.eof = true;
    } else if code != 0 && code != OPERATION_ABORTED {
        control.error.get_or_insert_with(|| error(code));
        control.stopped = true;
    }
    state.notify();
    if !control.stopped && !control.eof {
        state.start(&mut control, SLAB);
    } else {
        state.idle.set();
    }
}

struct CaptureReader {
    state: Arc<CaptureState>,
    _file: File,
    io: *mut c_void,
    _context: Box<Weak<CaptureState>>,
    source: NativeSource,
    closed: bool,
}
// No callback owns this wrapper; any dropping thread can cancel and synchronously drain.
unsafe impl Send for CaptureReader {}
impl CaptureReader {
    fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.state.request_stop(false);
        self.state.idle.wait()?;
        unsafe {
            WaitForThreadpoolIoCallbacks(self.io, 0);
        }
        lock(&self.state.control).closed = true;
        unsafe {
            CloseThreadpoolIo(self.io);
            *self.state.io.get() = ptr::null_mut();
        }
        self.closed = true;
        self.source.remove_and_drain()?;
        Ok(())
    }
}
impl Drop for CaptureReader {
    fn drop(&mut self) {
        if self.close().is_err() {
            std::process::abort();
        }
    }
}

pub struct NativeCaptureResult {
    pub buffer: SeparatedBuffer,
    pub interrupted: bool,
    pub error: Option<io::Error>,
}
pub struct NativeCapture {
    writer: File,
    state: Arc<CaptureState>,
    reader: Mutex<Option<CaptureReader>>,
}
impl NativeCapture {
    pub fn new(limit: usize, runtime: Arc<NativeRuntime>) -> io::Result<Arc<Self>> {
        let (file, writer) = named_pipe(true)?;
        let source = runtime.register_source()?;
        let state = Arc::new(CaptureState {
            file: file.as_raw_handle(),
            io: UnsafeCell::new(ptr::null_mut()),
            overlapped: UnsafeCell::new(Overlapped::new()),
            slab: UnsafeCell::new(vec![0; SLAB]),
            idle: Event::new(true, true)?,
            control: Mutex::new(CaptureControl {
                stopped: false,
                closed: false,
                pending: false,
                eof: false,
                cancelled: false,
                last_transfer: 0,
                error: None,
            }),
            buffer: IoBuffer::new_bounded(limit),
            poster: source.poster(),
            notified: std::sync::atomic::AtomicBool::new(false),
        });
        let mut context = Box::new(Arc::downgrade(&state));
        let io = unsafe {
            CreateThreadpoolIo(
                file.as_raw_handle(),
                capture_callback,
                (&mut *context as *mut Weak<CaptureState>).cast(),
                ptr::null(),
            )
        };
        if io.is_null() {
            return Err(io::Error::last_os_error());
        }
        unsafe {
            *state.io.get() = io;
        }
        let reader = CaptureReader {
            state: Arc::clone(&state),
            _file: file,
            io,
            _context: context,
            source,
            closed: false,
        };
        state.start(&mut lock(&state.control), SLAB);
        Ok(Arc::new(Self {
            writer,
            state,
            reader: Mutex::new(Some(reader)),
        }))
    }
    pub fn writer_file(&self) -> io::Result<File> {
        if lock(&self.reader).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "native capture already finished",
            ));
        }
        self.writer.try_clone()
    }
    pub(crate) fn writer_handle(&self) -> BorrowedHandle<'_> {
        self.writer.as_handle()
    }
    pub fn buffer(&self) -> IoBuffer {
        self.state.buffer.clone()
    }
    pub fn io_chain(self: &Arc<Self>) -> IoChain {
        let mut chain = IoChain::new();
        chain.set_endpoint(1, NativeEndpoint::capture(Arc::clone(self)), false);
        chain
    }
    pub fn cancel(&self) {
        self.state.request_stop(true);
    }
    pub(crate) fn synchronize(&self) -> io::Result<()> {
        let reader = lock(&self.reader);
        if reader.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "native capture already finished",
            ));
        }
        self.state.pause_and_drain()?;
        let mut control = lock(&self.state.control);
        if !control.cancelled && !control.eof && control.error.is_none() {
            control.stopped = false;
            self.state.start(&mut control, SLAB);
        }
        Ok(())
    }
    pub fn finish(&self) -> io::Result<NativeCaptureResult> {
        let mut reader = lock(&self.reader).take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "native capture can only finish once",
            )
        })?;
        let drain_error = self.state.pause_and_drain().err();
        let close_error = reader.close().err();
        drop(reader);
        let mut control = lock(&self.state.control);
        let error = control.error.take().or(drain_error).or(close_error);
        Ok(NativeCaptureResult {
            buffer: self.state.buffer.take_buffer(),
            interrupted: control.cancelled,
            error,
        })
    }
}
