//! Private kernel ownership for native directory notifications.
use crate::threads::{NativeCompletionPayload, NativePoster};
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::ptr::{self, NonNull};
use std::sync::{Mutex, MutexGuard};

const ERROR_MORE_DATA: i32 = 234;
const ERROR_NOTIFY_ENUM_DIR: i32 = 1022;
const ERROR_IO_PENDING: i32 = 997;
const ERROR_NOT_FOUND: i32 = 1168;
const ERROR_OPERATION_ABORTED: i32 = 995;
// SMB rejects buffers larger than 64 KiB. DWORD storage supplies the alignment
// required by ReadDirectoryChangesW, including on ARM64.
const BUFFER_WORDS: usize = 16 * 1024;

pub(super) fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|poison| poison.into_inner())
}

// A completed directory read with MORE_DATA is not a complete notification
// batch. In particular, an SMB redirector can return this warning instead of
// ENUM_DIR or a successful zero-byte overflow. Never filter its partial records:
// re-arm and invalidate/rescan the entire state instead.
//
// This classification applies only to completion of ReadDirectoryChangesW, not
// to opening/querying a directory or submitting an unsupported notification.
// https://learn.microsoft.com/windows/win32/debug/system-error-codes--0-499-
// https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-readdirectorychangesw
fn lost_detail(code: i32) -> bool {
    matches!(code, ERROR_MORE_DATA | ERROR_NOTIFY_ENUM_DIR)
}

#[repr(C)]
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

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateThreadpoolIo(
        file: RawHandle,
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
    fn WaitForThreadpoolIoCallbacks(io: *mut c_void, cancel: i32);
    fn CloseThreadpoolIo(io: *mut c_void);
    fn ReadDirectoryChangesW(
        directory: RawHandle,
        buffer: *mut c_void,
        length: u32,
        subtree: i32,
        filter: u32,
        returned: *mut u32,
        overlapped: *mut Overlapped,
        completion: *const c_void,
    ) -> i32;
    fn CancelIoEx(file: RawHandle, overlapped: *const Overlapped) -> i32;
    fn GetOverlappedResult(
        file: RawHandle,
        overlapped: *mut Overlapped,
        count: *mut u32,
        wait: i32,
    ) -> i32;
    fn CreateEventW(
        attributes: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> RawHandle;
    fn CompareStringOrdinal(
        first: *const u16,
        first_length: i32,
        second: *const u16,
        second_length: i32,
        ignore_case: i32,
    ) -> i32;
    fn ResetEvent(event: RawHandle) -> i32;
}

#[derive(Clone, Copy)]
struct Completion {
    code: u32,
    bytes: usize,
}
struct Context {
    overlapped: UnsafeCell<Overlapped>,
    buffer: UnsafeCell<Box<[u32; BUFFER_WORDS]>>,
    completion: Mutex<Option<Completion>>,
    poster: NativePoster,
}
// The kernel alone writes these cells while pending. The main-thread owner
// accesses them only after completion or cancellation-and-drain.
unsafe impl Send for Context {}
unsafe impl Sync for Context {}

unsafe extern "system" fn completed(
    _: *mut c_void,
    context: *mut c_void,
    _: *mut c_void,
    code: u32,
    bytes: usize,
    _: *mut c_void,
) {
    let context = unsafe { &*context.cast::<Context>() };
    let mut result = lock(&context.completion);
    *result = Some(Completion { code, bytes });
    // Publication is ordered before main-thread consumption. Callbacks never
    // parse filenames, touch an environment, re-arm I/O or invoke a parser.
    context.poster.post(NativeCompletionPayload::Ready);
}

pub(super) struct DirectoryIo {
    file: OwnedHandle,
    event: OwnedHandle,
    io: NonNull<c_void>,
    context: Box<Context>,
    pending: bool,
    subtree: bool,
}
// All mutation is protected by the watcher's mutex, not by the kernel callback.
unsafe impl Send for DirectoryIo {}

impl DirectoryIo {
    pub(super) fn new(file: OwnedHandle, subtree: bool, poster: NativePoster) -> io::Result<Self> {
        let raw = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let event = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut context = Box::new(Context {
            overlapped: UnsafeCell::new(Overlapped {
                internal: 0,
                internal_high: 0,
                offset: Offset { offsets: [0, 0] },
                event: event.as_raw_handle(),
            }),
            buffer: UnsafeCell::new(Box::new([0; BUFFER_WORDS])),
            completion: Mutex::new(None),
            poster,
        });
        let io = unsafe {
            CreateThreadpoolIo(
                file.as_raw_handle(),
                completed,
                (&mut *context as *mut Context).cast(),
                ptr::null(),
            )
        };
        let io = NonNull::new(io).ok_or_else(io::Error::last_os_error)?;
        let mut result = Self {
            file,
            event,
            io,
            context,
            pending: false,
            subtree,
        };
        result.arm()?;
        Ok(result)
    }

    pub(super) fn arm(&mut self) -> io::Result<()> {
        debug_assert!(!self.pending);
        if unsafe { ResetEvent(self.event.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        unsafe {
            *self.context.overlapped.get() = Overlapped {
                internal: 0,
                internal_high: 0,
                offset: Offset { offsets: [0, 0] },
                event: self.event.as_raw_handle(),
            };
            StartThreadpoolIo(self.io.as_ptr());
        }
        let result = unsafe {
            ReadDirectoryChangesW(
                self.file.as_raw_handle(),
                (*self.context.buffer.get()).as_mut_ptr().cast(),
                (BUFFER_WORDS * 4) as u32,
                i32::from(self.subtree),
                // Names, directory names, attributes, size, last-write,
                // creation and security: same-size writes must not disappear.
                0x0000_015f,
                ptr::null_mut(),
                self.context.overlapped.get(),
                ptr::null(),
            )
        };
        if result == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_IO_PENDING) {
                unsafe { CancelThreadpoolIo(self.io.as_ptr()) };
                return Err(error);
            }
        }
        self.pending = true;
        Ok(())
    }

    /// None is still pending. Empty/malformed or incomplete notification detail
    /// invalidates the whole state, including MORE_DATA and NOTIFY_ENUM_DIR.
    pub(super) fn take(&mut self, filter: Option<&[u16]>) -> Option<io::Result<bool>> {
        let result = lock(&self.context.completion).take()?;
        self.pending = false;
        if lost_detail(result.code as i32) {
            return Some(Ok(true));
        }
        if result.code != 0 {
            return Some(Err(io::Error::from_raw_os_error(result.code as i32)));
        }
        if result.bytes == 0 || result.bytes > BUFFER_WORDS * 4 || filter.is_none() {
            return Some(Ok(true));
        }
        let filter = filter.expect("checked filename filter");
        // The callback has published a completed kernel buffer; no new read is
        // submitted until this borrow ends. Unaligned fields avoid ABI casts.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (*self.context.buffer.get()).as_ptr().cast::<u8>(),
                result.bytes,
            )
        };
        let mut offset = 0;
        loop {
            if bytes.len() - offset < 12 {
                return Some(Ok(true));
            }
            let word = |at| {
                u32::from_ne_bytes(bytes[at..at + 4].try_into().expect("bounded DWORD")) as usize
            };
            let next = word(offset);
            let length = word(offset + 8);
            if !(1..=5).contains(&word(offset + 4)) || length == 0 {
                return Some(Ok(true));
            }
            if length % 2 != 0 || length > bytes.len() - offset - 12 {
                return Some(Ok(true));
            }
            // Directory notifications preserve the spelling used by the writer;
            // compare ordinally without case to include Windows aliases.
            let name = unsafe {
                std::slice::from_raw_parts(
                    bytes.as_ptr().add(offset + 12).cast::<u16>(),
                    length / 2,
                )
            };
            let Ok(filter_length) = i32::try_from(filter.len()) else {
                return Some(Ok(true));
            };
            let comparison = unsafe {
                CompareStringOrdinal(
                    name.as_ptr(),
                    name.len() as i32,
                    filter.as_ptr(),
                    filter_length,
                    1,
                )
            };
            if comparison == 0 || comparison == 2 {
                return Some(Ok(true));
            }
            if next == 0 {
                return Some(Ok(false));
            }
            if next % 4 != 0 || next < 12 + length || next > bytes.len() - offset {
                return Some(Ok(true));
            }
            offset += next;
        }
    }

    pub(super) fn shutdown(&mut self) -> io::Result<()> {
        let mut failure = None;
        if self.pending {
            if unsafe { CancelIoEx(self.file.as_raw_handle(), self.context.overlapped.get()) } == 0
            {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(ERROR_NOT_FOUND) {
                    failure = Some(error);
                }
            }
            let mut count = 0;
            if unsafe {
                GetOverlappedResult(
                    self.file.as_raw_handle(),
                    self.context.overlapped.get(),
                    &mut count,
                    1,
                )
            } == 0
            {
                let error = io::Error::last_os_error();
                // A read may already have completed with incomplete detail
                // before cancellation. Closing it needs no rescan, but still
                // drains its callback below before releasing native storage.
                if error.raw_os_error() != Some(ERROR_OPERATION_ABORTED)
                    && !error.raw_os_error().is_some_and(lost_detail)
                    && failure.is_none()
                {
                    failure = Some(error);
                }
            }
            self.pending = false;
        }
        // Never cancel callbacks instead of draining: their Context and kernel
        // OVERLAPPED/buffer ownership must remain live until every callback exits.
        unsafe { WaitForThreadpoolIoCallbacks(self.io.as_ptr(), 0) };
        lock(&self.context.completion).take();
        failure.map_or(Ok(()), Err)
    }
}
impl Drop for DirectoryIo {
    fn drop(&mut self) {
        let _ = self.shutdown();
        unsafe { CloseThreadpoolIo(self.io.as_ptr()) };
    }
}
