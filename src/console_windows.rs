//! Owned console-record input and native console modes for the shared reader.
//!
//! A session starts paused, without changing either console mode. The caller owns
//! terminal protocols: pause input before disabling protocols/restoring modes,
//! and enter editor modes before enabling protocols/resuming input. Native reads
//! consume one ready record on the main thread, leaving typeahead in the console.

use std::cell::{Cell, RefCell};
use std::io;
use std::marker::PhantomData;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, OwnedHandle, RawHandle};
use std::rc::Rc;

const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;
const WAIT_FAILED: u32 = u32::MAX;

const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
const ENABLE_LINE_INPUT: u32 = 0x0002;
const ENABLE_ECHO_INPUT: u32 = 0x0004;
const ENABLE_WINDOW_INPUT: u32 = 0x0008;
const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

const KEY_EVENT: u16 = 0x0001;
const MOUSE_EVENT: u16 = 0x0002;
const WINDOW_BUFFER_SIZE_EVENT: u16 = 0x0004;
const MENU_EVENT: u16 = 0x0008;
const FOCUS_EVENT: u16 = 0x0010;

/// A Win32 console record, without pointers, native unions, or decoding policy.
/// In particular, key-up UTF-16 units are retained: some hosts deliver committed
/// non-BMP text that way. The shared input decoder owns filtering and assembly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleRecord {
    Key {
        down: bool,
        repeat: u16,
        virtual_key: u16,
        scan_code: u16,
        utf16: u16,
        control_state: u32,
    },
    Resize,
    Mouse {
        x: i16,
        y: i16,
        buttons: u32,
        control_state: u32,
        flags: u32,
    },
    Focus(bool),
    Menu(u32),
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KeyEventRecord {
    down: i32,
    repeat: u16,
    virtual_key: u16,
    scan_code: u16,
    utf16: u16,
    control_state: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MouseEventRecord {
    position: [i16; 2],
    buttons: u32,
    control_state: u32,
    flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
union InputEvent {
    key: KeyEventRecord,
    mouse: MouseEventRecord,
    size: [i16; 2],
    menu: u32,
    focus: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InputRecord {
    event_type: u16,
    event: InputEvent,
}

impl InputRecord {
    fn translate(self) -> io::Result<ConsoleRecord> {
        // ReadConsoleInputW initializes the union member selected by EventType.
        // Every member is POD, including the UnicodeChar half of KEY_EVENT's
        // character union; ReadConsoleInputW always supplies the wide variant.
        unsafe {
            match self.event_type {
                KEY_EVENT => {
                    let key = self.event.key;
                    Ok(ConsoleRecord::Key {
                        down: key.down != 0,
                        repeat: key.repeat,
                        virtual_key: key.virtual_key,
                        scan_code: key.scan_code,
                        utf16: key.utf16,
                        control_state: key.control_state,
                    })
                }
                MOUSE_EVENT => {
                    let mouse = self.event.mouse;
                    Ok(ConsoleRecord::Mouse {
                        x: mouse.position[0],
                        y: mouse.position[1],
                        buttons: mouse.buttons,
                        control_state: mouse.control_state,
                        flags: mouse.flags,
                    })
                }
                WINDOW_BUFFER_SIZE_EVENT => Ok(ConsoleRecord::Resize),
                MENU_EVENT => Ok(ConsoleRecord::Menu(self.event.menu)),
                FOCUS_EVENT => Ok(ConsoleRecord::Focus(self.event.focus != 0)),
                kind => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unrecognized console input record type {kind:#x}"),
                )),
            }
        }
    }
}

#[repr(C)]
#[derive(Default)]
struct ConsoleScreenBufferInfo {
    size: [i16; 2],
    cursor: [i16; 2],
    attributes: u16,
    window: [i16; 4],
    maximum_size: [i16; 2],
}

#[repr(C)]
#[derive(Default)]
struct ConsoleScreenBufferInfoEx {
    byte_size: u32,
    size: [i16; 2],
    cursor: [i16; 2],
    attributes: u16,
    window: [i16; 4],
    maximum_size: [i16; 2],
    popup_attributes: u16,
    fullscreen_supported: i32,
    color_table: [u32; 16],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleMode(handle: RawHandle, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: RawHandle, mode: u32) -> i32;
    fn ReadConsoleInputW(
        input: RawHandle,
        buffer: *mut InputRecord,
        count: u32,
        read: *mut u32,
    ) -> i32;
    fn GetConsoleScreenBufferInfo(handle: RawHandle, info: *mut ConsoleScreenBufferInfo) -> i32;
    fn GetConsoleScreenBufferInfoEx(handle: RawHandle, info: *mut ConsoleScreenBufferInfoEx)
    -> i32;
    fn FlushConsoleInputBuffer(input: RawHandle) -> i32;
    fn WriteFile(
        output: RawHandle,
        bytes: *const u8,
        count: u32,
        written: *mut u32,
        overlapped: *mut std::ffi::c_void,
    ) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
}

struct Modes {
    original_input: u32,
    original_output: u32,
    // Once a mode has been touched, final restoration remains our duty even
    // while a child temporarily owns the console and changes that mode again.
    input_owned: bool,
    output_owned: bool,
    editor: bool,
}

/// Main-thread console ownership, admission, saved modes, and shared decoding.
/// No read is outstanding between calls, including while a child owns input.
pub struct ConsoleSession {
    input: OwnedHandle,
    output: OwnedHandle,
    modes: RefCell<Modes>,
    paused: Cell<bool>,
    decoder: RefCell<crate::input::NativeDecoder>,
}

impl ConsoleSession {
    /// Duplicate both handles without inheritance and validate each console mode
    /// independently. No modes or terminal protocols are changed.
    pub fn new(input: BorrowedHandle<'_>, output: BorrowedHandle<'_>) -> io::Result<Rc<Self>> {
        crate::threads::assert_is_main_thread();
        let input = input.try_clone_to_owned()?;
        let output = output.try_clone_to_owned()?;
        let original_input = console_mode(input.as_handle())?;
        let original_output = console_mode(output.as_handle())?;
        Ok(Rc::new(Self {
            input,
            output,
            modes: RefCell::new(Modes {
                original_input,
                original_output,
                input_owned: false,
                output_owned: false,
                editor: false,
            }),
            paused: Cell::new(true),
            decoder: RefCell::default(),
        }))
    }

    /// Protocol commands are ASCII, independent of the console code page.
    /// Use the retained terminal handle, not a renderer whose previous failure
    /// must remain sticky. In particular cleanup cannot replay an unsent frame.
    pub(crate) fn write_protocol(&self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let count = bytes.len().min(u32::MAX as usize) as u32;
            let mut written = 0;
            if unsafe {
                WriteFile(
                    self.output.as_raw_handle(),
                    bytes.as_ptr(),
                    count,
                    &mut written,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if written == 0 || written > count {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "console protocol write made no valid progress",
                ));
            }
            bytes = &bytes[written as usize..];
        }
        Ok(())
    }

    /// Enter native raw editor modes while input admission is paused. VT input
    /// preserves actual paste/protocol bytes delivered as console key records.
    /// Processed input stays OFF: Ctrl-C belongs to the shared key decoder.
    pub fn enter_editor_modes(&self) -> io::Result<()> {
        self.require_paused()?;
        let mut modes = self.modes.borrow_mut();
        if modes.editor {
            return Ok(());
        }
        let input_mode = (modes.original_input
            | ENABLE_WINDOW_INPUT
            | ENABLE_EXTENDED_FLAGS
            | ENABLE_VIRTUAL_TERMINAL_INPUT)
            & !(ENABLE_PROCESSED_INPUT
                | ENABLE_LINE_INPUT
                | ENABLE_ECHO_INPUT
                | ENABLE_QUICK_EDIT_MODE);
        let output_mode =
            modes.original_output | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
        modes.input_owned = true;
        let result = set_console_mode(self.input.as_handle(), input_mode).and_then(|()| {
            modes.output_owned = true;
            set_console_mode(self.output.as_handle(), output_mode)
        });
        if let Err(error) = result {
            // Try every touched handle even if the failing call itself may not
            // have changed a mode. Retain ownership so Drop retries restoration.
            return Err(with_rollback_error(
                error,
                self.restore_modes_locked(&mut modes),
            ));
        }
        modes.editor = true;
        Ok(())
    }

    /// Restore the exact original modes, attempting both even if one fails.
    /// The caller must already have paused input and disabled owned protocols.
    pub fn restore_modes(&self) -> io::Result<()> {
        self.require_paused()?;
        self.restore_modes_locked(&mut self.modes.borrow_mut())
    }

    fn restore_modes_locked(&self, modes: &mut Modes) -> io::Result<()> {
        modes.editor = false;
        let output = if modes.output_owned {
            set_console_mode(self.output.as_handle(), modes.original_output)
        } else {
            Ok(())
        };
        let input = if modes.input_owned {
            set_console_mode(self.input.as_handle(), modes.original_input)
        } else {
            Ok(())
        };
        match (output, input) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(output), Err(input)) => Err(io::Error::new(
                output.kind(),
                format!("restoring console output mode: {output}; restoring input mode: {input}"),
            )),
        }
    }

    fn require_paused(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        if !self.paused.get() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "console mode changes require paused input",
            ));
        }
        Ok(())
    }

    /// Prevent further admission without consuming or discarding any input.
    pub fn pause_input(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        self.paused.set(true);
        Ok(())
    }

    /// Resume admission only after reclaiming native editor modes.
    pub fn resume_input(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        if !self.modes.borrow().editor {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "console input requires editor modes before resuming",
            ));
        }
        self.paused.set(false);
        Ok(())
    }

    /// The actual retained console input handle, ready when records are present.
    /// Keep the session alive throughout any wait using this borrowed handle.
    pub fn notification_handle(&self) -> BorrowedHandle<'_> {
        self.input.as_handle()
    }

    /// Consume exactly one record, and only after a nonblocking readiness check.
    /// The main-thread owner has no competing read between the check and read.
    /// None means not ready; paused input and native failures are errors, not EOF.
    pub fn read_ready_record(&self) -> io::Result<Option<ConsoleRecord>> {
        crate::threads::assert_is_main_thread();
        if self.paused.get() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "console input is paused",
            ));
        }
        match unsafe { WaitForSingleObject(self.input.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => return Ok(None),
            WAIT_FAILED => return Err(io::Error::last_os_error()),
            result => {
                return Err(io::Error::other(format!(
                    "unexpected console input wait result {result}"
                )));
            }
        }
        let mut native = InputRecord {
            event_type: 0,
            event: InputEvent { focus: 0 },
        };
        let mut count = 0;
        if unsafe { ReadConsoleInputW(self.input.as_raw_handle(), &mut native, 1, &mut count) } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if count != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("ReadConsoleInputW returned {count} records for a one-record read"),
            ));
        }
        native.translate().map(Some)
    }

    /// Intentionally discard kernel input for a cancellation epoch, never for
    /// donation to a child. Decoder clearing belongs to the caller's same epoch.
    pub fn discard_input(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        if unsafe { FlushConsoleInputBuffer(self.input.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(crate) fn native_decoder(&self) -> &RefCell<crate::input::NativeDecoder> {
        &self.decoder
    }

    /// A child cannot receive the remainder of a scalar already partly decoded
    /// by fish. Keep it intact so a nested reader can finish it instead.
    pub fn ensure_external_input_boundary(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        if self.decoder.borrow().incomplete_scalar() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "cannot transfer console input with an incomplete Unicode scalar",
            ));
        }
        Ok(())
    }

    pub fn editor_modes_active(&self) -> bool {
        self.modes.borrow().editor
    }

    pub fn input_paused(&self) -> bool {
        self.paused.get()
    }

    /// Query the retained output handle without generating input-side replies.
    /// Screen buffer coordinates include scrollback; the reader uses zero-based
    /// coordinates relative to the visible viewport, just like a DSR response.
    pub fn cursor_position(&self) -> io::Result<Option<crate::key::ViewportPosition>> {
        let mut info = ConsoleScreenBufferInfo::default();
        if unsafe { GetConsoleScreenBufferInfo(self.output.as_raw_handle(), &mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let columns = viewport_extent(info.window[0], info.window[2])?;
        let rows = viewport_extent(info.window[1], info.window[3])?;
        let x = i32::from(info.cursor[0]) - i32::from(info.window[0]);
        let y = i32::from(info.cursor[1]) - i32::from(info.window[1]);
        if x < 0 || x >= i32::from(columns) || y < 0 || y >= i32::from(rows) {
            // A user may scroll the classic console viewport away from the
            // live cursor. There is then no visible cursor coordinate; retain
            // the screen model's unknown-position path, not a fatal input error.
            return Ok(None);
        }
        Ok(Some(crate::key::ViewportPosition {
            x: x as usize,
            y: y as usize,
        }))
    }

    /// Read the handle's actual background attribute and configured palette,
    /// rather than assuming an ANSI palette or asking the VT peer via OSC 11.
    pub fn background_color(&self) -> io::Result<xterm_color::Color> {
        let mut info = ConsoleScreenBufferInfoEx {
            byte_size: std::mem::size_of::<ConsoleScreenBufferInfoEx>() as u32,
            ..Default::default()
        };
        if unsafe { GetConsoleScreenBufferInfoEx(self.output.as_raw_handle(), &mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // COMMON_LVB_REVERSE_VIDEO exchanges the visible foreground/background.
        let shift = if info.attributes & 0x4000 != 0 { 0 } else { 4 };
        let background = usize::from((info.attributes >> shift) & 0x0f);
        let color = info.color_table[background];
        // COLORREF is 0x00bbggrr; expand each channel to xterm-color's 16 bits.
        Ok(xterm_color::Color::rgb(
            (color & 0xff) as u16 * 0x101,
            ((color >> 8) & 0xff) as u16 * 0x101,
            ((color >> 16) & 0xff) as u16 * 0x101,
        ))
    }

    pub fn viewport_origin(&self) -> io::Result<(i16, i16)> {
        let mut info = ConsoleScreenBufferInfo::default();
        if unsafe { GetConsoleScreenBufferInfo(self.output.as_raw_handle(), &mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((info.window[0], info.window[1]))
    }

    /// Visible columns and rows from srWindow, not the backing buffer extent.
    pub fn viewport(&self) -> io::Result<(u16, u16)> {
        let mut info = ConsoleScreenBufferInfo::default();
        if unsafe { GetConsoleScreenBufferInfo(self.output.as_raw_handle(), &mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let columns = viewport_extent(info.window[0], info.window[2])?;
        let rows = viewport_extent(info.window[1], info.window[3])?;
        Ok((columns, rows))
    }
}

impl Drop for ConsoleSession {
    fn drop(&mut self) {
        crate::threads::assert_is_main_thread();
        self.paused.set(true);
        if let Err(error) = self.restore_modes_locked(&mut self.modes.borrow_mut()) {
            eprintln!("fish: restoring console modes: {error}");
        }
    }
}

fn console_mode(handle: BorrowedHandle<'_>) -> io::Result<u32> {
    let mut mode = 0;
    if unsafe { GetConsoleMode(handle.as_raw_handle(), &mut mode) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(mode)
}

fn set_console_mode(handle: BorrowedHandle<'_>, mode: u32) -> io::Result<()> {
    if unsafe { SetConsoleMode(handle.as_raw_handle(), mode) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn with_rollback_error(error: io::Error, rollback: io::Result<()>) -> io::Error {
    match rollback {
        Ok(()) => error,
        Err(rollback) => io::Error::new(
            error.kind(),
            format!("setting console editor modes: {error}; restoring original modes: {rollback}"),
        ),
    }
}

fn viewport_extent(start: i16, end: i16) -> io::Result<u16> {
    u16::try_from(i32::from(end) - i32::from(start) + 1)
        .ok()
        .filter(|extent| *extent != 0)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid console visible viewport dimensions",
            )
        })
}

struct SessionRouting {
    next_scope: u64,
    sessions: Vec<(u64, Rc<ConsoleSession>)>,
}

thread_local! {
    static SESSION_ROUTING: RefCell<SessionRouting> = const {
        RefCell::new(SessionRouting { next_scope: 0, sessions: Vec::new() })
    };
}

/// Main-thread routing only. Installing a session does not change modes or input
/// ownership; nested reader/tool callers perform the explicit lifecycle above.
pub fn current_session() -> Option<Rc<ConsoleSession>> {
    crate::threads::assert_is_main_thread();
    SESSION_ROUTING.with(|routing| {
        routing
            .borrow()
            .sessions
            .last()
            .map(|(_, session)| Rc::clone(session))
    })
}

/// Scoped routing is neither Send nor Sync, and keeps every outer session alive.
/// Removing an outer scope early cannot overwrite a still-installed inner one.
#[must_use = "retain the scope for as long as the console session should be current"]
pub struct SessionScope {
    id: u64,
    _main_thread: PhantomData<Rc<()>>,
}

pub fn install_session(session: Rc<ConsoleSession>) -> SessionScope {
    crate::threads::assert_is_main_thread();
    let id = SESSION_ROUTING.with(|routing| {
        let mut routing = routing.borrow_mut();
        routing.next_scope = routing
            .next_scope
            .checked_add(1)
            .expect("console scope IDs exhausted");
        let id = routing.next_scope;
        routing.sessions.push((id, session));
        id
    });
    SessionScope {
        id,
        _main_thread: PhantomData,
    }
}

impl Drop for SessionScope {
    fn drop(&mut self) {
        crate::threads::assert_is_main_thread();
        let removed = SESSION_ROUTING.with(|routing| {
            let mut routing = routing.borrow_mut();
            let index = routing
                .sessions
                .iter()
                .position(|(id, _)| *id == self.id)
                .expect("console session scope lost its registration");
            routing.sessions.remove(index)
        });
        // Restoring a final session must not run under the TLS borrow.
        drop(removed);
    }
}
