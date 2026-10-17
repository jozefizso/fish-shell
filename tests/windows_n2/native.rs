use super::fixture::LIMIT;
use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::atomic::{AtomicU64, Ordering};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> *mut c_void;
    fn SetEvent(handle: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
}
pub struct Event {
    pub name: String,
    handle: OwnedHandle,
}
impl Event {
    pub fn new(label: &str) -> Self {
        Self::create(label, true)
    }
    pub fn auto(label: &str) -> Self {
        Self::create(label, false)
    }
    fn create(label: &str, manual: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let name = format!(
            "Local\\fish-n2-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let wide: Vec<_> = OsStr::new(&name).encode_wide().chain(Some(0)).collect();
        let raw = unsafe { CreateEventW(std::ptr::null(), i32::from(manual), 0, wide.as_ptr()) };
        assert!(
            !raw.is_null(),
            "CreateEventW: {}",
            std::io::Error::last_os_error()
        );
        Self {
            name,
            handle: unsafe { OwnedHandle::from_raw_handle(raw) },
        }
    }
    pub fn signal(&self) {
        assert_ne!(unsafe { SetEvent(self.handle.as_raw_handle()) }, 0);
    }
    pub fn wait(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.handle.as_raw_handle(), LIMIT.as_millis() as u32) },
            0,
            "event {} timed out",
            self.name
        );
    }
}
