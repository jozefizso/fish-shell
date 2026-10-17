use crate::wutil::wstr;
#[cfg(unix)]
use fish_widestring::wcs2zstring;
use std::fs::File;
#[cfg(unix)]
use std::{ffi::{CStr, OsStr}, fs::{self, Metadata}, os::unix::prelude::*};

/// Actual Unix filesystem object identity. Windows uses NativeFileIdentity, not invented inodes.
#[cfg(unix)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Hash)]
pub struct DevInode {
    pub device: u64,
    pub inode: u64,
}

/// A stable object identity plus change-generation observations. Timestamps are Unix-epoch
/// seconds/nanoseconds on both targets; Windows change time is not creation time.
#[derive(Debug, Clone, Hash, PartialEq, PartialOrd)]
pub struct FileId {
    #[cfg(unix)]
    pub dev_inode: DevInode,
    #[cfg(windows)]
    pub native_identity: super::NativeFileIdentity,
    pub size: u64,
    pub change_seconds: i64,
    pub change_nanoseconds: i64,
    pub mod_seconds: i64,
    pub mod_nanoseconds: i64,
}

impl FileId {
    #[cfg(unix)]
    pub fn from_md(buf: &Metadata) -> Self {
        #[allow(clippy::useless_conversion)]
        FileId {
            dev_inode: DevInode { device: buf.dev(), inode: buf.ino() },
            size: buf.size(),
            change_seconds: buf.ctime().into(),
            change_nanoseconds: buf.ctime_nsec().into(),
            mod_seconds: buf.mtime().into(),
            mod_nanoseconds: buf.mtime_nsec().into(),
        }
    }
}

pub fn file_id_for_file(file: &File) -> std::io::Result<FileId> {
    #[cfg(unix)]
    { file.metadata().map(|md| FileId::from_md(&md)) }
    #[cfg(windows)]
    {
        let native_identity = super::native_identity_for_file(file)?;
        let times: super::NativeBasicInfo = unsafe { super::native_information(file, 0)? };
        // FILETIME has 100ns units from 1601. Euclidean division also preserves pre-1970 times.
        let split = |ticks: i64| (ticks.div_euclid(10_000_000) - 11_644_473_600,
            ticks.rem_euclid(10_000_000) * 100);
        let (change_seconds, change_nanoseconds) = split(times.change_time);
        let (mod_seconds, mod_nanoseconds) = split(times.last_write_time);
        Ok(FileId { native_identity, size: file.metadata()?.len(), change_seconds,
            change_nanoseconds, mod_seconds, mod_nanoseconds })
    }
}

pub fn file_id_for_path(path: &wstr) -> std::io::Result<FileId> {
    #[cfg(unix)]
    { file_id_for_path_narrow(&wcs2zstring(path)) }
    #[cfg(windows)]
    {
        let path = super::native_realpath(path)?;
        let file = super::native_open(&crate::path::native_absolute_path(&path)?, 0x80, true)?;
        file_id_for_file(&file)
    }
}

#[cfg(unix)]
pub fn file_id_for_path_narrow(path: &CStr) -> std::io::Result<FileId> {
    let path = OsStr::from_bytes(path.to_bytes());
    fs::metadata(path).map(|md| FileId::from_md(&md))
}
