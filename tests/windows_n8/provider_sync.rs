//! Named-event barriers and retained capabilities for owned provider fixtures only.
use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
const LIMIT: u32 = 25_000;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(security: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn TerminateProcess(handle: *mut c_void, code: u32) -> i32;
    fn GetExitCodeProcess(handle: *mut c_void, code: *mut u32) -> i32;
}
fn wide(value: &OsStr) -> Vec<u16> { value.encode_wide().chain(Some(0)).collect() }
fn owned(raw: *mut c_void) -> OwnedHandle {
    assert!(!raw.is_null(), "required fixture capability: {}", std::io::Error::last_os_error());
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
pub struct Event { pub name: String, handle: OwnedHandle }
impl Event {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!("Local\\fish-n8-provider-{}-{}-{label}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let handle = owned(unsafe { CreateEventW(std::ptr::null(), 1, 0, wide(OsStr::new(&name)).as_ptr()) });
        Self { name, handle }
    }
    pub fn open(name: &OsStr) -> Self {
        let handle = owned(unsafe { OpenEventW(0x0010_0002, 0, wide(name).as_ptr()) });
        Self { name: name.to_str().unwrap().to_owned(), handle }
    }
    pub fn signal(&self) { assert_ne!(unsafe { SetEvent(self.handle.as_raw_handle()) }, 0); }
    pub fn wait(&self) { assert_eq!(unsafe { WaitForSingleObject(self.handle.as_raw_handle(), LIMIT) }, 0, "bounded event {}", self.name); }
}
impl Drop for Event { fn drop(&mut self) { unsafe { SetEvent(self.handle.as_raw_handle()); } } }

pub struct Process { handle: OwnedHandle }
impl Process {
    /// PID is supplied by our still-barrier-blocked child, never by enumeration of strangers.
    pub fn open(pid: u32) -> Self {
        let handle = owned(unsafe { OpenProcess(0x0010_1001, 0, pid) });
        let result = Self { handle };
        result.live();
        result
    }
    pub fn from_file(path: &Path) -> Self {
        Self::open(std::fs::read_to_string(path).unwrap().trim().parse().unwrap())
    }
    pub fn live(&self) { assert_eq!(unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 0) }, 258, "owned provider fixture exited early"); }
    pub fn wait(&self, expected: u32) {
        assert_eq!(unsafe { WaitForSingleObject(self.handle.as_raw_handle(), LIMIT) }, 0, "owned process did not exit within bound");
        let mut code = 0;
        assert_ne!(unsafe { GetExitCodeProcess(self.handle.as_raw_handle(), &mut code) }, 0);
        assert_eq!(code, expected);
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 0) } == 258 {
            assert_ne!(unsafe { TerminateProcess(self.handle.as_raw_handle(), 98) }, 0);
            assert_eq!(unsafe { WaitForSingleObject(self.handle.as_raw_handle(), LIMIT) }, 0);
        }
    }
}

pub struct Shell(pub std::process::Child);
impl Shell {
    pub fn wait(&mut self, expected: u32) {
        assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), LIMIT) }, 0, "fish fixture exceeded bound");
        assert_eq!(self.0.wait().unwrap().code().unwrap() as u32, expected);
    }
    pub fn kill(&mut self) { self.0.kill().unwrap(); assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), LIMIT) }, 0); self.0.wait().unwrap(); }
}
impl Drop for Shell { fn drop(&mut self) { if self.0.try_wait().unwrap().is_none() { let _ = self.0.kill(); let _ = self.0.wait(); } } }
