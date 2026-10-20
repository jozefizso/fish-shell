//! Owned console-record input and native console modes for the shared reader.
//!
//! A session starts paused, without changing either console mode. The caller owns
//! terminal protocols: pause input before disabling protocols/restoring modes,
//! and enter editor modes before enabling protocols/resuming input. Native reads
//! consume one ready record on the main thread, leaving typeahead in the console.

use std::cell::{Cell, RefCell};
use std::io;
use std::marker::PhantomData;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

const WAIT_TIMEOUT: u32 = 258;
const CONSOLE_READ_NOWAIT: u16 = 0x0002;
const EVALUATION_POLL_INTERVAL: Duration = Duration::from_millis(10);
const CANCEL_PEEK_RECORDS: usize = 4096;

const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
const ENABLE_LINE_INPUT: u32 = 0x0002;
const ENABLE_ECHO_INPUT: u32 = 0x0004;
const ENABLE_WINDOW_INPUT: u32 = 0x0008;
const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
const ENABLE_WRAP_AT_EOL_OUTPUT: u32 = 0x0002;
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
const DISABLE_NEWLINE_AUTO_RETURN: u32 = 0x0008;

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
        // The console input APIs initialize the union selected by EventType.
        // Every member is POD, including the wide KEY_EVENT character union.
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
struct ConsoleCoord {
    x: i16,
    y: i16,
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
    fn CreateConsoleScreenBuffer(
        access: u32,
        share: u32,
        attributes: *const std::ffi::c_void,
        flags: u32,
        reserved: *mut std::ffi::c_void,
    ) -> RawHandle;
    fn SetConsoleScreenBufferSize(handle: RawHandle, size: ConsoleCoord) -> i32;
    fn WriteConsoleW(
        handle: RawHandle,
        buffer: *const u16,
        count: u32,
        written: *mut u32,
        reserved: *mut std::ffi::c_void,
    ) -> i32;
    fn GetModuleHandleW(name: *const u16) -> RawHandle;
    fn GetProcAddress(module: RawHandle, name: *const u8) -> *mut std::ffi::c_void;
    fn PeekConsoleInputW(
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
    fn WaitForMultipleObjects(
        count: u32,
        handles: *const RawHandle,
        all: i32,
        milliseconds: u32,
    ) -> u32;
}

type ReadConsoleInputEx = unsafe extern "system" fn(
    RawHandle,
    *mut InputRecord,
    u32,
    *mut u32,
    u16,
) -> i32;

fn read_console_input_ex() -> io::Result<ReadConsoleInputEx> {
    static API: LazyLock<Result<ReadConsoleInputEx, i32>> = LazyLock::new(|| {
        // Resolve only from the already-loaded system module, never a DLL
        // search path. ReadConsoleInputExW is documented on Windows 7 and later,
        // but deliberately has no import-library/header dependency.
        let module = unsafe {
            GetModuleHandleW([107u16, 101, 114, 110, 101, 108, 51, 50, 0].as_ptr())
        };
        let address = if module.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { GetProcAddress(module, c"ReadConsoleInputExW".as_ptr().cast()) }
        };
        if address.is_null() {
            Err(io::Error::last_os_error().raw_os_error().unwrap_or(127))
        } else {
            // The documented system ABI above is the function's exact ABI.
            Ok(unsafe { std::mem::transmute::<*mut std::ffi::c_void, ReadConsoleInputEx>(address) })
        }
    });
    API.as_ref()
    .copied()
    .map_err(|code| io::Error::from_raw_os_error(*code))
}

/// Stream state belongs to the console session, not an individual reader. A
/// nested read can begin after the outer decoder consumed half a paste envelope.
/// A partial end remains pasted; a partial begin has not entered paste yet.
#[derive(Clone, Copy, Default)]
pub(crate) struct PasteFraming {
    pasted: bool,
    matched: usize,
    uncertain: bool,
}

impl PasteFraming {
    pub(crate) fn from_paste_buffer(paste: Option<&[u8]>) -> Self {
        let Some(paste) = paste else {
            return Self::default();
        };
        // Active paste retains partial closing bytes in its real buffer. Only
        // the exact delimiter suffix matters; do not rescan the pasted body.
        let marker = crate::input::PASTE_END;
        let matched = (1..marker.len()).rev()
            .find(|&length| paste.ends_with(&marker[..length]))
            .unwrap_or(0);
        Self { pasted: true, matched, uncertain: false }
    }

    pub(crate) fn pasted(self) -> bool {
        self.pasted
    }

    pub(crate) fn requires_consumer(self) -> bool {
        self.uncertain
    }

    pub(crate) fn observe(&mut self, record: ConsoleRecord) {
        if self.uncertain {
            return;
        }
        let ConsoleRecord::Key { down, repeat, virtual_key, utf16, control_state, .. } = record else {
            return;
        };
        let committed_alt_up = !down && virtual_key == 0x12 && utf16 != 0;
        if !down && !committed_alt_up {
            return;
        }
        if utf16 == 0 && virtual_key != 0 {
            // Ignored dead/modifier records cannot split a VT envelope. NUL
            // aliases and physical special keys do interrupt its byte stream.
            if (control_state & 0x0c != 0
                && control_state & 0x03 == 0
                && matches!(virtual_key, 0x20 | 0x32))
                || matches!(virtual_key,
                    0x08 | 0x09 | 0x0d | 0x1b | 0x20 | 0x21..=0x28 |
                    0x2c..=0x2e | 0x5d | 0x70..=0x7b)
            {
                self.matched = 0;
            }
            return;
        }
        let altgr_text = control_state & 0x0c != 0
            && control_state & 0x03 != 0
            && utf16 >= 32;
        if !self.pasted
            && virtual_key != 0
            && !committed_alt_up
            && !altgr_text
            && (utf16 == 0x1b || control_state & 0x0f != 0)
        {
            // NativeDecoder returns a physical key event, not an ESC stream
            // byte. A literal Escape key followed by '[200~' is not a paste.
            self.matched = 0;
            return;
        }
        if !self.pasted && utf16 == 0x1b && (self.matched != 0 || repeat > 1) {
            // The real parser permits one recursive Escape, then consumes the
            // third as legacy input. Do not reinterpret an arbitrary/repeated
            // Escape run as a new paste begin. Let the actual consumer decide.
            self.uncertain = true;
            self.matched = 0;
            return;
        }
        // Identical input units cannot complete more than one six-byte marker.
        for _ in 0..repeat.min(6) {
            let envelope = if self.pasted {
                crate::input::PASTE_END
            } else {
                crate::input::PASTE_BEGIN
            };
            if u16::from(envelope[self.matched]) == utf16 {
                self.matched += 1;
                if self.matched == envelope.len() {
                    self.pasted = !self.pasted;
                    self.matched = 0;
                }
            } else {
                if !self.pasted && self.matched != 0 {
                    // General CSI/SS3/OSC/DCS semantics belong to the actual
                    // decoder, never this observational paste recognizer.
                    self.uncertain = true;
                    self.matched = 0;
                    return;
                }
                self.matched = usize::from(utf16 == 0x1b);
            }
        }
    }

    fn typed_cancel(self, record: ConsoleRecord) -> bool {
        !self.uncertain
            && !self.pasted
            && matches!(record, ConsoleRecord::Key {
                down: true, repeat: 1.., virtual_key, utf16, control_state, ..
            } if control_state & 0x03 == 0
                && ((virtual_key == 0 && utf16 == 3)
                    || (control_state & 0x0c != 0
                        && (virtual_key == 0x43 || utf16 == 3))))
    }
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
    framing: Cell<PasteFraming>,
    // Retain scratch storage outside recursive evaluator stacks; polling does
    // not allocate or zero an 80 KiB input-record array on every cancel check.
    cancel_peek: RefCell<[InputRecord; CANCEL_PEEK_RECORDS]>,
    evaluation_depth: Cell<usize>,
    consumer_depth: Cell<usize>,
    evaluation_consumer_depth: Cell<usize>,
    evaluation_cancelled: Cell<bool>,
    evaluation_input_reset: Cell<bool>,
    next_evaluation_poll: Cell<Option<Instant>>,
}

/// Distinguish the documented legacy-console and wcwidth-compatible policies
/// on an inactive buffer. Never paint a probe or consume a reply on the user's
/// screen/input; this selects a host policy, not arbitrary glyph/font widths.
fn legacy_console_widths(output_mode: u32) -> io::Result<bool> {
    let raw = unsafe {
        CreateConsoleScreenBuffer(
            0xc000_0000, 3, std::ptr::null(), 1, std::ptr::null_mut(),
        )
    };
    if raw.is_null() || raw as usize == usize::MAX {
        return Err(io::Error::last_os_error());
    }
    let buffer = unsafe { OwnedHandle::from_raw_handle(raw) };
    set_console_mode(buffer.as_handle(), output_mode)?;
    let mut info = ConsoleScreenBufferInfo::default();
    if unsafe { GetConsoleScreenBufferInfo(raw, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.size[0] < 4
        && unsafe { SetConsoleScreenBufferSize(raw, ConsoleCoord { x: 4, y: info.size[1] }) } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let initial_row = info.cursor[1];
    let text = [b'\r' as u16, b'e' as u16, 0x0301];
    let mut written = 0;
    if unsafe { WriteConsoleW(raw, text.as_ptr(), 3, &mut written, std::ptr::null_mut()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if written != 3 {
        return Err(io::Error::new(io::ErrorKind::WriteZero, "incomplete console width-policy probe"));
    }
    if unsafe { GetConsoleScreenBufferInfo(raw, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    match (info.cursor[0], info.cursor[1] == initial_row) {
        (1, true) => Ok(false),
        (2, true) => Ok(true),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "unknown console text-measurement policy")),
    }
}

impl ConsoleSession {
    /// Duplicate both handles without inheritance and validate each console mode
    /// independently. No modes or terminal protocols are changed.
    pub fn new(input: BorrowedHandle<'_>, output: BorrowedHandle<'_>) -> io::Result<Rc<Self>> {
        crate::threads::assert_is_main_thread();
        let input = input.try_clone_to_owned()?;
        let output = output.try_clone_to_owned()?;
        read_console_input_ex()?;
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
            framing: Cell::default(),
            cancel_peek: RefCell::new([
                InputRecord { event_type: 0, event: InputEvent { focus: 0 } };
                CANCEL_PEEK_RECORDS
            ]),
            evaluation_depth: Cell::new(0),
            consumer_depth: Cell::new(0),
            evaluation_consumer_depth: Cell::new(0),
            evaluation_cancelled: Cell::new(false),
            evaluation_input_reset: Cell::new(false),
            next_evaluation_poll: Cell::new(None),
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
        // The screen model keeps a pending right-margin wrap until the next
        // printable cell. Immediate Win32 wrapping misplaces combining marks
        // and makes the following explicit movement advance an extra row.
        let output_mode = modes.original_output
            | ENABLE_PROCESSED_OUTPUT
            | ENABLE_WRAP_AT_EOL_OUTPUT
            | ENABLE_VIRTUAL_TERMINAL_PROCESSING
            | DISABLE_NEWLINE_AUTO_RETURN;
        modes.input_owned = true;
        let result = set_console_mode(self.input.as_handle(), input_mode).and_then(|()| {
            modes.output_owned = true;
            set_console_mode(self.output.as_handle(), output_mode)?;
            legacy_console_widths(output_mode)
        });
        let legacy_widths = match result {
            Ok(value) => value,
            Err(error) => {
                // Try every touched handle even if the failing call itself may not
                // have changed a mode. Retain ownership so Drop retries restoration.
                return Err(with_rollback_error(
                    error,
                    self.restore_modes_locked(&mut modes),
                ));
            }
        };
        if fish_fallback::FISH_CONSOLE_LEGACY_WIDTHS.swap(
            legacy_widths, std::sync::atomic::Ordering::Relaxed,
        ) != legacy_widths {
            crate::screen::invalidate_native_screen();
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

    /// Consume at most one record without ever blocking. A concurrent native
    /// control can flush a previously signaled buffer: NOWAIT is required, not
    /// a readiness check followed by ReadConsoleInputW. Zero records is not EOF.
    pub fn read_ready_record(&self) -> io::Result<Option<ConsoleRecord>> {
        crate::threads::assert_is_main_thread();
        if self.paused.get() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "console input is paused",
            ));
        }
        let mut native = InputRecord {
            event_type: 0,
            event: InputEvent { focus: 0 },
        };
        let mut count = 0;
        if unsafe {
            read_console_input_ex()?(
                self.input.as_raw_handle(), &mut native, 1, &mut count, CONSOLE_READ_NOWAIT,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if count == 0 {
            return Ok(None);
        }
        if count != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("ReadConsoleInputExW returned {count} records for a one-record read"),
            ));
        }
        let record = native.translate()?;
        let framing = if crate::tty_handoff::get_tty_protocols_active() {
            let mut framing = self.framing.get();
            framing.observe(record);
            framing
        } else {
            // A raw `read --null` deliberately disables envelopes. Literal
            // bytes consumed there cannot start a future editor paste epoch.
            PasteFraming::default()
        };
        self.framing.set(framing);
        Ok(Some(record))
    }

    /// Intentionally discard kernel input for a cancellation epoch, never for
    /// donation to a child. Decoder clearing belongs to the caller's same epoch.
    pub fn discard_input(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        if unsafe { FlushConsoleInputBuffer(self.input.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        self.framing.set(PasteFraming::default());
        Ok(())
    }

    pub(crate) fn native_decoder(&self) -> &RefCell<crate::input::NativeDecoder> {
        &self.decoder
    }

    /// Called only after the real shared decoder returns a complete event.
    /// Its paste state is authoritative. Reapply already-read native repeats
    /// which still belong to the next event, without consuming any input.
    pub(crate) fn synchronize_paste_framing(&self, paste: Option<&[u8]>) {
        let framing = PasteFraming::from_paste_buffer(paste);
        self.framing.set(self.decoder.borrow().pending_paste_framing(framing));
    }

    /// A child cannot receive the remainder of a scalar already partly decoded
    /// by fish. Keep it intact so a nested reader can finish it instead.
    pub fn ensure_external_input_boundary(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        let framing = self.framing.get();
        if self.decoder.borrow().incomplete_scalar()
            || framing.pasted
            || framing.matched != 0
            || framing.requires_consumer()
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "cannot transfer console input with an incomplete scalar or terminal frame",
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
    fn evaluation_polling_active(&self) -> bool {
        self.evaluation_depth.get() != 0
            && self.consumer_depth.get() == self.evaluation_consumer_depth.get()
            && self.modes.borrow().editor
            && !crate::tty_handoff::native_input_is_donated()
    }

    /// Normal input is never dequeued or replayed by evaluator polling. A full
    /// bounded prefix aborts only the callback, returning to the real consumer
    /// with all kernel/decoder/logical paste state unchanged. This also makes a
    /// typed Ctrl-C beyond the peek bound reachable without an unbounded scan.
    fn poll_evaluation_input(&self) -> io::Result<bool> {
        if self.evaluation_cancelled.get() {
            return Ok(true);
        }
        if !self.evaluation_polling_active() {
            return Ok(false);
        }
        let now = Instant::now();
        if self.next_evaluation_poll.get().is_some_and(|next| now < next) {
            return Ok(false);
        }
        self.next_evaluation_poll.set(Some(now + EVALUATION_POLL_INTERVAL));
        let before = crate::signal::console_control_epoch();
        if !crate::signal::pending_console_controls().is_empty() {
            return Ok(false);
        }
        let mut records = self.cancel_peek.borrow_mut();
        let mut count = 0;
        if unsafe {
            PeekConsoleInputW(
                self.input.as_raw_handle(), records.as_mut_ptr(),
                records.len() as u32, &mut count,
            )
        } == 0 {
            return Err(io::Error::last_os_error());
        }
        if count as usize > records.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "oversized console input peek"));
        }
        let mut framing = self.framing.get();
        let mut cancel_at = None;
        for (index, native) in records[..count as usize].iter().enumerate() {
            let record = native.translate()?;
            if framing.requires_consumer() && PasteFraming::default().typed_cancel(record) {
                // Ambiguous terminal framing belongs to the real input decoder.
                // Yield evaluation only when cancellation is actually queued;
                // an ordinary function key must not abort completion or a prompt.
                self.evaluation_cancelled.set(true);
                self.evaluation_input_reset.set(false);
                return Ok(true);
            }
            if framing.typed_cancel(record) {
                cancel_at = Some(index);
                break;
            }
            framing.observe(record);
        }
        let Some(cancel_at) = cancel_at else {
            if count as usize == records.len() {
                self.evaluation_cancelled.set(true);
                // This is backlog pressure, not a typed key or native receipt.
                // Resetting input here would expose the tail of a partial paste.
                self.evaluation_input_reset.set(false);
                return Ok(true);
            }
            return Ok(false);
        };
        // Only this main-thread owner can remove records. Appending user input
        // cannot change the prefix. A native control can flush it, so never use
        // a blocking read, and leave receipt acknowledgement to its real owner.
        if crate::signal::console_control_epoch() != before {
            return Ok(false);
        }
        let requested = (cancel_at + 1) as u32;
        count = 0;
        if unsafe {
            read_console_input_ex()?(
                self.input.as_raw_handle(), records.as_mut_ptr(), requested,
                &mut count, CONSOLE_READ_NOWAIT,
            )
        } == 0 {
            return Err(io::Error::last_os_error());
        }
        if count == 0 {
            return Ok(false);
        }
        if count > requested {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "oversized console cancellation read"));
        }
        let mut actual_framing = self.framing.get();
        let mut actual_cancel_at = None;
        for (index, native) in records[..count as usize].iter().enumerate() {
            let record = native.translate()?;
            if record == ConsoleRecord::Resize {
                crate::termsize::signal_safe_termsize_invalidate_tty();
                crate::screen::invalidate_native_screen();
            }
            if actual_framing.typed_cancel(record) {
                actual_cancel_at = Some(index);
            }
            actual_framing.observe(record);
        }
        self.framing.set(actual_framing);
        if crate::signal::console_control_epoch() != before {
            // Native control cancellation owns this whole interrupted epoch.
            // Its counted receipt and decoder cleanup remain Reader's duty.
            return Ok(false);
        }
        if count != requested || actual_cancel_at != Some(cancel_at) {
            // An external input-buffer mutation is not replayable: replay
            // appends after newer input. Fail closed rather than admitting a
            // possibly truncated paste as commands.
            return Err(io::Error::new(
                io::ErrorKind::InvalidData, "console input changed during cancellation discard",
            ));
        }
        self.framing.set(PasteFraming::default());
        *self.decoder.borrow_mut() = crate::input::NativeDecoder::default();
        self.evaluation_cancelled.set(true);
        self.evaluation_input_reset.set(true);
        TYPED_CANCEL_EPOCH.with(|epoch| epoch.set(epoch.get().wrapping_add(1)));
        Ok(true)
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
    static TYPED_CANCEL_EPOCH: Cell<u64> = const { Cell::new(0) };
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
/// A local evaluator-input epoch. This does not enable processed input, disable
/// paste envelopes, signal a native event, or invent a CTRL_C_EVENT receipt.
/// Keep the outer guard alive until its Reader has reconciled logical input.
#[must_use]
pub struct EvaluationInputGuard {
    session: Option<Rc<ConsoleSession>>,
    previous_consumer_depth: usize,
}

pub fn begin_evaluation_input() -> EvaluationInputGuard {
    crate::threads::assert_is_main_thread();
    let session = current_session();
    let mut previous_consumer_depth = 0;
    if let Some(session) = &session {
        if session.evaluation_depth.get() == 0 {
            session.evaluation_cancelled.set(false);
            session.evaluation_input_reset.set(false);
            session.next_evaluation_poll.set(None);
        }
        session.evaluation_depth.set(
            session.evaluation_depth.get().checked_add(1).expect("evaluation input scope overflow"),
        );
        previous_consumer_depth = session.evaluation_consumer_depth.replace(session.consumer_depth.get());
    }
    EvaluationInputGuard { session, previous_consumer_depth }
}

impl EvaluationInputGuard {
    pub fn cancelled(&self) -> bool {
        self.session.as_ref().is_some_and(|session| session.evaluation_cancelled.get())
    }

    /// Take the single reset owed by an actually removed typed-C prefix.
    /// Nested evaluator guards must not clear newer logical input twice.
    /// Backlog cancellation preserves partially read paste without a reset.
    pub fn take_input_reset(&self) -> bool {
        self.session.as_ref().is_some_and(|session| session.evaluation_input_reset.replace(false))
    }
}

impl Drop for EvaluationInputGuard {
    fn drop(&mut self) {
        if let Some(session) = &self.session {
            session.evaluation_consumer_depth.set(self.previous_consumer_depth);
            let depth = session.evaluation_depth.get().checked_sub(1).expect("evaluation input scope lost");
            session.evaluation_depth.set(depth);
            if depth == 0 {
                session.evaluation_cancelled.set(false);
                session.evaluation_input_reset.set(false);
                session.next_evaluation_poll.set(None);
            }
        }
    }
}

/// Actual nested input consumers retain the kernel FIFO, including ETX bytes.
/// An explicit nested evaluator guard (for an event callback) can still poll.
#[must_use]
pub(crate) struct EvaluationInputSuspension(Option<Rc<ConsoleSession>>);

pub(crate) fn suspend_evaluation_input() -> EvaluationInputSuspension {
    let session = crate::threads::is_registered_main_thread().then(current_session).flatten();
    if let Some(session) = &session {
        session.consumer_depth.set(
            session.consumer_depth.get().checked_add(1).expect("console consumer scope overflow"),
        );
    }
    EvaluationInputSuspension(session)
}

impl Drop for EvaluationInputSuspension {
    fn drop(&mut self) {
        if let Some(session) = &self.0 {
            session.consumer_depth.set(session.consumer_depth.get().checked_sub(1).expect("console consumer scope lost"));
        }
    }
}

pub(crate) fn evaluation_input_cancelled() -> bool {
    crate::threads::is_registered_main_thread()
        && current_session().is_some_and(|session| session.evaluation_cancelled.get())
}

pub(crate) fn typed_cancel_epoch() -> u64 {
    TYPED_CANCEL_EPOCH.with(Cell::get)
}

pub(crate) fn poll_evaluation_input() -> bool {
    if !crate::threads::is_registered_main_thread() {
        return false;
    }
    let Some(session) = current_session() else {
        return false;
    };
    match session.poll_evaluation_input() {
        Ok(cancelled) => cancelled,
        Err(error) => {
            // Transport errors terminate input rather than exposing a damaged
            // paste. This is a local unwind, never a native control receipt.
            session.evaluation_cancelled.set(true);
            crate::reader::native_input_failed(&error);
            true
        }
    }
}

/// Private bounded evaluator waits deliberately exclude the console handle:
/// ordinary typeahead keeps that handle signaled forever. Service real handles
/// normally, and slice only while this main thread owns an evaluator guard.
pub(crate) fn wait_for_evaluator_handles(handles: &[RawHandle], milliseconds: u32) -> io::Result<u32> {
    let start = Instant::now();
    loop {
        let polling = crate::threads::is_registered_main_thread()
            && current_session().is_some_and(|session| session.evaluation_polling_active());
        if polling {
            // Retained completion wins when cancellation and completion become
            // visible together, matching the native wait owners' prior policy.
            let ready = unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, 0) };
            if ready == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            if ready != WAIT_TIMEOUT {
                return Ok(ready);
            }
        }
        if poll_evaluation_input() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "typed evaluator cancellation"));
        }
        let remaining = if milliseconds == u32::MAX {
            u32::MAX
        } else {
            milliseconds.saturating_sub(start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32)
        };
        let interval = if polling {
            remaining.min(EVALUATION_POLL_INTERVAL.as_millis() as u32)
        } else {
            remaining
        };
        let result = unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, interval) };
        if result == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        if result != WAIT_TIMEOUT || interval == remaining {
            return Ok(result);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn text(unit: u16) -> ConsoleRecord {
        ConsoleRecord::Key {
            down: true, repeat: 1, virtual_key: 0, scan_code: 0,
            utf16: unit, control_state: 0,
        }
    }

    fn ctrl_c() -> ConsoleRecord {
        ConsoleRecord::Key {
            down: true, repeat: 1, virtual_key: 0x43, scan_code: 0x2e,
            utf16: 3, control_state: 8,
        }
    }

    #[test]
    fn evaluator_cancel_accepts_conpty_etx_without_native_receipt() {
        let framing = PasteFraming::default();
        assert!(framing.typed_cancel(ctrl_c()));
        assert!(framing.typed_cancel(text(3)));
        let mut released = ctrl_c();
        if let ConsoleRecord::Key { down, .. } = &mut released {
            *down = false;
        }
        assert!(!framing.typed_cancel(released));
    }

    #[test]
    fn evaluator_framing_distinguishes_physical_escape_and_modifier_records() {
        let mut framing = PasteFraming::default();
        let escape = ConsoleRecord::Key {
            down: true, repeat: 1, virtual_key: 0x1b, scan_code: 1,
            utf16: 0x1b, control_state: 0,
        };
        framing.observe(escape);
        for unit in "[200~".encode_utf16() {
            framing.observe(text(unit));
        }
        assert!(framing.typed_cancel(ctrl_c()), "physical Escape must not invent a paste");
        for unit in "\x1b[20".encode_utf16() {
            framing.observe(text(unit));
        }
        framing.observe(ConsoleRecord::Key {
            down: true, repeat: 1, virtual_key: 0x10, scan_code: 0,
            utf16: 0, control_state: 0x10,
        });
        for unit in "0~".encode_utf16() {
            framing.observe(text(unit));
        }
        assert!(!framing.typed_cancel(ctrl_c()), "modifier-only records cannot erase an envelope");
    }

    #[test]
    fn evaluator_cancel_preserves_fragmented_paste_envelopes() {
        let mut framing = PasteFraming::default();
        for unit in "\x1b[20".encode_utf16() {
            framing.observe(text(unit));
        }
        assert!(framing.typed_cancel(ctrl_c()), "physical Ctrl-C interrupts an unfinished begin");
        let mut peek = framing;
        for unit in "0~printf unsafe\n".encode_utf16() {
            peek.observe(text(unit));
        }
        assert!(!peek.typed_cancel(ctrl_c()), "pasted physical ETX is data");
        peek.observe(ctrl_c());
        for unit in "\x1b[201".encode_utf16() {
            peek.observe(text(unit));
        }
        assert!(!peek.typed_cancel(ctrl_c()), "partial end is still pasted data");
        peek.observe(text(b'~' as u16));
        assert!(peek.typed_cancel(ctrl_c()));
        assert_eq!(framing.matched, 4, "a nonconsuming peek cannot mutate stream state");
    }

    #[test]
    fn evaluator_paste_framing_ignores_key_up() {
        let mut framing = PasteFraming::default();
        framing.observe(text(0x1b));
        for unit in "[200~".encode_utf16() {
            framing.observe(text(unit));
        }
        assert!(framing.pasted);
        let mut release = text(b'x' as u16);
        if let ConsoleRecord::Key { down, .. } = &mut release {
            *down = false;
        }
        for unit in "\x1b[201".encode_utf16() {
            framing.observe(text(unit));
        }
        framing.observe(release);
        framing.observe(text(b'~' as u16));
        assert!(!framing.pasted);
    }
}
