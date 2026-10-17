//! Isolated probes exercising fish's production native endpoints, not alternate transports.
use fish::io::NativeOutput;
use std::ffi::c_void;
use std::io::{Read, Write};
use std::os::windows::io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::mpsc;
use std::time::Duration;

#[path = "conpty.rs"]
mod conpty;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreatePipe(
        read: *mut *mut c_void,
        write: *mut *mut c_void,
        attrs: *mut c_void,
        size: u32,
    ) -> i32;
    fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
    fn GetStdHandle(which: u32) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn FreeConsole() -> i32;
    fn AllocConsole() -> i32;
    fn SetConsoleCursorPosition(handle: *mut c_void, position: u32) -> i32;
    fn ReadConsoleOutputCharacterW(
        handle: *mut c_void,
        text: *mut u16,
        length: u32,
        position: u32,
        count: *mut u32,
    ) -> i32;
    fn GetConsoleMode(handle: *mut c_void, mode: *mut u32) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
}
fn pipe() -> (OwnedHandle, OwnedHandle) {
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
pub fn run(mode: &str) {
    match mode {
        "transport-console-write" => {
            let console = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("CONOUT$")
                .unwrap();
            let mut output = NativeOutput::owned(console.into());
            assert!(
                output.is_console(),
                "ConPTY client must expose a console endpoint"
            );
            output.write_all("native 雪 🐟".as_bytes()).unwrap();
            output.flush().unwrap();
            assert_eq!(
                output.write_all(b"\xff").unwrap_err().kind(),
                std::io::ErrorKind::InvalidData
            );
        }
        "transport-bytes" => {
            let mut input = fish::fds::BorrowedFdFile::stdin().unwrap();
            let mut data = Vec::new();
            input.read_to_end(&mut data).unwrap();
            let mut output = NativeOutput::stdout().unwrap();
            output.write_all(&data).unwrap();
            output.flush().unwrap();
        }
        "transport-errors" => {
            let (reader, writer) = pipe();
            drop(reader);
            let error = NativeOutput::owned(writer)
                .write_all(b"reader is closed")
                .expect_err("closed-reader write must fail");
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
            let standard = unsafe { GetStdHandle((-11i32) as u32) };
            assert_ne!(unsafe { CloseHandle(standard) }, 0);
            assert!(
                NativeOutput::stdout()
                    .and_then(|mut output| output.write_all(b"closed"))
                    .is_err(),
                "closed standard handle silently succeeded"
            );
            for which in [(-10i32) as u32, (-11i32) as u32, (-12i32) as u32] {
                for handle in [std::ptr::null_mut(), usize::MAX as *mut c_void] {
                    assert_ne!(unsafe { SetStdHandle(which, handle) }, 0);
                    match which {
                        value if value == (-10i32) as u32 => assert!(
                            fish::fds::BorrowedFdFile::stdin().is_err(),
                            "invalid stdin silently accepted"
                        ),
                        value if value == (-11i32) as u32 => assert!(
                            NativeOutput::stdout().is_err(),
                            "invalid stdout silently accepted"
                        ),
                        _ => assert!(
                            NativeOutput::stderr().is_err(),
                            "invalid stderr silently accepted"
                        ),
                    }
                }
            }
        }
        "transport-lifetime" => {
            let (reader, writer) = pipe();
            {
                let mut borrowed = NativeOutput::borrowed(writer.as_handle());
                borrowed.write_all(b"borrowed:").unwrap();
            }
            // The original owner must remain usable after dropping the borrowing wrapper.
            let mut original = std::fs::File::from(writer);
            original.write_all(b"owner").unwrap();
            let copy = original.try_clone().unwrap();
            drop(original);
            let (send, recv) = mpsc::channel();
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let result = std::fs::File::from(reader).read_to_end(&mut bytes);
                let _ = send.send((result, bytes));
            });
            drop(NativeOutput::owned(copy.into()));
            let (result, bytes) = recv
                .recv_timeout(Duration::from_secs(10))
                .expect("owned writers leaked: no EOF");
            result.unwrap();
            assert_eq!(bytes, b"borrowed:owner");
            let stdout = unsafe { GetStdHandle((-11i32) as u32) };
            assert!(!stdout.is_null());
            {
                NativeOutput::stdout()
                    .unwrap()
                    .write_all(b"standard-borrow:")
                    .unwrap();
            }
            std::io::stdout()
                .write_all(b"standard-owner-still-open")
                .unwrap();
        }
        "transport-console" => {
            unsafe {
                FreeConsole();
            } // Only this test-owned child detaches.
            assert_ne!(
                unsafe { AllocConsole() },
                0,
                "isolated native console unavailable"
            );
            let console = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("CONOUT$")
                .unwrap();
            assert_ne!(
                unsafe { SetConsoleCursorPosition(console.as_raw_handle(), 0) },
                0
            );
            // Legacy screen-buffer reads cannot represent astral characters. Observe
            // BMP here and retain full Unicode acceptance through ConPTY below.
            let text = "native 雪";
            let mut endpoint = NativeOutput::borrowed(console.as_handle());
            endpoint.write_all(text.as_bytes()).unwrap();
            endpoint.flush().unwrap();
            let expected: Vec<u16> = text.encode_utf16().collect();
            // The API counts screen cells, not UTF-16 units: request enough cells
            // to include wide glyphs, then discard only the untouched padding.
            let mut actual = vec![0u16; 32];
            let mut read = 0;
            assert_ne!(
                unsafe {
                    ReadConsoleOutputCharacterW(
                        console.as_raw_handle(),
                        actual.as_mut_ptr(),
                        actual.len() as u32,
                        0,
                        &mut read,
                    )
                },
                0
            );
            actual.truncate(read as usize);
            while actual.last() == Some(&(b' ' as u16)) {
                actual.pop();
            }
            assert_eq!(actual, expected);
            assert_eq!(
                endpoint.write_all(b"\xff").unwrap_err().kind(),
                std::io::ErrorKind::InvalidData
            );
            drop(endpoint);
            drop(console);
            conpty::observe();
        }
        _ => panic!("unknown transport probe {mode}"),
    }
}

/// Give the real selected fish explicit attached-console handles. A ConPTY host
/// may itself have redirected std handles; those pipes must not make this test
/// accidentally exercise the byte-transparent redirected-output path.
pub fn run_console_builtin(script: &std::ffi::OsStr) -> ! {
    use std::process::{Command, Stdio};
    let input = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONIN$")
        .unwrap();
    let output = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONOUT$")
        .unwrap();
    for handle in [input.as_raw_handle(), output.as_raw_handle()] {
        let mut mode = 0;
        assert_ne!(
            unsafe { GetConsoleMode(handle, &mut mode) },
            0,
            "builtin fixture requires real console handles"
        );
    }
    let fish = std::env::current_exe().unwrap().with_file_name("fish.exe");
    let mut command = Command::new(fish);
    command
        .args(["-N", "-c"])
        .arg(script)
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(output));
    let mut child = command.spawn().unwrap();
    drop(command);
    let wait = unsafe { WaitForSingleObject(child.as_raw_handle(), 10_000) };
    if wait != 0 {
        let _ = child.kill();
        let _ = child.wait();
        panic!("real fish console child exceeded bounded wait: {wait:#x}");
    }
    let status = child.wait().unwrap();
    drop(child);
    std::process::exit(status.code().expect("real fish console exit status"));
}
