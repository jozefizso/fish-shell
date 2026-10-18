//! Native input handles with endpoint-owned buffering. Logical descriptors live in `io`.
use std::io::{self, Read};
use std::ops::Deref;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, RawHandle};
use std::sync::{Arc, Mutex, MutexGuard};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(kind: u32) -> RawHandle;
    fn GetConsoleMode(handle: RawHandle, mode: *mut u32) -> i32;
    fn ReadFile(
        handle: RawHandle,
        buffer: *mut u8,
        count: u32,
        read: *mut u32,
        overlapped: *mut std::ffi::c_void,
    ) -> i32;
    fn ReadConsoleW(
        handle: RawHandle,
        buffer: *mut u16,
        count: u32,
        read: *mut u32,
        control: *mut std::ffi::c_void,
    ) -> i32;
}

pub(crate) fn standard_handle(kind: u32) -> io::Result<BorrowedHandle<'static>> {
    let raw = unsafe { GetStdHandle(kind) };
    if raw.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "standard handle is unavailable",
        ));
    }
    if raw == (-1isize) as RawHandle {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { BorrowedHandle::borrow_raw(raw) })
}

pub(crate) fn is_console(handle: BorrowedHandle<'_>) -> bool {
    let mut mode = 0;
    unsafe { GetConsoleMode(handle.as_raw_handle(), &mut mode) != 0 }
}

#[derive(Clone)]
enum InputHandle<'a> {
    Borrowed {
        handle: BorrowedHandle<'a>,
        pending: Arc<Mutex<NativeInputBuffer>>,
    },
    File {
        file: Arc<std::fs::File>,
        pending: Arc<Mutex<NativeInputBuffer>>,
    },
    Endpoint(Arc<crate::io::NativeEndpoint>),
}

impl InputHandle<'_> {
    fn native_handle(&self) -> Option<BorrowedHandle<'_>> {
        match self {
            Self::Borrowed { handle, .. } => Some(*handle),
            Self::File { file, .. } => Some(file.as_handle()),
            Self::Endpoint(endpoint) => endpoint.native_handle(),
        }
    }
    fn pending(&self) -> &Mutex<NativeInputBuffer> {
        match self {
            Self::Borrowed { pending, .. } | Self::File { pending, .. } => pending,
            Self::Endpoint(endpoint) => endpoint.input_buffer(),
        }
    }

    fn read(&self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Self::Endpoint(endpoint) = self {
            return endpoint.read(buffer);
        }
        let handle = self.native_handle().expect("native input handle");
        let mut count = 0;
        if unsafe {
            ReadFile(
                handle.as_raw_handle(),
                buffer.as_mut_ptr(),
                buffer.len().min(u32::MAX as usize) as u32,
                &mut count,
                std::ptr::null_mut(),
            )
        } == 0
        {
            let error = io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(109 | 232 | 233)) {
                return Ok(0);
            }
            return Err(error);
        }
        Ok(count as usize)
    }
}

#[derive(Default)]
pub(crate) struct NativeInputBuffer {
    bytes: Vec<u8>,
    start: usize,
    end: usize,
    error: Option<Arc<io::Error>>,
    peeked: bool,
}

impl NativeInputBuffer {
    pub(crate) fn has_pending(&self) -> bool {
        self.start < self.end && !self.peeked
    }
    pub(crate) fn take_pending(&mut self) -> Vec<u8> {
        if self.peeked {
            self.discard_peeked();
            return Vec::new();
        }
        if self.start == self.end {
            return Vec::new();
        }
        let mut bytes = std::mem::take(&mut self.bytes);
        bytes.truncate(self.end);
        bytes.drain(..self.start);
        self.start = 0;
        self.end = 0;
        bytes
    }
    pub(crate) fn discard_peeked(&mut self) {
        if self.peeked {
            self.start = 0;
            self.end = 0;
            self.peeked = false;
        }
    }
    pub(crate) fn restore_pending(&mut self, bytes: Vec<u8>) {
        self.start = 0;
        self.end = bytes.len();
        self.peeked = false;
        if !bytes.is_empty() {
            self.bytes = bytes;
        }
    }
}

/// A peek owns the buffer lock; a slice never outlives the shared endpoint storage.
pub struct NativeInputPeek<'a> {
    guard: MutexGuard<'a, NativeInputBuffer>,
    end: usize,
}

impl Deref for NativeInputPeek<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.guard.bytes[self.guard.start..self.end]
    }
}

/// Clones and successive builtin adapters share unconsumed bytes, including console UTF-8.
#[derive(Clone)]
pub struct BorrowedInput<'a> {
    handle: InputHandle<'a>,
    chunk_size: usize,
}

pub type BorrowedFdFile = BorrowedInput<'static>;

impl<'a> BorrowedInput<'a> {
    pub fn from_handle(handle: BorrowedHandle<'a>) -> Self {
        Self {
            handle: InputHandle::Borrowed {
                handle,
                pending: Arc::new(Mutex::new(NativeInputBuffer::default())),
            },
            chunk_size: 4096,
        }
    }

    pub fn is_console(&self) -> bool {
        self.handle.native_handle().is_some_and(is_console)
    }

    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size.clamp(1, 16384);
        self
    }

    pub fn take_error(&self) -> Option<Arc<io::Error>> {
        self.handle.pending().lock().unwrap().error.take()
    }

    fn fill_pending(&self, pending: &mut NativeInputBuffer) -> io::Result<()> {
        if pending.start < pending.end {
            return Ok(());
        }
        if pending.bytes.len() < 16384 {
            pending.bytes.resize(16384, 0);
        }
        pending.start = 0;
        pending.end = 0;
        pending.peeked = false;
        let result = if self.is_console() {
            self.read_console(&mut pending.bytes)
        } else {
            match &self.handle {
                InputHandle::Endpoint(endpoint) => {
                    match endpoint.peek_input(&mut pending.bytes[..self.chunk_size]) {
                        Ok(Some(count)) => {
                            pending.peeked = true;
                            Ok(count)
                        }
                        Ok(None) => self.handle.read(&mut pending.bytes[..self.chunk_size]),
                        Err(error) => Err(error),
                    }
                }
                _ => self.handle.read(&mut pending.bytes[..self.chunk_size]),
            }
        };
        match result {
            Ok(count) => {
                pending.end = count;
                Ok(())
            }
            Err(error) => Err(Self::record_error(pending, error)),
        }
    }

    fn consume_peeked(&self, pending: &mut NativeInputBuffer, count: usize) -> io::Result<()> {
        if !pending.peeked {
            return Ok(());
        }
        let mut consumed = 0;
        while consumed < count {
            let start = pending.start + consumed;
            let end = pending.start + count;
            match self.handle.read(&mut pending.bytes[start..end]) {
                Ok(0) => {
                    return Err(Self::record_error(
                        pending,
                        io::ErrorKind::UnexpectedEof.into(),
                    ));
                }
                Ok(count) => consumed += count,
                Err(error) => return Err(Self::record_error(pending, error)),
            }
        }
        Ok(())
    }

    fn record_error(pending: &mut NativeInputBuffer, error: io::Error) -> io::Error {
        let error = Arc::new(error);
        let returned = io::Error::new(error.kind(), Arc::clone(&error));
        pending.error = Some(error);
        returned
    }

    fn read_console(&self, bytes: &mut [u8]) -> io::Result<usize> {
        let handle = self
            .handle
            .native_handle()
            .expect("console has a native handle");
        let mut wide = [0u16; 4097];
        let mut count = 0;
        if unsafe {
            ReadConsoleW(
                handle.as_raw_handle(),
                wide.as_mut_ptr(),
                4096,
                &mut count,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if count > 0 && (0xd800..=0xdbff).contains(&wide[count as usize - 1]) {
            let mut extra = 0;
            if unsafe {
                ReadConsoleW(
                    handle.as_raw_handle(),
                    wide[count as usize..].as_mut_ptr(),
                    1,
                    &mut extra,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            count += extra;
        }
        let mut end = 0;
        for ch in char::decode_utf16(wide[..count as usize].iter().copied()) {
            let ch = ch.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            end += ch.encode_utf8(&mut bytes[end..]).len();
        }
        Ok(end)
    }

    pub fn fill_buf(&mut self) -> io::Result<NativeInputPeek<'_>> {
        let mut pending = self.handle.pending().lock().unwrap();
        self.fill_pending(&mut pending)?;
        let end = pending
            .end
            .min(pending.start.saturating_add(self.chunk_size));
        Ok(NativeInputPeek {
            guard: pending,
            end,
        })
    }

    pub fn read_until(&mut self, delimiter: u8, output: &mut Vec<u8>) -> io::Result<usize> {
        let original = output.len();
        let mut pending = self.handle.pending().lock().unwrap();
        loop {
            self.fill_pending(&mut pending)?;
            if pending.start == pending.end {
                return Ok(output.len() - original);
            }
            let available = &pending.bytes[pending.start..pending.end];
            let found = available.iter().position(|&byte| byte == delimiter);
            let count = found.map_or(available.len(), |index| index + 1);
            self.consume_peeked(&mut pending, count)?;
            output.extend_from_slice(&pending.bytes[pending.start..pending.start + count]);
            pending.start += count;
            if found.is_some() {
                return Ok(output.len() - original);
            }
        }
    }

    /// Unlike `Read::read_to_end`, permanent native cancellation is not retried forever.
    pub fn read_to_end(&mut self, output: &mut Vec<u8>) -> io::Result<usize> {
        let original = output.len();
        let mut pending = self.handle.pending().lock().unwrap();
        loop {
            self.fill_pending(&mut pending)?;
            if pending.start == pending.end {
                return Ok(output.len() - original);
            }
            let count = pending.end - pending.start;
            self.consume_peeked(&mut pending, count)?;
            output.extend_from_slice(&pending.bytes[pending.start..pending.end]);
            pending.start = pending.end;
        }
    }
}

impl BorrowedInput<'static> {
    pub fn from_file(file: Arc<std::fs::File>) -> Self {
        Self {
            handle: InputHandle::File {
                file,
                pending: Arc::new(Mutex::new(NativeInputBuffer::default())),
            },
            chunk_size: 4096,
        }
    }

    pub fn from_endpoint(endpoint: Arc<crate::io::NativeEndpoint>) -> Self {
        Self {
            handle: InputHandle::Endpoint(endpoint),
            chunk_size: 4096,
        }
    }

    pub fn stdin() -> io::Result<Self> {
        let endpoint = crate::io::root_descriptors()?.endpoint(0)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "standard input is closed")
        })?;
        Ok(Self::from_endpoint(endpoint))
    }
}

impl Read for BorrowedInput<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let mut pending = self.handle.pending().lock().unwrap();
        if pending.start == pending.end && !self.is_console() {
            return self
                .handle
                .read(bytes)
                .map_err(|error| Self::record_error(&mut pending, error));
        }
        self.fill_pending(&mut pending)?;
        let count = bytes.len().min(pending.end - pending.start);
        self.consume_peeked(&mut pending, count)?;
        bytes[..count].copy_from_slice(&pending.bytes[pending.start..pending.start + count]);
        pending.start += count;
        Ok(count)
    }
}
