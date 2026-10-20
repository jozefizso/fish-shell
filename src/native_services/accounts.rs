//! Local SAM enumeration and explicit Windows SID identity lookup. No UID/GID mapping.
use super::{Handle, decode, invalid, text, unsupported, wide};
use fish_widestring::{WString, wstr};
use std::ffi::c_void;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::PathBuf;

#[repr(C)]
struct User20 { name: *const u16, full_name: *const u16, comment: *const u16, flags: u32, id: u32 }
#[repr(C)]
struct Group1 { name: *const u16, comment: *const u16 }
#[repr(C)]
struct SidAndAttributes { sid: *mut c_void, attributes: u32 }
#[repr(C)]
struct Guid { a: u32, b: u16, c: u16, d: [u8; 8] }

#[link(name = "netapi32")]
unsafe extern "system" {
    fn NetUserEnum(server: *const u16, level: u32, filter: u32, buffer: *mut *mut u8, preferred: u32, read: *mut u32, total: *mut u32, resume: *mut u32) -> u32;
    fn NetLocalGroupEnum(server: *const u16, level: u32, buffer: *mut *mut u8, preferred: u32, read: *mut u32, total: *mut u32, resume: *mut usize) -> u32;
    fn NetApiBufferFree(buffer: *mut c_void) -> u32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(token: Handle, class: i32, buffer: *mut c_void, size: u32, needed: *mut u32) -> i32;
    fn LookupAccountNameW(system: *const u16, name: *const u16, sid: *mut c_void, sid_size: *mut u32, domain: *mut u16, domain_size: *mut u32, kind: *mut i32) -> i32;
    fn LookupAccountSidW(system: *const u16, sid: *const c_void, name: *mut u16, name_size: *mut u32, domain: *mut u16, domain_size: *mut u32, kind: *mut i32) -> i32;
    fn ConvertSidToStringSidW(sid: *const c_void, value: *mut *mut u16) -> i32;
    fn RegGetValueW(key: Handle, subkey: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, size: *mut u32) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn GetWindowsDirectoryW(buffer: *mut u16, size: u32) -> u32;
    fn GetComputerNameW(buffer: *mut u16, size: *mut u32) -> i32;
}
#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: Handle, path: *mut *mut u16) -> i32;
}
#[link(name = "ole32")]
unsafe extern "system" { fn CoTaskMemFree(memory: *mut c_void); }

struct NetBuffer(*mut u8);
impl Drop for NetBuffer { fn drop(&mut self) { unsafe { NetApiBufferFree(self.0.cast()); } } }

#[derive(Debug)]
pub struct AccountEntry { pub name: String, pub description: String }
#[derive(Debug)]
pub struct AccountIdentity { pub name: String, pub domain: String, pub sid: String, pub kind: i32 }

pub fn local_users() -> io::Result<Vec<AccountEntry>> {
    let mut result = Vec::new();
    let mut resume = 0;
    loop {
        let (mut buffer, mut read, mut total) = (std::ptr::null_mut(), 0, 0);
        let status = unsafe { NetUserEnum(std::ptr::null(), 20, 0, &mut buffer, 65536, &mut read, &mut total, &mut resume) };
        let buffer = NetBuffer(buffer);
        if status != 0 && status != 234 { return Err(io::Error::from_raw_os_error(status as i32)); }
        for index in 0..read as usize {
            let entry = unsafe { &*buffer.0.cast::<User20>().add(index) };
            result.push(AccountEntry { name: unsafe { text(entry.name)? }, description: unsafe { text(entry.full_name)? } });
        }
        if status == 0 { break; }
    }
    Ok(result)
}

pub fn local_groups() -> io::Result<Vec<AccountEntry>> {
    let mut result = Vec::new();
    let mut resume = 0;
    loop {
        let (mut buffer, mut read, mut total) = (std::ptr::null_mut(), 0, 0);
        let status = unsafe { NetLocalGroupEnum(std::ptr::null(), 1, &mut buffer, 65536, &mut read, &mut total, &mut resume) };
        let buffer = NetBuffer(buffer);
        if status != 0 && status != 234 { return Err(io::Error::from_raw_os_error(status as i32)); }
        for index in 0..read as usize {
            let entry = unsafe { &*buffer.0.cast::<Group1>().add(index) };
            result.push(AccountEntry { name: unsafe { text(entry.name)? }, description: unsafe { text(entry.comment)? } });
        }
        if status == 0 { break; }
    }
    Ok(result)
}

fn token() -> io::Result<OwnedHandle> {
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), 8, &mut token) } == 0 { return Err(io::Error::last_os_error()); }
    Ok(unsafe { OwnedHandle::from_raw_handle(token) })
}

fn sid_string(sid: *const c_void) -> io::Result<String> {
    let mut value = std::ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut value) } == 0 { return Err(io::Error::last_os_error()); }
    let result = unsafe { text(value) };
    unsafe { LocalFree(value.cast()); }
    result
}

fn identity(sid: *const c_void) -> io::Result<AccountIdentity> {
    let (mut name_size, mut domain_size, mut kind) = (0, 0, 0);
    unsafe { LookupAccountSidW(std::ptr::null(), sid, std::ptr::null_mut(), &mut name_size, std::ptr::null_mut(), &mut domain_size, &mut kind); }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(122) { return Err(error); }
    let mut name = vec![0; name_size as usize];
    let mut domain = vec![0; domain_size as usize];
    if unsafe { LookupAccountSidW(std::ptr::null(), sid, name.as_mut_ptr(), &mut name_size, domain.as_mut_ptr(), &mut domain_size, &mut kind) } == 0 { return Err(io::Error::last_os_error()); }
    Ok(AccountIdentity { name: decode(&name[..name_size as usize])?, domain: decode(&domain[..domain_size as usize])?, sid: sid_string(sid)?, kind })
}

pub fn current_account() -> io::Result<AccountIdentity> {
    let token = token()?;
    let mut needed = 0;
    unsafe { GetTokenInformation(token.as_raw_handle(), 1, std::ptr::null_mut(), 0, &mut needed); }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(122) { return Err(error); }
    // usize allocation supplies the alignment required by TOKEN_USER and its SID.
    let mut data = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe { GetTokenInformation(token.as_raw_handle(), 1, data.as_mut_ptr().cast(), needed, &mut needed) } == 0 { return Err(io::Error::last_os_error()); }
    identity(unsafe { (*data.as_ptr().cast::<SidAndAttributes>()).sid })
}

/// Lookup accepts explicit DOMAIN\\name and local names. The shell's .\\name
/// shorthand is expanded to the OS NetBIOS computer name, not an environment value.
/// Builtin aliases must use their resolved domain (which need not be the machine).
/// Domain resolution errors are returned rather than manufacturing a local identity.
pub fn lookup(name: &str) -> io::Result<AccountIdentity> {
    let name = if let Some(local) = name.strip_prefix(".\\") {
        let mut computer = [0u16; 16]; // MAX_COMPUTERNAME_LENGTH plus NUL.
        let mut length = computer.len() as u32;
        if unsafe { GetComputerNameW(computer.as_mut_ptr(), &mut length) } == 0 {
            return Err(io::Error::last_os_error());
        }
        wide(&format!("{}\\{local}", decode(&computer[..length as usize])?))?
    } else {
        wide(name)?
    };
    let (mut sid_size, mut domain_size, mut kind) = (0, 0, 0);
    unsafe { LookupAccountNameW(std::ptr::null(), name.as_ptr(), std::ptr::null_mut(), &mut sid_size, std::ptr::null_mut(), &mut domain_size, &mut kind); }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(122) { return Err(error); }

    let mut sid = vec![0usize; (sid_size as usize).div_ceil(std::mem::size_of::<usize>())];
    let mut domain = vec![0; domain_size as usize];
    if unsafe { LookupAccountNameW(std::ptr::null(), name.as_ptr(), sid.as_mut_ptr().cast(), &mut sid_size, domain.as_mut_ptr(), &mut domain_size, &mut kind) } == 0 { return Err(io::Error::last_os_error()); }
    identity(sid.as_ptr().cast())
}
/// Effective process-token groups, including disabled/deny-only entries. Attributes are
/// Windows SE_GROUP flags, not an assertion that every listed SID grants access.
pub fn token_groups() -> io::Result<Vec<(String, u32)>> {
    let token = token()?;
    let mut needed = 0;
    unsafe { GetTokenInformation(token.as_raw_handle(), 2, std::ptr::null_mut(), 0, &mut needed); }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(122) { return Err(error); }
    let mut data = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe { GetTokenInformation(token.as_raw_handle(), 2, data.as_mut_ptr().cast(), needed, &mut needed) } == 0 { return Err(io::Error::last_os_error()); }
    #[repr(C)]
    struct TokenGroups { count: u32, groups: [SidAndAttributes; 1] }
    let groups = data.as_ptr().cast::<TokenGroups>();
    let count = unsafe { (*groups).count } as usize;
    let first = unsafe { std::ptr::addr_of!((*groups).groups).cast::<SidAndAttributes>() };
    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let entry = unsafe { &*first.add(index) };
        result.push((sid_string(entry.sid)?, entry.attributes));
    }
    Ok(result)
}

fn checked_profile(path: &str) -> io::Result<WString> {
    let path = crate::path::native_path_text(&PathBuf::from(path))?;
    let result = crate::wutil::native_realpath(&path)?;
    if !std::fs::metadata(crate::path::native_absolute_path(&result)?)?.is_dir() { return Err(invalid("registered account profile is not a directory")); }
    Ok(result)
}

/// Token/known-folder profile, not HOME or USERPROFILE (both are shell overrides).
pub fn current_profile() -> io::Result<WString> {
    const PROFILE: Guid = Guid { a: 0x5e6c858f, b: 0x0e22, c: 0x4760, d: [0x9a, 0xfe, 0xea, 0x33, 0x17, 0xb6, 0x71, 0x73] };
    let token = token()?;
    let mut path = std::ptr::null_mut();
    let status = unsafe { SHGetKnownFolderPath(&PROFILE, 0, token.as_raw_handle(), &mut path) };
    if status < 0 { return Err(io::Error::from_raw_os_error(status)); }
    let value = unsafe { text(path) };
    unsafe { CoTaskMemFree(path.cast()); }
    checked_profile(&value?)
}

/// Resolve ~USER by Windows account SID and registered profile. Never guess C:\\Users\\name.
/// The current account uses its token's Known Folder, other accounts require a readable
/// local ProfileList registration and an accessible existing directory.
pub fn profile_for_user(user: &wstr) -> io::Result<WString> {
    let native = fish_widestring::shell_text_to_osstring(user)?
        .into_string().map_err(|_| invalid("invalid native account Unicode"))?;
    let account = lookup(&native)?;
    if account.kind != 1 { return Err(invalid("profile lookup requires a Windows user account, not a group or alias")); }
    if account.sid == current_account()?.sid { return current_profile(); }
    let key = wide(&format!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\ProfileList\\{}", account.sid))?;
    let value = wide("ProfileImagePath")?;
    let mut size = 0;
    let flags = 2 | 4 | 0x10000000 | 0x10000; // REG_SZ|REG_EXPAND_SZ, NOEXPAND, 64-bit registry
    let machine = 0x80000002u32 as i32 as isize as Handle;
    let status = unsafe { RegGetValueW(machine, key.as_ptr(), value.as_ptr(), flags, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) };
    if status != 0 { return Err(io::Error::from_raw_os_error(status)); }
    let mut data = vec![0u16; (size as usize).div_ceil(2)];
    let status = unsafe { RegGetValueW(machine, key.as_ptr(), value.as_ptr(), flags, std::ptr::null_mut(), data.as_mut_ptr().cast(), &mut size) };
    if status != 0 { return Err(io::Error::from_raw_os_error(status)); }
    let length = data.iter().position(|&c| c == 0).ok_or_else(|| invalid("unterminated profile registration"))?;
    let mut path = decode(&data[..length])?;
    if path.get(..13).is_some_and(|prefix| prefix.eq_ignore_ascii_case("%SystemDrive%")) {
        let mut windows = vec![0u16; 32768];
        let length = unsafe { GetWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32) } as usize;
        if length == 0 { return Err(io::Error::last_os_error()); }
        if length >= windows.len() { return Err(invalid("Windows directory exceeds API buffer")); }
        let windows = decode(&windows[..length])?;
        let root = windows.get(..2).filter(|root| root.ends_with(':')).ok_or_else(|| invalid("Windows directory lacks drive identity"))?;
        path.replace_range(..13, root);
    }
    if path.contains('%') { return Err(unsupported("profile registration uses unsupported environment expansion; refusing inherited environment guesses")); }
    checked_profile(&path)
}
