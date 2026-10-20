//! Same named-event protocol as the retained N4/N5 real process helpers.
use std::{ffi::c_void, os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle}, sync::atomic::{AtomicU64, Ordering}};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn WaitForSingleObject(event: *mut c_void, timeout: u32) -> u32;
}
pub struct Event { handle: OwnedHandle, pub name: String }
impl Event {
    pub fn new(label: &str) -> Self { Self::create(label, 1) }
    pub fn auto(label: &str) -> Self { Self::create(label, 0) }
    fn create(label: &str, manual: i32) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!("Local\\fish-n11-{}-{}-{label}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let raw = unsafe { CreateEventW(std::ptr::null(), manual, 0, wide.as_ptr()) };
        assert!(!raw.is_null(), "CreateEventW: {}", std::io::Error::last_os_error());
        Self { handle: unsafe { OwnedHandle::from_raw_handle(raw) }, name }
    }
    pub fn wait(&self) { assert_eq!(unsafe { WaitForSingleObject(self.handle.as_raw_handle(), super::fixture::LIMIT.as_millis() as u32) }, 0, "barrier {}", self.name); }
    pub fn signal(&self) { assert_ne!(unsafe { SetEvent(self.handle.as_raw_handle()) }, 0); }
}
pub struct Release<'a>(pub &'a Event);
impl Drop for Release<'_> { fn drop(&mut self) { self.0.signal(); } }
