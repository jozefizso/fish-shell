//! Cooperating native state transactions. The persistent parent lock is never replaced or
//! opened with delete sharing. Data files deliberately share read, write and deletion so old
//! readers may retain their snapshots across replacement. Lock/ACL failures are errors, not an
//! invitation to perform an unlocked write.
//!
//! Acknowledgement means the complete replacement was flushed, installed and reopened with its
//! checked native identity. It is a process-crash consistency boundary, not a power-loss promise:
//! FlushFileBuffers does not make ReplaceFileW's directory-name update durable on every device or
//! SMB server. Every writer of a protected file must participate in this lock protocol.
//!
//! Existing-target staging files receive that target handle's DACL at creation, protected from
//! broader parent inheritance before any contents are written. First-create staging retains
//! parent inheritance. Reading the target DACL requires READ_CONTROL; inability to obtain or
//! apply it fails the transaction. This does not copy owner, SACL, or mandatory integrity policy,
//! protect against principals allowed by that DACL or privileged administrators, or revoke
//! directory delete-child rights. Interrupted writes may leave protected orphan siblings;
//! ordinary cleanup is identity-checked and best-effort, not crash-time garbage collection.
use crate::{
    path::{native_absolute_path, native_path_text, native_resolve_path},
    prelude::*,
    wutil::{
        FileId, NativeFileIdentity, file_id_for_file, native_final_path, native_identity_for_file,
        native_realpath, wbasename, wdirname,
    },
};
use fish_tempfile::random_filename;
use fish_widestring::shell_text_to_osstring;
use std::{
    ffi::{OsString, c_void},
    fs::{File, OpenOptions},
    io,
    os::windows::{
        ffi::OsStrExt as _,
        fs::{MetadataExt as _, OpenOptionsExt as _},
        io::{AsRawHandle as _, FromRawHandle as _, RawHandle},
    },
    path::{Path, PathBuf},
};

const SHARE_ALL: u32 = 1 | 2 | 4;
const READ_ATTRIBUTES: u32 = 0x80;
const DELETE: u32 = 0x0001_0000;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;
const REPARSE_POINT: u32 = 0x400;

#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit_handle: i32,
}

#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: RawHandle,
}

impl Default for Overlapped {
    fn default() -> Self {
        Self {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            event: std::ptr::null_mut(),
        }
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        creation: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn LockFileEx(
        file: RawHandle,
        flags: u32,
        reserved: u32,
        low: u32,
        high: u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn ReplaceFileW(
        replaced: *const u16,
        replacement: *const u16,
        backup: *const u16,
        flags: u32,
        exclude: *mut c_void,
        reserved: *mut c_void,
    ) -> i32;
    fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    fn SetFileInformationByHandle(
        file: RawHandle,
        class: i32,
        information: *const c_void,
        size: u32,
    ) -> i32;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn GetSecurityInfo(
        handle: RawHandle,
        kind: u32,
        information: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut c_void,
        sacl: *mut *mut c_void,
        descriptor: *mut *mut c_void,
    ) -> u32;
    fn SetSecurityDescriptorControl(descriptor: *mut c_void, mask: u16, bits: u16) -> i32;
}

/// GetSecurityInfo's self-relative descriptor owns the effective DACL (including inherited
/// entries). Keep the allocation alive until CreateFileW has consumed SECURITY_ATTRIBUTES.
struct StagingSecurity(*mut c_void);

impl StagingSecurity {
    fn for_editor_directory() -> io::Result<Self> {
        // Protected, inheritable owner/SYSTEM-only access. OWNER RIGHTS refers to
        // the object's real native owner, not a guessed account name or Unix uid.
        const TEXT: [u16; 36] = {
            let bytes = b"D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;OW)\0";
            let mut wide = [0; 36];
            let mut index = 0;
            while index < bytes.len() {
                wide[index] = bytes[index] as u16;
                index += 1;
            }
            wide
        };
        let mut descriptor = Self(std::ptr::null_mut());
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                TEXT.as_ptr(),
                1,
                &mut descriptor.0,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(descriptor)
    }

    fn for_target(file: &File) -> io::Result<Self> {
        let mut descriptor = Self(std::ptr::null_mut());
        let error = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                1,
                4,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descriptor.0,
            )
        };
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        // SE_DACL_PROTECTED prevents creation from augmenting this effective DACL with the
        // staging directory's potentially broader inheritable ACEs. NULL/empty DACLs retain
        // their native meaning; there is deliberately no permissive fallback on query failure.
        if unsafe { SetSecurityDescriptorControl(descriptor.0, 0x1000, 0x1000) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(descriptor)
    }
}

impl Drop for StagingSecurity {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}

fn changed() -> io::Error {
    io::Error::other("native state file changed outside its parent lock")
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn checked_regular(file: &File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native state files and lock files must be regular, non-reparse files",
        ));
    }
    Ok(())
}

fn open_data(path: &Path, access: u32) -> io::Result<File> {
    let file = OpenOptions::new()
        .access_mode(access)
        .share_mode(SHARE_ALL)
        .custom_flags(OPEN_REPARSE_POINT)
        .open(path)?;
    checked_regular(&file)?;
    Ok(file)
}

/// Retain every physical ancestor without delete sharing, not merely a stale spelling of the
/// parent. An ancestor cannot be renamed or exchanged for a junction while path-based Win32
/// replacement is in progress. Reparse points are resolved before these guards are acquired and
/// then rejected if any component was exchanged during acquisition.
struct ParentLock {
    _lock_file: File,
    _ancestors: Vec<File>,
    path: PathBuf,
}

fn pin_directory_ancestors(parent: &Path) -> io::Result<Vec<File>> {
    let mut ancestors = Vec::new();
    // Resolve aliases first, then pin from the root down. No subsequent path
    // operation can traverse an ancestor exchanged for a junction or rename.
    let mut prefixes: Vec<_> = parent.ancestors().collect();
    prefixes.reverse();
    for prefix in prefixes {
        let file = OpenOptions::new()
            .access_mode(READ_ATTRIBUTES)
            .share_mode(1 | 2)
            .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
            .open(prefix)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native parent changed or is a reparse point",
            ));
        }
        ancestors.push(file);
    }
    Ok(ancestors)
}

impl ParentLock {
    fn new(path: &wstr, exclusive: bool) -> io::Result<(Self, PathBuf)> {
        let absolute = match native_absolute_path(path) {
            Ok(path) => path,
            Err(_) => {
                let cwd = native_path_text(&std::env::current_dir()?)?;
                native_resolve_path(path, &cwd)?
            }
        };
        let absolute = native_path_text(&absolute)?;
        let leaf = wbasename(&absolute);
        if leaf.is_empty() || leaf == "." || leaf == ".." {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "missing state file name",
            ));
        }
        let parent = native_absolute_path(&native_realpath(wdirname(&absolute))?)?;
        let ancestors = pin_directory_ancestors(&parent)?;
        // Use the guarded object's physical spelling, never the caller's possibly aliased one.
        let parent = native_absolute_path(&native_final_path(ancestors.last().unwrap())?)?;
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(1 | 2)
            .custom_flags(OPEN_REPARSE_POINT)
            .open(parent.join(".fish_lock"))?;
        checked_regular(&lock_file)?;
        let mut overlapped = Overlapped::default();
        if unsafe {
            LockFileEx(
                lock_file.as_raw_handle(),
                if exclusive { 2 } else { 0 },
                0,
                1,
                0,
                &mut overlapped,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let target = parent.join(shell_text_to_osstring(leaf)?);
        Ok((
            Self {
                _lock_file: lock_file,
                _ancestors: ancestors,
                path: parent,
            },
            target,
        ))
    }
}

pub enum LockingMode {
    Shared,
    Exclusive(WriteMethod),
}

pub enum WriteMethod {
    Append,
    RenameIntoPlace,
}

/// A data handle and its stable parent lock. Dropping the handle releases the kernel lock.
pub struct LockedFile {
    data_file: File,
    _snapshot: Option<Temporary>,
    _parent: ParentLock,
}

impl LockedFile {
    pub fn new(mode: LockingMode, path: &wstr) -> io::Result<Self> {
        let (parent, path) = ParentLock::new(path, matches!(mode, LockingMode::Exclusive(_)))?;
        let mut snapshot = None;
        let data_file = match mode {
            LockingMode::Shared => open_data(&path, GENERIC_READ)?,
            LockingMode::Exclusive(WriteMethod::Append) => {
                let file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .share_mode(SHARE_ALL)
                    .custom_flags(OPEN_REPARSE_POINT)
                    .open(&path)?;
                checked_regular(&file)?;
                file
            }
            LockingMode::Exclusive(WriteMethod::RenameIntoPlace) => {
                match open_data(&path, GENERIC_READ | GENERIC_WRITE) {
                    Ok(file) => file,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        let mut empty = Temporary::new(&parent.path, None)?;
                        let file = empty.file.take().unwrap();
                        snapshot = Some(empty);
                        file
                    }
                    Err(error) => return Err(error),
                }
            }
        };
        if snapshot.is_none()
            && file_id_for_file(&open_data(&path, READ_ATTRIBUTES)?)?
                != file_id_for_file(&data_file)?
        {
            return Err(changed());
        }
        Ok(Self {
            data_file,
            _snapshot: snapshot,
            _parent: parent,
        })
    }

    pub fn get(&self) -> &File {
        &self.data_file
    }
    pub fn get_mut(&mut self) -> &mut File {
        &mut self.data_file
    }
}

/// FlushFileBuffers on the actual native handle; errors are never treated as a saved update.
pub fn fsync(file: &File) -> io::Result<()> {
    file.sync_all()
}

pub struct PotentialUpdate<UserData> {
    pub do_save: bool,
    pub data: UserData,
}

pub fn lock_and_load<F, UserData>(path: &wstr, load: F) -> io::Result<(FileId, UserData)>
where
    F: Fn(&File, FileId) -> io::Result<UserData>,
{
    let (_parent, path) = ParentLock::new(path, false)?;
    let file = open_data(&path, GENERIC_READ)?;
    let id = file_id_for_file(&file)?;
    let data = load(&file, id.clone())?;
    if file_id_for_file(&file)? != id
        || file_id_for_file(&open_data(&path, READ_ATTRIBUTES)?)? != id
    {
        return Err(changed());
    }
    Ok((id, data))
}

/// Delete only the object we created, even if its random name was subsequently exchanged.
fn remove_owned(path: &Path, identity: NativeFileIdentity) -> io::Result<()> {
    let file = match open_data(path, DELETE | READ_ATTRIBUTES) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if native_identity_for_file(&file)? != identity {
        return Err(changed());
    }
    mark_delete(&file)
}

fn mark_delete(file: &File) -> io::Result<()> {
    let delete = 1u8; // FILE_DISPOSITION_INFO::DeleteFile (BOOLEAN, not Win32 BOOL).
    if unsafe {
        SetFileInformationByHandle(file.as_raw_handle(), 4, (&delete as *const u8).cast(), 1)
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

struct Temporary {
    path: PathBuf,
    file: Option<File>,
    identity: NativeFileIdentity,
    remove_on_drop: bool,
}

impl Temporary {
    fn new(parent: &Path, security: Option<&StagingSecurity>) -> io::Result<Self> {
        let attributes = security.map(|security| SecurityAttributes {
            length: std::mem::size_of::<SecurityAttributes>() as u32,
            descriptor: security.0,
            inherit_handle: 0,
        });
        loop {
            let path = parent.join(random_filename(OsString::from(".fish_tmp_")));
            let raw = unsafe {
                CreateFileW(
                    wide(&path).as_ptr(),
                    GENERIC_READ | GENERIC_WRITE | DELETE,
                    SHARE_ALL,
                    attributes.as_ref().map_or(std::ptr::null(), |a| {
                        (a as *const SecurityAttributes).cast()
                    }),
                    1,
                    OPEN_REPARSE_POINT | 0x80,
                    std::ptr::null_mut(),
                )
            };
            let opened = if raw == (-1isize) as RawHandle {
                Err(io::Error::last_os_error())
            } else {
                Ok(unsafe { File::from_raw_handle(raw) })
            };
            match opened {
                Ok(file) => {
                    let identity = match native_identity_for_file(&file) {
                        Ok(identity) => identity,
                        Err(error) => {
                            mark_delete(&file)?;
                            return Err(error);
                        }
                    };
                    return Ok(Self {
                        path,
                        file: Some(file),
                        identity,
                        remove_on_drop: true,
                    });
                }
                // Retrying a random-name collision is not retrying a failed transaction.
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    fn flush_close(&mut self) -> io::Result<()> {
        fsync(self.file.as_ref().unwrap())?;
        // ReplaceFileW opens its replacement without sharing. A live replacement handle would
        // therefore make an otherwise valid transaction fail with ERROR_SHARING_VIOLATION.
        drop(self.file.take());
        Ok(())
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        drop(self.file.take());
        if self.remove_on_drop {
            if let Err(error) = remove_owned(&self.path, self.identity) {
                crate::flog!(
                    synced_file_access,
                    "Unable to remove owned native temporary:",
                    self.path.display(),
                    error
                );
            }
        }
    }
}

fn checked_committed(path: &Path, identity: NativeFileIdentity) -> io::Result<FileId> {
    let file = open_data(path, GENERIC_READ | GENERIC_WRITE)?;
    if native_identity_for_file(&file)? != identity {
        return Err(changed());
    }
    fsync(&file)?;
    let id = file_id_for_file(&file)?;
    if file_id_for_file(&open_data(path, READ_ATTRIBUTES)?)? != id {
        return Err(changed());
    }
    Ok(id)
}

fn commit_first(target: &Path, temporary: &mut Temporary) -> io::Result<FileId> {
    temporary.flush_close()?;
    // No MOVEFILE_REPLACE_EXISTING: an independently created target is never blindly erased.
    if unsafe { MoveFileExW(wide(&temporary.path).as_ptr(), wide(target).as_ptr(), 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    temporary.remove_on_drop = false;
    checked_committed(target, temporary.identity)
}

fn commit_replacement(
    target: &Path,
    old: &File,
    old_id: &FileId,
    temporary: &mut Temporary,
    parent: &Path,
) -> io::Result<FileId> {
    temporary.flush_close()?;
    if file_id_for_file(old)? != *old_id
        || file_id_for_file(&open_data(target, READ_ATTRIBUTES)?)? != *old_id
    {
        return Err(changed());
    }
    // A backup is essential: documented ReplaceFileW failures 1176/1177 can otherwise remove
    // the old name. Reserve a collision-free sibling, then allow ReplaceFileW to create it.
    let mut backup = Temporary::new(parent, None)?;
    drop(backup.file.take());
    remove_owned(&backup.path, backup.identity)?;
    backup.remove_on_drop = false;
    let replaced = unsafe {
        ReplaceFileW(
            wide(target).as_ptr(),
            wide(&temporary.path).as_ptr(),
            wide(&backup.path).as_ptr(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if replaced == 0 {
        let error = io::Error::last_os_error();
        // Restore only an absent target, never overwrite an unowned competing replacement.
        if matches!(&open_data(target, READ_ATTRIBUTES), Err(e) if e.kind() == io::ErrorKind::NotFound)
        {
            if let Ok(saved) = open_data(&backup.path, READ_ATTRIBUTES) {
                let ours = native_identity_for_file(&saved).ok() == Some(old_id.native_identity);
                drop(saved);
                if ours
                    && unsafe { MoveFileExW(wide(&backup.path).as_ptr(), wide(target).as_ptr(), 0) }
                        == 0
                {
                    return Err(io::Error::new(
                        error.kind(),
                        format!(
                            "{error}; prior state retained at {} (restore failed: {})",
                            backup.path.display(),
                            io::Error::last_os_error()
                        ),
                    ));
                }
            }
        }
        // A backup, if present, is the prior valid object. Never delete it on a failed commit.
        return Err(error);
    }
    temporary.remove_on_drop = false;
    // Roll back a post-install flush failure when the two checked objects are still ours. If
    // rollback itself fails, retain the old backup and identify it in the error, never claim a
    // save. This is recovery from an API failure, not an arbitrary retry of the transaction.
    let id = match checked_committed(target, temporary.identity) {
        Ok(id) => id,
        Err(error) => {
            let current = open_data(target, READ_ATTRIBUTES).ok();
            let saved = open_data(&backup.path, READ_ATTRIBUTES).ok();
            let ours = current
                .as_ref()
                .and_then(|f| native_identity_for_file(f).ok())
                == Some(temporary.identity)
                && saved
                    .as_ref()
                    .and_then(|f| native_identity_for_file(f).ok())
                    == Some(old_id.native_identity);
            drop(current);
            drop(saved);
            if ours
                && unsafe {
                    ReplaceFileW(
                        wide(target).as_ptr(),
                        wide(&backup.path).as_ptr(),
                        wide(&temporary.path).as_ptr(),
                        0,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                } != 0
            {
                temporary.remove_on_drop = true;
                return Err(error);
            }
            return Err(io::Error::new(
                error.kind(),
                format!("{error}; prior state retained at {}", backup.path.display()),
            ));
        }
    };
    remove_owned(&backup.path, old_id.native_identity)?;
    Ok(id)
}

/// Reread and merge exactly once while holding the exclusive stable-parent lock, then commit a
/// complete flushed sibling. The closure's returned data must not be published until this
/// function succeeds. Existing target DACLs protect staging from its creation and are preserved
/// by ReplaceFileW with no ignore flags; first-created files inherit their parent ACL. No Unix
/// permission bits are invented.
pub fn rewrite_via_temporary_file<F, UserData>(
    path: &wstr,
    rewrite: F,
) -> io::Result<(FileId, PotentialUpdate<UserData>)>
where
    F: Fn(&File, &mut File) -> io::Result<PotentialUpdate<UserData>>,
{
    let (parent, path) = ParentLock::new(path, true)?;
    let existing = match open_data(&path, GENERIC_READ) {
        Ok(file) => Some(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let empty = if existing.is_none() {
        Some(Temporary::new(&parent.path, None)?)
    } else {
        None
    };
    let old = existing
        .as_ref()
        .unwrap_or_else(|| empty.as_ref().unwrap().file.as_ref().unwrap());
    let old_id = file_id_for_file(old)?;
    let security = existing
        .as_ref()
        .map(StagingSecurity::for_target)
        .transpose()?;
    let mut temporary = Temporary::new(&parent.path, security.as_ref())?;
    let update = rewrite(old, temporary.file.as_mut().unwrap())?;
    let id = if update.do_save {
        if existing.is_some() {
            commit_replacement(&path, old, &old_id, &mut temporary, &parent.path)?
        } else {
            commit_first(&path, &mut temporary)?
        }
    } else if existing.is_some() {
        if file_id_for_file(old)? != old_id
            || file_id_for_file(&open_data(&path, READ_ATTRIBUTES)?)? != old_id
        {
            return Err(changed());
        }
        old_id
    } else {
        // There is no committed target identity in this case. Return the real empty snapshot's
        // identity with do_save=false, without installing a file or signalling a save.
        old_id
    };
    Ok((id, update))
}

/// The directory handle is the cleanup capability. Keep it and its physical
/// ancestors open without delete sharing until explicit successful cleanup.
/// This protects against path replacement, not the owner or privileged principals
/// deliberately modifying their own files or security descriptors.
struct EditorDirectory {
    directory: File,
    identity: NativeFileIdentity,
    _ancestors: Vec<File>,
}

static EDITOR_DIRECTORIES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, EditorDirectory>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

struct UnpublishedEditorDirectory {
    directory: File,
    _ancestors: Vec<File>,
}

// If name/identity initialization and exact-handle cleanup both fail, retain
// the creation capability as well. Shutdown may retry deletion of that handle,
// but must never infer a path or recursively sweep an unregistered directory.
static UNPUBLISHED_EDITOR_DIRECTORIES: std::sync::Mutex<Vec<UnpublishedEditorDirectory>> =
    std::sync::Mutex::new(Vec::new());

#[repr(C)]
struct EditorUnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

#[repr(C)]
struct EditorObjectAttributes {
    length: u32,
    root: RawHandle,
    name: *mut EditorUnicodeString,
    attributes: u32,
    security: *mut c_void,
    quality_of_service: *mut c_void,
}

#[repr(C)]
struct EditorIoStatus {
    status: usize,
    information: usize,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtCreateFile(
        file: *mut RawHandle,
        access: u32,
        attributes: *const EditorObjectAttributes,
        status: *mut EditorIoStatus,
        allocation: *const i64,
        file_attributes: u32,
        sharing: u32,
        disposition: u32,
        options: u32,
        ea: *const c_void,
        ea_length: u32,
    ) -> i32;
    fn RtlNtStatusToDosError(status: i32) -> u32;
}

fn create_editor_directory(parent: &File, security: &StagingSecurity) -> io::Result<File> {
    loop {
        let name = random_filename(OsString::from("fish_editor_"));
        let mut name: Vec<u16> = name.encode_wide().collect();
        let mut name = EditorUnicodeString {
            length: (name.len() * 2) as u16,
            maximum_length: (name.len() * 2) as u16,
            buffer: name.as_mut_ptr(),
        };
        let attributes = EditorObjectAttributes {
            length: std::mem::size_of::<EditorObjectAttributes>() as u32,
            root: parent.as_raw_handle(),
            name: &mut name,
            attributes: 0,
            security: security.0,
            quality_of_service: std::ptr::null_mut(),
        };
        let mut status = EditorIoStatus {
            status: 0,
            information: 0,
        };
        let mut handle = std::ptr::null_mut();
        // NtCreateFile FILE_CREATE atomically creates a new directory and gives
        // us its no-delete-sharing handle. There is no CreateDirectory/open gap
        // where another process could substitute a different object.
        // https://learn.microsoft.com/windows/win32/api/winternl/nf-winternl-ntcreatefile
        let result = unsafe {
            NtCreateFile(
                &mut handle,
                READ_ATTRIBUTES | DELETE | 0x0010_0000, // SYNCHRONIZE
                &attributes,
                &mut status,
                std::ptr::null(),
                0x80,                          // FILE_ATTRIBUTE_NORMAL
                1 | 2,                         // Share read/write, never delete.
                2,                             // FILE_CREATE: never open/reuse an existing name.
                1 | 0x20 | OPEN_REPARSE_POINT, // DIRECTORY, synchronous nonalert, no links
                std::ptr::null(),
                0,
            )
        };
        if result >= 0 {
            return Ok(unsafe { File::from_raw_handle(handle) });
        }
        let error = io::Error::from_raw_os_error(unsafe { RtlNtStatusToDosError(result) as i32 });
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(error);
        }
    }
}

/// Allocate a private workspace for the external command-buffer editor.
/// It remains registered until remove_editor_directory succeeds; no path-only
/// recursive-delete Drop is used, including on a cleanup error.
pub fn make_editor_directory() -> io::Result<WString> {
    let base = native_path_text(&std::env::temp_dir())?;
    let base = native_absolute_path(&native_realpath(&base)?)?;
    let ancestors = pin_directory_ancestors(&base)?;
    let parent = ancestors
        .last()
        .ok_or_else(|| io::Error::other("temporary directory has no physical root"))?;
    let security = StagingSecurity::for_editor_directory()?;
    let directory = create_editor_directory(parent, &security)?;
    let identity_and_path = (|| {
        let identity = native_identity_for_file(&directory)?;
        let path = native_absolute_path(&native_final_path(&directory)?)?;
        let text = native_path_text(&path)?;
        Ok::<_, io::Error>((identity, path, text))
    })();
    let (identity, path, text) = match identity_and_path {
        Ok(result) => result,
        Err(error) => {
            // The handle was atomically created by us and is still unpublished.
            // Even initialization failure cleans only that exact empty object.
            if let Err(cleanup) = delete_editor_entry(&directory) {
                UNPUBLISHED_EDITOR_DIRECTORIES
                    .lock()
                    .unwrap()
                    .push(UnpublishedEditorDirectory {
                        directory,
                        _ancestors: ancestors,
                    });
                return Err(io::Error::new(
                    error.kind(),
                    format!("{error}; editor workspace cleanup also failed: {cleanup}"),
                ));
            }
            return Err(error);
        }
    };
    EDITOR_DIRECTORIES.lock().unwrap().insert(
        path,
        EditorDirectory {
            directory,
            identity,
            _ancestors: ancestors,
        },
    );
    Ok(text)
}

fn delete_editor_entry(file: &File) -> io::Result<()> {
    // FILE_DISPOSITION_INFO deletes the object opened by this handle, never a
    // subsequently substituted path. The handle was opened with DELETE access.
    let delete: u8 = 1;
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            4,
            (&delete as *const u8).cast(),
            std::mem::size_of_val(&delete) as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn clear_editor_directory(path: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();
        let file = OpenOptions::new()
            .access_mode(READ_ATTRIBUTES | DELETE)
            .share_mode(1)
            .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
            .open(&path)?;
        let metadata = file.metadata()?;
        // Pin each real child before traversing it. Reparse points are removed
        // as links, including directory junctions; their targets are never opened.
        if metadata.is_dir() && metadata.file_attributes() & REPARSE_POINT == 0 {
            clear_editor_directory(&path)?;
        }
        delete_editor_entry(&file)?;
    }
    Ok(())
}

/// Cleanup only a workspace capability created by this process. Aliases and
/// arbitrary directories are rejected, even if their names share our prefix.
/// Sharing/access failures retain the capability for an explicit retry.
pub fn remove_editor_directory(path: &Path) -> io::Result<()> {
    let mut directories = EDITOR_DIRECTORIES.lock().unwrap();
    let owner = directories.get(path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "not an owned editor workspace",
        )
    })?;
    // Deny new writers while inspecting/traversing the root. Existing writers
    // fail this open instead of racing a conversion into a directory junction.
    // Delete sharing here permits our already-retained DELETE-capable owner;
    // that original handle still prevents anyone else from renaming the root.
    let guard = OpenOptions::new()
        .access_mode(READ_ATTRIBUTES)
        .share_mode(1 | 4)
        .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
        .open(path)?;
    let metadata = guard.metadata()?;
    if !metadata.is_dir()
        || metadata.file_attributes() & REPARSE_POINT != 0
        || native_identity_for_file(&guard)? != owner.identity
    {
        return Err(io::Error::other("editor workspace changed before cleanup"));
    }
    if native_identity_for_file(&owner.directory)? != owner.identity {
        return Err(io::Error::other("editor workspace identity changed"));
    }
    clear_editor_directory(path)?;
    delete_editor_entry(&owner.directory)?;
    drop(guard);
    // Closing the retained root handle completes its pending deletion.
    directories.remove(path);
    Ok(())
}

/// Finalize canceled editor actions on orderly native shutdown. Continue after
/// individual failures, retaining every failed capability for a possible retry.
pub fn cleanup_editor_directories() -> io::Result<()> {
    let paths: Vec<_> = EDITOR_DIRECTORIES.lock().unwrap().keys().cloned().collect();
    let mut first_error = None;
    for path in paths {
        if let Err(error) = remove_editor_directory(&path) {
            if first_error.is_none() {
                first_error = Some(io::Error::new(
                    error.kind(),
                    format!("editor workspace {}: {error}", path.display()),
                ));
            }
        }
    }
    let mut unpublished = UNPUBLISHED_EDITOR_DIRECTORIES.lock().unwrap();
    let mut index = 0;
    while index < unpublished.len() {
        match delete_editor_entry(&unpublished[index].directory) {
            Ok(()) => {
                unpublished.swap_remove(index);
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(io::Error::new(
                        error.kind(),
                        format!("unpublished editor workspace cleanup: {error}"),
                    ));
                }
                index += 1;
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
