//! Native console clients only: never evaluates fish syntax or models its reader.
#[path = "native_console_probe.rs"]
mod native_queries;
use std::{
    ffi::{OsStr, OsString, c_void},
    fs,
    os::windows::{ffi::OsStrExt, io::{AsRawHandle, FromRawHandle, OwnedHandle}},
    path::Path,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};
type Handle = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Key {
    down: i32,
    repeat: u16,
    vk: u16,
    scan: u16,
    unicode: u16,
    controls: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
union Payload {
    key: Key,
    bytes: [u32; 4],
}
#[repr(C)]
struct Record {
    kind: u16,
    data: Payload,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: Handle, manual: i32, initial: i32, name: *const u16) -> Handle;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn SetEvent(event: Handle) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn WaitForSingleObject(handle: Handle, timeout: u32) -> u32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn GetExitCodeProcess(process: Handle, status: *mut u32) -> i32;
    fn GetConsoleMode(handle: Handle, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: Handle, mode: u32) -> i32;
    fn ReadConsoleInputW(input: Handle, records: *mut Record, count: u32, read: *mut u32) -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain([0]).collect()
}
fn check(value: i32) {
    assert_ne!(
        value,
        0,
        "Win32 fixture: {}",
        std::io::Error::last_os_error()
    );
}
pub struct Event {
    handle: Handle,
    pub name: String,
}
impl Event {
    pub fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let name = format!(
            "Local\\fish-n6-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let handle = unsafe { CreateEventW(null_mut(), 1, 0, wide(OsStr::new(&name)).as_ptr()) };
        assert!(
            !handle.is_null(),
            "CreateEventW: {}",
            std::io::Error::last_os_error()
        );
        Self { handle, name }
    }
    fn open(name: &OsStr) -> Self {
        let handle = unsafe { OpenEventW(0x00100000 | 2, 0, wide(name).as_ptr()) };
        assert!(
            !handle.is_null(),
            "required event: {}",
            std::io::Error::last_os_error()
        );
        Self {
            handle,
            name: name.to_string_lossy().into_owned(),
        }
    }
    pub fn set(&self) {
        check(unsafe { SetEvent(self.handle) });
    }
    pub fn wait(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.handle, 30_000) },
            0,
            "bounded native event barrier {}",
            self.name
        );
    }
    pub fn is_set(&self) -> bool {
        unsafe { WaitForSingleObject(self.handle, 0) == 0 }
    }
}
impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
/// Open only an acknowledged owned client, while its release event is held.
/// Keeping the handle makes its eventual DWORD independent of PID reuse.
pub struct RetainedProcess(OwnedHandle);
impl RetainedProcess {
    pub fn acknowledged(pid: u32) -> Self {
        let raw = unsafe { OpenProcess(0x0010_1000, 0, pid) };
        assert!(!raw.is_null(), "open acknowledged owned process {pid}: {}", std::io::Error::last_os_error());
        Self(unsafe { OwnedHandle::from_raw_handle(raw) })
    }
    pub fn running(&self, description: &str) {
        assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) }, 258, "{description}");
    }
    pub fn wait_status(&self, description: &str) -> u32 {
        assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), 30_000) }, 0,
            "acknowledged {description} did not exit inside its bound");
        let mut status = 0;
        check(unsafe { GetExitCodeProcess(self.0.as_raw_handle(), &mut status) });
        status
    }
}
static CONTROL: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
unsafe extern "system" fn ignore_control(kind: u32) -> i32 {
    if kind <= 1 {
        let event = CONTROL.load(Ordering::Relaxed);
        if !event.is_null() {
            unsafe {
                SetEvent(event);
            }
        }
        1
    } else {
        0
    }
}
fn modes(input: Handle, output: Handle) -> (u32, u32) {
    let (mut i, mut o) = (0, 0);
    check(unsafe { GetConsoleMode(input, &mut i) });
    check(unsafe { GetConsoleMode(output, &mut o) });
    (i, o)
}
pub fn run(mode: &str, args: Vec<OsString>) {
    match mode {
        "n6-case-native-queries" => {
            assert_eq!(args.len(), 1);
            native_queries::run(Path::new(&args[0]));
        }
        "n6-case-tui" | "n6-case-ignore-control" => {
            assert_eq!(args.len(), 3);
            let root = Path::new(&args[0]);
            let release = Event::open(&args[1]);
            let controlled = Event::open(&args[2]);
            let input = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("CONIN$")
                .expect("real console input");
            let output = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("CONOUT$")
                .expect("real console output");
            let i = input.as_raw_handle();
            let o = output.as_raw_handle();
            let original = modes(i, o);
            // Capture before the first read or any mode change by this child.
            fs::write(
                root.join("child-original-modes"),
                format!("{}:{}", original.0, original.1),
            )
            .unwrap();
            check(unsafe { SetConsoleMode(i, (original.0 & !(2 | 4 | 0x40)) | 0x80 | 8) });
            check(unsafe { SetConsoleMode(o, original.1 ^ 8) });
            if mode == "n6-case-ignore-control" {
                CONTROL.store(controlled.handle, Ordering::Relaxed);
                check(unsafe { SetConsoleCtrlHandler(Some(ignore_control), 1) });
                fs::write(root.join("child-pid"), std::process::id().to_string()).unwrap();
            }
            let changed = modes(i, o);
            fs::write(
                root.join("child-changed-modes"),
                format!("{}:{}", changed.0, changed.1),
            )
            .unwrap();
            {
                use std::io::Write;
                let mut out = std::io::stdout().lock();
                out.write_all(b"N6-CHILD-FIRST-READ\r\n").unwrap();
                out.flush().unwrap();
            }
            fs::write(root.join("child-ready"), b"ready").unwrap();
            if mode == "n6-case-tui" {
                let mut text = Vec::new();
                loop {
                    assert_eq!(
                        unsafe { WaitForSingleObject(i, 30_000) },
                        0,
                        "bounded child console read"
                    );
                    let mut record = Record {
                        kind: 0,
                        data: Payload { bytes: [0; 4] },
                    };
                    let mut n = 0;
                    check(unsafe { ReadConsoleInputW(i, &mut record, 1, &mut n) });
                    assert_eq!(n, 1);
                    if record.kind != 1 {
                        continue;
                    }
                    let key = unsafe { record.data.key };
                    // ConPTY commits Unicode on Alt key-up, unlike ordinary
                    // key-up echoes. This child consumes genuine native records.
                    if key.unicode == 0 || (key.down == 0 && key.vk != 0x12) {
                        continue;
                    }
                    if key.unicode == 13 {
                        break;
                    }
                    text.extend(std::iter::repeat_n(key.unicode, key.repeat as usize));
                }
                fs::write(
                    root.join("child-input"),
                    String::from_utf16(&text).expect("committed child UTF16"),
                )
                .unwrap();
                fs::write(root.join("child-read-complete"), b"read").unwrap();
            }
            release.wait();
            // Deliberately leave changed modes behind. Fish must reclaim them.
        }
        "n6-case-stdin" => {
            use std::io::Read;
            let mut data = Vec::new();
            std::io::stdin().read_to_end(&mut data).unwrap();
            fs::write(&args[0], data).unwrap();
            fs::write(&args[1], b"done").unwrap();
        }
        "n6-case-tool-streams" => {
            use std::process::{Command, Stdio};
            assert_eq!(args.len(), 2);
            let root = Path::new(&args[1]);
            // Obtain and validate real console handles independently. Each
            // launch redirects exactly one side, not both incidental pipes.
            let input = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("CONIN$")
                .unwrap();
            let output = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("CONOUT$")
                .unwrap();
            let _ = modes(input.as_raw_handle(), output.as_raw_handle());
            let first = Command::new(&args[0])
                .stdin(Stdio::from(input))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .unwrap();
            assert!(!first.status.success());
            assert!(first.stdout.is_empty());
            assert!(!first.stderr.is_empty());
            fs::write(root.join("tool-output-error"), first.stderr).unwrap();
            let second = Command::new(&args[0])
                .stdin(Stdio::null())
                .stdout(Stdio::from(output))
                .stderr(Stdio::piped())
                .output()
                .unwrap();
            assert!(!second.status.success());
            assert!(!second.stderr.is_empty());
            fs::write(root.join("tool-input-error"), second.stderr).unwrap();
            fs::write(root.join("tool-streams-ready"), b"checked").unwrap();
        }
        "n6-case-invalid-utf8" => {
            use std::io::Write;
            std::io::stdout().write_all(b"\xff").unwrap();
        }
        _ => panic!("unregistered N6 native case {mode}"),
    }
}
