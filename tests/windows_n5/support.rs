//! Barrier-controlled real children; cleanup retains ownership of each process object.
use super::fixture::{Captured, Fixture, LIMIT, quote};
use super::native::Event;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::PathBuf;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn TerminateProcess(handle: *mut c_void, status: u32) -> i32;
    fn GetExitCodeProcess(handle: *mut c_void, status: *mut u32) -> i32;
}

pub struct Controlled {
    pub ready: Event,
    pub release: Event,
    pub pid_file: PathBuf,
    handle: Option<OwnedHandle>,
}
impl Controlled {
    pub fn new(f: &Fixture, label: &str) -> Self {
        Self {
            ready: Event::new(&format!("n5-{label}-ready")),
            release: Event::new(&format!("n5-{label}-release")),
            pid_file: f.root.join(format!("{label}.pid")),
            handle: None,
        }
    }
    pub fn command(&self, f: &Fixture, code: u32) -> String {
        format!(
            "{} n5-jobs-barrier {} {} {} {code}",
            f.probe(),
            quote(&self.ready.name),
            quote(&self.release.name),
            quote(self.pid_file.to_str().unwrap()),
        )
    }
    pub fn started(&mut self) -> u32 {
        self.ready.wait();
        let pid: u32 = std::fs::read_to_string(&self.pid_file)
            .expect("ready child must publish its process identity")
            .parse()
            .expect("native child PID");
        // The ready child remains blocked until our event is released, so this
        // initial open cannot retarget an exited/recycled process identity.
        let raw = unsafe { OpenProcess(0x0010_1001, 0, pid) };
        assert!(
            !raw.is_null(),
            "retain ready child {pid}: {}",
            std::io::Error::last_os_error()
        );
        self.handle = Some(unsafe { OwnedHandle::from_raw_handle(raw) });
        self.assert_live();
        pid
    }
    pub fn assert_live(&self) {
        assert_eq!(
            unsafe {
                WaitForSingleObject(
                    self.handle.as_ref().expect("started child").as_raw_handle(),
                    0,
                )
            },
            258,
            "owned fixture exited before controlled release"
        );
    }
    pub fn wait(&self, code: u32) {
        let handle = self.handle.as_ref().expect("started child");
        assert_eq!(
            unsafe { WaitForSingleObject(handle.as_raw_handle(), LIMIT.as_millis() as u32) },
            0,
            "owned fixture did not exit after release"
        );
        let mut actual = 0;
        assert_ne!(
            unsafe { GetExitCodeProcess(handle.as_raw_handle(), &mut actual) },
            0
        );
        assert_eq!(actual, code, "raw child DWORD changed");
    }
}
impl Drop for Controlled {
    fn drop(&mut self) {
        self.release.signal();
        if let Some(handle) = &self.handle {
            let result =
                unsafe { WaitForSingleObject(handle.as_raw_handle(), LIMIT.as_millis() as u32) };
            if result == 258 {
                // This is the process object retained while it acknowledged our
                // private barrier, not a fresh PID lookup or an enclosing job.
                unsafe {
                    TerminateProcess(handle.as_raw_handle(), 98);
                }
                assert_eq!(
                    unsafe {
                        WaitForSingleObject(handle.as_raw_handle(), LIMIT.as_millis() as u32)
                    },
                    0,
                    "owned cleanup failed to reap fixture"
                );
            } else {
                assert_eq!(result, 0, "owned cleanup wait failed");
            }
        }
    }
}

#[track_caller]
pub fn success(result: Captured, family: &str) -> String {
    assert_eq!(
        result.code,
        0,
        "{family}: status={} stdout={} stderr={}",
        result.code,
        String::from_utf8_lossy(&result.out),
        String::from_utf8_lossy(&result.err)
    );
    assert!(
        result.err.is_empty(),
        "{family}: unexpected stderr: {}",
        String::from_utf8_lossy(&result.err)
    );
    String::from_utf8(result.out).expect("fixture protocol is UTF-8")
}

#[track_caller]
pub fn records(output: &str, label: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.strip_prefix(&format!("{label}=")).map(str::to_owned))
        .collect()
}

#[track_caller]
pub fn one(output: &str, label: &str) -> String {
    let values = records(output, label);
    assert_eq!(
        values.len(),
        1,
        "expected one {label} record; output={output:?}"
    );
    values.into_iter().next().unwrap()
}

pub fn assert_pending(event: &Event) {
    let name: Vec<_> = std::ffi::OsStr::new(&event.name)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let raw = unsafe { OpenEventW(0x0010_0000, 0, name.as_ptr()) };
    assert!(
        !raw.is_null(),
        "open owned observation event: {}",
        std::io::Error::last_os_error()
    );
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) },
        258,
        "root evaluator returned while its synchronous function was still waiting"
    );
}
