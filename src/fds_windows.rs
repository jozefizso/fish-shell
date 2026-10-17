//! Synchronous Windows standard-stream handles. No Unix descriptors are involved.
use std::io::{self, Read};
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, RawHandle};
use std::sync::Arc;

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

/// Borrow an inherited standard handle without taking responsibility for closing it.
/// The shell does not replace its standard handles during batch evaluation.
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
    Borrowed(BorrowedHandle<'a>),
    File(Arc<std::fs::File>),
}

impl AsHandle for InputHandle<'_> {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        match self {
            Self::Borrowed(handle) => *handle,
            Self::File(file) => file.as_handle(),
        }
    }
}

/// An input adapter borrowing an inherited handle or retaining an owned file redirection.
/// Console reads are UTF-16 text converted to UTF-8; redirected reads are raw bytes.
pub struct BorrowedInput<'a> {
    handle: InputHandle<'a>,
    pending: Vec<u8>,
    pending_offset: usize,
}

impl Clone for BorrowedInput<'_> {
    fn clone(&self) -> Self {
        // Like a second reader of the same OS stream, the clone has independent
        // read buffering. Never duplicate bytes already read into this wrapper.
        Self {
            handle: self.handle.clone(),
            pending: Vec::new(),
            pending_offset: 0,
        }
    }
}

pub type BorrowedFdFile = BorrowedInput<'static>;

impl<'a> BorrowedInput<'a> {
    pub fn from_handle(handle: BorrowedHandle<'a>) -> Self {
        Self {
            handle: InputHandle::Borrowed(handle),
            pending: Vec::new(),
            pending_offset: 0,
        }
    }
}

impl BorrowedInput<'static> {
    pub fn from_file(file: Arc<std::fs::File>) -> Self {
        Self {
            handle: InputHandle::File(file),
            pending: Vec::new(),
            pending_offset: 0,
        }
    }

    pub fn stdin() -> io::Result<Self> {
        standard_handle(-10i32 as u32).map(Self::from_handle)
    }
}

impl AsRawHandle for BorrowedInput<'_> {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle.as_handle().as_raw_handle()
    }
}

impl Read for BorrowedInput<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.pending_offset < self.pending.len() {
            let count = buffer.len().min(self.pending.len() - self.pending_offset);
            buffer[..count]
                .copy_from_slice(&self.pending[self.pending_offset..self.pending_offset + count]);
            self.pending_offset += count;
            return Ok(count);
        }
        if is_console(self.handle.as_handle()) {
            let mut wide = [0u16; 4097];
            let mut count = 0;
            if unsafe {
                ReadConsoleW(
                    self.as_raw_handle(),
                    wide.as_mut_ptr(),
                    4096,
                    &mut count,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if count > 0 && (0xD800..=0xDBFF).contains(&wide[count as usize - 1]) {
                // A console read may end between the UTF-16 code units of one
                // supplementary scalar. Complete it before validating the text.
                let mut extra = 0;
                if unsafe {
                    ReadConsoleW(
                        self.as_raw_handle(),
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
            let text = String::from_utf16(&wide[..count as usize])
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            self.pending = text.into_bytes();
            self.pending_offset = 0;
            let count = buffer.len().min(self.pending.len());
            buffer[..count].copy_from_slice(&self.pending[..count]);
            self.pending_offset = count;
            return Ok(count);
        }
        let mut count = 0;
        if unsafe {
            ReadFile(
                self.as_raw_handle(),
                buffer.as_mut_ptr(),
                buffer.len().min(u32::MAX as usize) as u32,
                &mut count,
                std::ptr::null_mut(),
            )
        } == 0
        {
            let error = io::Error::last_os_error();
            // A pipe whose writers have all closed is EOF, not a transport failure.
            if error.raw_os_error() == Some(109) {
                return Ok(0);
            }
            return Err(error);
        }
        Ok(count as usize)
    }
}
