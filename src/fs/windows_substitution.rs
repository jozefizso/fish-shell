//! Handle-pinned private file storage for the non-evaluator substitution keeper.
use super::*;
use std::io::{Read, Write};
use std::ffi::OsStr;
use std::os::windows::{
    ffi::OsStrExt as _,
    fs::{MetadataExt as _, OpenOptionsExt as _},
    io::{AsRawHandle as _, FromRawHandle as _},
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn ReOpenFile(file: RawHandle, access: u32, share: u32, flags: u32) -> RawHandle;
}

#[repr(C)]
struct UnicodeName { length: u16, maximum: u16, buffer: *mut u16 }
#[repr(C)]
struct ObjectAttributes {
    length: u32, root: RawHandle, name: *mut UnicodeName, attributes: u32,
    security: *mut c_void, quality: *mut c_void,
}
#[repr(C)]
struct IoStatus { status: usize, information: usize }
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtCreateFile(file: *mut RawHandle, access: u32, attributes: *const ObjectAttributes,
        status: *mut IoStatus, allocation: *const i64, file_attributes: u32, sharing: u32,
        disposition: u32, options: u32, ea: *const c_void, ea_length: u32) -> i32;
    fn RtlNtStatusToDosError(status: i32) -> u32;
}
fn relative_file(parent: &File, name: &OsStr, access: u32, sharing: u32,
    disposition: u32, directory: bool, security: Option<&StagingSecurity>) -> io::Result<File> {
    let mut units = [0u16; 255];
    let mut length = 0;
    for unit in name.encode_wide() {
        if length == units.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "psub component exceeds 255 UTF-16 units"));
        }
        units[length] = unit;
        length += 1;
    }
    let mut name = UnicodeName { length: (length * 2) as u16, maximum: (length * 2) as u16, buffer: units.as_mut_ptr() };
    let attributes = ObjectAttributes { length: std::mem::size_of::<ObjectAttributes>() as u32,
        root: parent.as_raw_handle(), name: &mut name, attributes: 0,
        security: security.map_or(std::ptr::null_mut(), |security| security.0), quality: std::ptr::null_mut() };
    let mut status = IoStatus { status: 0, information: 0 };
    let mut raw = std::ptr::null_mut();
    let result = unsafe { NtCreateFile(&mut raw, access | 0x0010_0000, &attributes, &mut status,
        std::ptr::null(), 0x80, sharing, disposition,
        OPEN_REPARSE_POINT | 0x20 | if directory { 1 } else { 0x40 }, std::ptr::null(), 0) };
    if result < 0 {
        return Err(io::Error::from_raw_os_error(unsafe { RtlNtStatusToDosError(result) as i32 }));
    }
    Ok(unsafe { File::from_raw_handle(raw) })
}
fn delete_created(file: &File) -> io::Result<()> {
    let raw = unsafe { ReOpenFile(file.as_raw_handle(), DELETE, SHARE_ALL, OPEN_REPARSE_POINT) };
    if raw == (-1isize) as RawHandle { return Err(io::Error::last_os_error()); }
    mark_delete(&unsafe { File::from_raw_handle(raw) })
}

/// No pathname supplied by a user is ever accepted for deletion. This capability
/// is constructed only by exclusive creation in the physical temporary directory.
pub(crate) struct NativeSubstitutionFile {
    file: Option<File>,
    directory: Option<File>,
    leaf: OsString,
    path: PathBuf,
    identity: NativeFileIdentity,
    _ancestors: Vec<File>,
}

pub(crate) fn validate_substitution_suffix(suffix: &str) -> io::Result<()> {
    if suffix.encode_utf16().count() > 180
        || suffix.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c))
        || suffix.ends_with([' ', '.'])
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "psub suffix must be a literal Windows filename suffix (no separators, ADS, controls, trailing dot/space; at most 180 UTF-16 units)"));
    }
    Ok(())
}

impl NativeSubstitutionFile {
    pub(crate) fn create(suffix: &str) -> io::Result<Self> {
        validate_substitution_suffix(suffix)?;
        let parent = native_absolute_path(&native_realpath(&native_path_text(&std::env::temp_dir())?)?)?;
        let mut prefixes: Vec<_> = parent.ancestors().collect();
        prefixes.reverse();
        let mut ancestors = Vec::with_capacity(prefixes.len());
        for prefix in prefixes {
            let file = OpenOptions::new().access_mode(READ_ATTRIBUTES).share_mode(1 | 2)
                .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS).open(prefix)?;
            let metadata = file.metadata()?;
            if !metadata.is_dir() || metadata.file_attributes() & REPARSE_POINT != 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "psub temporary ancestor is a reparse point"));
            }
            ancestors.push(file);
        }
        let parent = ancestors.last().ok_or_else(|| io::Error::other("psub temporary root is unavailable"))?;
        // The exact N6 private workspace descriptor is applied atomically.
        // A private root also excludes other temporary-directory users from
        // the leaf's identity-checked close/open deletion boundary.
        let security = StagingSecurity::for_editor_directory()?;
        let directory = loop {
            let name = random_filename(OsString::from("fish_psub_"));
            match relative_file(parent, &name, READ_ATTRIBUTES | DELETE, 1 | 2, 2, true, Some(&security)) {
                Ok(directory) => break directory,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };
        let mut leaf = OsString::from("psub");
        leaf.push(suffix);
        let created = (|| -> io::Result<(File, NativeFileIdentity, PathBuf)> {
            let file = relative_file(&directory, &leaf, GENERIC_READ | GENERIC_WRITE, SHARE_ALL, 2, false, Some(&security))?;
            let information = (|| {
                let identity = native_identity_for_file(&file)?;
                let path = native_absolute_path(&native_final_path(&file)?)?;
                Ok::<_, io::Error>((identity, path))
            })();
            match information {
                Ok((identity, path)) => Ok((file, identity, path)),
                Err(error) => { delete_created(&file)?; Err(error) }
            }
        })();
        match created {
            Ok((file, identity, path)) => Ok(Self { file: Some(file), directory: Some(directory),
                leaf, path, identity, _ancestors: ancestors }),
            Err(error) => { mark_delete(&directory)?; Err(error) }
        }
    }

    pub(crate) fn write_frames(&mut self, input: &mut impl Read) -> io::Result<()> {
        let mut slab = [0u8; 65536];
        loop {
            let mut length = [0; 4];
            input.read_exact(&mut length)?;
            let length = u32::from_le_bytes(length) as usize;
            if length == 0 { break; }
            if length > slab.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "oversized psub data frame"));
            }
            input.read_exact(&mut slab[..length])?;
            self.file.as_mut().unwrap().write_all(&slab[..length])?;
        }
        self.file.as_ref().unwrap().sync_all()?;
        // Metadata-only opens do not participate in Windows sharing checks.
        // Retain read-data access so denying FILE_SHARE_DELETE really pins the
        // name, without DELETE access that CRT readers would have to share.
        let pin = relative_file(self.directory.as_ref().unwrap(), &self.leaf,
            GENERIC_READ, 1 | 2, 1, false, None)?;
        if native_identity_for_file(&pin)? != self.identity { return Err(changed()); }
        self.file = Some(pin);
        Ok(())
    }

    pub(crate) fn path(&self) -> &Path { &self.path }

    /// Helper-owned published files may be unlinked by an explicitly transferred
    /// consumer. psub never calls this and keeps its original sharing policy.
    pub(crate) fn allow_consumer_delete(&mut self) -> io::Result<()> {
        let pin = relative_file(self.directory.as_ref().unwrap(), &self.leaf,
            READ_ATTRIBUTES, SHARE_ALL, 1, false, None)?;
        if native_identity_for_file(&pin)? != self.identity { return Err(changed()); }
        self.file = Some(pin);
        Ok(())
    }

    pub(crate) fn remove(&mut self) -> io::Result<()> {
        drop(self.file.take());
        let Some(directory) = self.directory.as_ref() else { return Ok(()); };
        match relative_file(directory, &self.leaf, DELETE | READ_ATTRIBUTES, SHARE_ALL, 1, false, None) {
            Ok(file) => {
                checked_regular(&file)?;
                if native_identity_for_file(&file)? != self.identity { return Err(changed()); }
                mark_delete(&file)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        // Remove exactly the empty, owned root by its retained DELETE handle.
        // Unknown additions are never traversed or recursively deleted.
        mark_delete(directory)?;
        drop(self.directory.take());
        Ok(())
    }
}
impl Drop for NativeSubstitutionFile {
    fn drop(&mut self) {
        if let Err(error) = self.remove() {
            crate::flog!(synced_file_access, "Unable to remove owned psub file:", self.path.display(), error);
        }
    }
}
