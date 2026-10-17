//! Test-only ConPTY host. Production remains synchronous console I/O, not a PTY backend.
//! API/lifetime contract: https://learn.microsoft.com/windows/console/creating-a-pseudoconsole-session
use super::pipe;
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

fn observe_process(
    executable: &std::path::Path,
    arguments: &str,
    expected: &[u8],
    expected_code: u32,
) {
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
    std::thread::spawn(move || {
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
    drop(process_handle);
    drop(console); // Final console frame is drained concurrently before EOF.
    drop(input_write);
    let (result, bytes) = recv
        .recv_timeout(Duration::from_secs(10))
        .expect("ConPTY output did not reach EOF");
    result.unwrap();
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
