//! Bounded internal output and transactional native-input materialization.
use super::{NativeEndpoint, SeparatedBuffer};
use std::io;
use std::os::windows::io::BorrowedHandle;
use std::sync::{Arc, Condvar, Mutex, Weak, mpsc};
use std::thread::JoinHandle;

/// The pump signals only after bytes reach its pipe or its writer closes. A
/// lookahead can therefore wait without consuming input or polling the kernel.
pub(super) struct NativeInputReadiness {
    changed: Mutex<()>,
    ready: Condvar,
}
impl NativeInputReadiness {
    fn new() -> Self {
        Self {
            changed: Mutex::new(()),
            ready: Condvar::new(),
        }
    }
    fn notify(&self) {
        let _changed = self.changed.lock().unwrap();
        self.ready.notify_all();
    }
    pub(super) fn peek(&self, handle: BorrowedHandle<'_>, bytes: &mut [u8]) -> io::Result<usize> {
        let mut changed = self.changed.lock().unwrap();
        loop {
            let (count, eof) = super::pipe::peek_available(handle, bytes)?;
            if count != 0 || eof {
                return Ok(count);
            }
            changed = self.ready.wait(changed).unwrap();
        }
    }
}

pub(super) enum NativeInputPayload {
    Buffer {
        prefix: Vec<u8>,
        buffer: SeparatedBuffer,
        element: usize,
        offset: usize,
    },
    Replay {
        prefix: Vec<u8>,
    },
}

/// No worker reads or writes until commit transfers the original cursor. Failed
/// setup owns only new resources, and failed commit returns the complete payload.
pub(super) struct PreparedNativeInput {
    writer: Option<NativeDeferredWriter>,
    sender: Option<mpsc::SyncSender<NativeInputPayload>>,
    source: Weak<NativeEndpoint>,
    pub(super) readiness: Arc<NativeInputReadiness>,
}
impl PreparedNativeInput {
    pub(super) fn commit(
        mut self,
        payload: NativeInputPayload,
    ) -> Result<NativeDeferredWriter, (io::Error, NativeInputPayload)> {
        let sender = self.sender.take().expect("prepared input sender");
        #[cfg(test)]
        if take_setup_failure(InputSetupFailure::Commit) {
            drop(sender);
            return Err((io::Error::other("injected input commit failure"), payload));
        }
        match sender.send(payload) {
            Ok(()) => {
                let mut writer = self.writer.take().expect("prepared input worker");
                writer.source = self.source.clone();
                Ok(writer)
            }
            Err(payload) => Err((
                io::Error::other("input worker stopped before commit"),
                payload.0,
            )),
        }
    }
}
impl Drop for PreparedNativeInput {
    fn drop(&mut self) {
        // Close the activation channel BEFORE the uncommitted worker is joined.
        // Its source cancellation remains unset until a successful commit.
        drop(self.sender.take());
    }
}

pub(crate) struct NativeDeferredWriter {
    worker: Option<JoinHandle<io::Result<()>>>,
    target: Weak<NativeEndpoint>,
    source: Weak<NativeEndpoint>,
    source_error: Option<Arc<Mutex<Option<Arc<io::Error>>>>>,
}
impl NativeDeferredWriter {
    pub(crate) fn start(buffer: SeparatedBuffer, target: Arc<NativeEndpoint>) -> io::Result<Self> {
        validate_target(&target)?;
        if buffer.discarded() {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "buffered pipeline exceeds fish_read_limit",
            ));
        }
        if buffer.len() == 0
            && !buffer
                .elements()
                .iter()
                .any(|element| element.is_explicitly_separated())
        {
            drop(target);
            return Ok(Self {
                worker: None,
                target: Weak::new(),
                source: Weak::new(),
                source_error: None,
            });
        }
        let cancellation = Arc::downgrade(&target);
        let worker = std::thread::Builder::new().spawn(move || {
            let result = write_buffer(&buffer, 0, 0, &target, None);
            drop(target);
            result
        })?;
        Ok(Self {
            worker: Some(worker),
            target: cancellation,
            source: Weak::new(),
            source_error: None,
        })
    }

    pub(super) fn prepare_input(
        source: Option<Arc<NativeEndpoint>>,
        target: Arc<NativeEndpoint>,
    ) -> io::Result<PreparedNativeInput> {
        validate_target(&target)?;
        if let Some(source) = &source {
            source.prepare_input_io()?;
        }
        let cancellation = Arc::downgrade(&target);
        let source_cancellation = source.as_ref().map(Arc::downgrade).unwrap_or_default();
        let source_error = source.as_ref().map(|_| Arc::new(Mutex::new(None)));
        let observed_error = source_error.clone();
        let readiness = Arc::new(NativeInputReadiness::new());
        let notify = Arc::clone(&readiness);
        let (sender, receive) = mpsc::sync_channel(1);
        target.prepare_output_io()?;
        #[cfg(test)]
        if take_setup_failure(InputSetupFailure::Worker) {
            return Err(io::Error::other("injected input worker setup failure"));
        }
        let worker = std::thread::Builder::new().spawn(move || {
            let result = (|| {
                let Ok(payload) = receive.recv() else {
                    return Ok(());
                };
                match payload {
                    NativeInputPayload::Buffer {
                        prefix,
                        buffer,
                        element,
                        offset,
                    } => {
                        write_all(&target, &prefix, Some(&notify))?;
                        write_buffer(&buffer, element, offset, &target, Some(&notify))
                    }
                    NativeInputPayload::Replay { prefix } => {
                        write_all(&target, &prefix, Some(&notify))?;
                        let source = source.as_ref().expect("native replay source prepared");
                        let mut slab = [0u8; 16384];
                        loop {
                            let count = source.read(&mut slab).map_err(|error| {
                                let error = Arc::new(error);
                                *observed_error
                                    .as_ref()
                                    .expect("native replay error slot")
                                    .lock()
                                    .unwrap() = Some(Arc::clone(&error));
                                io::Error::new(error.kind(), error)
                            })?;
                            if count == 0 {
                                break;
                            }
                            write_all(&target, &slab[..count], Some(&notify))?;
                        }
                        Ok(())
                    }
                }
            })();
            // EOF and errors must be visible before waking a nonconsuming peek.
            drop(target);
            notify.notify();
            result
        })?;
        Ok(PreparedNativeInput {
            writer: Some(Self {
                worker: Some(worker),
                target: cancellation,
                source: Weak::new(),
                source_error,
            }),
            sender: Some(sender),
            source: source_cancellation,
            readiness,
        })
    }

    pub(crate) fn source_error(&self) -> Option<Arc<io::Error>> {
        self.source_error.as_ref()?.lock().unwrap().clone()
    }
    pub(crate) fn finish(mut self) -> io::Result<()> {
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| io::Error::other("native pipeline writer panicked"))?,
            None => Ok(()),
        }
    }
}
impl Drop for NativeDeferredWriter {
    fn drop(&mut self) {
        let Some(worker) = self.worker.take() else {
            return;
        };
        if let Some(target) = self.target.upgrade() {
            target.cancel_overlapped_io();
        }
        if let Some(source) = self.source.upgrade() {
            source.cancel_input_io();
        }
        if worker.join().is_err() {
            std::process::abort();
        }
    }
}
fn validate_target(target: &NativeEndpoint) -> io::Result<()> {
    if !target.is_overlapped() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "deferred native output requires a cancellable pipe",
        ));
    }
    Ok(())
}
fn write_buffer(
    buffer: &SeparatedBuffer,
    element_index: usize,
    offset: usize,
    target: &NativeEndpoint,
    ready: Option<&NativeInputReadiness>,
) -> io::Result<()> {
    for (index, element) in buffer.elements().iter().enumerate().skip(element_index) {
        let start = if index == element_index { offset } else { 0 };
        write_all(target, &element.contents[start..], ready)?;
        if element.is_explicitly_separated() {
            write_all(target, b"\n", ready)?;
        }
    }
    Ok(())
}
fn write_all(
    target: &NativeEndpoint,
    mut bytes: &[u8],
    ready: Option<&NativeInputReadiness>,
) -> io::Result<()> {
    while !bytes.is_empty() {
        let count = target.write(&bytes[..bytes.len().min(16384)])?;
        if count == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        bytes = &bytes[count..];
        if let Some(ready) = ready {
            ready.notify();
        }
    }
    Ok(())
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum InputSetupFailure {
    Worker,
    Commit,
}
#[cfg(test)]
thread_local! {
    static INPUT_SETUP_FAILURE: std::cell::Cell<Option<InputSetupFailure>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(super) fn fail_next_input_setup(failure: InputSetupFailure) {
    INPUT_SETUP_FAILURE.with(|slot| {
        assert!(slot.replace(Some(failure)).is_none());
    });
}
#[cfg(test)]
fn take_setup_failure(failure: InputSetupFailure) -> bool {
    INPUT_SETUP_FAILURE.with(|slot| {
        if slot.get() == Some(failure) {
            slot.set(None);
            true
        } else {
            false
        }
    })
}
