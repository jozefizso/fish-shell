//! Test-only ConPTY host. Production remains synchronous console I/O, not a PTY backend.
//! API/lifetime contract: https://learn.microsoft.com/windows/console/creating-a-pseudoconsole-session
use std::ffi::c_void;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::mpsc;
use std::time::Duration;

type Handle = *mut c_void;
#[repr(C)]
#[derive(Default)]
struct Coord {
    x: i16,
    y: i16,
}
#[repr(C)]
struct StartupInfo {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_chars: u32,
    y_chars: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved_bytes: *mut u8,
    stdin: Handle,
    stdout: Handle,
    stderr: Handle,
}
#[repr(C)]
struct StartupInfoEx {
    info: StartupInfo,
    attributes: Handle,
}
#[repr(C)]
struct ProcessInfo {
    process: Handle,
    thread: Handle,
    process_id: u32,
    thread_id: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreatePipe(read: *mut Handle, write: *mut Handle, attributes: Handle, size: u32) -> i32;
    fn CreatePseudoConsole(
        size: Coord,
        input: Handle,
        output: Handle,
        flags: u32,
        console: *mut Handle,
    ) -> i32;
    fn ClosePseudoConsole(console: Handle);
    fn InitializeProcThreadAttributeList(
        list: Handle,
        count: u32,
        flags: u32,
        bytes: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: Handle,
        flags: u32,
        attribute: usize,
        value: Handle,
        size: usize,
        previous: Handle,
        returned: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: Handle);
    fn CreateProcessW(
        application: *const u16,
        command: *mut u16,
        process_attributes: Handle,
        thread_attributes: Handle,
        inherit: i32,
        flags: u32,
        environment: Handle,
        cwd: *const u16,
        startup: *mut StartupInfo,
        process: *mut ProcessInfo,
    ) -> i32;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn TerminateProcess(handle: Handle, code: u32) -> i32;
    fn GetExitCodeProcess(handle: Handle, code: *mut u32) -> i32;
}

pub(super) fn pipe() -> (OwnedHandle, OwnedHandle) {
    let (mut read, mut write) = (std::ptr::null_mut(), std::ptr::null_mut());
    assert_ne!(
        unsafe { CreatePipe(&mut read, &mut write, std::ptr::null_mut(), 0) },
        0
    );
    unsafe {
        (
            OwnedHandle::from_raw_handle(read),
            OwnedHandle::from_raw_handle(write),
        )
    }
}
struct Console(Handle);
impl Drop for Console {
    fn drop(&mut self) {
        unsafe { ClosePseudoConsole(self.0) };
    }
}
struct Attributes(Vec<usize>);
impl Attributes {
    fn pointer(&mut self) -> Handle {
        self.0.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.pointer()) };
    }
}

pub fn observe() {
    let probe = std::env::current_exe().unwrap();
    observe_process(
        &probe,
        "transport-console-write",
        "native 雪 🐟".as_bytes(),
        0,
    );
    // The native launcher gives the actual sibling fish explicit CONIN$/CONOUT$
    // handles so inherited host pipes cannot disguise a non-console test.
    for script in [
        "printf '\\xC3\\xA9\\xF0\\x9F\\x90\\x9F'",
        "echo -en '\\xC3\\xA9\\xF0\\x9F\\x90\\x9F'",
    ] {
        observe_process(
            &probe,
            &format!("transport-console-builtin \"{script}\""),
            "é🐟".as_bytes(),
            0,
        );
    }
    // Incomplete and invalid final command output must still fail validation.
    for script in ["printf '\\xC3'", "echo -en '\\xF0\\x9F'", "printf '\\xff'"] {
        observe_process(
            &probe,
            &format!("transport-console-builtin \"{script}\""),
            b"",
            1,
        );
    }
}

pub struct Session {
    pub process: OwnedHandle,
    console: Option<Console>,
    input: Option<OwnedHandle>,
    output: mpsc::Receiver<(std::io::Result<usize>, Vec<u8>)>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl Session {
    /// Arguments are the fixture's already encoded Win32 command-line tail.
    pub fn spawn(executable: &std::path::Path, arguments: &str) -> Self {
        let (input_read, input_write) = pipe();
        let (output_read, output_write) = pipe();
        let mut console = std::ptr::null_mut();
        let result = unsafe {
            CreatePseudoConsole(
                Coord { x: 80, y: 25 },
                input_read.as_raw_handle(),
                output_write.as_raw_handle(),
                0,
                &mut console,
            )
        };
        assert_eq!(result, 0, "CreatePseudoConsole: {result:#x}");
        let console = Console(console);
        // Drain before launching or closing: both operations may synchronously produce output.
        let (send, recv) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = std::fs::File::from(output_read).read_to_end(&mut bytes);
            let _ = send.send((result, bytes));
        });
        let mut required = 0;
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut required);
        }
        assert!(required > 0);
        let words = required.div_ceil(std::mem::size_of::<usize>());
        let mut storage = vec![0usize; words];
        assert_ne!(
            unsafe {
                InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut required)
            },
            0
        );
        let mut attributes = Attributes(storage);
        assert_ne!(
            unsafe {
                UpdateProcThreadAttribute(
                    attributes.pointer(),
                    0,
                    0x00020016,
                    console.0,
                    std::mem::size_of::<Handle>(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let application: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let mut command: Vec<u16> = format!("\"{}\" {arguments}", executable.display())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut startup: StartupInfoEx = unsafe { std::mem::zeroed() };
        startup.info.cb = std::mem::size_of::<StartupInfoEx>() as u32;
        startup.attributes = attributes.pointer();
        let mut process: ProcessInfo = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe {
                CreateProcessW(
                    application.as_ptr(),
                    command.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                    0x00080000,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &mut startup.info,
                    &mut process,
                )
            },
            0,
            "CreateProcessW: {}",
            std::io::Error::last_os_error()
        );
        let process_handle = unsafe { OwnedHandle::from_raw_handle(process.process) };
        let thread_handle = unsafe { OwnedHandle::from_raw_handle(process.thread) };
        drop(thread_handle);
        drop(attributes);
        drop(input_read);
        drop(output_write);
        Self {
            process: process_handle,
            console: Some(console),
            input: Some(input_write),
            output: recv,
            reader: Some(reader),
        }
    }

    pub fn close(&mut self) {
        // ClosePseudoConsole sends actual CTRL_CLOSE to attached clients. Since
        // 24H2 it returns before they exit; keep process observation separate.
        drop(self.console.take());
        drop(self.input.take());
    }

    pub fn drain(&mut self) -> Vec<u8> {
        self.close();
        let (result, bytes) = self
            .output
            .recv_timeout(Duration::from_secs(10))
            .expect("ConPTY output did not reach EOF");
        result.unwrap();
        self.reader.take().unwrap().join().unwrap();
        bytes
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
        let wait = unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) };
        if wait == 258 {
            // Failure cleanup only: callers must prove their expected exit
            // before this guard, never treat forced cleanup as close success.
            assert_ne!(
                unsafe { TerminateProcess(self.process.as_raw_handle(), 1) },
                0
            );
            assert_eq!(
                unsafe { WaitForSingleObject(self.process.as_raw_handle(), 10_000) },
                0
            );
        } else {
            assert_eq!(wait, 0, "query retained ConPTY client");
        }
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
    }
}

fn observe_process(
    executable: &std::path::Path,
    arguments: &str,
    expected: &[u8],
    expected_code: u32,
) {
    let mut session = Session::spawn(executable, arguments);
    let process_handle = &session.process;
    let wait = unsafe { WaitForSingleObject(process_handle.as_raw_handle(), 10_000) };
    if wait != 0 {
        unsafe {
            TerminateProcess(process_handle.as_raw_handle(), 1);
            WaitForSingleObject(process_handle.as_raw_handle(), 10_000);
        }
        panic!("ConPTY child exceeded bounded wait: {wait:#x}");
    }
    let mut code = 0;
    assert_ne!(
        unsafe { GetExitCodeProcess(process_handle.as_raw_handle(), &mut code) },
        0
    );
    assert_eq!(
        code,
        expected_code,
        "production console child failed: {} {arguments}",
        executable.display()
    );
    let bytes = session.drain();
    let text = std::str::from_utf8(&bytes).expect("ConPTY must emit valid UTF-8");
    assert_eq!(
        visible_text(text.as_bytes()),
        expected,
        "unexpected ConPTY observation: {text:?}"
    );
}

// Decode only framing emitted for this single-line, single-write fixture. Unexpected
// VT operations fail rather than hiding output or accepting replacement characters.
fn visible_text(bytes: &[u8]) -> Vec<u8> {
    let mut visible = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0x1b {
            visible.push(bytes[index]);
            index += 1;
            continue;
        }
        index += 1;
        match bytes.get(index).copied().expect("truncated VT escape") {
            b'[' => {
                index += 1;
                while index < bytes.len() && !(0x40..=0x7e).contains(&bytes[index]) {
                    index += 1;
                }
                assert!(index < bytes.len(), "truncated CSI");
                index += 1;
            }
            b']' => {
                index += 1;
                loop {
                    match bytes.get(index).copied().expect("truncated OSC") {
                        7 => {
                            index += 1;
                            break;
                        }
                        0x1b if bytes.get(index + 1) == Some(&b'\\') => {
                            index += 2;
                            break;
                        }
                        _ => index += 1,
                    }
                }
            }
            byte => panic!("unexpected VT escape {byte:#x}"),
        }
    }
    visible
}
