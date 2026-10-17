//! Private, versioned descriptor handoff between instances of this native fish.
//!
//! Descriptor numbers are logical fish descriptors; the wire handle values are
//! inherited Windows handles, never CRT descriptors. The launcher must restrict
//! this protocol to the actual running fish image and its explicit handle list.

use crate::io::NativeIoDirection;
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, OwnedHandle, RawHandle};
use std::ptr;
use std::sync::Arc;

/// Reserved for one launch only; never import this key as a fish variable.
pub const PRIVATE_FD_ENV: &str = "FISH_INTERNAL_NATIVE_FDS";

const VERSION: &str = "v1";
const HANDLE_FLAG_INHERIT: u32 = 1;
const HANDLE_FLAG_PROTECT_FROM_CLOSE: u32 = 2;
const DUPLICATE_SAME_ACCESS: u32 = 2;
const FILE_READ_DATA: u32 = 1;
const FILE_WRITE_DATA: u32 = 2;
const FILE_APPEND_DATA: u32 = 4;

#[repr(C)]
#[derive(Default)]
struct ObjectBasicInformation {
    attributes: u32,
    granted_access: u32,
    handle_count: u32,
    pointer_count: u32,
    reserved: [u32; 10],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetHandleInformation(handle: RawHandle, flags: *mut u32) -> i32;
    fn SetHandleInformation(handle: RawHandle, mask: u32, flags: u32) -> i32;
    fn GetFileType(handle: RawHandle) -> u32;
    fn GetStdHandle(which: u32) -> RawHandle;
    fn GetCurrentProcess() -> RawHandle;
    fn CompareStringOrdinal(
        left: *const u16,
        left_length: i32,
        right: *const u16,
        right_length: i32,
        ignore_case: i32,
    ) -> i32;
    fn DuplicateHandle(
        source_process: RawHandle,
        source: RawHandle,
        target_process: RawHandle,
        target: *mut RawHandle,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryObject(
        handle: RawHandle,
        information_class: u32,
        information: *mut ObjectBasicInformation,
        length: u32,
        returned_length: *mut u32,
    ) -> i32;
    fn RtlNtStatusToDosError(status: i32) -> u32;
}

/// A borrowed entry in the launcher's child-specific inherited-handle list.
#[derive(Clone, Copy)]
pub struct BridgeHandle {
    pub fd: i32,
    pub handle: RawHandle,
    pub direction: NativeIoDirection,
    pub overlapped: bool,
}

/// One imported logical binding. Raw-handle aliases share exactly one owner.
#[derive(Clone)]
pub struct ImportedBinding {
    pub fd: i32,
    pub handle: Arc<OwnedHandle>,
    pub direction: NativeIoDirection,
    pub overlapped: bool,
}

struct ValidatedHandle {
    raw: RawHandle,
    granted_access: u32,
    overlapped: bool,
}

struct ValidatedBindings {
    handles: Vec<ValidatedHandle>,
    identities: HashMap<usize, usize>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn validate_file_handle(raw: RawHandle) -> io::Result<u32> {
    if raw.is_null() || raw as usize == usize::MAX {
        return Err(invalid(
            "native descriptor bridge contains an invalid handle",
        ));
    }
    let mut flags = 0;
    if unsafe { GetHandleInformation(raw, &mut flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if flags & HANDLE_FLAG_INHERIT == 0 {
        return Err(invalid(
            "native descriptor bridge handle is not inheritable",
        ));
    }
    if flags & HANDLE_FLAG_PROTECT_FROM_CLOSE != 0 {
        return Err(invalid(
            "native descriptor bridge handle is protected from close",
        ));
    }
    // Disk, character (including native NUL/console), and pipe handles are file
    // handles. Events, processes, jobs and other kernel objects are not streams.
    match unsafe { GetFileType(raw) } {
        1..=3 => (),
        _ => return Err(invalid("native descriptor bridge requires a file handle")),
    }
    let mut basic = ObjectBasicInformation::default();
    let status = unsafe {
        NtQueryObject(
            raw,
            0, // ObjectBasicInformation
            &mut basic,
            std::mem::size_of::<ObjectBasicInformation>() as u32,
            ptr::null_mut(),
        )
    };
    if status < 0 {
        return Err(io::Error::from_raw_os_error(unsafe {
            RtlNtStatusToDosError(status) as i32
        }));
    }
    Ok(basic.granted_access)
}

fn validate(bindings: &[BridgeHandle]) -> io::Result<ValidatedBindings> {
    let mut descriptors = HashSet::with_capacity(bindings.len());
    let mut identities = HashMap::with_capacity(bindings.len());
    let mut handles: Vec<ValidatedHandle> = Vec::with_capacity(bindings.len());
    for binding in bindings {
        if binding.fd < 0 || !descriptors.insert(binding.fd) {
            return Err(invalid(
                "native descriptor bridge contains an invalid or duplicate fd",
            ));
        }
        let identity = binding.handle as usize;
        let index = match identities.get(&identity).copied() {
            Some(index) => index,
            None => {
                let granted_access = validate_file_handle(binding.handle)?;
                let index = handles.len();
                handles.push(ValidatedHandle {
                    raw: binding.handle,
                    granted_access,
                    overlapped: binding.overlapped,
                });
                identities.insert(identity, index);
                index
            }
        };
        let handle = &handles[index];
        if handle.overlapped != binding.overlapped {
            return Err(invalid(
                "native descriptor bridge aliases disagree on overlapped capability",
            ));
        }
        let readable = handle.granted_access & FILE_READ_DATA != 0;
        let writable = handle.granted_access & (FILE_WRITE_DATA | FILE_APPEND_DATA) != 0;
        let allowed = match binding.direction {
            NativeIoDirection::Input => readable,
            NativeIoDirection::Output => writable,
            NativeIoDirection::InOut => readable && writable,
        };
        if !allowed {
            return Err(invalid(
                "native descriptor bridge direction exceeds handle access",
            ));
        }
    }
    Ok(ValidatedBindings {
        handles,
        identities,
    })
}

/// Encode borrowed, already-inheritable handles from exactly one child's list.
///
/// The canonical format is `v1;fd,hex_handle,i|o|b,0|1;...`; `v1` is an empty
/// map. Neither encoding nor environment filtering changes process-global state.
pub fn encode(bindings: &[BridgeHandle]) -> io::Result<OsString> {
    validate(bindings)?;
    let mut value = String::from(VERSION);
    for binding in bindings {
        let direction = match binding.direction {
            NativeIoDirection::Input => 'i',
            NativeIoDirection::Output => 'o',
            NativeIoDirection::InOut => 'b',
        };
        // Formatting into a String is infallible, unlike writing to an I/O sink.
        write!(
            value,
            ";{},{:x},{},{}",
            binding.fd,
            binding.handle as usize,
            direction,
            u8::from(binding.overlapped)
        )
        .expect("writing to a String cannot fail");
    }
    Ok(OsString::from(value))
}

fn parse(value: &OsStr) -> io::Result<Vec<BridgeHandle>> {
    let text = value
        .to_str()
        .ok_or_else(|| invalid("native descriptor bridge is not ASCII"))?;
    let mut records = text.split(';');
    if records.next() != Some(VERSION) {
        return Err(invalid("unsupported native descriptor bridge version"));
    }
    let mut bindings = Vec::new();
    for record in records {
        let mut fields = record.split(',');
        let descriptor = fields.next().unwrap_or_default();
        let raw = fields.next().unwrap_or_default();
        let direction = fields.next().unwrap_or_default();
        let overlapped = fields.next().unwrap_or_default();
        if fields.next().is_some()
            || descriptor.is_empty()
            || !descriptor.bytes().all(|byte| byte.is_ascii_digit())
            || (descriptor.len() > 1 && descriptor.starts_with('0'))
            || raw.is_empty()
            || !raw
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || (raw.len() > 1 && raw.starts_with('0'))
        {
            return Err(invalid("malformed native descriptor bridge record"));
        }
        let fd = descriptor
            .parse::<i32>()
            .map_err(|_| invalid("native descriptor bridge fd overflow"))?;
        let handle = usize::from_str_radix(raw, 16)
            .map_err(|_| invalid("native descriptor bridge handle overflow"))?
            as RawHandle;
        let direction = match direction {
            "i" => NativeIoDirection::Input,
            "o" => NativeIoDirection::Output,
            "b" => NativeIoDirection::InOut,
            _ => return Err(invalid("invalid native descriptor bridge direction")),
        };
        let overlapped = match overlapped {
            "0" => false,
            "1" => true,
            _ => {
                return Err(invalid(
                    "invalid native descriptor bridge overlapped capability",
                ));
            }
        };
        bindings.push(BridgeHandle {
            fd,
            handle,
            direction,
            overlapped,
        });
    }
    Ok(bindings)
}

fn duplicate_noninheritable(raw: RawHandle) -> io::Result<OwnedHandle> {
    let process = unsafe { GetCurrentProcess() };
    let mut duplicate = ptr::null_mut();
    if unsafe {
        DuplicateHandle(
            process,
            raw,
            process,
            &mut duplicate,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // DuplicateHandle succeeded and transferred one new, non-inheritable handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(duplicate) })
}

/// Import once at process startup, before exposing any descriptor endpoints.
///
/// The whole message (including real handle access, type and inheritance) is
/// validated before ownership is transferred or any handle flags are changed.
/// On validation failure no incoming handle is closed. Once adopted, all extra
/// handles have RAII cleanup even if a later duplicate/flag operation fails.
/// Standard-handle aliases are duplicated instead of adopting process globals.
pub fn decode(value: &OsStr) -> io::Result<Vec<ImportedBinding>> {
    let bindings = parse(value)?;
    let validated = validate(&bindings)?;
    let standards = [
        unsafe { GetStdHandle(-10i32 as u32) },
        unsafe { GetStdHandle(-11i32 as u32) },
        unsafe { GetStdHandle(-12i32 as u32) },
    ];
    // First take every nonstandard owner, without performing a fallible API call.
    // A later failure therefore cannot leave another accepted extra handle loose.
    let mut owners: Vec<Option<Arc<OwnedHandle>>> = validated
        .handles
        .iter()
        .map(|handle| {
            if standards.contains(&handle.raw) {
                None
            } else {
                // All identities are unique, valid, inherited file handles; the
                // startup protocol is their ownership-transfer boundary.
                Some(Arc::new(unsafe {
                    OwnedHandle::from_raw_handle(handle.raw)
                }))
            }
        })
        .collect();
    for (handle, owner) in validated.handles.iter().zip(&mut owners) {
        if owner.is_none() {
            *owner = Some(Arc::new(duplicate_noninheritable(handle.raw)?));
        } else if unsafe { SetHandleInformation(handle.raw, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(bindings
        .into_iter()
        .map(|binding| ImportedBinding {
            fd: binding.fd,
            handle: Arc::clone(
                owners[validated.identities[&(binding.handle as usize)]]
                    .as_ref()
                    .expect("every validated handle has an owner"),
            ),
            direction: binding.direction,
            overlapped: binding.overlapped,
        })
        .collect())
}

/// Match the reserved key with Windows' ordinal, case-insensitive environment
/// semantics, including non-ASCII characters Windows maps to an ASCII letter.
pub fn is_private_environment_key(key: &OsStr) -> bool {
    const PRIVATE_WIDE: [u16; PRIVATE_FD_ENV.len()] = {
        let mut wide = [0; PRIVATE_FD_ENV.len()];
        let bytes = PRIVATE_FD_ENV.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            wide[index] = bytes[index] as u16;
            index += 1;
        }
        wide
    };
    let mut candidate = [0u16; PRIVATE_FD_ENV.len()];
    let mut units = key.encode_wide();
    for slot in &mut candidate {
        let Some(unit) = units.next() else {
            return false;
        };
        *slot = unit;
    }
    if units.next().is_some() {
        return false;
    }
    unsafe {
        CompareStringOrdinal(
            candidate.as_ptr(),
            candidate.len() as i32,
            PRIVATE_WIDE.as_ptr(),
            PRIVATE_WIDE.len() as i32,
            1,
        ) == 2 // CSTR_EQUAL
    }
}

/// Remove stale or user-supplied bridge metadata from a child environment.
/// The launch owner may then add exactly the metadata generated for that child.
pub fn sanitize_environment(environment: &[(OsString, OsString)]) -> Vec<(OsString, OsString)> {
    environment
        .iter()
        .filter(|(key, _)| !is_private_environment_key(key))
        .cloned()
        .collect()
}
