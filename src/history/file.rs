//! Implementation of the YAML-like history file format.

use super::HistoryItem;
use super::yaml_backend::{
    FIRST_ADDED_TIMESTAMP_KEY, LAST_ADDED_TIMESTAMP_KEY, decode_item_fish_2_0,
    escape_yaml_fish_2_0, offset_of_next_item_fish_2_0,
};
#[cfg(unix)]
use crate::path::{DirRemoteness, path_get_data_remoteness};
use crate::{flog::flog, wutil::FileId};
use fish_widestring::wcs2bytes;
#[cfg(unix)]
use nix::errno::Errno;
#[cfg(unix)]
use nix::sys::mman::{MapFlags, ProtFlags};
#[cfg(unix)]
use std::num::NonZeroUsize;
#[cfg(unix)]
use std::ops::{Deref, DerefMut};
#[cfg(unix)]
use std::ptr::NonNull;
use std::{
    fs::File,
    io::Read as _,
    time::{SystemTime, UNIX_EPOCH},
};

/// History file types.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum HistoryFileType {
    Fish1_x, // old format with just timestamp and item
    Fish2_0, // YAML-style format
}

/// A type wrapping up the logic around mmap and munmap.
#[cfg(unix)]
struct MmapRegion {
    ptr: NonNull<u8>,
    len: NonZeroUsize,
}

#[cfg(unix)]
impl MmapRegion {
    /// Map a region `[0, len)` from a locked file.
    pub fn map_file(file: &File, len: NonZeroUsize) -> nix::Result<Self> {
        let ptr = unsafe {
            nix::sys::mman::mmap(
                None,
                len,
                ProtFlags::PROT_READ,
                MapFlags::MAP_PRIVATE,
                file,
                0,
            )
        }?;

        Ok(Self {
            ptr: ptr.cast(),
            len,
        })
    }

    /// Map anonymous memory of a given length.
    pub fn map_anon(len: NonZeroUsize) -> nix::Result<Self> {
        let ptr = unsafe {
            nix::sys::mman::mmap_anonymous(
                None,
                len,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_PRIVATE,
            )
        }?;

        Ok(Self {
            ptr: ptr.cast(),
            len,
        })
    }
}

// SAFETY: MmapRegion has exclusive mutable access to the region
#[cfg(unix)]
unsafe impl Send for MmapRegion {}
// SAFETY: MmapRegion does not offer interior mutability
#[cfg(unix)]
unsafe impl Sync for MmapRegion {}

#[cfg(unix)]
impl Deref for MmapRegion {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len.get()) }
    }
}

#[cfg(unix)]
impl DerefMut for MmapRegion {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len.get()) }
    }
}

#[cfg(unix)]
impl Drop for MmapRegion {
    fn drop(&mut self) {
        _ = unsafe { nix::sys::mman::munmap(self.ptr.cast(), self.len.get()) };
    }
}

#[cfg(unix)]
type HistoryRegion = MmapRegion;
// Owned snapshots do not keep a file mapping alive across native atomic replacements.
// Item decoding and the byte-offset index are shared with the mapped Unix backend.
#[cfg(windows)]
type HistoryRegion = Vec<u8>;

/// RawHistoryFile holds the read-only contents of a file, before decoding it.
pub struct RawHistoryFile {
    region: HistoryRegion,
}

impl RawHistoryFile {
    /// Construct a history file contents from a [`File`] reference and its file id.
    #[cfg(unix)]
    pub fn create(history_file: &File, file_id: FileId) -> std::io::Result<Self> {
        // Check the file size.
        let len: usize = match file_id.size.try_into() {
            Ok(len) => len,
            Err(err) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("Cannot convert u64 to usize: {err}"),
                ));
            }
        };
        let Ok(len) = NonZeroUsize::try_from(len) else {
            return Err(std::io::Error::other(
                "History file is empty. Cannot create memory mapping with length 0.",
            ));
        };
        let map_anon = |mut file: &File, len: NonZeroUsize| -> std::io::Result<MmapRegion> {
            let mut region = MmapRegion::map_anon(len)?;
            // If we mapped anonymous memory, we have to read from the file.
            file.read_exact(&mut region)?;
            Ok(region)
        };
        let region = if should_mmap() {
            match MmapRegion::map_file(history_file, len) {
                Ok(region) => region,
                Err(err) => {
                    if err == Errno::ENODEV {
                        // Our mmap failed with ENODEV, which means the underlying
                        // filesystem does not support mapping.
                        // Create an anonymous mapping and read() the file into it.
                        map_anon(history_file, len)?
                    } else {
                        return Err(std::io::Error::from(err));
                    }
                }
            }
        } else {
            map_anon(history_file, len)?
        };

        region.try_into()
    }

    #[cfg(windows)]
    pub fn create(mut history_file: &File, file_id: FileId) -> std::io::Result<Self> {
        use std::io::{Seek as _, SeekFrom};
        let len = usize::try_from(file_id.size)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Unsupported, err))?;
        history_file.seek(SeekFrom::Start(0))?;
        let mut region = vec![0; len];
        history_file.read_exact(&mut region)?;
        // The advertised immutable snapshot must be complete, not a prefix of a changed file.
        let mut extra = [0];
        if history_file.read(&mut extra)? != 0 || history_file.metadata()?.len() != file_id.size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "History changed while reading its snapshot",
            ));
        }
        region.try_into()
    }

    /// Decode an item at a given offset.
    pub fn decode_item(&self, offset: usize) -> Option<HistoryItem> {
        let contents = &self.region[offset..];
        decode_item_fish_2_0(contents)
    }

    /// Returns an iterator over item offsets with an optional cutoff time.
    /// If cutoff is given, skip items whose timestamp is newer than cutoff.
    pub fn offsets(&self, cutoff: Option<SystemTime>) -> impl Iterator<Item = usize> + '_ {
        HistoryFileOffsetIter {
            contents: self,
            cursor: 0,
            cutoff,
        }
    }

    /// Returns a view of the file contents.
    pub fn contents(&self) -> &[u8] {
        &self.region
    }

    /// Decode this history file.
    /// If cutoff is given, skip items whose timestamp is newer than cutoff.
    pub fn decode(self, cutoff: Option<SystemTime>) -> HistoryFile {
        let offsets = self.offsets(cutoff).collect();
        HistoryFile {
            contents: Some(self),
            offsets,
        }
    }
}

/// A combination of a history file and its offsets.
pub struct HistoryFile {
    // Contents of the file. May be None if there was no file.
    contents: Option<RawHistoryFile>,
    // Offsets of items within the file. Always empty if contents is None.
    offsets: Vec<usize>,
}

impl HistoryFile {
    /// Create an empty history file.
    pub fn create_empty() -> Self {
        Self {
            contents: None,
            offsets: Vec::new(),
        }
    }

    /// Return whether this file is empty.
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// Return the offsets of items in this file.
    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }

    /// Decode an item at a given offset.
    pub fn decode_item(&self, offset: usize) -> Option<HistoryItem> {
        self.contents.as_ref()?.decode_item(offset)
    }

    /// Remove a command from the native session's visible baseline without reloading disk.
    #[cfg(windows)]
    pub fn remove_command(&mut self, command: &fish_widestring::wstr) {
        let contents = &self.contents;
        self.offsets.retain(|&offset| {
            contents
                .as_ref()
                .and_then(|file| file.decode_item(offset))
                .is_none_or(|item| item.str() != command)
        });
    }
}

/// Iterator over offsets within a history file.
struct HistoryFileOffsetIter<'a> {
    // The file contents.
    contents: &'a RawHistoryFile,
    // Current offset within the file.
    cursor: usize,
    // Optional cutoff time. If given, skip items newer than this.
    cutoff: Option<SystemTime>,
}

impl<'a> Iterator for HistoryFileOffsetIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let cursor: &mut usize = &mut self.cursor;
        let cutoff = self.cutoff;
        offset_of_next_item_fish_2_0(self.contents.contents(), cursor, cutoff)
    }
}

/// Try to infer the history file type based on inspecting the data.
fn infer_file_type(contents: &[u8]) -> HistoryFileType {
    assert!(!contents.is_empty(), "File should never be empty");
    if contents[0] == b'#' {
        HistoryFileType::Fish1_x
    } else {
        // assume new fish
        HistoryFileType::Fish2_0
    }
}

impl TryFrom<HistoryRegion> for RawHistoryFile {
    type Error = std::io::Error;

    fn try_from(region: HistoryRegion) -> std::io::Result<Self> {
        if !region.is_empty() && infer_file_type(&region) == HistoryFileType::Fish1_x {
            let error_message = "unsupported history file format 1.x";
            flog!(error, error_message);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                error_message,
            ));
        }
        Ok(Self { region })
    }
}

impl HistoryItem {
    /// Write this history item to some writer.
    pub fn write_to(&self, writer: &mut impl std::io::Write) -> std::io::Result<()> {
        assert!(self.should_write_to_disk(), "Item should not be persisted");

        let mut cmd = wcs2bytes(self.str());
        escape_yaml_fish_2_0(&mut cmd);
        writer.write_all(b"- cmd: ")?;
        writer.write_all(&cmd)?;
        writer.write_all(b"\n")?;
        let last_added = self.last_added_timestamp();
        let first_added = self.first_added_timestamp();
        writeln!(
            writer,
            "  {}: {}",
            LAST_ADDED_TIMESTAMP_KEY,
            time_to_seconds(last_added)
        )?;
        if first_added != last_added {
            writeln!(
                writer,
                "  {}: {}",
                FIRST_ADDED_TIMESTAMP_KEY,
                time_to_seconds(first_added)
            )?;
        }

        let paths = self.get_required_paths();
        if !paths.is_empty() {
            writeln!(writer, "  paths:")?;
            for path in paths {
                let mut path = wcs2bytes(path);
                escape_yaml_fish_2_0(&mut path);
                writer.write_all(b"    - ")?;
                writer.write_all(&path)?;
                writer.write_all(b"\n")?;
            }
        }
        Ok(())
    }
}

/// Check if we should mmap the file.
/// Don't try mmap() on non-local filesystems.
#[cfg(unix)]
fn should_mmap() -> bool {
    // mmap only if we are known not-remote.
    path_get_data_remoteness() != DirRemoteness::Remote
}

pub fn time_to_seconds(ts: SystemTime) -> i64 {
    match ts.duration_since(UNIX_EPOCH) {
        Ok(d) => {
            // after epoch
            i64::try_from(d.as_secs()).unwrap()
        }
        Err(e) => {
            // before epoch
            -i64::try_from(e.duration().as_secs()).unwrap()
        }
    }
}
