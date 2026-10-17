//! Logical descriptor bindings; native handles never double as descriptor numbers.
use super::{
    BufferedOutputStream, FdOutputStream, IoBuffer, NativeCapture, NativeOutput, OutputStream,
    SeparatedBuffer,
};
use crate::env::Environment;
use crate::fds::{BorrowedFdFile, NativeInputBuffer, standard_handle};
use crate::prelude::*;
use crate::redirection::{RedirectionMode, RedirectionSpecList};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::windows::io::{AsHandle, BorrowedHandle, OwnedHandle};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NativeIoDirection {
    Input,
    Output,
    InOut,
}

#[derive(Clone)]
pub(super) enum NativeHandle {
    Borrowed(BorrowedHandle<'static>),
    File(Arc<File>),
    Owned(Arc<OwnedHandle>),
}
impl AsHandle for NativeHandle {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        match self {
            Self::Borrowed(handle) => *handle,
            Self::File(file) => file.as_handle(),
            Self::Owned(handle) => handle.as_handle(),
        }
    }
}

enum EndpointKind {
    Native(NativeHandle),
    Capture(Arc<NativeCapture>),
    Buffer(IoBuffer),
    Memory(Mutex<Option<MemoryInput>>),
}

struct MemoryInput {
    buffer: SeparatedBuffer,
    element: usize,
    offset: usize,
}
impl MemoryInput {
    fn read(&mut self, output: &mut [u8]) -> usize {
        let mut written = 0;
        while written < output.len() {
            let Some(element) = self.buffer.elements().get(self.element) else {
                break;
            };
            let count = (element.contents.len() - self.offset).min(output.len() - written);
            output[written..written + count]
                .copy_from_slice(&element.contents[self.offset..self.offset + count]);
            written += count;
            self.offset += count;
            if self.offset != element.contents.len() {
                break;
            }
            if element.is_explicitly_separated() {
                if written == output.len() {
                    break;
                }
                output[written] = b'\n';
                written += 1;
            }
            self.element += 1;
            self.offset = 0;
        }
        written
    }
}

/// A shared endpoint also retains its input cursor across successive builtin adapters.
pub struct NativeEndpoint {
    kind: EndpointKind,
    direction: NativeIoDirection,
    overlapped: bool,
    input: Mutex<NativeInputBuffer>,
    async_io: OnceLock<Result<super::pipe::NativeOverlappedFile, StoredError>>,
    sync_input: OnceLock<Result<super::pipe::NativeSynchronousInput, StoredError>>,
}

impl NativeEndpoint {
    fn new(kind: EndpointKind, direction: NativeIoDirection, overlapped: bool) -> Arc<Self> {
        Arc::new(Self {
            kind,
            direction,
            overlapped,
            input: Mutex::new(NativeInputBuffer::default()),
            async_io: OnceLock::new(),
            sync_input: OnceLock::new(),
        })
    }
    pub fn from_file(file: File, direction: NativeIoDirection) -> Arc<Self> {
        Self::new(
            EndpointKind::Native(NativeHandle::File(Arc::new(file))),
            direction,
            false,
        )
    }
    pub(crate) fn from_overlapped(file: File, direction: NativeIoDirection) -> Arc<Self> {
        Self::new(
            EndpointKind::Native(NativeHandle::File(Arc::new(file))),
            direction,
            true,
        )
    }
    pub(crate) fn from_imported(
        handle: Arc<OwnedHandle>,
        direction: NativeIoDirection,
        overlapped: bool,
    ) -> Arc<Self> {
        Self::new(
            EndpointKind::Native(NativeHandle::Owned(handle)),
            direction,
            overlapped,
        )
    }
    pub(crate) fn capture(capture: Arc<NativeCapture>) -> Arc<Self> {
        Self::new(
            EndpointKind::Capture(capture),
            NativeIoDirection::Output,
            false,
        )
    }
    pub(crate) fn buffer(buffer: IoBuffer) -> Arc<Self> {
        Self::new(
            EndpointKind::Buffer(buffer),
            NativeIoDirection::Output,
            false,
        )
    }
    pub(crate) fn deferred_input() -> Arc<Self> {
        Self::new(
            EndpointKind::Memory(Mutex::new(None)),
            NativeIoDirection::Input,
            false,
        )
    }
    pub(crate) fn complete_buffered_input(&self, buffer: SeparatedBuffer) -> io::Result<()> {
        let EndpointKind::Memory(input) = &self.kind else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a deferred internal input",
            ));
        };
        let mut input = input.lock().unwrap();
        if input.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "internal producer already completed",
            ));
        }
        *input = Some(MemoryInput {
            buffer,
            element: 0,
            offset: 0,
        });
        Ok(())
    }
    pub fn direction(&self) -> NativeIoDirection {
        self.direction
    }
    pub fn is_overlapped(&self) -> bool {
        self.overlapped
    }
    pub fn native_handle(&self) -> Option<BorrowedHandle<'_>> {
        match &self.kind {
            EndpointKind::Native(handle) => Some(handle.as_handle()),
            EndpointKind::Capture(capture) => Some(capture.writer_handle()),
            EndpointKind::Buffer(_) | EndpointKind::Memory(_) => None,
        }
    }
    pub fn child_handle(&self) -> io::Result<BorrowedHandle<'_>> {
        self.native_handle().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "buffered internal endpoint cannot be passed to a native child",
            )
        })
    }
    pub(crate) fn input_buffer(&self) -> &Mutex<NativeInputBuffer> {
        &self.input
    }
    fn overlapped_file(&self) -> io::Result<&super::pipe::NativeOverlappedFile> {
        self.async_io
            .get_or_init(|| {
                let EndpointKind::Native(handle) = &self.kind else {
                    return Err(StoredError::from(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "overlapped endpoint requires a native handle owner",
                    )));
                };
                super::pipe::NativeOverlappedFile::new(handle.clone()).map_err(StoredError::from)
            })
            .as_ref()
            .map_err(StoredError::to_error)
    }
    pub(crate) fn read(&self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if let EndpointKind::Memory(input) = &self.kind {
            let mut input = input.lock().unwrap();
            let input = input.as_mut().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "internal producer has not completed",
                )
            })?;
            if input.buffer.discarded() {
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "buffered pipeline exceeds fish_read_limit",
                ));
            }
            return Ok(input.read(output));
        }
        if self.overlapped {
            return self.overlapped_file()?.read(output);
        }
        let handle = self.child_handle()?;
        if super::pipe::is_pipe(handle) {
            if let EndpointKind::Native(owner) = &self.kind {
                return self
                    .sync_input
                    .get_or_init(|| {
                        super::pipe::NativeSynchronousInput::new(owner.clone())
                            .map_err(StoredError::from)
                    })
                    .as_ref()
                    .map_err(StoredError::to_error)?
                    .read(output);
            }
        }
        super::pipe::read_synchronous(handle, output)
    }
    pub(crate) fn write(&self, bytes: &[u8]) -> io::Result<usize> {
        if self.overlapped {
            return self.overlapped_file()?.write(bytes);
        }
        let handle = self.child_handle()?;
        NativeOutput::borrowed(handle).write(bytes)
    }
    pub(crate) fn cancel_overlapped_io(&self) {
        // Force lazy initialization before cancelling: otherwise a worker could
        // submit its first operation just after cancellation inspected an empty cell.
        // A failed initialization has no pending operation; the writer receives it.
        if let Ok(io) = self.overlapped_file() {
            io.cancel();
        }
    }
    fn output_stream(self: &Arc<Self>) -> io::Result<OutputStream> {
        match &self.kind {
            EndpointKind::Buffer(buffer) => Ok(OutputStream::Buffered(BufferedOutputStream::new(
                buffer.clone(),
            ))),
            EndpointKind::Capture(capture) => {
                capture.synchronize()?;
                Ok(OutputStream::Buffered(BufferedOutputStream::new(
                    capture.buffer(),
                )))
            }
            _ => Ok(OutputStream::Fd(FdOutputStream::from_endpoint(Arc::clone(
                self,
            )))),
        }
    }
}

#[derive(Clone)]
struct Binding {
    endpoint: Option<Arc<NativeEndpoint>>,
    redirected: bool,
    piped: bool,
}
impl Binding {
    fn closed() -> Self {
        Self {
            endpoint: None,
            redirected: true,
            piped: false,
        }
    }
}

pub struct NativeChildBinding {
    pub fd: i32,
    pub endpoint: Option<Arc<NativeEndpoint>>,
}

/// An ordered-redirection overlay. The process root map is immutable after startup.
#[derive(Clone, Default)]
pub struct IoChain {
    bindings: BTreeMap<i32, Binding>,
}

impl IoChain {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
    pub fn len(&self) -> usize {
        self.bindings.len()
    }
    pub fn clear(&mut self) {
        self.bindings.clear();
    }
    pub fn append(&mut self, other: &Self) -> bool {
        self.bindings.extend(
            other
                .bindings
                .iter()
                .map(|(&fd, binding)| (fd, binding.clone())),
        );
        true
    }
    fn binding(&self, fd: i32) -> io::Result<Binding> {
        Ok(self
            .bindings
            .get(&fd)
            .or(root_descriptors()?.bindings.get(&fd))
            .cloned()
            .unwrap_or_else(Binding::closed))
    }
    pub(crate) fn set_endpoint(&mut self, fd: i32, endpoint: Arc<NativeEndpoint>, piped: bool) {
        self.bindings.insert(
            fd,
            Binding {
                endpoint: Some(endpoint),
                redirected: true,
                piped,
            },
        );
    }
    pub(crate) fn close(&mut self, fd: i32) {
        self.bindings.insert(fd, Binding::closed());
    }
    pub(crate) fn endpoint(&self, fd: i32) -> io::Result<Option<Arc<NativeEndpoint>>> {
        Ok(self.binding(fd)?.endpoint)
    }
    pub(crate) fn flags(&self, fd: i32) -> (bool, bool) {
        let binding = self
            .binding(fd)
            .expect("native descriptors initialized before evaluation");
        (binding.redirected, binding.piped)
    }
    pub(crate) fn input_reader(&self) -> Option<BorrowedFdFile> {
        self.binding(0)
            .expect("native descriptors initialized before evaluation")
            .endpoint
            .map(BorrowedFdFile::from_endpoint)
    }
    pub(crate) fn output_stream(&self, fd: i32) -> io::Result<OutputStream> {
        let binding = self.binding(fd)?;
        match binding.endpoint {
            Some(endpoint) => endpoint.output_stream(),
            // Explicit builtin close is the shared evaluator's discard policy.
            None if binding.redirected => Ok(OutputStream::Null),
            // An inherited unavailable native stream fails only if actually written.
            None => Ok(OutputStream::Fd(FdOutputStream::unavailable())),
        }
    }
    pub fn child_bindings(&self) -> io::Result<Vec<NativeChildBinding>> {
        let mut bindings = root_descriptors()?.bindings.clone();
        bindings.extend(
            self.bindings
                .iter()
                .map(|(&fd, binding)| (fd, binding.clone())),
        );
        Ok(bindings
            .into_iter()
            .map(|(fd, binding)| NativeChildBinding {
                fd,
                endpoint: binding.endpoint,
            })
            .collect())
    }
    pub fn with_specs(
        &self,
        specs: &RedirectionSpecList,
        vars: &dyn Environment,
    ) -> io::Result<Self> {
        let mut result = self.clone();
        for spec in specs {
            if spec.fd < 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid logical descriptor",
                ));
            }
            if spec.is_close() {
                result.close(spec.fd);
                continue;
            }
            if spec.mode == RedirectionMode::Fd {
                let source = spec
                    .get_target_as_fd()
                    .filter(|&fd| fd >= 0)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "invalid redirection source descriptor",
                        )
                    })?;
                let source = result.binding(source)?;
                let endpoint = source.endpoint.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "redirection source descriptor is closed",
                    )
                })?;
                result.bindings.insert(
                    spec.fd,
                    Binding {
                        endpoint: Some(endpoint),
                        redirected: true,
                        piped: source.piped,
                    },
                );
                continue;
            }
            let input = matches!(
                spec.mode,
                RedirectionMode::Input | RedirectionMode::TryInput
            );
            let null = is_null_target(&spec.target);
            if null && spec.mode == RedirectionMode::NoClob {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "native null device already exists",
                ));
            }
            let path = if null {
                std::path::PathBuf::from(r"\\.\NUL")
            } else {
                crate::path::native_resolve_path_for_environment(&spec.target, vars)?
            };
            let mut options = OpenOptions::new();
            if input {
                options.read(true);
            } else {
                options.write(true);
                if !null {
                    match spec.mode {
                        RedirectionMode::Overwrite => {
                            options.create(true).truncate(true);
                        }
                        RedirectionMode::Append => {
                            options.append(true).create(true);
                        }
                        RedirectionMode::NoClob => {
                            options.create_new(true);
                        }
                        _ => unreachable!(),
                    }
                }
            }
            let file = match options.open(path) {
                Ok(file) => file,
                Err(_) if spec.mode == RedirectionMode::TryInput => File::open(r"\\.\NUL")?,
                Err(error) => return Err(error),
            };
            let direction = if input {
                NativeIoDirection::Input
            } else {
                NativeIoDirection::Output
            };
            result.set_endpoint(spec.fd, NativeEndpoint::from_file(file, direction), false);
        }
        Ok(result)
    }
}

fn is_null_target(target: &wstr) -> bool {
    target == L!("/dev/null")
        || target.chars().eq("NUL".chars())
        || {
            let mut chars = target.chars();
            chars.next().is_some_and(|ch| ch.eq_ignore_ascii_case(&'n'))
                && chars.next().is_some_and(|ch| ch.eq_ignore_ascii_case(&'u'))
                && chars.next().is_some_and(|ch| ch.eq_ignore_ascii_case(&'l'))
                && matches!(
                    (chars.next(), chars.next()),
                    (None, None) | (Some(':'), None)
                )
        }
        || target
            .chars()
            .map(|ch| ch.to_ascii_lowercase())
            .eq(r"\\.\nul".chars())
}

struct StoredError {
    code: Option<i32>,
    kind: io::ErrorKind,
    message: String,
}
impl From<io::Error> for StoredError {
    fn from(error: io::Error) -> Self {
        Self {
            code: error.raw_os_error(),
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}
impl StoredError {
    fn to_error(&self) -> io::Error {
        self.code
            .map(io::Error::from_raw_os_error)
            .unwrap_or_else(|| io::Error::new(self.kind, self.message.clone()))
    }
}

static ROOT_DESCRIPTORS: LazyLock<Result<IoChain, StoredError>> =
    LazyLock::new(load_root_descriptors);

pub(crate) fn root_descriptors() -> io::Result<&'static IoChain> {
    ROOT_DESCRIPTORS.as_ref().map_err(StoredError::to_error)
}

fn load_root_descriptors() -> Result<IoChain, StoredError> {
    let mut root = IoChain::new();
    for (fd, kind, direction) in [
        (0, -10i32 as u32, NativeIoDirection::Input),
        (1, -11i32 as u32, NativeIoDirection::Output),
        (2, -12i32 as u32, NativeIoDirection::Output),
    ] {
        let endpoint = standard_handle(kind).ok().map(|handle| {
            NativeEndpoint::new(
                EndpointKind::Native(NativeHandle::Borrowed(handle)),
                direction,
                false,
            )
        });
        root.bindings.insert(
            fd,
            Binding {
                endpoint,
                redirected: false,
                piped: false,
            },
        );
    }
    let mut protocol = None;
    for (key, value) in std::env::vars_os() {
        if crate::native_fd_bridge::is_private_environment_key(&key) {
            if protocol.replace(value).is_some() {
                return Err(StoredError::from(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "duplicate private native descriptor metadata",
                )));
            }
        }
    }
    if let Some(protocol) = protocol {
        let mut endpoints = BTreeMap::new();
        for binding in crate::native_fd_bridge::decode(&protocol).map_err(StoredError::from)? {
            let endpoint = endpoints
                .entry(Arc::as_ptr(&binding.handle))
                .or_insert_with(|| {
                    NativeEndpoint::from_imported(
                        binding.handle,
                        binding.direction,
                        binding.overlapped,
                    )
                });
            root.bindings.insert(
                binding.fd,
                Binding {
                    endpoint: Some(Arc::clone(endpoint)),
                    redirected: binding.fd > 2,
                    piped: false,
                },
            );
        }
    }
    Ok(root)
}
