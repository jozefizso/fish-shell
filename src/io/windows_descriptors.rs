//! Logical descriptor bindings; native handles never double as descriptor numbers.
use super::deferred::{NativeInputPayload, NativeInputReadiness};
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
    Buffer {
        buffer: IoBuffer,
        capture: OnceLock<Result<Arc<NativeCapture>, StoredError>>,
    },
    Memory {
        input: Mutex<Option<MemoryInput>>,
        native: OnceLock<MaterializedInput>,
    },
}

struct MaterializedInput {
    endpoint: Arc<NativeEndpoint>,
    writer: Mutex<Option<super::NativeDeferredWriter>>,
    readiness: Arc<NativeInputReadiness>,
}
impl MaterializedInput {
    fn read(&self, output: &mut [u8]) -> io::Result<usize> {
        self.finish_read(self.endpoint.read(output)?)
    }
    fn peek(&self, output: &mut [u8]) -> io::Result<usize> {
        let handle = self
            .endpoint
            .native_handle()
            .expect("materialized native input pipe");
        self.finish_read(self.readiness.peek(handle, output)?)
    }
    fn finish_read(&self, count: usize) -> io::Result<usize> {
        if count == 0 {
            if let Some(writer) = self.writer.lock().unwrap().take() {
                writer.finish()?;
            }
        }
        Ok(count)
    }
    fn source_error(&self) -> Option<Arc<io::Error>> {
        self.writer.lock().unwrap().as_ref()?.source_error()
    }
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
    async_io: OnceLock<Arc<super::pipe::NativeOverlappedFile>>,
    sync_input: OnceLock<Arc<super::pipe::NativeSynchronousInput>>,
    replay: OnceLock<MaterializedInput>,
    forward: OnceLock<Result<Arc<NativeCapture>, StoredError>>,
    materialization: Mutex<()>,
    io_initialization: Mutex<()>,
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
            replay: OnceLock::new(),
            forward: OnceLock::new(),
            materialization: Mutex::new(()),
            io_initialization: Mutex::new(()),
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
            EndpointKind::Buffer {
                buffer,
                capture: OnceLock::new(),
            },
            NativeIoDirection::Output,
            false,
        )
    }
    pub(crate) fn deferred_input() -> Arc<Self> {
        Self::new(
            EndpointKind::Memory {
                input: Mutex::new(None),
                native: OnceLock::new(),
            },
            NativeIoDirection::Input,
            false,
        )
    }
    pub(crate) fn complete_buffered_input(&self, buffer: SeparatedBuffer) -> io::Result<()> {
        let EndpointKind::Memory { input, .. } = &self.kind else {
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
        if self.forward.get().is_some() {
            return false;
        }
        if let Some(replay) = self.replay.get() {
            return replay.endpoint.is_overlapped();
        }
        if let EndpointKind::Memory { native, .. } = &self.kind {
            if let Some(native) = native.get() {
                return native.endpoint.is_overlapped();
            }
        }
        self.overlapped
    }
    pub fn native_handle(&self) -> Option<BorrowedHandle<'_>> {
        if let Some(forward) = self.forward.get() {
            return forward.as_ref().ok().map(|capture| capture.writer_handle());
        }
        if let Some(replay) = self.replay.get() {
            return replay.endpoint.native_handle();
        }
        match &self.kind {
            EndpointKind::Native(handle) => Some(handle.as_handle()),
            EndpointKind::Capture(capture) => Some(capture.writer_handle()),
            EndpointKind::Buffer { capture, .. } => capture
                .get()
                .and_then(|capture| capture.as_ref().ok())
                .map(|capture| capture.writer_handle()),
            EndpointKind::Memory { native, .. } => native
                .get()
                .and_then(|native| native.endpoint.native_handle()),
        }
    }
    pub fn child_handle(&self) -> io::Result<BorrowedHandle<'_>> {
        if let EndpointKind::Native(handle) = &self.kind {
            if self.direction == NativeIoDirection::Input {
                let _materialization = self.materialization.lock().unwrap();
                let mut pending = self.input.lock().unwrap();
                if let Some(replay) = self.replay.get() {
                    pending.discard_peeked();
                    return replay.endpoint.child_handle();
                }
                if self.overlapped || pending.has_pending() {
                    self.prepare_input_io()?;
                    let source = Self::new(
                        EndpointKind::Native(handle.clone()),
                        self.direction,
                        self.overlapped,
                    );
                    if let Some(io) = self.async_io.get() {
                        let _ = source.async_io.set(io.clone());
                    }
                    if let Some(input) = self.sync_input.get() {
                        let _ = source.sync_input.set(input.clone());
                    }
                    let (endpoint, target) = super::pipe::native_output_pipe()?;
                    let prepared =
                        super::NativeDeferredWriter::prepare_input(Some(source), target)?;
                    let readiness = Arc::clone(&prepared.readiness);
                    let payload = NativeInputPayload::Replay {
                        prefix: pending.take_pending(),
                    };
                    let writer = match prepared.commit(payload) {
                        Ok(writer) => writer,
                        Err((error, NativeInputPayload::Replay { prefix })) => {
                            pending.restore_pending(prefix);
                            return Err(error);
                        }
                        Err(_) => unreachable!("native replay payload"),
                    };
                    assert!(
                        self.replay
                            .set(MaterializedInput {
                                endpoint,
                                writer: Mutex::new(Some(writer)),
                                readiness,
                            })
                            .is_ok()
                    );
                    return self
                        .replay
                        .get()
                        .expect("published replay")
                        .endpoint
                        .child_handle();
                }
            }
            if self.direction == NativeIoDirection::Output && self.overlapped {
                let forward = self
                    .forward
                    .get_or_init(|| {
                        self.overlapped_file().map_err(StoredError::from)?;
                        let target =
                            Self::new(EndpointKind::Native(handle.clone()), self.direction, true);
                        if let Some(io) = self.async_io.get() {
                            let _ = target.async_io.set(io.clone());
                        }
                        NativeCapture::forwarding(target).map_err(StoredError::from)
                    })
                    .as_ref()
                    .map_err(StoredError::to_error)?;
                return Ok(forward.writer_handle());
            }
        }
        match &self.kind {
            EndpointKind::Buffer { buffer, capture } => {
                let capture = capture
                    .get_or_init(|| {
                        NativeCapture::with_buffer(
                            buffer.clone(),
                            crate::threads::NativeRuntime::new()?,
                        )
                        .map_err(StoredError::from)
                    })
                    .as_ref()
                    .map_err(StoredError::to_error)?;
                return Ok(capture.writer_handle());
            }
            EndpointKind::Memory { input, native } => {
                let _materialization = self.materialization.lock().unwrap();
                let mut pending = self.input.lock().unwrap();
                if let Some(native) = native.get() {
                    pending.discard_peeked();
                    return native.endpoint.child_handle();
                }
                let mut memory = input.lock().unwrap();
                let current = memory.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "internal producer has not completed",
                    )
                })?;
                if current.buffer.discarded() {
                    return Err(io::Error::new(
                        io::ErrorKind::FileTooLarge,
                        "buffered pipeline exceeds fish_read_limit",
                    ));
                }
                let (endpoint, target) = super::pipe::native_output_pipe()?;
                let prepared = super::NativeDeferredWriter::prepare_input(None, target)?;
                let readiness = Arc::clone(&prepared.readiness);
                let current = memory
                    .take()
                    .expect("memory cursor retained until prepared");
                let payload = NativeInputPayload::Buffer {
                    prefix: pending.take_pending(),
                    buffer: current.buffer,
                    element: current.element,
                    offset: current.offset,
                };
                let writer = match prepared.commit(payload) {
                    Ok(writer) => writer,
                    Err((
                        error,
                        NativeInputPayload::Buffer {
                            prefix,
                            buffer,
                            element,
                            offset,
                        },
                    )) => {
                        pending.restore_pending(prefix);
                        *memory = Some(MemoryInput {
                            buffer,
                            element,
                            offset,
                        });
                        return Err(error);
                    }
                    Err(_) => unreachable!("memory input payload"),
                };
                assert!(
                    native
                        .set(MaterializedInput {
                            endpoint,
                            writer: Mutex::new(Some(writer)),
                            readiness,
                        })
                        .is_ok()
                );
                return native
                    .get()
                    .expect("published memory input")
                    .endpoint
                    .child_handle();
            }
            _ => {}
        }
        self.native_handle().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "buffered internal endpoint cannot be passed to a native child",
            )
        })
    }
    pub(crate) fn has_materialized_input(&self) -> bool {
        self.replay.get().is_some()
            || matches!(&self.kind, EndpointKind::Memory { native, .. } if native.get().is_some())
    }
    pub(crate) fn input_error(&self) -> Option<Arc<io::Error>> {
        if let Some(replay) = self.replay.get() {
            return replay.source_error();
        }
        if let EndpointKind::Memory { native, .. } = &self.kind {
            if let Some(native) = native.get() {
                return native.source_error();
            }
        }
        None
    }
    pub(crate) fn peek_input(&self, output: &mut [u8]) -> io::Result<Option<usize>> {
        if let Some(replay) = self.replay.get() {
            return replay.peek(output).map(Some);
        }
        if let EndpointKind::Memory { native, .. } = &self.kind {
            if let Some(native) = native.get() {
                return native.peek(output).map(Some);
            }
        }
        Ok(None)
    }
    pub(crate) fn prepare_input_io(&self) -> io::Result<()> {
        if self.overlapped {
            self.overlapped_file()?;
        } else if let EndpointKind::Native(owner) = &self.kind {
            if super::pipe::is_pipe(owner.as_handle()) {
                self.synchronous_input(owner)?;
            }
        }
        Ok(())
    }
    pub(crate) fn prepare_output_io(&self) -> io::Result<()> {
        self.overlapped_file().map(|_| ())
    }
    pub(crate) fn input_buffer(&self) -> &Mutex<NativeInputBuffer> {
        &self.input
    }
    fn overlapped_file(&self) -> io::Result<&super::pipe::NativeOverlappedFile> {
        if self.async_io.get().is_none() {
            let _initialization = self.io_initialization.lock().unwrap();
            if self.async_io.get().is_none() {
                let EndpointKind::Native(handle) = &self.kind else {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "overlapped endpoint requires a native handle owner",
                    ));
                };
                let io = Arc::new(super::pipe::NativeOverlappedFile::new(handle.clone())?);
                assert!(self.async_io.set(io).is_ok());
            }
        }
        Ok(self.async_io.get().expect("initialized native IO").as_ref())
    }
    fn synchronous_input(
        &self,
        owner: &NativeHandle,
    ) -> io::Result<&super::pipe::NativeSynchronousInput> {
        if self.sync_input.get().is_none() {
            let _initialization = self.io_initialization.lock().unwrap();
            if self.sync_input.get().is_none() {
                let input = Arc::new(super::pipe::NativeSynchronousInput::new(owner.clone())?);
                assert!(self.sync_input.set(input).is_ok());
            }
        }
        Ok(self
            .sync_input
            .get()
            .expect("initialized native input")
            .as_ref())
    }
    pub(crate) fn read(&self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if let Some(replay) = self.replay.get() {
            return replay.read(output);
        }
        if let EndpointKind::Memory { input, native } = &self.kind {
            if let Some(native) = native.get() {
                return native.read(output);
            }
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
        let handle = self.native_handle().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "input endpoint has no native handle",
            )
        })?;
        if super::pipe::is_pipe(handle) {
            if let EndpointKind::Native(owner) = &self.kind {
                return self.synchronous_input(owner)?.read(output);
            }
        }
        super::pipe::read_synchronous(handle, output)
    }
    pub(crate) fn write(&self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(forward) = self.forward.get() {
            forward
                .as_ref()
                .map_err(StoredError::to_error)?
                .synchronize()?;
        }
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
    pub(crate) fn cancel_input_io(&self) {
        if self.overlapped {
            self.cancel_overlapped_io();
        } else if let EndpointKind::Native(owner) = &self.kind {
            if super::pipe::is_pipe(owner.as_handle()) {
                if let Ok(input) = self.synchronous_input(owner) {
                    input.cancel();
                }
            }
        }
    }
    fn output_stream(self: &Arc<Self>) -> io::Result<OutputStream> {
        match &self.kind {
            EndpointKind::Buffer { buffer, capture } => {
                if let Some(capture) = capture.get() {
                    capture
                        .as_ref()
                        .map_err(StoredError::to_error)?
                        .synchronize()?;
                }
                Ok(OutputStream::Buffered(BufferedOutputStream::new(
                    buffer.clone(),
                )))
            }
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
    pub(crate) fn finish_forwarding(&self) -> io::Result<()> {
        if let Some(forward) = self.forward.get() {
            let (interrupted, error) = forward
                .as_ref()
                .map_err(StoredError::to_error)?
                .finish_in_place()?;
            if let Some(error) = error {
                return Err(error);
            }
            if interrupted {
                return Err(io::ErrorKind::Interrupted.into());
            }
        }
        Ok(())
    }

    /// Finish only the buffer owned by this internal pipeline stage, not an
    /// enclosing evaluation's capture. The same buffer retains explicit elements.
    pub(crate) fn finish_buffered_output(&self) -> io::Result<()> {
        if let EndpointKind::Buffer { buffer, capture } = &self.kind {
            if let Some(capture) = capture.get() {
                let (interrupted, error) = capture
                    .as_ref()
                    .map_err(StoredError::to_error)?
                    .finish_in_place()?;
                if let Some(error) = error {
                    return Err(error);
                }
                if interrupted {
                    return Err(io::ErrorKind::Interrupted.into());
                }
            }
            if buffer.discarded() {
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "buffered pipeline exceeds fish_read_limit",
                ));
            }
        }
        Ok(())
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

#[derive(Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::deferred::{InputSetupFailure, fail_next_input_setup};
    use std::io::Read as _;

    #[test]
    fn input_setup_failures_preserve_owned_native_and_memory_cursors() {
        let bytes = b"one\0two\0three\0";
        for failure in [InputSetupFailure::Worker, InputSetupFailure::Commit] {
            for memory in [false, true] {
                let input = if memory {
                    let mut buffer = SeparatedBuffer::new(1024);
                    assert!(buffer.append(bytes, super::super::SeparationType::Inferred));
                    let input = NativeEndpoint::deferred_input();
                    input.complete_buffered_input(buffer).unwrap();
                    input
                } else {
                    let (input, output) = super::super::pipe::native_input_pipe().unwrap();
                    assert_eq!(output.write(bytes).unwrap(), bytes.len());
                    drop(output);
                    input
                };
                let mut reader = BorrowedFdFile::from_endpoint(Arc::clone(&input));
                assert_eq!(&*reader.fill_buf().unwrap(), bytes);
                let mut first = [0; 4];
                reader.read_exact(&mut first).unwrap();
                assert_eq!(&first, b"one\0");
                fail_next_input_setup(failure);
                assert!(input.child_handle().is_err());
                // Neither fallible resource creation nor commit may publish an
                // error cell, cancel shared IO, or lose the already-read prefix.
                let mut rest = Vec::new();
                reader.read_to_end(&mut rest).unwrap();
                assert_eq!(rest, b"two\0three\0");
            }
        }
    }

    #[test]
    fn materialized_input_peeks_chunks_without_consuming_child_input() {
        let mut buffer = SeparatedBuffer::new(1024);
        assert!(buffer.append(
            b"dir/a\0dir/b\0dir/c\0",
            super::super::SeparationType::Inferred
        ));
        let input = NativeEndpoint::deferred_input();
        input.complete_buffered_input(buffer).unwrap();
        input.child_handle().unwrap();
        let mut reader = BorrowedFdFile::from_endpoint(Arc::clone(&input));
        assert_eq!(&*reader.fill_buf().unwrap(), b"dir/a\0dir/b\0dir/c\0");
        // Handing the endpoint to another child invalidates lookahead only.
        input.child_handle().unwrap();
        let mut first = [0; 6];
        reader.read_exact(&mut first).unwrap();
        assert_eq!(&first, b"dir/a\0");
        assert_eq!(&*reader.fill_buf().unwrap(), b"dir/b\0dir/c\0");
        input.child_handle().unwrap();
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, b"dir/b\0dir/c\0");
    }

    #[test]
    fn first_materialization_exposes_only_unread_bytes_to_native_handle() {
        let bytes = b"dir/first\ndir/second\n";
        for memory in [false, true] {
            let input = if memory {
                let mut buffer = SeparatedBuffer::new(1024);
                assert!(buffer.append(bytes, super::super::SeparationType::Inferred));
                let input = NativeEndpoint::deferred_input();
                input.complete_buffered_input(buffer).unwrap();
                input
            } else {
                let (input, output) = super::super::pipe::native_input_pipe().unwrap();
                assert_eq!(output.write(bytes).unwrap(), bytes.len());
                drop(output);
                input
            };
            let mut builtin = BorrowedFdFile::from_endpoint(Arc::clone(&input));
            assert_eq!(&*builtin.fill_buf().unwrap(), bytes);
            let mut first = Vec::new();
            builtin.read_until(b'\n', &mut first).unwrap();
            assert_eq!(first, b"dir/first\n");
            // Use the actual exposed HANDLE, with a fresh adapter/cache, rather
            // than continuing through the original logical endpoint.
            let handle = input.child_handle().unwrap();
            let mut native = crate::fds::BorrowedInput::from_handle(handle);
            let mut rest = Vec::new();
            native.read_to_end(&mut rest).unwrap();
            assert_eq!(rest, b"dir/second\n");
        }
    }
}
