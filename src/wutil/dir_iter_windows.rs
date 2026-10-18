//! Native directory enumeration. Win32 enumerates every name; fish, not FindFirstFile, matches globs.
use crate::{
    path::{native_absolute_path, native_path_text},
    prelude::*,
    wutil::{
        NativeFileIdentity, native_directory_case_sensitive, native_file_identity,
        native_realpath_with_cwd,
    },
};
use fish_widestring::{native_to_shell_text_owned, osstr2wcstring};
use std::{
    cell::OnceCell,
    ffi::{OsString, c_void},
    fs, io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DirEntryType {
    Fifo = 1,
    Chr,
    Dir,
    Blk,
    Reg,
    Lnk,
    Sock,
    Whiteout,
}

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xa0000003;
const IO_REPARSE_TAG_SYMLINK: u32 = 0xa000000c;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_NO_MORE_FILES: i32 = 18;

#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}

#[repr(C)]
struct FindData {
    attributes: u32,
    creation: FileTime,
    access: FileTime,
    write: FileTime,
    size_high: u32,
    size_low: u32,
    reparse_tag: u32,
    reserved: u32,
    name: [u16; 260],
    alternate_name: [u16; 14],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn FindFirstFileW(pattern: *const u16, data: *mut FindData) -> *mut c_void;
    fn FindNextFileW(handle: *mut c_void, data: *mut FindData) -> i32;
    fn FindClose(handle: *mut c_void) -> i32;
}

struct FindHandle(*mut c_void);
impl Drop for FindHandle {
    fn drop(&mut self) {
        unsafe {
            FindClose(self.0);
        }
    }
}

/// The name is checked UTF-16 converted exactly once to fish's canonical identity text.
/// Entries borrow no enumeration buffer or handle; metadata opens are short lived.
pub struct DirEntry {
    pub name: WString,
    path: PathBuf,
    policy_path: PathBuf,
    attributes: u32,
    reparse_tag: u32,
    typ: OnceCell<Option<DirEntryType>>,
    identity: OnceCell<NativeFileIdentity>,
}

impl DirEntry {
    /// Validate a selected name before following or publishing it. An unrelated
    /// unsupported entry must not prevent matching another name in this directory.
    pub fn validate(&self) -> io::Result<()> {
        native_absolute_path(&native_path_text(&self.policy_path)?)?;
        if self.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            && !matches!(
                self.reparse_tag,
                IO_REPARSE_TAG_SYMLINK | IO_REPARSE_TAG_MOUNT_POINT
            )
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "unsupported native reparse tag {:#x} at {}",
                    self.reparse_tag,
                    self.path.display()
                ),
            ));
        }
        Ok(())
    }

    pub fn check_type(&self) -> Option<DirEntryType> {
        *self.typ.get_or_init(|| {
            if self.attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
                return Some(if self.attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                    DirEntryType::Dir
                } else {
                    DirEntryType::Reg
                });
            }
            if !matches!(
                self.reparse_tag,
                IO_REPARSE_TAG_SYMLINK | IO_REPARSE_TAG_MOUNT_POINT
            ) {
                // Selected entries are validated before traversal; never infer a target here.
                return None;
            }
            match fs::metadata(&self.path) {
                Ok(metadata) if metadata.is_dir() => Some(DirEntryType::Dir),
                Ok(metadata) if metadata.is_file() => Some(DirEntryType::Reg),
                Ok(_) => None,
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        || error.raw_os_error() == Some(1921) =>
                {
                    Some(DirEntryType::Lnk)
                }
                Err(_) => None,
            }
        })
    }

    pub fn is_dir(&self) -> bool {
        self.check_type() == Some(DirEntryType::Dir)
    }

    pub fn is_possible_link(&self) -> Option<bool> {
        Some(self.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }

    pub fn file_identity(&self) -> io::Result<NativeFileIdentity> {
        if let Some(identity) = self.identity.get() {
            return Ok(*identity);
        }
        let identity = native_file_identity(&native_path_text(&self.path)?, true)?;
        let _ = self.identity.set(identity);
        Ok(identity)
    }
}

/// A single owned enumeration handle. EOF, errors, rewind and Drop all close it.
/// No process cwd changes, lossy names, Win32 user-pattern matching or retained metadata handles.
pub struct DirIter {
    path: PathBuf,
    policy_path: PathBuf,
    handle: Option<FindHandle>,
    first: bool,
    done: bool,
    dots: u8,
    with_dots: bool,
    case_sensitive: bool,
    identity: NativeFileIdentity,
    data: FindData,
    entry: DirEntry,
}

impl DirIter {
    pub fn new(path: &wstr) -> io::Result<Self> {
        Self::new_impl(path, false)
    }
    pub fn new_with_dots(path: &wstr) -> io::Result<Self> {
        Self::new_impl(path, true)
    }

    fn new_impl(path: &wstr, with_dots: bool) -> io::Result<Self> {
        // Enumeration has no ambient-cwd fallback: relative callers resolve against their owning
        // Environment before opening this backend.
        let policy_path = native_absolute_path(path)?;
        let text = native_path_text(&policy_path)?;
        // Physical resolution supplies an extended identity without lexically erasing link/..
        // or blindly prepending a verbatim prefix to an ordinary path.
        let text = native_realpath_with_cwd(&text, &text, false)?;
        let path = native_absolute_path(&text)?;
        let case_sensitive = native_directory_case_sensitive(&text)?;
        let identity = native_file_identity(&text, true)?;
        let mut iter = Self {
            path,
            policy_path,
            handle: None,
            first: false,
            done: false,
            dots: if with_dots { 2 } else { 0 },
            with_dots,
            case_sensitive,
            identity,
            data: unsafe { std::mem::zeroed() },
            entry: DirEntry {
                name: WString::new(),
                path: PathBuf::new(),
                policy_path: PathBuf::new(),
                attributes: 0,
                reparse_tag: 0,
                typ: OnceCell::new(),
                identity: OnceCell::new(),
            },
        };
        iter.start()?;
        Ok(iter)
    }

    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }
    pub fn file_identity(&self) -> NativeFileIdentity {
        self.identity
    }

    fn start(&mut self) -> io::Result<()> {
        let mut pattern: Vec<u16> = self.path.as_os_str().encode_wide().collect();
        if !pattern.last().is_some_and(|unit| matches!(*unit, 47 | 92)) {
            pattern.push(92);
        }
        pattern.extend([42, 0]);
        let handle = unsafe { FindFirstFileW(pattern.as_ptr(), &mut self.data) };
        if handle == (-1isize) as *mut c_void {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND)
                && fs::metadata(&self.path)?.is_dir()
            {
                self.done = true;
                return Ok(());
            }
            return Err(error);
        }
        self.handle = Some(FindHandle(handle));
        self.first = true;
        self.done = false;
        Ok(())
    }

    /// Reopening errors are delivered by next(), rather than being mistaken for EOF.
    pub fn rewind(&mut self) {
        self.handle = None;
        self.first = false;
        self.done = false;
        self.dots = if self.with_dots { 2 } else { 0 };
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<io::Result<&DirEntry>> {
        if self.dots != 0 {
            let name = if self.dots == 2 { L!(".") } else { L!("..") };
            self.dots -= 1;
            self.entry.name = name.to_owned();
            self.entry.path = self.path.clone();
            self.entry.policy_path = self
                .policy_path
                .join(if self.dots == 1 { "." } else { ".." });
            if self.dots == 0 {
                self.entry.path.pop();
            }
            self.entry.attributes = FILE_ATTRIBUTE_DIRECTORY;
            self.entry.reparse_tag = 0;
            self.entry.typ.take();
            self.entry.identity.take();
            return Some(Ok(&self.entry));
        }
        loop {
            if self.done {
                return None;
            }
            if self.handle.is_none() {
                if let Err(error) = self.start() {
                    self.done = true;
                    return Some(Err(error));
                }
                if self.done {
                    return None;
                }
            }
            if self.first {
                self.first = false;
            } else if unsafe { FindNextFileW(self.handle.as_ref().unwrap().0, &mut self.data) } == 0
            {
                let error = io::Error::last_os_error();
                self.handle = None;
                self.done = true;
                return if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) {
                    None
                } else {
                    Some(Err(error))
                };
            }
            let len = self
                .data
                .name
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(self.data.name.len());
            let units = &self.data.name[..len];
            if units == [46] || units == [46, 46] {
                continue;
            }
            let name = OsString::from_wide(units);
            let path = self.path.join(&name);
            self.entry.policy_path = self.policy_path.join(&name);
            let result = osstr2wcstring(&name)
                .map(native_to_shell_text_owned)
                .map_err(io::Error::from);
            let name = match result {
                Ok(name) => name,
                Err(error) => {
                    self.handle = None;
                    self.done = true;
                    return Some(Err(error));
                }
            };
            self.entry.name = name;
            self.entry.path = path;
            self.entry.attributes = self.data.attributes;
            self.entry.reparse_tag = self.data.reparse_tag;
            self.entry.typ.take();
            self.entry.identity.take();
            return Some(Ok(&self.entry));
        }
    }
}
