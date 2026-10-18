//! Native streams and typed logical descriptors for the shared evaluator.
use crate::{
    fds::{BorrowedFdFile, is_console, standard_handle},
    prelude::*,
    proc::JobGroupRef,
};
use fish_widestring::bytes2wcstring;
use std::io::{self, Write};
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, OwnedHandle, RawHandle};
use std::sync::Arc;

#[path = "io_buffer.rs"]
mod buffer;
pub use buffer::{BufferElement, BufferedOutputStream, IoBuffer, SeparatedBuffer, SeparationType};
#[path = "io/windows_descriptors.rs"]
mod descriptors;
pub(crate) use descriptors::root_descriptors;
pub use descriptors::{IoChain, NativeChildBinding, NativeEndpoint, NativeIoDirection};
#[path = "io/windows_pipe.rs"]
mod pipe;
pub use pipe::{NativeCapture, NativeCaptureResult};
pub(crate) use pipe::{native_input_pipe, native_output_pipe, native_synchronous_pipe};
#[path = "io/windows_deferred.rs"]
mod deferred;
pub(crate) use deferred::NativeDeferredWriter;

#[repr(C)]
#[derive(Default)]
struct ConsoleScreenBufferInfo {
    size: [i16; 2],
    cursor: [i16; 2],
    attributes: u16,
    window: [i16; 4],
    maximum_size: [i16; 2],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleScreenBufferInfo(handle: RawHandle, info: *mut ConsoleScreenBufferInfo) -> i32;
    fn WriteFile(
        handle: RawHandle,
        buffer: *const u8,
        count: u32,
        written: *mut u32,
        overlapped: *mut std::ffi::c_void,
    ) -> i32;
    fn WriteConsoleW(
        handle: RawHandle,
        buffer: *const u16,
        count: u32,
        written: *mut u32,
        reserved: *mut std::ffi::c_void,
    ) -> i32;
}

enum OutputHandle<'a> {
    Borrowed(BorrowedHandle<'a>),
    Owned(OwnedHandle),
    Endpoint(Arc<NativeEndpoint>),
}

/// A native output endpoint. Borrowed endpoints never close the underlying handle;
/// owned endpoints close it exactly once, using `OwnedHandle`'s normal ownership.
/// Redirected handles carry arbitrary bytes; console writes require complete UTF-8.
pub struct NativeOutput<'a> {
    handle: OutputHandle<'a>,
}

impl<'a> NativeOutput<'a> {
    pub fn borrowed(handle: BorrowedHandle<'a>) -> Self {
        Self {
            handle: OutputHandle::Borrowed(handle),
        }
    }

    pub fn owned(handle: OwnedHandle) -> Self {
        Self {
            handle: OutputHandle::Owned(handle),
        }
    }

    fn handle(&self) -> BorrowedHandle<'_> {
        match &self.handle {
            OutputHandle::Borrowed(handle) => *handle,
            OutputHandle::Owned(handle) => handle.as_handle(),
            OutputHandle::Endpoint(endpoint) => endpoint
                .native_handle()
                .expect("handle-backed output endpoint"),
        }
    }

    pub fn is_console(&self) -> bool {
        is_console(self.handle())
    }

    /// Native cancellation is terminal for this append, not a transient Unix
    /// EINTR. The default Write::write_all would retry it forever.
    fn write_all_cancellable(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let count = self.write(bytes)?;
            if count == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            bytes = &bytes[count..];
        }
        Ok(())
    }
}

impl NativeOutput<'static> {
    pub fn stdout() -> io::Result<Self> {
        standard_handle(-11i32 as u32).map(Self::borrowed)
    }

    pub fn stderr() -> io::Result<Self> {
        standard_handle(-12i32 as u32).map(Self::borrowed)
    }
}

impl Write for NativeOutput<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let OutputHandle::Endpoint(endpoint) = &self.handle {
            return endpoint.write(buffer);
        }
        if buffer.is_empty() {
            return Ok(0);
        }
        if !self.is_console() {
            let mut written = 0;
            if unsafe {
                WriteFile(
                    self.handle().as_raw_handle(),
                    buffer.as_ptr(),
                    buffer.len().min(u32::MAX as usize) as u32,
                    &mut written,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if written == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            return Ok(written as usize);
        }
        let text = std::str::from_utf8(buffer)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        // Bounded stack storage avoids allocating for console transcoding. A chunk
        // always ends between Unicode scalars, never between a surrogate pair.
        let mut chars = text.chars().peekable();
        let mut wide = [0u16; 4096];
        while chars.peek().is_some() {
            let mut count = 0;
            while let Some(&ch) = chars.peek() {
                if count + ch.len_utf16() > wide.len() {
                    break;
                }
                ch.encode_utf16(&mut wide[count..]);
                count += ch.len_utf16();
                chars.next();
            }
            let mut offset = 0;
            while offset < count {
                let mut written = 0;
                if unsafe {
                    WriteConsoleW(
                        self.handle().as_raw_handle(),
                        wide[offset..].as_ptr(),
                        (count - offset) as u32,
                        &mut written,
                        std::ptr::null_mut(),
                    )
                } == 0
                {
                    return Err(io::Error::last_os_error());
                }
                if written == 0 {
                    return Err(io::ErrorKind::WriteZero.into());
                }
                offset += written as usize;
            }
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // WriteFile/WriteConsoleW above are synchronous and this wrapper buffers
        // nothing. FlushFileBuffers would incorrectly require disk/pipe semantics
        // (and can wait for a pipe reader); it is not a stream flush operation.
        Ok(())
    }
}

/// Flush Rust's standard-stream buffers before handing inherited streams to a child.
pub fn flush_standard_streams() -> io::Result<()> {
    let out = std::io::stdout().flush();
    let err = std::io::stderr().flush();
    out.and(err)
}

pub enum OutputStream {
    /// Intentional discard sink, used when a caller explicitly does not want output.
    Null,
    Fd(FdOutputStream),
    String(StringOutputStream),
    Buffered(BufferedOutputStream),
}

impl OutputStream {
    pub fn stdout() -> Self {
        Self::Fd(FdOutputStream::new(1))
    }
    pub fn stderr() -> Self {
        Self::Fd(FdOutputStream::new(2))
    }
    pub fn contents(&self) -> &wstr {
        match self {
            Self::String(stream) => &stream.contents,
            _ => L!(""),
        }
    }
    pub fn take(self) -> WString {
        match self {
            Self::String(stream) => stream.contents,
            _ => WString::new(),
        }
    }
    pub fn flush_and_check_error(&mut self) -> libc::c_int {
        match self {
            Self::Fd(stream) => stream.flush_and_check_error(),
            Self::Buffered(stream) => stream.flush_and_check_error(),
            _ => 0,
        }
    }
    pub fn append(&mut self, s: impl IntoCharIter) -> bool {
        match self {
            Self::Null => true,
            Self::Fd(stream) => stream.append(s),
            Self::String(stream) => stream.append(s),
            Self::Buffered(stream) => stream.append(s),
        }
    }
    /// Append explicitly byte-oriented output, without reinterpreting text PUA.
    pub fn append_bytes(&mut self, bytes: &[u8]) -> bool {
        match self {
            Self::Null => true,
            Self::Fd(stream) => stream.append_bytes(bytes),
            Self::String(stream) => stream.append(&bytes2wcstring(bytes)),
            Self::Buffered(stream) => stream.buffer.append(bytes, SeparationType::Inferred),
        }
    }
    pub fn appendln(&mut self, s: impl IntoCharIter) -> bool {
        self.append(s.chars().chain(std::iter::once('\n')))
    }
    pub fn append_with_separation(
        &mut self,
        s: impl IntoCharIter,
        typ: SeparationType,
        want_newline: bool,
    ) -> bool {
        if let Self::Buffered(stream) = self {
            return stream.append_with_separation(s, typ, want_newline);
        }
        if typ == SeparationType::Explicitly && want_newline {
            self.appendln(s)
        } else {
            self.append(s)
        }
    }
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Fd(stream) if stream.error.is_none() && stream.console)
    }
}

/// Compatibility name for the builtin API. `new` accepts only standard-stream
/// indices, never Unix descriptors and never integer representations of HANDLEs.
pub struct FdOutputStream {
    endpoint: Option<NativeOutput<'static>>,
    error: Option<io::Error>,
    console: bool,
    // Numeric escapes may append one byte at a time. A console write must see
    // complete UTF-8, while redirected output must remain byte-transparent.
    pending_utf8: [u8; 4],
    pending_len: usize,
}
impl FdOutputStream {
    pub fn new(stream: i32) -> Self {
        let endpoint = match stream {
            1 => NativeOutput::stdout(),
            2 => NativeOutput::stderr(),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "only inherited standard output streams are supported",
            )),
        };
        match endpoint {
            Ok(endpoint) => Self {
                console: endpoint.is_console(),
                endpoint: Some(endpoint),
                error: None,
                pending_utf8: [0; 4],
                pending_len: 0,
            },
            Err(error) => Self {
                console: false,
                endpoint: None,
                error: Some(error),
                pending_utf8: [0; 4],
                pending_len: 0,
            },
        }
    }
    fn from_endpoint(endpoint: Arc<NativeEndpoint>) -> Self {
        let console = endpoint.native_handle().is_some_and(is_console);
        Self {
            endpoint: Some(NativeOutput {
                handle: OutputHandle::Endpoint(endpoint),
            }),
            error: None,
            console,
            pending_utf8: [0; 4],
            pending_len: 0,
        }
    }
    fn unavailable() -> Self {
        Self {
            endpoint: None,
            error: None,
            console: false,
            pending_utf8: [0; 4],
            pending_len: 0,
        }
    }
    pub fn error(&self) -> Option<&io::Error> {
        self.error.as_ref()
    }
    fn append(&mut self, s: impl IntoCharIter) -> bool {
        if self.error.is_some() {
            return false;
        }
        let mut buffer = [0u8; 4096];
        let mut used = 0;
        for ch in s.chars() {
            let byte = fish_widestring::decode_byte_from_char(ch);
            let length = byte.map_or_else(|| ch.len_utf8(), |_| 1);
            if used + length > buffer.len() {
                if !self.append_bytes(&buffer[..used]) {
                    return false;
                }
                used = 0;
            }
            if let Some(byte) = byte {
                buffer[used] = byte;
            } else {
                ch.encode_utf8(&mut buffer[used..]);
            }
            used += length;
        }
        self.append_bytes(&buffer[..used])
    }

    pub fn append_bytes(&mut self, mut bytes: &[u8]) -> bool {
        if self.error.is_some() {
            return false;
        }
        if bytes.is_empty() {
            return true;
        }
        let Some(endpoint) = self.endpoint.as_mut() else {
            self.error = Some(io::Error::from_raw_os_error(6));
            return false;
        };
        if !self.console {
            if let Err(error) = endpoint.write_all_cancellable(bytes) {
                self.error = Some(error);
            }
            return self.error.is_none();
        }
        if self.pending_len != 0 {
            let required = utf8_sequence_len(self.pending_utf8[0]);
            let count = (required - self.pending_len).min(bytes.len());
            self.pending_utf8[self.pending_len..self.pending_len + count]
                .copy_from_slice(&bytes[..count]);
            self.pending_len += count;
            bytes = &bytes[count..];
            if self.pending_len < required {
                return true;
            }
            if let Err(error) = endpoint.write_all_cancellable(&self.pending_utf8[..required]) {
                self.error = Some(error);
                return false;
            }
            self.pending_len = 0;
        }
        // Keep only a potentially incomplete final scalar. NativeOutput still
        // validates every complete sequence, including invalid continuation,
        // overlong and surrogate encodings; this never substitutes bad bytes.
        let mut complete = bytes.len();
        if !bytes.is_empty() {
            let mut start = bytes.len() - 1;
            while start > 0 && bytes[start] & 0xc0 == 0x80 && bytes.len() - start < 4 {
                start -= 1;
            }
            if bytes.len() - start < utf8_sequence_len(bytes[start]) {
                complete = start;
            }
        }
        if let Err(error) = endpoint.write_all_cancellable(&bytes[..complete]) {
            self.error = Some(error);
            return false;
        }
        self.pending_len = bytes.len() - complete;
        self.pending_utf8[..self.pending_len].copy_from_slice(&bytes[complete..]);
        true
    }
    fn flush_and_check_error(&mut self) -> libc::c_int {
        if self.error.is_none() {
            if let Some(endpoint) = &mut self.endpoint {
                if self.pending_len != 0 {
                    if let Err(error) =
                        endpoint.write_all_cancellable(&self.pending_utf8[..self.pending_len])
                    {
                        self.error = Some(error);
                    }
                    self.pending_len = 0;
                }
                if self.error.is_none() {
                    if let Err(error) = endpoint.flush() {
                        self.error = Some(error);
                    }
                }
            }
        }
        i32::from(self.error.is_some())
    }
}

fn utf8_sequence_len(lead: u8) -> usize {
    match lead {
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 1,
    }
}

#[derive(Default)]
pub struct StringOutputStream {
    contents: WString,
}
impl StringOutputStream {
    pub fn new() -> Self {
        Self::default()
    }
    fn append(&mut self, s: impl IntoCharIter) -> bool {
        if !s.extend_wstring(&mut self.contents) {
            self.contents.extend(s.chars());
        }
        true
    }
}

pub struct IoStreams<'a> {
    pub out: &'a mut OutputStream,
    pub err: &'a mut OutputStream,
    pub stdin_file: Option<BorrowedFdFile>,
    pub stdin_is_directly_redirected: bool,
    pub out_is_piped: bool,
    pub err_is_piped: bool,
    pub out_is_redirected: bool,
    pub err_is_redirected: bool,
    pub io_chain: &'a IoChain,
    pub job_group: Option<JobGroupRef>,
}
impl<'a> IoStreams<'a> {
    pub fn new(
        out: &'a mut OutputStream,
        err: &'a mut OutputStream,
        io_chain: &'a IoChain,
    ) -> Self {
        let (out_is_redirected, out_is_piped) = io_chain.flags(1);
        let (err_is_redirected, err_is_piped) = io_chain.flags(2);
        Self {
            out,
            err,
            stdin_file: io_chain.input_reader(),
            stdin_is_directly_redirected: false,
            out_is_piped,
            err_is_piped,
            out_is_redirected,
            err_is_redirected,
            io_chain,
            job_group: None,
        }
    }
    pub fn out_is_terminal(&self) -> bool {
        self.out.is_terminal()
    }
    pub(crate) fn output_width(&self) -> Option<usize> {
        let endpoint = self.io_chain.endpoint(1).ok().flatten()?;
        let handle = endpoint.native_handle()?;
        let mut info = ConsoleScreenBufferInfo::default();
        if unsafe { GetConsoleScreenBufferInfo(handle.as_raw_handle(), &mut info) } == 0 {
            return None;
        }
        let width = i32::from(info.window[2]) - i32::from(info.window[0]) + 1;
        usize::try_from(width).ok().filter(|width| *width > 0)
    }
    pub fn is_terminal(&self, fd: i64) -> bool {
        match fd {
            0 => self
                .stdin_file
                .as_ref()
                .is_some_and(BorrowedFdFile::is_console),
            1 => self.out.is_terminal(),
            2 => self.err.is_terminal(),
            _ => i32::try_from(fd)
                .ok()
                .and_then(|fd| self.io_chain.endpoint(fd).ok().flatten())
                .and_then(|endpoint| endpoint.native_handle().map(is_console))
                .unwrap_or(false),
        }
    }
    pub fn is_stdin_closed(&self) -> bool {
        self.stdin_file.is_none()
    }
}
