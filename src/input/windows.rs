//! Main-thread console records feed the shared VT decoder and logical-key queue.
//! Nested readers share the session's scalar/repeat decoder; unread records stay
//! in the console input buffer so native children can receive ordinary typeahead.
use super::input::{CharEvent, ImplicitEvent, InputEventTrigger, KeyEvent};
use crate::console_windows::{ConsoleRecord, ConsoleSession, current_session};
use crate::fd_readable_set::Timeout;
use crate::key::{self, Key, Modifiers, ViewportPosition};
use std::io;
use std::os::windows::io::{AsRawHandle, BorrowedHandle, RawHandle};
use std::rc::Rc;
use std::time::Instant;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitForMultipleObjects(
        count: u32,
        handles: *const RawHandle,
        all: i32,
        milliseconds: u32,
    ) -> u32;
}

#[derive(Default)]
pub(crate) struct WaitSources<'a> {
    pub workers: Option<BorrowedHandle<'a>>,
    pub state: Option<BorrowedHandle<'a>>,
    pub jobs: Option<BorrowedHandle<'a>>,
}

pub(crate) struct NativeInput {
    session: Option<Rc<ConsoleSession>>,
}

/// Only already-consumed keyboard state; never owns a console session or reader.
#[derive(Default)]
pub(crate) struct NativeDecoder {
    high_surrogate: Option<(u16, u16)>,
    bytes: [u8; 4],
    byte_count: usize,
    byte_position: usize,
    repeats: u16,
    repeated_key: Option<(KeyEvent, u16)>,
}

impl Default for NativeInput {
    fn default() -> Self {
        Self {
            session: crate::threads::is_registered_main_thread()
                .then(current_session)
                .flatten(),
        }
    }
}

impl NativeDecoder {
    /// A child cannot inherit half a native scalar or half its UTF-8 encoding.
    pub(crate) fn incomplete_scalar(&self) -> bool {
        self.high_surrogate.is_some() || self.byte_position != 0
    }

    fn buffered_byte(&mut self) -> Option<u8> {
        if self.repeats == 0 {
            return None;
        }
        let byte = self.bytes[self.byte_position];
        self.byte_position += 1;
        if self.byte_position == self.byte_count {
            self.byte_position = 0;
            self.repeats -= 1;
        }
        Some(byte)
    }

    fn text(&mut self, unit: u16, repeat: u16) -> io::Result<()> {
        if repeat == 0 {
            return Err(invalid("zero console key repeat count"));
        }
        if (0xd800..=0xdbff).contains(&unit) {
            if self.high_surrogate.replace((unit, repeat)).is_some() {
                return Err(invalid("unpaired high surrogate in console input"));
            }
            return Ok(());
        }
        let scalar = if (0xdc00..=0xdfff).contains(&unit) {
            let Some((high, high_repeat)) = self.high_surrogate.take() else {
                return Err(invalid("unpaired low surrogate in console input"));
            };
            if high_repeat != repeat {
                return Err(invalid("mismatched console surrogate repeat counts"));
            }
            0x10000 + ((u32::from(high) - 0xd800) << 10) + u32::from(unit) - 0xdc00
        } else {
            if self.high_surrogate.take().is_some() {
                return Err(invalid("unpaired high surrogate before console text"));
            }
            u32::from(unit)
        };
        let character =
            char::from_u32(scalar).ok_or_else(|| invalid("invalid console Unicode scalar"))?;
        self.byte_count = character.encode_utf8(&mut self.bytes).len();
        self.byte_position = 0;
        self.repeats = repeat;
        Ok(())
    }

    fn record(&mut self, record: ConsoleRecord) -> io::Result<Option<CharEvent>> {
        match record {
            ConsoleRecord::Key {
                down,
                repeat,
                virtual_key,
                utf16,
                control_state,
                ..
            } => {
                // ConPTY commits Alt+numpad Unicode, including surrogate halves,
                // on Alt key-up. Ordinary key-up text is only an echo and is ignored.
                let committed_alt_up = !down && virtual_key == 0x12 && utf16 != 0;
                if !down && !committed_alt_up {
                    return Ok(None);
                }
                if repeat == 0 {
                    return Err(invalid("zero console key repeat count"));
                }
                let modifiers = Modifiers {
                    ctrl: control_state & 0x0c != 0,
                    alt: control_state & 0x03 != 0,
                    shift: control_state & 0x10 != 0,
                    sup: false,
                };
                // ConPTY represents committed NUL as Ctrl-Shift-2 with
                // UnicodeChar zero, even for Ctrl-Space or a literal NUL byte.
                // Preserve the standard legacy NUL aliases as text; zero on
                // an unrelated dead/modifier key is not committed text.
                if utf16 == 0
                    && modifiers.ctrl
                    && !modifiers.alt
                    && matches!(virtual_key, 0x20 | 0x32)
                {
                    self.text(0, repeat)?;
                    return Ok(None);
                }
                if virtual_key == 0
                    || committed_alt_up
                    || (modifiers.ctrl && modifiers.alt && utf16 >= 32)
                {
                    self.text(utf16, repeat)?;
                    return Ok(None);
                }
                if matches!(virtual_key, 0x10 | 0x11 | 0x12 | 0x14 | 0x90 | 0x91) && utf16 == 0 {
                    return Ok(None);
                }
                let special = match virtual_key {
                    0x08 => Some(key::BACKSPACE),
                    0x09 => Some(key::TAB),
                    0x0d => Some(key::ENTER),
                    0x1b => Some(key::ESCAPE),
                    0x20 => Some(key::SPACE),
                    0x21 => Some(key::PAGE_UP),
                    0x22 => Some(key::PAGE_DOWN),
                    0x23 => Some(key::END),
                    0x24 => Some(key::HOME),
                    0x25 => Some(key::LEFT),
                    0x26 => Some(key::UP),
                    0x27 => Some(key::RIGHT),
                    0x28 => Some(key::DOWN),
                    0x2c => Some(key::PRINT_SCREEN),
                    0x2d => Some(key::INSERT),
                    0x2e => Some(key::DELETE),
                    0x5d => Some(key::MENU),
                    0x70..=0x7b => Some(key::function_key((virtual_key - 0x70 + 1) as u8)),
                    _ => None,
                };
                let key = if let Some(special) = special {
                    Some(Key::new(modifiers, special))
                } else if (modifiers.ctrl || modifiers.alt) && (0x30..=0x5a).contains(&virtual_key)
                {
                    Some(Key::new(
                        modifiers,
                        char::from_u32(u32::from(virtual_key))
                            .unwrap()
                            .to_ascii_lowercase(),
                    ))
                } else if (modifiers.ctrl || modifiers.alt)
                    && utf16 >= 32
                    && !(0xd800..=0xdfff).contains(&utf16)
                {
                    // OEM/layout text has no portable physical base identity.
                    // Preserve its committed character and command modifiers.
                    let mut command_modifiers = modifiers;
                    command_modifiers.shift = false;
                    Some(Key::new(
                        command_modifiers,
                        char::from_u32(u32::from(utf16)).unwrap(),
                    ))
                } else {
                    None
                };
                if let Some(key) = key {
                    if self.high_surrogate.take().is_some() {
                        return Err(invalid("incomplete console text before a physical key"));
                    }
                    let shifted = (key.modifiers.shift && utf16 >= 32)
                        .then(|| char::from_u32(u32::from(utf16)))
                        .flatten();
                    let mut event = KeyEvent::new_with(
                        key.modifiers,
                        true,
                        key.codepoint,
                        shifted,
                        None,
                        Default::default(),
                    );
                    event.native_text = if utf16 != 0 {
                        char::from_u32(u32::from(utf16))
                    } else {
                        None
                    };
                    if repeat > 1 {
                        self.repeated_key = Some((event.clone(), repeat - 1));
                    }
                    return Ok(Some(CharEvent::from_key(event)));
                }
                if utf16 != 0 {
                    self.text(utf16, repeat)?;
                }
                Ok(None)
            }
            _ => Err(invalid(
                "non-keyboard record passed to native scalar decoder",
            )),
        }
    }
}

impl NativeInput {
    /// Native control receipt deliberately cancels this input epoch, including
    /// queued kernel input and any already-decoded scalar or physical repeats.
    pub(crate) fn interrupt(&mut self) -> io::Result<()> {
        if let Some(session) = &self.session {
            session.pause_input()?;
            let result = session.discard_input();
            *session.native_decoder().borrow_mut() = NativeDecoder::default();
            result?;
        }
        Ok(())
    }

    pub(crate) fn resume(&self) -> io::Result<()> {
        self.session
            .as_ref()
            .ok_or_else(|| invalid("no native console input session"))?
            .resume_input()
    }

    pub(crate) fn next(&mut self, timeout: Timeout, sources: WaitSources<'_>) -> InputEventTrigger {
        match self.next_checked(timeout, sources) {
            Ok(event) => event,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                InputEventTrigger::Interrupted
            }
            Err(error) => {
                // A malformed consumed record ends this tool's input epoch,
                // not the surrounding reader. Leave future kernel input intact.
                if let Some(session) = &self.session {
                    *session.native_decoder().borrow_mut() = NativeDecoder::default();
                }
                InputEventTrigger::Error(error)
            }
        }
    }

    fn next_checked(
        &mut self,
        timeout: Timeout,
        sources: WaitSources<'_>,
    ) -> io::Result<InputEventTrigger> {
        let session = self.session.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "interactive input has no native console session",
            )
        })?;
        // No parser or notification callbacks execute while this shared borrow
        // is held. Returning the trigger releases it before a nested reader runs.
        let mut decoder = session.native_decoder().borrow_mut();
        let deadline = match timeout {
            Timeout::Forever => None,
            Timeout::Duration(duration) => Some(
                Instant::now()
                    .checked_add(duration)
                    .ok_or_else(|| invalid("console input timeout overflow"))?,
            ),
        };
        let cancellation = crate::signal::console_control_cancellation();
        let candidates = [
            cancellation.as_ref().map(|token| token.handle()),
            sources.state,
            sources.jobs,
            sources.workers,
        ];
        let mut handles = [std::ptr::null_mut(); 5];
        let mut kinds = [0u8; 5];
        let mut service_count = 0;
        for (kind, handle) in candidates.into_iter().enumerate() {
            if let Some(handle) = handle {
                handles[service_count] = handle.as_raw_handle();
                kinds[service_count] = kind as u8;
                service_count += 1;
            }
        }
        handles[service_count] = session.notification_handle().as_raw_handle();
        kinds[service_count] = 4;
        let mut consumed_record = false;
        loop {
            // Once the first byte was returned, finish that scalar before any
            // notification. At the next scalar boundary services get priority,
            // even when a record repeats a character or physical key 65535 times.
            if decoder.byte_position != 0 {
                return Ok(InputEventTrigger::Byte(decoder.buffered_byte().unwrap()));
            }
            if service_count != 0 {
                let ready =
                    unsafe { WaitForMultipleObjects(service_count as u32, handles.as_ptr(), 0, 0) };
                if ready < service_count as u32 {
                    return Ok(notification(kinds[ready as usize]));
                }
                if ready != 258 {
                    return Err(wait_error(ready));
                }
            }
            if let Some(byte) = decoder.buffered_byte() {
                return Ok(InputEventTrigger::Byte(byte));
            }
            if let Some((event, remaining)) = decoder.repeated_key.as_mut() {
                let event = event.clone();
                *remaining -= 1;
                if *remaining == 0 {
                    decoder.repeated_key = None;
                }
                return Ok(InputEventTrigger::Event(CharEvent::from_key(event)));
            }
            if consumed_record && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return Ok(InputEventTrigger::TimeoutElapsed);
            }
            // Pausing must never turn a ready kernel handle into a busy loop or
            // admit a read while the console is leased to a native child.
            if session.input_paused() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "native console input is paused",
                ));
            }
            let milliseconds = if let Some(deadline) = deadline {
                let remaining = deadline.saturating_duration_since(Instant::now());
                remaining
                    .as_millis()
                    .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
                    .min(u128::from(u32::MAX - 1)) as u32
            } else {
                u32::MAX
            };
            let count = service_count + 1;
            let ready =
                unsafe { WaitForMultipleObjects(count as u32, handles.as_ptr(), 0, milliseconds) };
            if ready < service_count as u32 {
                return Ok(notification(kinds[ready as usize]));
            }
            if ready == service_count as u32 {
                if let Some(record) = session.read_ready_record()? {
                    consumed_record = true;
                    if let Some(event) = Self::record(session, &mut decoder, record)? {
                        return Ok(InputEventTrigger::Event(event));
                    }
                }
            } else if ready == 258 {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    return Ok(InputEventTrigger::TimeoutElapsed);
                }
            } else {
                return Err(wait_error(ready));
            }
        }
    }

    fn record(
        session: &ConsoleSession,
        decoder: &mut NativeDecoder,
        record: ConsoleRecord,
    ) -> io::Result<Option<CharEvent>> {
        match record {
            ConsoleRecord::Resize => {
                crate::termsize::signal_safe_termsize_invalidate_tty();
                crate::screen::invalidate_native_screen();
                Ok(Some(CharEvent::Implicit(ImplicitEvent::NewWindowHeight)))
            }
            ConsoleRecord::Focus(focused) => Ok(Some(CharEvent::Implicit(if focused {
                ImplicitEvent::FocusIn
            } else {
                ImplicitEvent::FocusOut
            }))),
            ConsoleRecord::Mouse {
                x,
                y,
                buttons,
                flags,
                ..
            } => {
                if flags == 0 && buttons & 1 != 0 && x >= 0 && y >= 0 {
                    let (left, top) = session.viewport_origin()?;
                    let column = i32::from(x) - i32::from(left);
                    let row = i32::from(y) - i32::from(top);
                    if column < 0 || row < 0 {
                        return Ok(None);
                    }
                    Ok(Some(CharEvent::Implicit(ImplicitEvent::MouseLeft(
                        ViewportPosition {
                            x: column as usize + 1,
                            y: row as usize + 1,
                        },
                    ))))
                } else {
                    Ok(None)
                }
            }
            ConsoleRecord::Menu(_) => Ok(None),
            ConsoleRecord::Key { .. } => decoder.record(record),
        }
    }
}

fn notification(kind: u8) -> InputEventTrigger {
    match kind {
        0 => InputEventTrigger::Interrupted,
        1 => InputEventTrigger::UvarNotified,
        2 => InputEventTrigger::JobNotified,
        3 => InputEventTrigger::IOPortNotified,
        _ => unreachable!(),
    }
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn wait_error(result: u32) -> io::Error {
    if result == u32::MAX {
        io::Error::last_os_error()
    } else {
        io::Error::other(format!("unexpected native console wait result {result}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(
        down: bool,
        virtual_key: u16,
        utf16: u16,
        repeat: u16,
        control_state: u32,
    ) -> ConsoleRecord {
        ConsoleRecord::Key {
            down,
            virtual_key,
            utf16,
            repeat,
            control_state,
            scan_code: 0,
        }
    }

    fn text(input: &mut NativeDecoder) -> String {
        let mut bytes = Vec::new();
        while let Some(byte) = input.buffered_byte() {
            bytes.push(byte);
        }
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn unicode_record_commits_and_repeats() {
        let mut input = NativeDecoder::default();
        input.record(key(false, 0x12, 0xd83d, 3, 0)).unwrap();
        assert_eq!(text(&mut input), "");
        input.record(key(false, 0x12, 0xdc1f, 3, 0)).unwrap();
        assert_eq!(text(&mut input), "🐟🐟🐟");
        input.record(key(true, 0x41, b'a'.into(), 2, 0)).unwrap();
        assert_eq!(text(&mut input), "aa");
        input.record(key(false, 0x41, b'a'.into(), 2, 0)).unwrap();
        assert_eq!(
            text(&mut input),
            "",
            "ordinary key-up duplicated committed text"
        );
    }

    #[test]
    fn native_nul_chords_preserve_text_and_legacy_binding_identity() {
        use crate::input::{InputEventQueue, InputEventQueuer as _};
        let mut decoder = NativeDecoder::default();
        for (virtual_key, modifiers) in [(0x20, 8), (0x32, 8), (0x32, 0x18)] {
            assert!(
                decoder
                    .record(key(true, virtual_key, 0, 1, modifiers))
                    .unwrap()
                    .is_none()
            );
            let bytes = text(&mut decoder);
            assert_eq!(bytes, "\0");
            let mut queue = InputEventQueue::new(0, None);
            queue.service_input_event(InputEventTrigger::Byte(bytes.as_bytes()[0]));
            assert_eq!(
                queue.try_pop().unwrap().get_key().unwrap().key.key,
                key::ctrl(key::SPACE)
            );
        }
        assert!(decoder.record(key(true, 0xde, 0, 1, 0)).unwrap().is_none());
        assert_eq!(text(&mut decoder), "");
    }

    #[test]
    fn modified_physical_keys_remain_bindable() {
        let mut input = NativeDecoder::default();
        let event = input.record(key(true, 0x45, 5, 1, 0x0a)).unwrap().unwrap();
        let event = event.get_key().unwrap();
        assert_eq!(
            event.key.key,
            Key::new(
                Modifiers {
                    ctrl: true,
                    alt: true,
                    shift: false,
                    sup: false
                },
                'e'
            )
        );
        assert!(
            event.key.text_to_insert().is_none(),
            "a shortcut became inserted text"
        );
        assert!(
            input
                .record(key(true, 0x32, b'@'.into(), 1, 0x09))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            text(&mut input),
            "@",
            "AltGr committed text became Ctrl+Alt shortcut"
        );
        let event = input.record(key(true, 0x74, 0, 1, 0x18)).unwrap().unwrap();
        assert_eq!(
            event.get_key().unwrap().key.key,
            Key::new(
                Modifiers {
                    ctrl: true,
                    alt: false,
                    shift: true,
                    sup: false
                },
                key::function_key(5),
            )
        );
    }

    #[test]
    fn malformed_console_unicode_has_no_replacement_or_cross_epoch_tail() {
        let mut input = NativeDecoder::default();
        input.record(key(true, 0, 0xd83d, 2, 0)).unwrap();
        assert_eq!(
            input.record(key(true, 0, 0xdc1f, 1, 0)).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(text(&mut input), "");
        input = NativeDecoder::default();
        assert_eq!(
            input.record(key(true, 0, 0xdc1f, 1, 0)).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        input = NativeDecoder::default();
        input.record(key(true, 0, b'x'.into(), 1, 0)).unwrap();
        assert_eq!(text(&mut input), "x");
        assert_eq!(
            input
                .record(key(true, 0, b'y'.into(), 0, 0))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(text(&mut input), "");
    }

    #[test]
    fn nested_consumers_share_scalar_tails_and_repeat_boundaries() {
        let shared = std::cell::RefCell::new(NativeDecoder::default());
        shared
            .borrow_mut()
            .record(key(false, 0x12, 0xd83d, 2, 0))
            .unwrap();
        assert!(
            shared.borrow().incomplete_scalar(),
            "a child cannot inherit half a surrogate pair"
        );
        {
            let mut nested = shared.borrow_mut();
            nested.record(key(false, 0x12, 0xdc1f, 2, 0)).unwrap();
            assert!(
                !nested.incomplete_scalar(),
                "an unstarted repeat belongs to the editor"
            );
            assert_eq!(nested.buffered_byte(), Some(0xf0));
            assert!(
                nested.incomplete_scalar(),
                "a partial UTF-8 scalar cannot be donated"
            );
        }
        let mut outer = shared.borrow_mut();
        let tail: Vec<_> = (0..3).map(|_| outer.buffered_byte().unwrap()).collect();
        assert_eq!(tail, [0x9f, 0x90, 0x9f]);
        assert!(
            !outer.incomplete_scalar(),
            "services must run between complete repeated scalars"
        );
        assert_eq!(text(&mut outer), "🐟");
        outer.record(key(true, 0, 0xf600, 1, 0)).unwrap();
        assert_eq!(
            text(&mut outer),
            "\u{f600}",
            "valid native private-use text must not disappear"
        );
    }

    #[test]
    fn physical_text_controls_cannot_escape_an_active_paste() {
        use crate::input::{InputEventQueue, InputEventQueuer as _};
        let mut decoder = NativeDecoder::default();
        let mut queue = InputEventQueue::new(0, None);
        queue.get_input_data_mut().paste_buffer = Some(Vec::new());
        for (virtual_key, text, modifiers) in [(0x0d, 13, 0), (0x09, 9, 0), (0x47, 7, 8)] {
            let event = decoder
                .record(key(true, virtual_key, text, 1, modifiers))
                .unwrap()
                .unwrap();
            assert!(
                queue
                    .service_input_event(InputEventTrigger::Event(event))
                    .is_none()
            );
        }
        assert_eq!(
            queue.get_input_data().paste_buffer.as_deref(),
            Some(&b"\r\t\x07"[..])
        );
        assert!(
            !queue.has_lookahead(),
            "paste text must not reach binding dispatch"
        );
    }
}
