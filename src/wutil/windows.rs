//! Native filesystem services. All identity ingress is checked, all opens share deletion, and
//! errors remain Win32 errors. Relative names here use the ambient startup cwd; evaluator callers
//! must first resolve against their parser's cwd with native_resolve_path.
use crate::path::{
    native_absolute_path, native_path_text, native_resolve_path, native_root_length,
};
use crate::prelude::*;
use fish_widestring::{native_to_shell_text_owned, shell_text_to_osstring, utf16_to_wcstring};
use std::{
    ffi::c_void,
    fs::{self, File},
    io,
    mem::{MaybeUninit, size_of},
    ops::{BitOr, BitOrAssign},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, RawHandle},
    },
    path::{Path, PathBuf},
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn GetFileInformationByHandleEx(
        handle: RawHandle,
        class: i32,
        information: *mut c_void,
        size: u32,
    ) -> i32;
    fn GetFinalPathNameByHandleW(
        handle: RawHandle,
        buffer: *mut u16,
        length: u32,
        flags: u32,
    ) -> u32;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AccessFlags(u8);
impl AccessFlags {
    pub const F_OK: Self = Self(0);
    pub const R_OK: Self = Self(1);
    pub const W_OK: Self = Self(2);
    pub const X_OK: Self = Self(4);
    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}
impl BitOr for AccessFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl BitOrAssign for AccessFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.insert(rhs);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NativeFileIdentity {
    pub volume: u64,
    pub file_id: [u8; 16],
}

#[repr(C)]
pub(crate) struct NativeBasicInfo {
    pub creation_time: i64,
    pub last_access_time: i64,
    pub last_write_time: i64,
    pub change_time: i64,
    pub attributes: u32,
}

#[repr(C)]
struct NativeStandardInfo {
    allocation_size: i64,
    end_of_file: i64,
    number_of_links: u32,
    delete_pending: u8,
    directory: u8,
}

/// # Safety
/// T must be the exact Win32 output layout for class, containing no invalid bit patterns.
pub(crate) unsafe fn native_information<T>(file: &File, class: i32) -> io::Result<T> {
    let mut result = MaybeUninit::<T>::uninit();
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            class,
            result.as_mut_ptr().cast(),
            size_of::<T>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { result.assume_init() })
}

fn path_components(path: &wstr) -> impl Iterator<Item = &wstr> {
    path.as_char_slice()
        .split(|c| *c == '/' || *c == '\\')
        .map(wstr::from_char_slice)
}

/// Open a validated absolute filesystem name. Long ordinary paths are prefixed only after checked
/// namespace parsing and lexical normalization; explicit extended paths retain literal spelling.
pub fn native_open(path: &Path, access: u32, follow: bool) -> io::Result<File> {
    let text = native_path_text(path)?;
    native_absolute_path(&text)?;
    if path_components(&text).any(|c| c == "." || c == "..") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native_open requires component-resolved paths",
        ));
    }
    let text = if text.starts_with(L!("\\\\?\\")) {
        text
    } else {
        normalize_path(&text, true)
    };
    let os = shell_text_to_osstring(&text)?;
    let mut name: Vec<u16> = os.encode_wide().collect();
    if !text.starts_with(L!("\\\\?\\")) {
        for c in &mut name {
            if *c == u16::from(b'/') {
                *c = u16::from(b'\\');
            }
        }
        let mut extended: Vec<u16> = if name.starts_with(&[92, 92]) {
            "\\\\?\\UNC\\".encode_utf16().collect()
        } else {
            "\\\\?\\".encode_utf16().collect()
        };
        extended.extend_from_slice(if name.starts_with(&[92, 92]) {
            &name[2..]
        } else {
            &name
        });
        name = extended;
    }
    name.push(0);
    let flags = 0x0200_0000 | if follow { 0 } else { 0x0020_0000 }; // BACKUP_SEMANTICS, OPEN_REPARSE_POINT
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            access,
            1 | 2 | 4,
            std::ptr::null(),
            3,
            flags,
            std::ptr::null_mut(),
        )
    };
    if handle == (-1isize) as RawHandle {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

/// Current physical name of an owned handle, including renames. A deleted/unnameable object fails;
/// it is not silently replaced by the parser's stale logical spelling.
pub fn native_final_path(file: &File) -> io::Result<WString> {
    let standard: NativeStandardInfo = unsafe { native_information(file, 1)? };
    if standard.delete_pending != 0 {
        return Err(io::Error::from_raw_os_error(303));
    }
    let mut name = vec![0u16; 256];
    loop {
        let count = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                name.as_mut_ptr(),
                name.len() as u32,
                0,
            )
        };
        if count == 0 {
            return Err(io::Error::last_os_error());
        }
        if count as usize >= name.len() {
            name.resize(count as usize + 1, 0);
            continue;
        }
        name.truncate(count as usize);
        let text = native_to_shell_text_owned(utf16_to_wcstring(&name)?);
        // Keep the extended prefix: removing it would alias literal trailing dots/spaces or devices.
        native_absolute_path(&text)?;
        return Ok(text);
    }
}

fn process_cwd() -> io::Result<WString> {
    let cwd = native_path_text(&std::env::current_dir()?)?;
    native_absolute_path(&cwd)?;
    Ok(cwd)
}

fn resolved(path: &wstr) -> io::Result<PathBuf> {
    match native_absolute_path(path) {
        Ok(path) => Ok(path),
        Err(_) => native_resolve_path(path, &process_cwd()?),
    }
}

/// Physically resolve one component at a time. In particular link/.. walks the link target's
/// parent, rather than letting Win32's lexical full-path conversion erase the link first.
/// Missing-final mode never accepts an absent/non-directory intermediate component.
pub fn native_realpath_with_cwd(
    path: &wstr,
    cwd: &wstr,
    allow_missing_final: bool,
) -> io::Result<WString> {
    let absolute = native_resolve_path(path, cwd)?;
    let text = native_path_text(&absolute)?;
    let root = native_root_length(&text);
    native_walk_relative(
        &native_absolute_path(text.slice_to(root))?,
        text.slice_from(root),
        allow_missing_final,
    )
}

/// Resolve ordinary relative syntax without ever stripping a literal extended cwd identity.
pub(crate) fn native_walk_relative(
    base: &Path,
    relative: &wstr,
    allow_missing_final: bool,
) -> io::Result<WString> {
    let mut cursor = base.to_path_buf();
    let mut handle = native_open(&cursor, 0, true)?;
    cursor = native_absolute_path(&native_final_path(&handle)?)?;
    let mut components = path_components(relative)
        .filter(|c| !c.is_empty())
        .peekable();
    while let Some(component) = components.next() {
        if !handle.metadata()?.is_dir() {
            return Err(io::Error::from_raw_os_error(267));
        }
        if component == "." {
            continue;
        }
        if component == ".." {
            // PathBuf::pop cannot remove a drive, UNC share or volume root.
            cursor.pop();
        } else {
            cursor.push(shell_text_to_osstring(component)?);
        }
        match native_open(&cursor, 0, true) {
            Ok(opened) => {
                handle = opened;
                cursor = native_absolute_path(&native_final_path(&handle)?)?;
            }
            Err(error)
                if allow_missing_final
                    && components.peek().is_none()
                    && error.kind() == io::ErrorKind::NotFound =>
            {
                // Missing-target errors also describe dangling reparses. Only an absent
                // name, not an existing unresolved object, permits lexical final output.
                match fs::symlink_metadata(&cursor) {
                    Err(missing) if missing.kind() == io::ErrorKind::NotFound => {
                        return native_path_text(&cursor);
                    }
                    Err(metadata_error) => return Err(metadata_error),
                    Ok(_) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    native_final_path(&handle)
}

pub fn native_realpath(path: &wstr) -> io::Result<WString> {
    let absolute = resolved(path)?;
    let text = native_path_text(&absolute)?;
    native_realpath_with_cwd(&text, &text, false)
}

pub fn native_realpath_allow_missing(path: &wstr) -> io::Result<WString> {
    let absolute = resolved(path)?;
    let text = native_path_text(&absolute)?;
    native_realpath_with_cwd(&text, &text, true)
}

/// fish realpath permits a missing final component, but not a missing parent.
pub fn wrealpath(path: &wstr) -> io::Result<WString> {
    native_realpath_allow_missing(path)
}

fn object_path(path: &wstr, follow: bool) -> io::Result<PathBuf> {
    if follow {
        return native_absolute_path(&native_realpath(path)?);
    }
    let absolute = resolved(path)?;
    let text = native_path_text(&absolute)?;
    let leaf = wbasename(&text);
    if native_root_length(&text) == text.len() || leaf == "." || leaf == ".." {
        return native_absolute_path(&native_realpath(&text)?);
    }
    let parent = native_realpath(wdirname(&text))?;
    let mut result = native_absolute_path(&parent)?;
    result.push(shell_text_to_osstring(leaf)?);
    Ok(result)
}

pub fn native_file_identity(path: &wstr, follow: bool) -> io::Result<NativeFileIdentity> {
    let file = native_open(&object_path(path, follow)?, 0, follow)?;
    native_identity_for_file(&file)
}

pub(crate) fn native_identity_for_file(file: &File) -> io::Result<NativeFileIdentity> {
    unsafe { native_information(file, 18) } // FileIdInfo: full 64-bit volume and 128-bit object ID.
}

pub fn native_directory_case_sensitive(path: &wstr) -> io::Result<bool> {
    let file = native_open(&object_path(path, true)?, 0x80, true)?; // FILE_READ_ATTRIBUTES
    if !file.metadata()?.is_dir() {
        return Err(io::Error::from_raw_os_error(267));
    }
    let flags: u32 = unsafe { native_information(&file, 23)? }; // FileCaseSensitiveInfo
    Ok(flags & 1 != 0)
}

/// These predicates ask whether this token can actually open the object for the requested data
/// rights, not whether a read-only attribute or guessed POSIX mode looks permissive. They do not
/// promise a subsequent operation cannot race. X_OK is FILE_TRAVERSE for directories and the same
/// .exe provider + FILE_EXECUTE policy as discovery for regular files.
pub fn waccess(path: &wstr, mode: AccessFlags) -> io::Result<()> {
    let original = path;
    let path = object_path(path, true)?;
    let mut rights = 0;
    if mode.contains(AccessFlags::R_OK) {
        rights |= 1;
    }
    if mode.contains(AccessFlags::W_OK) {
        rights |= 2;
    }
    if mode.contains(AccessFlags::X_OK) {
        let metadata = native_open(&path, 0, true)?.metadata()?;
        if !metadata.is_dir() {
            if !metadata.is_file() {
                return Err(io::Error::from_raw_os_error(5));
            }
            let original = native_path_text(&resolved(original)?)?;
            crate::path::native_resolve_executable(&original)?;
        }
        rights |= 0x20;
    }
    native_open(&path, rights, true).map(|_| ())
}

pub fn wstat(path: &wstr) -> io::Result<fs::Metadata> {
    native_open(&object_path(path, true)?, 0, true)?.metadata()
}
pub fn lwstat(path: &wstr) -> io::Result<fs::Metadata> {
    native_open(&object_path(path, false)?, 0, false)?.metadata()
}
pub fn wreadlink(path: &wstr) -> Option<WString> {
    fs::read_link(object_path(path, false).ok()?)
        .ok()
        .and_then(|p| native_path_text(&p).ok())
}
pub fn wunlink(path: &wstr) -> io::Result<()> {
    fs::remove_file(object_path(path, false)?)
}
pub fn wgetcwd() -> io::Result<WString> {
    process_cwd()
}

/// Lexical normalization never opens an object and never claims two spellings identify one file.
/// Explicit extended names are literal, including trailing dots/spaces; they are not normalized.
pub fn normalize_path(path: &wstr, _allow_leading_double_slashes: bool) -> WString {
    if path.starts_with(L!("\\\\?\\")) {
        return path.to_owned();
    }
    let root = native_root_length(path);
    if root == 0
        && path
            .as_char_slice()
            .get(..2)
            .is_some_and(|s| s.iter().all(|c| *c == '/' || *c == '\\'))
    {
        return path.to_owned();
    }
    let mut result = WString::with_capacity(path.len().max(1));
    result.push_utfstr(path.slice_to(root));
    for c in result.as_char_slice_mut() {
        if *c == '\\' {
            *c = '/';
        }
    }
    let rooted = root > 0
        && path
            .as_char_slice()
            .get(root - 1)
            .is_some_and(|c| *c == '/' || *c == '\\');
    for part in path_components(path.slice_from(root)) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            let last = result
                .as_char_slice()
                .iter()
                .rposition(|c| *c == '/')
                .map_or(root, |position| position + 1);
            if last < result.len() && result.slice_from(last) != ".." {
                result.truncate(if last > root { last - 1 } else { root });
                continue;
            }
            if rooted {
                continue;
            }
        }
        if !result.is_empty()
            && !result.ends_with('/')
            && !(root == 2 && result.len() == root && result.ends_with(':'))
        {
            result.push('/');
        }
        result.push_utfstr(part);
    }
    if result.is_empty() {
        result.push('.');
    }
    result
}

/// Collapse leading dot components against cwd so a deleted logical cwd can be escaped. Internal
/// missing/.. components remain in the candidate and must be validated by the physical walker.
pub fn path_normalize_for_cd(cwd: &wstr, path: &wstr) -> WString {
    if native_root_length(path) != 0 {
        return crate::path::path_apply_working_directory(path, cwd);
    }
    let mut base = cwd.to_owned();
    let mut components = path_components(path).peekable();
    while let Some(component) = components.peek() {
        if component.is_empty() || *component == "." {
            components.next();
        } else if *component == ".." {
            let parent = wdirname(&base);
            let (length, dot) = (parent.len(), parent == ".");
            if dot {
                base.clear();
                base.push('.');
            } else {
                base.truncate(length);
            }
            components.next();
        } else {
            break;
        }
    }
    let mut tail = WString::new();
    for component in components {
        crate::path::append_path_component(&mut tail, component);
    }
    if tail.is_empty() {
        base
    } else {
        crate::path::path_apply_working_directory(&tail, &base)
    }
}

pub fn wdirname(mut path: &wstr) -> &wstr {
    if path.is_empty() {
        return L!(".");
    }
    let root = native_root_length(path);
    while path.len() > root && (path.ends_with('/') || path.ends_with('\\')) {
        path = path.slice_to(path.len() - 1);
    }
    if path.len() <= root {
        return path;
    }
    let last = path.chars().rposition(|c| c == '/' || c == '\\');
    let Some(last) = last else {
        return if root == 2 {
            path.slice_to(root)
        } else {
            L!(".")
        };
    };
    if last < root {
        return path.slice_to(root);
    }
    path = path.slice_to(last);
    while path.len() > root && (path.ends_with('/') || path.ends_with('\\')) {
        path = path.slice_to(path.len() - 1);
    }
    if path.is_empty() { L!(".") } else { path }
}

pub fn wbasename(mut path: &wstr) -> &wstr {
    if path.is_empty() {
        return L!(".");
    }
    let root = native_root_length(path);
    while path.len() > root && (path.ends_with('/') || path.ends_with('\\')) {
        path = path.slice_to(path.len() - 1);
    }
    if path.len() <= root {
        return path;
    }
    let start = path
        .chars()
        .rposition(|c| c == '/' || c == '\\')
        .map_or(root, |i| i + 1);
    path.slice_from(start)
}
