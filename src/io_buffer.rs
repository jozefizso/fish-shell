//! Shared separated-output storage; platform modules own collection and finalization.

use crate::builtins::STATUS_READ_TOO_MUCH;
use crate::prelude::*;
use fish_widestring::wcs2bytes;
use std::sync::{Arc, Mutex};

/// separated_buffer_t represents a buffer of output from commands, prepared to be turned into a
/// variable. For example, command substitutions output into one of these. Most commands just
/// produce a stream of bytes, and those get stored directly. However other commands produce
/// explicitly separated output, in particular `string` like `string collect` and `string split0`.
/// The buffer tracks a sequence of elements. Some elements are explicitly separated and should not
/// be further split; other elements have inferred separation and may be split by IFS (or not,
/// depending on its value).
#[derive(Clone, Copy, PartialEq)]
pub enum SeparationType {
    /// this element should be further separated by IFS
    Inferred,
    /// this element is explicitly separated and should not be further split
    Explicitly,
}

pub struct BufferElement {
    pub contents: Vec<u8>,
    pub separation: SeparationType,
}

impl BufferElement {
    pub fn new(contents: Vec<u8>, separation: SeparationType) -> Self {
        BufferElement {
            contents,
            separation,
        }
    }
    pub fn is_explicitly_separated(&self) -> bool {
        self.separation == SeparationType::Explicitly
    }
}

/// A separated_buffer_t contains a list of elements, some of which may be separated explicitly and
/// others which must be separated further by the user (e.g. via IFS).
pub struct SeparatedBuffer {
    /// Limit on how much data we'll buffer. Zero means no limit.
    buffer_limit: usize,
    /// Current size of all contents.
    contents_size: usize,
    /// List of buffer elements.
    elements: Vec<BufferElement>,
    /// True if we're discarding input because our buffer_limit has been exceeded.
    discard: bool,
    /// Optional native retention budget also bounds explicitly empty elements.
    storage_limit: Option<usize>,
    storage_size: usize,
}

impl SeparatedBuffer {
    pub fn new(limit: usize) -> Self {
        SeparatedBuffer {
            buffer_limit: limit,
            contents_size: 0,
            elements: vec![],
            discard: false,
            storage_limit: None,
            storage_size: 0,
        }
    }
    fn new_bounded(limit: usize) -> Self {
        let mut result = Self::new(limit);
        result.storage_limit =
            (limit != 0).then(|| limit.saturating_add(std::mem::size_of::<BufferElement>()));
        result
    }

    /// Return the buffer limit size, or 0 for no limit.
    pub fn limit(&self) -> usize {
        self.buffer_limit
    }

    /// Return the contents size.
    pub fn len(&self) -> usize {
        self.contents_size
    }

    /// Return whether the output has been discarded.
    pub fn discarded(&self) -> bool {
        self.discard
    }

    /// Serialize the contents to a single string, where explicitly separated elements have a
    /// newline appended.
    pub fn newline_serialized(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(self.len().saturating_add(self.elements.len()));
        for elem in &self.elements {
            result.extend_from_slice(&elem.contents);
            if elem.is_explicitly_separated() {
                result.push(b'\n');
            }
        }
        result
    }

    /// Return the list of elements.
    pub fn elements(&self) -> &[BufferElement] {
        &self.elements
    }

    /// Append the given data with separation type `sep`.
    pub fn append(&mut self, data: &[u8], sep: SeparationType) -> bool {
        let merge = sep == SeparationType::Inferred && self.last_inferred();
        if !self.try_add_size(data.len()) {
            return false;
        }
        if let Some(limit) = self.storage_limit {
            let overhead = if merge {
                0
            } else {
                std::mem::size_of::<BufferElement>()
            };
            let proposed = self
                .storage_size
                .checked_add(data.len())
                .and_then(|size| size.checked_add(overhead));
            let Some(proposed) = proposed.filter(|&size| size <= limit) else {
                self.clear();
                self.discard = true;
                return false;
            };
            self.storage_size = proposed;
        }
        // Try merging with the last element.
        if merge {
            self.elements
                .last_mut()
                .unwrap()
                .contents
                .extend_from_slice(data);
        } else {
            self.elements.push(BufferElement::new(data.to_vec(), sep));
        }
        true
    }

    /// Remove all elements and unset the discard flag.
    pub fn clear(&mut self) {
        self.elements.clear();
        self.contents_size = 0;
        self.storage_size = 0;
        self.discard = false;
    }

    /// Return true if our last element has an inferred separation type.
    fn last_inferred(&self) -> bool {
        !self.elements.is_empty() && !self.elements.last().unwrap().is_explicitly_separated()
    }

    /// Mark that we are about to add the given size `delta` to the buffer. Return true if we
    /// succeed, false if we exceed buffer_limit.
    fn try_add_size(&mut self, delta: usize) -> bool {
        if self.discard {
            return false;
        }
        let proposed_size = self.contents_size.checked_add(delta);
        if proposed_size.is_none_or(|size| self.buffer_limit > 0 && size > self.buffer_limit) {
            self.clear();
            self.discard = true;
            return false;
        }
        self.contents_size = proposed_size.unwrap();
        true
    }
}

/// Type wrapping a lock-protected separated buffer.
#[derive(Clone)]
pub struct IoBuffer(pub(super) Arc<Mutex<SeparatedBuffer>>);

impl IoBuffer {
    /// Create a new IoBuffer.
    pub fn new(buffer_limit: usize) -> Self {
        IoBuffer(Arc::new(Mutex::new(SeparatedBuffer::new(buffer_limit))))
    }
    /// Keep byte limits and a finite element-storage budget, even for empty fields.
    pub fn new_bounded(buffer_limit: usize) -> Self {
        IoBuffer(Arc::new(Mutex::new(SeparatedBuffer::new_bounded(
            buffer_limit,
        ))))
    }

    /// Append a string to the buffer.
    pub fn append(&self, data: &[u8], typ: SeparationType) -> bool {
        self.0.lock().unwrap().append(data, typ)
    }

    /// Return true if output was discarded due to exceeding the read limit.
    pub fn discarded(&self) -> bool {
        self.0.lock().unwrap().discarded()
    }

    /// Transfer the collected elements after the platform collector has drained.
    #[cfg(windows)]
    pub(crate) fn take_buffer(&self) -> SeparatedBuffer {
        let mut buffer = self.0.lock().unwrap();
        let mut replacement = SeparatedBuffer::new(buffer.limit());
        replacement.storage_limit = buffer.storage_limit;
        std::mem::replace(&mut *buffer, replacement)
    }
}

/// An output stream for builtins which writes into a separated buffer.
pub struct BufferedOutputStream {
    /// The buffer we are filling.
    pub(super) buffer: IoBuffer,
}
impl BufferedOutputStream {
    pub fn new(buffer: IoBuffer) -> Self {
        Self { buffer }
    }
    pub(super) fn append(&mut self, s: impl IntoCharIter) -> bool {
        self.buffer.append(&wcs2bytes(s), SeparationType::Inferred)
    }
    pub(super) fn append_with_separation(
        &mut self,
        s: impl IntoCharIter,
        typ: SeparationType,
        _want_newline: bool,
    ) -> bool {
        self.buffer.append(&wcs2bytes(s), typ)
    }
    pub(super) fn flush_and_check_error(&mut self) -> libc::c_int {
        if self.buffer.discarded() {
            return STATUS_READ_TOO_MUCH;
        }
        0
    }
}
