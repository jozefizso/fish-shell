//! Query an owned inactive native screen buffer, never the user's console state.
use std::{
    ffi::c_void,
    fs,
    os::windows::io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle},
    path::Path,
};
type Handle = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Coord {
    x: i16,
    y: i16,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i16,
    top: i16,
    right: i16,
    bottom: i16,
}
#[repr(C)]
#[derive(Default)]
struct InfoEx {
    size_bytes: u32,
    size: Coord,
    cursor: Coord,
    attributes: u16,
    window: Rect,
    maximum_size: Coord,
    popup_attributes: u16,
    fullscreen_supported: i32,
    palette: [u32; 16],
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn FreeConsole() -> i32;
    fn AllocConsole() -> i32;
    fn CreateConsoleScreenBuffer(
        access: u32,
        sharing: u32,
        security: Handle,
        flags: u32,
        reserved: Handle,
    ) -> Handle;
    fn SetConsoleScreenBufferSize(output: Handle, size: Coord) -> i32;
    fn SetConsoleWindowInfo(output: Handle, absolute: i32, window: *const Rect) -> i32;
    fn SetConsoleCursorPosition(output: Handle, position: Coord) -> i32;
    fn GetConsoleScreenBufferInfoEx(output: Handle, info: *mut InfoEx) -> i32;
    fn SetConsoleScreenBufferInfoEx(output: Handle, info: *const InfoEx) -> i32;
    fn SetConsoleTextAttribute(output: Handle, attributes: u16) -> i32;
}
fn check(result: i32) {
    assert_ne!(
        result,
        0,
        "owned native screen buffer: {}",
        std::io::Error::last_os_error()
    );
}
pub fn run(result: &Path) {
    fish::threads::init();
    // ConPTY intentionally fixes its viewport origin at zero. Use a separate
    // owned classic console to exercise nonzero scrollback coordinates, not a
    // synthetic screen model or a modified user console.
    check(unsafe { FreeConsole() });
    check(unsafe { AllocConsole() });
    let raw = unsafe {
        CreateConsoleScreenBuffer(0xc0000000, 3, std::ptr::null_mut(), 1, std::ptr::null_mut())
    };
    assert_ne!(
        raw as isize,
        -1,
        "create owned screen buffer: {}",
        std::io::Error::last_os_error()
    );
    assert!(!raw.is_null());
    let output = unsafe { OwnedHandle::from_raw_handle(raw) };
    let input = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONIN$")
        .unwrap();
    check(unsafe { SetConsoleScreenBufferSize(raw, Coord { x: 120, y: 80 }) });
    let mut info = InfoEx {
        size_bytes: std::mem::size_of::<InfoEx>() as u32,
        ..Default::default()
    };
    check(unsafe { GetConsoleScreenBufferInfoEx(raw, &mut info) });
    info.palette[5] = 0x00563412;
    info.palette[3] = 0x00c3b2a1;
    // The setter's rectangle upper bounds are exclusive, unlike its getter.
    info.window.right += 1;
    info.window.bottom += 1;
    check(unsafe { SetConsoleScreenBufferInfoEx(raw, &info) });
    let window = Rect {
        left: 10,
        top: 12,
        right: 49,
        bottom: 35,
    };
    check(unsafe { SetConsoleWindowInfo(raw, 1, &window) });
    check(unsafe { SetConsoleCursorPosition(raw, Coord { x: 15, y: 18 }) });
    let session =
        fish::console_windows::ConsoleSession::new(input.as_handle(), output.as_handle()).unwrap();
    assert_eq!(session.viewport_origin().unwrap(), (10, 12));
    assert_eq!(session.viewport().unwrap(), (40, 24));
    let position = session.cursor_position().unwrap().unwrap();
    assert_eq!((position.x, position.y), (5, 6));
    check(unsafe { SetConsoleTextAttribute(output.as_raw_handle(), 0x0053) });
    assert_eq!(
        session.background_color().unwrap(),
        xterm_color::Color::rgb(0x1212, 0x3434, 0x5656)
    );
    check(unsafe { SetConsoleTextAttribute(raw, 0x4053) });
    assert_eq!(
        session.background_color().unwrap(),
        xterm_color::Color::rgb(0xa1a1, 0xb2b2, 0xc3c3)
    );
    // Scrolling away from the live cursor is not a malformed console or EOF.
    check(unsafe { SetConsoleCursorPosition(raw, Coord { x: 5, y: 5 }) });
    check(unsafe { SetConsoleWindowInfo(raw, 1, &window) });
    assert!(session.cursor_position().unwrap().is_none());
    fs::write(
        result,
        b"viewport=40:24;cursor=5:6;palette=123456;reverse=a1b2c3;offscreen=unknown",
    )
    .unwrap();
}
