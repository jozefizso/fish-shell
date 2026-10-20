//! Owned, serialized startup fixtures for a dedicated native acceptance account.
//! Never overwrite an existing AutoRun value or PowerShell profile. Provision a
//! clean account when these prerequisites fail; do not disable or skip the case.
use std::ffi::{OsStr, c_void};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};

const HKCU: *mut c_void = 0x8000_0001u32 as i32 as isize as *mut c_void;
const PROCESSOR: &str = "Software\\Microsoft\\Command Processor";
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const c_void, owner: i32, name: *const u16) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    fn ReleaseMutex(handle: *mut c_void) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegCreateKeyExW(key: *mut c_void, subkey: *const u16, reserved: u32, class: *mut u16,
        options: u32, access: u32, attributes: *const c_void, result: *mut *mut c_void, disposition: *mut u32) -> i32;
    fn RegQueryValueExW(key: *mut c_void, name: *const u16, reserved: *mut u32,
        kind: *mut u32, data: *mut u8, size: *mut u32) -> i32;
    fn RegSetValueExW(key: *mut c_void, name: *const u16, reserved: u32,
        kind: u32, data: *const u8, size: u32) -> i32;
    fn RegDeleteValueW(key: *mut c_void, name: *const u16) -> i32;
    fn RegCloseKey(key: *mut c_void) -> i32;
    fn RegDeleteKeyW(key: *mut c_void, subkey: *const u16) -> i32;
}
fn wide(value: impl AsRef<OsStr>) -> Vec<u16> { value.as_ref().encode_wide().chain(Some(0)).collect() }
fn cleanup(ok: bool, what: &str) {
    if !ok {
        if std::thread::panicking() { eprintln!("N8-TOOLS startup fixture cleanup failed: {what}"); }
        else { panic!("N8-TOOLS startup fixture cleanup failed: {what}"); }
    }
}
pub struct Lock(OwnedHandle);
impl Lock {
    pub fn acquire() -> Self {
        assert_eq!(
            std::env::var("FISH_NATIVE_N8_DEDICATED_ACCOUNT").as_deref(),
            Ok("1"),
            "startup markers modify HKCU/profiles: provision a disposable dedicated account and explicitly attest FISH_NATIVE_N8_DEDICATED_ACCOUNT=1; never run this case in a normal user's account"
        );
        let name = wide("Local\\fish-native-n8-tool-startup-fixtures");
        let raw = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        assert!(!raw.is_null(), "required startup fixture mutex: {}", std::io::Error::last_os_error());
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        let wait = unsafe { WaitForSingleObject(handle.as_raw_handle(), 25_000) };
        assert!(wait == 0 || wait == 0x80, "bounded startup fixture mutex wait failed: {wait:#x}");
        Self(handle)
    }
}
impl Drop for Lock {
    fn drop(&mut self) { cleanup(unsafe { ReleaseMutex(self.0.as_raw_handle()) } != 0, "release startup mutex"); }
}
pub struct Autorun { key: *mut c_void, installed: bool, created_key: bool }
impl Autorun {
    pub fn install(marker: &Path) -> Self {
        let mut key = std::ptr::null_mut();
        let mut disposition = 0;
        let result = unsafe { RegCreateKeyExW(HKCU, wide(PROCESSOR).as_ptr(), 0, std::ptr::null_mut(),
            0, 3, std::ptr::null(), &mut key, &mut disposition) };
        assert_eq!(result, 0, "N8-TOOLS prerequisite: dedicated account needs HKCU Command Processor query/set access ({result})");
        let mut owner = Self { key, installed: false, created_key: disposition == 1 };
        let result = unsafe { RegQueryValueExW(key, wide("AutoRun").as_ptr(), std::ptr::null_mut(),
            std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
        assert_eq!(result, 2, "N8-TOOLS prerequisite: use a clean dedicated account without HKCU Command Processor AutoRun; existing values are never overwritten (query={result})");
        let text = marker.to_str().expect("reversible marker path");
        assert!(!text.contains(['%', '!', '^', '&', '|', '<', '>', '"', '\r', '\n']), "autorun fixture marker path must be an ordinary batch path");
        let command = wide(format!("@echo autorun-marker>\"{text}\""));
        let result = unsafe { RegSetValueExW(key, wide("AutoRun").as_ptr(), 0, 1,
            command.as_ptr().cast(), (command.len() * 2) as u32) };
        assert_eq!(result, 0, "required owned AutoRun marker installation: {result}");
        owner.installed = true;
        owner
    }
}
impl Drop for Autorun {
    fn drop(&mut self) {
        let removed = !self.installed || unsafe { RegDeleteValueW(self.key, wide("AutoRun").as_ptr()) } == 0;
        let closed = unsafe { RegCloseKey(self.key) } == 0;
        let key_removed = !self.created_key || unsafe { RegDeleteKeyW(HKCU, wide(PROCESSOR).as_ptr()) } == 0;
        cleanup(removed && closed && key_removed, "remove owned AutoRun fixture and close registry handle");
    }
}
pub struct Profiles { files: Vec<PathBuf>, directories: Vec<PathBuf> }
impl Profiles {
    pub fn install(paths: &[PathBuf], marker: &Path) -> Self {
        let mut owner = Self { files: Vec::new(), directories: Vec::new() };
        let source = format!("[IO.File]::WriteAllText('{}','profile-marker')\n", marker.to_str().unwrap().replace('\'', "''"));
        for path in paths {
            assert!(path.is_absolute(), "PowerShell must report an absolute real profile path: {}", path.display());
            if owner.files.contains(path) { continue; }
            let parent = path.parent().unwrap();
            let mut absent = Vec::new();
            let mut cursor = parent;
            while !cursor.exists() {
                absent.push(cursor.to_owned());
                cursor = cursor.parent().expect("absolute profile parent");
            }
            for directory in absent.into_iter().rev() {
                fs::create_dir(&directory).unwrap();
                owner.directories.push(directory);
            }
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)
                .unwrap_or_else(|e| panic!("N8-TOOLS prerequisite: use a clean dedicated account with no existing PowerShell profile at {}; never overwrite user profiles: {e}", path.display()));
            owner.files.push(path.to_owned());
            file.write_all(b"\xef\xbb\xbf").unwrap();
            file.write_all(source.as_bytes()).unwrap();
        }
        owner
    }
}
impl Drop for Profiles {
    fn drop(&mut self) {
        let mut ok = true;
        for path in self.files.iter().rev() { ok &= fs::remove_file(path).is_ok(); }
        for path in self.directories.iter().rev() { ok &= fs::remove_dir(path).is_ok(); }
        cleanup(ok, "remove exclusively created PowerShell profiles/directories");
    }
}
