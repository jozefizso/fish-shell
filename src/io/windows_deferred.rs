//! Bounded internal output drained independently of the main-thread consumer.
use super::{NativeEndpoint, SeparatedBuffer};
use std::io;
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

/// The worker, not this wrapper, owns the last writer reference. Closing it as
/// soon as copying completes lets downstream readers observe EOF before join.
/// Cancellation drains the endpoint's overlapped operation before freeing data.
pub(crate) struct NativeDeferredWriter {
    worker: Option<JoinHandle<io::Result<()>>>,
    target: Weak<NativeEndpoint>,
}

impl NativeDeferredWriter {
    pub(crate) fn start(buffer: SeparatedBuffer, target: Arc<NativeEndpoint>) -> io::Result<Self> {
        if !target.is_overlapped() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deferred native output requires a cancellable pipe",
            ));
        }
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
            // Empty serialization still has a real effect: close the writer now.
            drop(target);
            return Ok(Self {
                worker: None,
                target: Weak::new(),
            });
        }
        let cancellation = Arc::downgrade(&target);
        let worker = std::thread::Builder::new().spawn(move || {
            let result = (|| {
                for element in buffer.elements() {
                    write_all(&target, &element.contents)?;
                    if element.is_explicitly_separated() {
                        write_all(&target, b"\n")?;
                    }
                }
                Ok(())
            })();
            // Every write has completed/drained before endpoint destruction. A
            // main-thread strong reference here would postpone EOF until join.
            drop(target);
            result
        })?;
        Ok(Self {
            worker: Some(worker),
            target: cancellation,
        })
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
        // Only this owned writer is cancelled; buffers/handles stay alive until
        // the worker's actual I/O and callbacks are drained, even on rollback.
        if worker.join().is_err() {
            std::process::abort();
        }
    }
}

fn write_all(target: &NativeEndpoint, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        let count = target.write(&bytes[..bytes.len().min(16384)])?;
        if count == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        bytes = &bytes[count..];
    }
    Ok(())
}
