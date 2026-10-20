//! Native child-output barrier; never reads console input or supplies shell results.
use std::{ffi::{OsString, c_void}, fs, os::windows::{ffi::OsStrExt, io::{AsRawHandle, FromRawHandle, OwnedHandle}}, path::Path};
type Handle = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Coord { x: i16, y: i16 }
#[repr(C)]
#[derive(Default)]
struct BufferInfo { size: Coord, cursor: Coord, attributes: u16, window: [i16; 4], maximum: Coord }
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn GetStdHandle(which: u32) -> Handle;
    fn GetConsoleScreenBufferInfo(output: Handle, info: *mut BufferInfo) -> i32;
    fn WriteConsoleW(output: Handle, text: *const u16, len: u32, written: *mut u32, reserved: Handle) -> i32;
    fn SetConsoleCursorPosition(output: Handle, position: Coord) -> i32;
}
pub fn try_main(mode: &str, args: &[OsString]) -> Option<i32> {
    if mode != "n7-render-background" { return None; }
    assert_eq!(args.len(), 3, "ready file, release event, done file");
    let name: Vec<_> = args[1].encode_wide().chain([0]).collect();
    let event = unsafe { OpenEventW(0x0010_0000, 0, name.as_ptr()) };
    assert!(!event.is_null(), "open owned output barrier: {}", std::io::Error::last_os_error());
    let event = unsafe { OwnedHandle::from_raw_handle(event) };
    fs::write(Path::new(&args[0]), b"ready").unwrap();
    assert_eq!(unsafe { WaitForSingleObject(event.as_raw_handle(), 30_000) }, 0, "release owned output barrier");
    let output = unsafe { GetStdHandle(-11i32 as u32) };
    let mut before = BufferInfo::default();
    assert_ne!(unsafe { GetConsoleScreenBufferInfo(output, &mut before) }, 0, "background child needs actual inherited console output");
    let text: Vec<_> = "\rN7-BACKGROUND-OVERWRITE".encode_utf16().collect();
    let mut written = 0;
    assert_ne!(unsafe { WriteConsoleW(output, text.as_ptr(), text.len() as u32, &mut written, std::ptr::null_mut()) }, 0);
    assert_eq!(written, text.len() as u32);
    // A cursor-only state comparison cannot detect this actual overwrite. Job/output
    // generation invalidation must repaint the original editor text after child exit.
    assert_ne!(unsafe { SetConsoleCursorPosition(output, before.cursor) }, 0);
    fs::write(Path::new(&args[2]), b"written").unwrap();
    Some(0)
}
