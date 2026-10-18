use super::{
    binding::ReadlineCmd,
    decode::{DisplayBytes, LONG_READ_TIMEOUT, on_byte_read},
};
#[cfg(unix)]
use crate::{
    common::{WSL, is_windows_subsystem_for_linux, shell_modes},
    fd_readable_set::FdReadableSet,
    universal_notifier::default_notifier,
};
use crate::{
    env::{EnvStack, Environment as _},
    fd_readable_set::Timeout,
    flog::{FloggableDebug, flog},
    key::{self, Key, Modifiers, ViewportPosition},
    prelude::*,
    reader::reader_test_and_clear_interrupted,
    text_helpers::char_to_symbol,
    wutil::{fish_is_pua, fish_wcstol},
};
#[cfg(unix)]
use fish_common::read_blocked;
use fish_feature_flags::{FeatureFlag, feature_test};
#[cfg(unix)]
use nix::errno::Errno;
#[cfg(unix)]
use nix::sys::{select::FdSet, signal::SigSet, time::TimeSpec};
#[cfg(unix)]
use std::os::fd::{BorrowedFd, RawFd};
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
#[cfg(windows)]
type RawFd = i32; // Logical selector only; native transport retains HANDLEs.

pub(super) fn interrupt_key() -> u8 {
    #[cfg(unix)]
    {
        shell_modes().control_chars[libc::VINTR]
    }
    #[cfg(windows)]
    {
        3
    } // Logical Ctrl-C key, not a POSIX signal number.
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadlineCmdEvent {
    pub cmd: ReadlineCmd,
    /// The triggering key events.
    /// Note that the generic self-insert case does not have any characters, so this would be empty.
    /// This is also empty for invalid Unicode code points, which produce multiple characters.
    pub key_events: Vec<CharEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyInputEvent {
    // The key.
    pub key: KeyEvent,
    // The style to use when inserting characters into the command line.
    pub input_style: CharInputStyle,
    /// The sequence of characters in the input mapping which generated this event.
    /// Note that the generic self-insert case does not have any characters, so this would be empty.
    /// This is also empty for invalid Unicode code points, which produce multiple characters.
    pub seq: WString,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImplicitEvent {
    /// end-of-file was reached.
    Eof,
    /// An event was handled internally, or an interrupt was received. Check to see if the reader
    /// loop should exit.
    CheckExit,
    /// Our terminal window gained focus.
    FocusIn,
    /// Our terminal window lost focus.
    FocusOut,
    /// Mouse left click.
    MouseLeft(ViewportPosition),
    /// Terminal color theme change (light/dark mode).
    NewColorTheme,
    /// Window height changed.
    NewWindowHeight,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryResponse {
    PrimaryDeviceAttribute,
    BackgroundColor(xterm_color::Color),
    CursorPosition(ViewportPosition),
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryResultEvent {
    Response(QueryResponse),
    Timeout,
    /// Canceled with ctrl-c.
    Interrupted,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CharEvent {
    /// A character was entered.
    Key(KeyInputEvent),

    /// A readline event.
    Readline(ReadlineCmdEvent),

    /// A shell command.
    Command(WString),

    /// Any event that has no user-visible representation.
    Implicit(ImplicitEvent),

    QueryResult(QueryResultEvent),
}
impl FloggableDebug for CharEvent {}

impl CharEvent {
    pub fn is_char(&self) -> bool {
        matches!(self, CharEvent::Key(_))
    }

    pub fn is_readline(&self) -> bool {
        matches!(self, CharEvent::Readline(_))
    }

    pub fn is_readline_or_command(&self) -> bool {
        matches!(self, CharEvent::Readline(_) | CharEvent::Command(_))
    }

    #[cfg(test)]
    pub fn get_char(&self) -> char {
        let CharEvent::Key(kevt) = self else {
            panic!("Not a char type");
        };
        kevt.key.codepoint
    }

    pub fn get_key(&self) -> Option<&KeyInputEvent> {
        match self {
            CharEvent::Key(kevt) => Some(kevt),
            _ => None,
        }
    }
    pub fn get_key_mut(&mut self) -> Option<&mut KeyInputEvent> {
        match self {
            CharEvent::Key(kevt) => Some(kevt),
            _ => None,
        }
    }

    pub fn get_readline(&self) -> ReadlineCmd {
        let CharEvent::Readline(c) = self else {
            panic!("Not a readline type");
        };
        c.cmd
    }

    pub fn get_command(&self) -> Option<&wstr> {
        match self {
            CharEvent::Command(c) => Some(c),
            _ => None,
        }
    }

    pub fn from_char(c: char) -> CharEvent {
        Self::from_key(KeyEvent::from_raw(c))
    }

    pub fn from_key(key: KeyEvent) -> CharEvent {
        Self::from_key_seq(key, WString::new())
    }

    pub fn from_key_seq(key: KeyEvent, seq: WString) -> CharEvent {
        CharEvent::Key(KeyInputEvent {
            key,
            input_style: CharInputStyle::Normal,
            seq,
        })
    }

    pub fn from_readline(cmd: ReadlineCmd, key_events: Vec<CharEvent>) -> CharEvent {
        Self::Readline(ReadlineCmdEvent { cmd, key_events })
    }

    pub fn from_check_exit() -> CharEvent {
        CharEvent::Implicit(ImplicitEvent::CheckExit)
    }
}

type Text = [char; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyEvent {
    pub key: Key,
    pub(crate) explicit_modifiers: bool,
    pub shifted_codepoint: char,
    base_layout_codepoint: char,
    pub associated_text: Text,
    /// Committed console text, distinct from a physical key's binding identity.
    #[cfg(windows)]
    pub(crate) native_text: Option<char>,
}

pub type CharIterator = std::iter::TakeWhile<std::array::IntoIter<char, 4>, fn(&char) -> bool>;

impl KeyEvent {
    pub(super) fn new(modifiers: Modifiers, codepoint: char) -> Self {
        Self::from(Key::new(modifiers, codepoint))
    }
    pub(super) fn new_with(
        modifiers: Modifiers,
        explicit_modifiers: bool,
        codepoint: char,
        shifted_key: Option<char>,
        base_layout_key: Option<char>,
        associated_text: Text,
    ) -> Self {
        Self {
            key: Key::new(modifiers, codepoint),
            explicit_modifiers,
            shifted_codepoint: shifted_key.unwrap_or_default(),
            base_layout_codepoint: base_layout_key.unwrap_or_default(),
            associated_text,
            #[cfg(windows)]
            native_text: None,
        }
    }
    pub(super) fn from_raw(codepoint: char) -> Self {
        Self::from(Key::from_raw(codepoint))
    }
    pub fn from_single_byte(c: u8) -> Self {
        Self::from(Key::from_single_byte(c))
    }

    pub fn text_to_insert(&self) -> Option<CharIterator> {
        let until_zero = |cs: Text| {
            Some(
                cs.into_iter()
                    .take_while((|c| *c != '\0') as fn(&char) -> bool),
            )
        };
        if self.associated_text[0] != '\0' {
            return until_zero(self.associated_text);
        }
        until_zero([self.codepoint_text()?, '\0', '\0', '\0'])
    }

    pub fn codepoint_text(&self) -> Option<char> {
        let mut modifiers = self.modifiers;
        let mut c = self.codepoint;
        if self.shifted_codepoint != '\0' && modifiers.shift {
            modifiers.shift = false;
            c = self.shifted_codepoint;
        }
        if modifiers.is_some() {
            return None;
        }
        if c == key::SPACE {
            return Some(' ');
        }
        if c == key::ENTER {
            return Some('\n');
        }
        if c == key::TAB {
            return Some('\t');
        }
        if fish_is_pua(c) || u32::from(c) <= 27 {
            return None;
        }
        Some(c)
    }

    pub fn base_layout_codepoint(&self) -> Option<char> {
        (self.modifiers.is_some() && self.base_layout_codepoint != '\0')
            .then_some(self.base_layout_codepoint)
    }
}

impl From<Key> for KeyEvent {
    fn from(key: Key) -> Self {
        Self::new_with(
            key.modifiers,
            false,
            key.codepoint,
            None,
            None,
            Default::default(),
        )
    }
}

impl std::ops::Deref for KeyEvent {
    type Target = Key;
    fn deref(&self) -> &Self::Target {
        &self.key
    }
}

impl std::ops::DerefMut for KeyEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.key
    }
}

/// Hackish: the input style, which describes how char events (only) are applied to the command
/// line. Note this is set only after applying bindings; it is not set from readb().
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CharInputStyle {
    // Insert characters normally.
    Normal,

    // Insert characters only if the cursor is not at the beginning. Otherwise, discard them.
    NotFirst,
}

/// Time in milliseconds to wait for another byte to be available for reading
/// after \x1B is read before assuming that escape key was pressed, and not an
/// escape sequence.
const WAIT_ON_ESCAPE_DEFAULT: usize = 30;
static WAIT_ON_ESCAPE_MS: AtomicUsize = AtomicUsize::new(WAIT_ON_ESCAPE_DEFAULT);

const WAIT_ON_SEQUENCE_KEY_INFINITE: usize = usize::MAX;
static WAIT_ON_SEQUENCE_KEY_MS: AtomicUsize = AtomicUsize::new(WAIT_ON_SEQUENCE_KEY_INFINITE);

// Update the wait_on_escape_ms value in response to the fish_escape_delay_ms user variable being
// set.
pub fn update_wait_on_escape_ms(vars: &EnvStack) {
    let fish_escape_delay_ms = vars.get_unless_empty(L!("fish_escape_delay_ms"));
    let Some(fish_escape_delay_ms) = fish_escape_delay_ms else {
        WAIT_ON_ESCAPE_MS.store(WAIT_ON_ESCAPE_DEFAULT, Ordering::Relaxed);
        return;
    };
    let fish_escape_delay_ms = fish_escape_delay_ms.as_string();
    match fish_wcstol(&fish_escape_delay_ms) {
        Ok(val) if (10..5000).contains(&val) => {
            WAIT_ON_ESCAPE_MS.store(val.try_into().unwrap(), Ordering::Relaxed);
        }
        _ => {
            eprintf!(
                concat!(
                    "ignoring fish_escape_delay_ms: value '%s' ",
                    "is not an integer or is < 10 or >= 5000 ms\n"
                ),
                fish_escape_delay_ms
            );
        }
    }
}

// Update the wait_on_sequence_key_ms value in response to the fish_sequence_key_delay_ms user
// variable being set.
pub fn update_wait_on_sequence_key_ms(vars: &EnvStack) {
    let sequence_key_time_ms = vars.get_unless_empty(L!("fish_sequence_key_delay_ms"));
    let Some(sequence_key_time_ms) = sequence_key_time_ms else {
        WAIT_ON_SEQUENCE_KEY_MS.store(WAIT_ON_SEQUENCE_KEY_INFINITE, Ordering::Relaxed);
        return;
    };
    let sequence_key_time_ms = sequence_key_time_ms.as_string();
    match fish_wcstol(&sequence_key_time_ms) {
        Ok(val) if (10..5000).contains(&val) => {
            WAIT_ON_SEQUENCE_KEY_MS.store(val.try_into().unwrap(), Ordering::Relaxed);
        }
        _ => {
            eprintf!(
                concat!(
                    "ignoring fish_sequence_key_delay_ms: value '%s' ",
                    "is not an integer or is < 10 or >= 5000 ms\n"
                ),
                sequence_key_time_ms
            );
        }
    }
}

// A data type used by the input machinery.
#[derive(Default)]
pub struct InputData {
    // The file descriptor from which we read input, often stdin.
    pub in_fd: RawFd,

    // Queue of unread characters.
    pub queue: VecDeque<CharEvent>,

    // The current paste buffer, if any.
    pub paste_buffer: Option<Vec<u8>>,

    // The arguments to the most recently invoked input function.
    pub input_function_args: Vec<char>,

    // The return status of the most recently invoked input function.
    pub function_status: bool,

    // Transient storage to avoid repeated allocations.
    pub event_storage: Vec<CharEvent>,

    // How long to wait for responses for TTY queries.
    pub blocking_query_timeout: Option<Duration>,

    // If set, events will be buffered until the query finishes.
    pub blocking_query: Option<TerminalQuery>,

    #[cfg(windows)]
    pub(crate) native: super::windows::NativeInput,
    #[cfg(windows)]
    pub(crate) native_error: Option<std::io::Error>,
    #[cfg(windows)]
    pub(crate) native_cancelled: bool,
    #[cfg(windows)]
    pub(crate) native_exit_status: Option<i32>,
}

impl InputData {
    /// Construct from the fd from which to read.
    pub fn new(in_fd: RawFd, blocking_query_timeout: Option<Duration>) -> Self {
        Self {
            in_fd,
            queue: VecDeque::new(),
            paste_buffer: None,
            input_function_args: Vec::new(),
            function_status: false,
            event_storage: Vec::new(),
            blocking_query_timeout,
            blocking_query: None,
            #[cfg(windows)]
            native_exit_status: None,
            #[cfg(windows)]
            native: super::windows::NativeInput::default(),
            #[cfg(windows)]
            native_error: None,
            #[cfg(windows)]
            native_cancelled: false,
        }
    }

    /// Enqueue a char event to the queue of unread characters that input_readch will return before
    /// actually reading from fd 0.
    pub fn queue_char(&mut self, ch: CharEvent) {
        self.queue.push_back(ch);
    }

    /// Sets the return status of the most recently executed input function.
    pub fn function_set_status(&mut self, status: bool) {
        self.function_status = status;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BackgroundColorQuery {
    pub result: Option<xterm_color::Color>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CursorPositionQueryReason {
    NewPrompt,
    WindowHeightChange,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CursorPositionQuery {
    pub reason: CursorPositionQueryReason,
    pub result: Option<ViewportPosition>,
}

impl CursorPositionQuery {
    pub fn new(reason: CursorPositionQueryReason) -> Self {
        Self {
            reason,
            result: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecurrentQuery {
    pub background_color: Option<BackgroundColorQuery>,
    pub cursor_position: Option<CursorPositionQuery>,
}

#[derive(Clone, PartialEq)]
pub enum TerminalQuery {
    Initial,
    Recurrent(RecurrentQuery),
}

pub(super) fn is_event_blocked_when_querying(evt: &CharEvent) -> bool {
    use ImplicitEvent::*;
    match evt {
        CharEvent::QueryResult(_) | CharEvent::Implicit(CheckExit | Eof) => false,
        CharEvent::Key(_)
        | CharEvent::Readline(_)
        | CharEvent::Command(_)
        | CharEvent::Implicit(_) => {
            true // No code execution while blocked.
        }
    }
}

/// A trait which knows how to produce a stream of input events.
/// Note this is conceptually a "base class" with override points.
pub trait InputEventQueuer {
    /// Return the next event in the queue, or none if the queue is empty.
    fn try_pop(&mut self) -> Option<CharEvent> {
        if self.is_blocked_querying()
            && is_event_blocked_when_querying(self.get_input_data().queue.front()?)
        {
            #[cfg(windows)]
            {
                // A terminal reply must not make us prefetch a later command or
                // nested reader's input. Finalize the bounded query first; its
                // owner then consumes this untouched event in normal order.
                return Some(CharEvent::QueryResult(QueryResultEvent::Interrupted));
            }
            #[cfg(unix)]
            return None;
        }
        self.get_input_data_mut().queue.pop_front()
    }

    fn wait_input_event(&mut self, timeout: Timeout) -> InputEventTrigger {
        #[cfg(unix)]
        {
            next_input_event(self.get_in_fd(), self.get_ioport_fd(), timeout)
        }
        #[cfg(windows)]
        {
            self.get_input_data_mut()
                .native
                .next(timeout, Default::default())
        }
    }

    #[cfg(windows)]
    fn native_jobs_notified(&mut self) {}

    #[cfg(windows)]
    fn input_error(&mut self, error: std::io::Error) {
        self.get_input_data_mut().native_error = Some(error);
    }

    fn service_input_event(&mut self, event: InputEventTrigger) -> Option<CharEvent> {
        match event {
            InputEventTrigger::Eof => return Some(CharEvent::Implicit(ImplicitEvent::Eof)),
            InputEventTrigger::Interrupted => self.select_interrupted(),
            InputEventTrigger::UvarNotified => self.uvar_change_notified(),
            InputEventTrigger::IOPortNotified => self.ioport_notified(),
            InputEventTrigger::Byte(byte) => on_byte_read(self, byte),
            InputEventTrigger::TimeoutElapsed => {
                return Some(CharEvent::QueryResult(QueryResultEvent::Timeout));
            }
            #[cfg(windows)]
            InputEventTrigger::Event(event) => {
                if self.paste_is_buffering() {
                    if let CharEvent::Key(key) = &event {
                        if let Some(text) = key.key.native_text {
                            let mut bytes = [0; 4];
                            self.get_input_data_mut()
                                .paste_buffer
                                .as_mut()
                                .unwrap()
                                .extend_from_slice(text.encode_utf8(&mut bytes).as_bytes());
                            return None;
                        }
                    }
                }
                super::decode::InputEventQueuerExt::enqueue_event(self, event);
            }
            #[cfg(windows)]
            InputEventTrigger::JobNotified => self.native_jobs_notified(),
            #[cfg(windows)]
            InputEventTrigger::Error(error) => {
                self.input_error(error);
                return Some(CharEvent::from_check_exit());
            }
        }
        None
    }

    /// Read the next event, such as a UTF-8-encoded codepoint.
    fn readch(&mut self) -> CharEvent {
        loop {
            // Do we have something enqueued already?
            // Note this may be initially true, or it may become true through calls to
            // iothread_service_main() or env_universal_barrier() below.
            if let Some(mevt) = self.try_pop() {
                return mevt;
            }

            // We are going to block; but first allow any override to inject events.
            self.prepare_to_select();
            if let Some(mevt) = self.try_pop() {
                return mevt;
            }

            let timeout = if self.is_blocked_querying() {
                Timeout::Duration(self.get_input_data().blocking_query_timeout.unwrap())
            } else {
                Timeout::Forever
            };
            let event = self.wait_input_event(timeout);
            if let Some(event) = self.service_input_event(event) {
                return event;
            }
        }
    }

    fn readch_timed_esc(&mut self) -> Option<CharEvent> {
        self.readch_timed(WAIT_ON_ESCAPE_MS.load(Ordering::Relaxed))
    }

    fn readch_timed_sequence_key(&mut self) -> Option<CharEvent> {
        let wait_on_sequence_key_ms = WAIT_ON_SEQUENCE_KEY_MS.load(Ordering::Relaxed);
        if wait_on_sequence_key_ms == WAIT_ON_SEQUENCE_KEY_INFINITE {
            return Some(self.readch());
        }
        self.readch_timed(wait_on_sequence_key_ms)
    }

    /// Like readch(), except it will wait at most wait_time_ms milliseconds for a
    /// character to be available for reading.
    /// Return None on timeout, the event on success.
    #[cfg(unix)]
    fn readch_timed(&mut self, wait_time_ms: usize) -> Option<CharEvent> {
        if let Some(evt) = self.try_pop() {
            return Some(evt);
        }

        check_fd_readable(
            unsafe { BorrowedFd::borrow_raw(self.get_in_fd()) },
            Duration::from_millis(u64::try_from(wait_time_ms).unwrap()),
        )
        .then(|| self.readch())
    }

    #[cfg(windows)]
    fn readch_timed(&mut self, wait_time_ms: usize) -> Option<CharEvent> {
        let Some(deadline) = std::time::Instant::now()
            .checked_add(Duration::from_millis(u64::try_from(wait_time_ms).unwrap()))
        else {
            self.input_error(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "native input timeout overflow",
            ));
            return Some(CharEvent::from_check_exit());
        };
        loop {
            if let Some(event) = self.try_pop() {
                return Some(event);
            }
            self.prepare_to_select();
            if let Some(event) = self.try_pop() {
                return Some(event);
            }
            let event = self.wait_input_event(Timeout::Duration(
                deadline.saturating_duration_since(std::time::Instant::now()),
            ));
            if matches!(event, InputEventTrigger::TimeoutElapsed) {
                return None;
            }
            if let Some(event) = self.service_input_event(event) {
                return Some(event);
            }
        }
    }

    /// Return the fd from which to read.
    fn get_in_fd(&self) -> RawFd {
        self.get_input_data().in_fd
    }

    /// Return the fd of the IO port, or -1 if none.
    fn get_ioport_fd(&self) -> RawFd {
        -1
    }

    /// Return the input data. This is to be implemented by the concrete type.
    fn get_input_data(&self) -> &InputData;
    fn get_input_data_mut(&mut self) -> &mut InputData;

    // Support for "bracketed paste"
    // The way it works is that we acknowledge our support by printing
    // \e\[?2004h
    // then the terminal will "bracket" every paste in
    // \e\[200~ and \e\[201~
    // Every character in between those two will be part of the paste and should not cause a binding to execute (like \n executing commands).
    //
    // We enable it after every command and disable it before, see the terminal protocols logic.
    //
    // Support for this seems to be ubiquitous - emacs enables it unconditionally (!) since 25.1
    // (though it only supports it since then, it seems to be the last term to gain support).
    //
    // See http://thejh.net/misc/website-terminal-copy-paste.

    fn paste_start_buffering(&mut self) {
        self.get_input_data_mut().paste_buffer = Some(Vec::new());
    }

    fn paste_is_buffering(&self) -> bool {
        self.get_input_data().paste_buffer.is_some()
    }

    fn paste_push_char(&mut self, b: u8) {
        self.get_input_data_mut()
            .paste_buffer
            .as_mut()
            .unwrap()
            .push(b);
    }

    fn paste_commit(&mut self) {
        self.get_input_data_mut().paste_buffer = None;
    }

    /// Enqueue a character or a readline function to the queue of unread characters that
    /// readch will return before actually reading from fd 0.
    fn push_back(&mut self, ch: CharEvent) {
        self.get_input_data_mut().queue.push_back(ch);
    }

    /// Add a character or a readline function to the front of the queue of unread characters.  This
    /// will be the next character returned by readch.
    fn push_front(&mut self, ch: CharEvent) {
        self.get_input_data_mut().queue.push_front(ch);
    }

    /// Find the first sequence of non-char events, and promote them to the front.
    fn promote_interruptions_to_front(&mut self) {
        // Find the first sequence of non-char events.
        // EOF is considered a char: we don't want to pull EOF in front of real chars.
        let queue = &mut self.get_input_data_mut().queue;
        let is_char = |evt: &CharEvent| {
            evt.is_char() || matches!(evt, CharEvent::Implicit(ImplicitEvent::Eof))
        };
        // Find the index of the first non-char event.
        // If there's none, we're done.
        let Some(first): Option<usize> = queue.iter().position(|e| !is_char(e)) else {
            return;
        };
        let last = queue
            .range(first..)
            .position(is_char)
            .map_or(queue.len(), |x| x + first);
        // Move the non-char events to the front, retaining their order.
        let elems: Vec<CharEvent> = queue.drain(first..last).collect();
        for elem in elems.into_iter().rev() {
            queue.push_front(elem);
        }
    }

    /// Add multiple readline events to the front of the queue of unread characters.
    /// The order of the provided events is not changed, i.e. they are not inserted in reverse
    /// order. That is, the first element in evts will be the first element returned.
    fn insert_front<I>(&mut self, evts: I)
    where
        I: IntoIterator<Item = CharEvent>,
        I::IntoIter: DoubleEndedIterator,
    {
        let queue = &mut self.get_input_data_mut().queue;
        let iter = evts.into_iter().rev();
        queue.reserve(iter.size_hint().0);
        for evt in iter {
            queue.push_front(evt);
        }
    }

    /// Forget all enqueued readline events in the front of the queue.
    fn drop_leading_readline_events(&mut self) {
        let queue = &mut self.get_input_data_mut().queue;
        while let Some(evt) = queue.front() {
            if evt.is_readline_or_command() {
                queue.pop_front();
            } else {
                break;
            }
        }
    }

    fn blocking_query_mut(&mut self) -> &mut Option<TerminalQuery> {
        &mut self.get_input_data_mut().blocking_query
    }
    fn is_blocked_querying(&self) -> bool {
        self.get_input_data().blocking_query.is_some()
    }

    /// Override point for when we are about to (potentially) block in select(). The default does
    /// nothing.
    fn prepare_to_select(&mut self) {}

    /// Called when select() is interrupted by a signal.
    fn select_interrupted(&mut self) {}

    fn enqueue_interrupt_key(&mut self) {
        let vintr = interrupt_key();
        if vintr != 0 {
            let interrupt_evt = CharEvent::from_key(KeyEvent::from_single_byte(vintr));
            if stop_query(self.blocking_query_mut()) {
                flog!(
                    reader,
                    "Received interrupt, giving up on waiting for terminal response"
                );
                self.get_input_data_mut().queue.clear();
                self.push_front(CharEvent::QueryResult(QueryResultEvent::Interrupted));
            } else {
                self.push_front(interrupt_evt);
            }
        }
    }

    /// Override point for when when select() is interrupted by the universal variable notifier.
    /// The default does nothing.
    fn uvar_change_notified(&mut self) {}

    /// Override point for when the ioport is ready.
    /// The default does nothing.
    fn ioport_notified(&mut self) {}

    /// Get the function status.
    fn function_status(&self) -> bool {
        self.get_input_data().function_status
    }

    #[cfg(unix)]
    fn read_sequence_byte(&mut self, buffer: &mut Vec<u8>) -> Option<u8> {
        let fd = self.get_in_fd();
        let strict = feature_test(FeatureFlag::OmitTermWorkarounds);
        let historical_millis = |ms| {
            if strict {
                LONG_READ_TIMEOUT
            } else {
                Duration::from_millis(ms)
            }
        };
        if !check_fd_readable(
            unsafe { BorrowedFd::borrow_raw(fd) },
            if self.paste_is_buffering() || self.is_blocked_querying() {
                historical_millis(300)
            } else if buffer == b"\x1b" {
                Duration::from_millis(1) // distinguish legacy escape
            } else {
                historical_millis(30)
            },
        ) {
            flog!(
                reader,
                format!("Incomplete escape sequence: {}", DisplayBytes(buffer))
            );
            if buffer != b"\x1b" && strict {
                flog!(
                    error,
                    format!(
                        "Incomplete escape sequence seen (logging because omit-term-workarounds is on): {}",
                        DisplayBytes(buffer)
                    )
                );
            }
            return None;
        }
        let next = readb(fd)?;
        buffer.push(next);
        Some(next)
    }

    #[cfg(windows)]
    fn read_sequence_byte(&mut self, buffer: &mut Vec<u8>) -> Option<u8> {
        let duration = if feature_test(FeatureFlag::OmitTermWorkarounds)
            || self.paste_is_buffering()
            || self.is_blocked_querying()
        {
            LONG_READ_TIMEOUT
        } else {
            // Native input has an owned worker admission/completion boundary.
            // Use the configured escape deadline, not a Unix 1-ms fd probe.
            Duration::from_millis(WAIT_ON_ESCAPE_MS.load(Ordering::Relaxed) as u64)
        };
        let deadline = std::time::Instant::now() + duration;
        loop {
            let event = self.wait_input_event(Timeout::Duration(
                deadline.saturating_duration_since(std::time::Instant::now()),
            ));
            match event {
                InputEventTrigger::Byte(byte) => {
                    buffer.push(byte);
                    return Some(byte);
                }
                InputEventTrigger::TimeoutElapsed => return None,
                InputEventTrigger::Interrupted => {
                    self.select_interrupted();
                    return None;
                }
                InputEventTrigger::Event(event) => {
                    super::decode::InputEventQueuerExt::enqueue_event(self, event);
                    return None;
                }
                event => {
                    if let Some(event) = self.service_input_event(event) {
                        self.push_front(event);
                        return None;
                    }
                }
            }
        }
    }

    /// Return if we have any lookahead.
    #[cfg(test)]
    fn has_lookahead(&self) -> bool {
        !self.get_input_data().queue.is_empty()
    }

    fn get_bind_mode(&self) -> WString {
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(cfg!(test));
        }
        WString::from("test-bind-mode")
    }
}

/// Internal function used by readch to read one byte.
/// This calls select() on three fds: input (e.g. stdin), the ioport notifier fd (for main thread
/// requests), and the uvar notifier. This returns either the byte which was read, or one of the
/// special values below.
pub(crate) enum InputEventTrigger {
    // A byte was successfully read.
    Byte(u8),

    // The in fd has been closed.
    Eof,

    // select() was interrupted by a signal.
    Interrupted,

    // Our uvar notifier reported a change (either through poll() or its fd).
    UvarNotified,

    // Our ioport reported a change, so service main thread requests.
    IOPortNotified,

    // No file descriptor was ready within the query timeout.
    TimeoutElapsed,
    #[cfg(windows)]
    Event(CharEvent),
    #[cfg(windows)]
    JobNotified,
    #[cfg(windows)]
    Error(std::io::Error),
}

#[cfg(unix)]
pub(super) fn readb(in_fd: RawFd) -> Option<u8> {
    assert!(in_fd >= 0, "Invalid in fd");
    let mut arr: [u8; 1] = [0];
    if read_blocked(in_fd, &mut arr) != Ok(1) {
        // The terminal has been closed.
        return None;
    }
    let c = arr[0];
    flog!(reader, "Read byte", char_to_symbol(char::from(c), true));
    // The common path is to return a u8.
    Some(c)
}

#[cfg(unix)]
pub(super) fn next_input_event(
    in_fd: RawFd,
    ioport_fd: RawFd,
    timeout: Timeout,
) -> InputEventTrigger {
    let mut fdset = FdReadableSet::new();
    loop {
        fdset.clear();
        fdset.add(in_fd);

        // Add the completion ioport (possibly -1 - a no-op).
        fdset.add(ioport_fd);

        // Get the uvar notifier fd (possibly none).
        let notifier = default_notifier();
        let notifier_fd = notifier.notification_fd();
        if let Some(notifier_fd) = notifier_fd {
            fdset.add(notifier_fd);
        }

        // Here's where we call select().
        let select_res = fdset.check_readable(timeout);
        if select_res < 0 {
            let err = Errno::last();
            if err == Errno::EINTR || err == Errno::EAGAIN {
                // A signal.
                return InputEventTrigger::Interrupted;
            } else {
                // Some fd was invalid, so probably the tty has been closed.
                return InputEventTrigger::Eof;
            }
        }
        if select_res == 0 {
            assert!(!matches!(timeout, Timeout::Forever));
            return InputEventTrigger::TimeoutElapsed;
        }

        // select() did not return an error, so we may have a readable fd.
        // The priority order is: uvars, stdin, ioport.
        // Check to see if we want a universal variable barrier.
        if let Some(notifier_fd) = notifier_fd {
            if fdset.test(notifier_fd) && notifier.notification_fd_became_readable(notifier_fd) {
                return InputEventTrigger::UvarNotified;
            }
        }

        // Check stdin.
        if fdset.test(in_fd) {
            return readb(in_fd).map_or(InputEventTrigger::Eof, InputEventTrigger::Byte);
        }

        // Check for iothread completions only if there is no data to be read from the stdin.
        // This gives priority to the foreground.
        if fdset.test(ioport_fd) {
            return InputEventTrigger::IOPortNotified;
        }
    }
}

#[cfg(unix)]
pub fn check_fd_readable(in_fd: BorrowedFd, timeout: Duration) -> bool {
    // We are not prepared to handle a signal immediately; we only want to know if we get input on
    // our fd before the timeout. Use pselect to block all signals; we will handle signals
    // before the next call to readch().

    // We have one fd of interest.
    let mut readfds = {
        let mut set = FdSet::new();
        set.insert(in_fd);
        set
    };

    let res = nix::sys::select::pselect(
        None,
        &mut readfds,
        None,
        None,
        &TimeSpec::from_duration(timeout),
        &SigSet::all(),
    )
    .unwrap();

    // Prevent signal starvation on WSL causing the `torn_escapes.py` test to fail
    if is_windows_subsystem_for_linux(WSL::V1) {
        // Merely querying the current thread's sigmask is sufficient to deliver a pending signal
        _ = SigSet::thread_get_mask().expect("Failed to get sigmask!");
    }

    res > 0
}

pub(crate) fn stop_query(query: &mut Option<TerminalQuery>) -> bool {
    query.take().is_some()
}

/// A simple, concrete implementation of InputEventQueuer.
pub struct InputEventQueue {
    data: InputData,
}

impl InputEventQueue {
    pub fn new(in_fd: RawFd, blocking_query_timeout: Option<Duration>) -> Self {
        Self {
            data: InputData::new(in_fd, blocking_query_timeout),
        }
    }
}

impl InputEventQueuer for InputEventQueue {
    fn get_input_data(&self) -> &InputData {
        &self.data
    }

    fn get_input_data_mut(&mut self) -> &mut InputData {
        &mut self.data
    }

    #[cfg(unix)]
    fn select_interrupted(&mut self) {
        if reader_test_and_clear_interrupted() != 0 {
            self.enqueue_interrupt_key();
        }
    }

    #[cfg(windows)]
    fn select_interrupted(&mut self) {
        let controls = crate::signal::pending_console_controls();
        self.data.native_cancelled |= controls.ctrl_c != 0 || controls.ctrl_break != 0;
        if let Err(error) = self.data.native.interrupt() {
            self.input_error(error);
            self.push_front(CharEvent::from_check_exit());
            return;
        }
        if controls.close != 0 {
            crate::reader::native_console_closed();
        }
        // This queue has no parser. Its owner services actual handlers and
        // acknowledges/rearms before resuming, never erasing a receipt here.
        if stop_query(self.blocking_query_mut()) {
            self.push_front(CharEvent::QueryResult(QueryResultEvent::Interrupted));
        } else {
            self.push_front(CharEvent::from_check_exit());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CharEvent, InputEventQueue, InputEventQueuer as _, ReadlineCmd};

    #[cfg(windows)]
    #[test]
    fn native_query_yields_without_consuming_later_reader_input() {
        use super::{QueryResultEvent, TerminalQuery, stop_query};
        let mut queue = InputEventQueue::new(0, None);
        *queue.blocking_query_mut() = Some(TerminalQuery::Initial);
        queue.push_back(CharEvent::from_char('x'));
        assert!(matches!(
            queue.readch(),
            CharEvent::QueryResult(QueryResultEvent::Interrupted)
        ));
        assert!(stop_query(queue.blocking_query_mut()));
        assert_eq!(queue.readch().get_char(), 'x');
        assert!(!queue.has_lookahead());
    }

    #[test]
    fn test_push_front_back() {
        let mut queue = InputEventQueue::new(0, None);
        queue.push_front(CharEvent::from_char('a'));
        queue.push_front(CharEvent::from_char('b'));
        queue.push_back(CharEvent::from_char('c'));
        queue.push_back(CharEvent::from_char('d'));
        assert_eq!(queue.try_pop().unwrap().get_char(), 'b');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'a');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'c');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'd');
        assert!(queue.try_pop().is_none());
    }

    #[test]
    fn test_promote_interruptions_to_front() {
        let mut queue = InputEventQueue::new(0, None);
        queue.push_back(CharEvent::from_char('a'));
        queue.push_back(CharEvent::from_char('b'));
        queue.push_back(CharEvent::from_readline(ReadlineCmd::Undo, vec![]));
        queue.push_back(CharEvent::from_readline(ReadlineCmd::Redo, vec![]));
        queue.push_back(CharEvent::from_char('c'));
        queue.push_back(CharEvent::from_char('d'));
        queue.promote_interruptions_to_front();

        assert_eq!(queue.try_pop().unwrap().get_readline(), ReadlineCmd::Undo);
        assert_eq!(queue.try_pop().unwrap().get_readline(), ReadlineCmd::Redo);
        assert_eq!(queue.try_pop().unwrap().get_char(), 'a');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'b');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'c');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'd');
        assert!(!queue.has_lookahead());

        queue.push_back(CharEvent::from_char('e'));
        queue.promote_interruptions_to_front();
        assert_eq!(queue.try_pop().unwrap().get_char(), 'e');
        assert!(!queue.has_lookahead());
    }

    #[test]
    fn test_insert_front() {
        let mut queue = InputEventQueue::new(0, None);
        queue.push_back(CharEvent::from_char('a'));
        queue.push_back(CharEvent::from_char('b'));

        let events = vec![
            CharEvent::from_char('A'),
            CharEvent::from_char('B'),
            CharEvent::from_char('C'),
        ];
        queue.insert_front(events);
        assert_eq!(queue.try_pop().unwrap().get_char(), 'A');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'B');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'C');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'a');
        assert_eq!(queue.try_pop().unwrap().get_char(), 'b');
    }
}
