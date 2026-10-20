//! Owned real process/pipe barriers for evaluator-input console families.
use std::ffi::{OsStr, OsString, c_void};
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::path::Path;

type Handle = *mut c_void;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn GetStdHandle(kind: u32) -> Handle;
    fn SetStdHandle(kind: u32, handle: Handle) -> i32;
    fn SetEvent(event: Handle) -> i32;
    fn CreateFileW(name: *const u16, access: u32, sharing: u32, security: Handle, disposition: u32, flags: u32, template: Handle) -> Handle;
    fn ReadConsoleW(input: Handle, buffer: *mut u16, count: u32, read: *mut u32, reserved: Handle) -> i32;
    fn CreateNamedPipeW(name: *const u16, open: u32, mode: u32, instances: u32, out_size: u32, in_size: u32, timeout: u32, security: Handle) -> Handle;
    fn ConnectNamedPipe(pipe: Handle, overlapped: Handle) -> i32;
    fn WriteFile(file: Handle, bytes: *const u8, count: u32, written: *mut u32, overlapped: Handle) -> i32;
    fn FlushFileBuffers(file: Handle) -> i32;
    fn SetConsoleCtrlHandler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn owned(handle: Handle, operation: &str) -> OwnedHandle {
    assert!(!handle.is_null() && handle as usize != usize::MAX, "{operation}: {}", std::io::Error::last_os_error());
    unsafe { OwnedHandle::from_raw_handle(handle) }
}
fn release(name: &OsStr) -> OwnedHandle {
    owned(unsafe { OpenEventW(0x0010_0000, 0, wide(name).as_ptr()) }, "OpenEventW(release)")
}
fn wait_release(handle: &OwnedHandle) {
    assert_eq!(unsafe { WaitForSingleObject(handle.as_raw_handle(), 30_000) }, 0, "owned release wait failed or timed out");
}
unsafe extern "system" fn retain_until_release(control: u32) -> i32 {
    i32::from(matches!(control, 0 | 1))
}

pub fn try_main(mode: &str, args: &[OsString]) -> Option<i32> {
    match mode {
        "n7-eval-io-client" => {
            assert_eq!(args.len(), 3);
            fish::threads::init();
            let original_input = unsafe { GetStdHandle(-10i32 as u32) };
            let output = unsafe { GetStdHandle(-11i32 as u32) };
            let session = fish::console_windows::ConsoleSession::new(
                unsafe { BorrowedHandle::borrow_raw(original_input) },
                unsafe { BorrowedHandle::borrow_raw(output) },
            ).unwrap();
            let _routing = fish::console_windows::install_session(std::rc::Rc::clone(&session));
            fish::tty_handoff::initialize_tty_protocols(&fish::env::EnvNull::new());
            let mut editor = fish::tty_handoff::NativeEditorScope::enter().unwrap();
            editor.enable_protocols().unwrap();
            let available = release(&args[1]);
            wait_release(&available);
            let pipe = owned(unsafe {
                CreateFileW(wide(&args[0]).as_ptr(), 0x8000_0000, 3, std::ptr::null_mut(), 3, 0x80, std::ptr::null_mut())
            }, "CreateFileW(real pipe input)");
            // As in the retained N5 pipe gates, import actual native stdin
            // before the lazy root descriptor table. Console polling retains
            // its own independent console handle, never reads on a worker.
            assert_ne!(unsafe { SetStdHandle(-10i32 as u32, pipe.as_raw_handle()) }, 0);
            let vars = fish::env::EnvStack::new();
            vars.initialize_native().unwrap();
            let mut parser = fish::parser::Parser::new(vars, fish::parser::CancelBehavior::Return).unwrap();
            let mut controls = fish::signal::initialize_console_controls().unwrap();
            let before = fish::signal::console_control_epoch();
            let guard = fish::console_windows::begin_evaluation_input();
            let code: fish::prelude::WString = "read -l n7_partial; printf bad > io-tail".into();
            let result = parser.eval(&code, &fish::io::IoChain::new());
            assert_eq!(result.status.status_value(), 130);
            assert_eq!(result.status.native_raw(), None);
            assert!(guard.cancelled() && guard.take_input_reset());
            assert!(!guard.take_input_reset(), "a nested owner must not reset later input twice");
            assert_eq!(fish::signal::console_control_epoch(), before);
            assert!(fish::signal::pending_console_controls().is_empty());
            assert!(!Path::new("io-tail").exists());
            drop(guard);
            assert_eq!(fish::signal::signal_check_cancel(), 0);
            let code: fish::prelude::WString = "true".into();
            assert_eq!(parser.eval(&code, &fish::io::IoChain::new()).status.status_value(), 0);
            assert_ne!(unsafe { SetStdHandle(-10i32 as u32, original_input) }, 0);
            editor.finish().unwrap();
            controls.shutdown().unwrap();
            fs::write(Path::new(&args[2]), b"cancelled:recovered:no-receipt").unwrap();
            Some(0)
        }
        "n7-eval-hold" => {
            assert_eq!(args.len(), 3);
            let release = release(&args[1]);
            // A cancellation request is not an acknowledgement or an exit. The
            // test explicitly owns the only release and observes the real code.
            assert_ne!(unsafe { SetConsoleCtrlHandler(Some(retain_until_release), 1) }, 0);
            fs::write(Path::new(&args[0]), b"ready").unwrap();
            wait_release(&release);
            Some(args[2].to_str().unwrap().parse::<u32>().unwrap() as i32)
        }
        "n7-eval-stdin" => {
            assert_eq!(args.len(), 2);
            fs::write(Path::new(&args[0]), b"ready").unwrap();
            let input = unsafe { GetStdHandle(-10i32 as u32) };
            let mut text = Vec::new();
            loop {
                let mut buffer = [0u16; 128];
                let mut read = 0;
                assert_ne!(unsafe { ReadConsoleW(input, buffer.as_mut_ptr(), buffer.len() as u32, &mut read, std::ptr::null_mut()) }, 0);
                assert!(read != 0 && read as usize <= buffer.len());
                text.extend_from_slice(&buffer[..read as usize]);
                assert!(text.len() <= 4096, "unbounded child input");
                if text.last() == Some(&10) {
                    break;
                }
            }
            fs::write(Path::new(&args[1]), String::from_utf16(&text).unwrap()).unwrap();
            Some(0)
        }
        "n7-eval-pipe-server" => {
            assert_eq!(args.len(), 5);
            let release = release(&args[3]);
            let pipe = owned(unsafe {
                CreateNamedPipeW(wide(&args[0]).as_ptr(), 2 | 0x0008_0000, 0, 1, 4096, 4096, 0, std::ptr::null_mut())
            }, "CreateNamedPipeW");
            fs::write(Path::new(&args[1]), b"ready").unwrap();
            let available = owned(unsafe {
                OpenEventW(2, 0, wide(&args[4]).as_ptr())
            }, "OpenEventW(pipe available)");
            assert_ne!(unsafe { SetEvent(available.as_raw_handle()) }, 0);
            if unsafe { ConnectNamedPipe(pipe.as_raw_handle(), std::ptr::null_mut()) } == 0 {
                assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(535), "ConnectNamedPipe");
            }
            let mut written = 0;
            assert_ne!(unsafe { WriteFile(pipe.as_raw_handle(), b"partial".as_ptr(), 7, &mut written, std::ptr::null_mut()) }, 0);
            assert_eq!(written, 7);
            assert_ne!(unsafe { FlushFileBuffers(pipe.as_raw_handle()) }, 0, "FlushFileBuffers proves real consumer progress");
            fs::write(Path::new(&args[2]), b"consumed").unwrap();
            wait_release(&release);
            Some(0)
        }
        _ => None,
    }
}
