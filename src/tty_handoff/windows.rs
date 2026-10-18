//! Console ownership is a scoped input/mode lease, never terminal-pgroup control.
use super::{Outputter, TTY_INVALID, TTY_PROTOCOLS_ACTIVE, tty_protocols};
use crate::console_windows::{ConsoleSession, current_session};
use crate::global_safety::RelaxedAtomicBool;
use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;

// A failed partial write may already have enabled a protocol. Retain ownership
// until its disable sequence has actually completed; ACTIVE means success only.
static PROTOCOLS_OWNED: RelaxedAtomicBool = RelaxedAtomicBool::new(false);

pub(super) fn set_protocols_active(enable: bool) -> io::Result<()> {
    if (enable && TTY_PROTOCOLS_ACTIVE.load())
        || (!enable && !TTY_PROTOCOLS_ACTIVE.load() && !PROTOCOLS_OWNED.load())
    {
        return Ok(());
    }
    crate::threads::assert_is_main_thread();
    if TTY_INVALID.load() {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "terminal is closed",
        ));
    }
    let protocols = tty_protocols().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotConnected,
            "terminal protocols have not been initialized",
        )
    })?;
    if enable {
        PROTOCOLS_OWNED.store(true);
    }
    let mut rendered = Outputter::stdoutput().borrow_mut().flush();
    if enable {
        rendered?;
        rendered = Ok(());
    }
    // Disabling must remain possible after a sticky screen write failure.
    // Never clear that failure or flush its unsent tail during cleanup.
    current_session()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "no console session"))?
        .write_protocol(protocols.get_commands(enable))?;
    TTY_PROTOCOLS_ACTIVE.store(enable);
    PROTOCOLS_OWNED.store(enable);
    if enable { Ok(()) } else { rendered }
}

struct Snapshot {
    session: Rc<ConsoleSession>,
    editor: bool,
    paused: bool,
    protocols: bool,
}
impl Snapshot {
    fn capture(session: Rc<ConsoleSession>) -> Self {
        Self {
            editor: session.editor_modes_active(),
            paused: session.input_paused(),
            protocols: TTY_PROTOCOLS_ACTIVE.load(),
            session,
        }
    }

    fn restore(&self) -> io::Result<()> {
        if native_input_is_donated() && (self.editor || self.protocols || !self.paused) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "cannot restore editor ownership while a native child is live",
            ));
        }
        let mut first = self.session.pause_input().err();
        let mut record = |result: io::Result<()>| {
            if let Err(error) = result {
                if first.is_none() {
                    first = Some(error);
                }
            }
        };
        // Disable before cooked modes; enable only after VT editor modes are
        // restored. Attempt mode cleanup even when protocol cleanup failed.
        if !self.protocols {
            record(set_protocols_active(false));
        }
        let modes = if self.editor {
            self.session.enter_editor_modes()
        } else {
            self.session.restore_modes()
        };
        let modes_restored = modes.is_ok();
        record(modes);
        if self.protocols && modes_restored {
            record(set_protocols_active(true));
        }
        drop(record);
        if first.is_none() && !self.paused {
            self.session.resume_input()?;
        }
        crate::screen::invalidate_native_screen();
        match first {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[derive(Default)]
struct Donations {
    depth: usize,
    editors: usize,
    saved: Option<Snapshot>,
    shutting_down: bool,
}
thread_local! {
    static DONATIONS: RefCell<Donations> = RefCell::new(Donations::default());
}

pub fn native_input_is_donated() -> bool {
    DONATIONS.with(|state| state.borrow().depth != 0)
}
pub(super) fn native_shutting_down() -> bool {
    DONATIONS.with(|state| state.borrow().shutting_down)
}
/// Called before releasing live jobs on shell exit. Their last lease must not
/// restart an editor after the shell has chosen to leave accepted children alive.
pub fn native_begin_shutdown() {
    DONATIONS.with(|state| state.borrow_mut().shutting_down = true);
}

/// Enter a real reader/tool without changing the surrounding reader's ownership.
/// The process main installs one ConsoleSession; nested scopes never spawn readers.
pub struct NativeEditorScope {
    saved: Option<Snapshot>,
    admitted: bool,
    _main_thread: PhantomData<Rc<()>>,
}
impl NativeEditorScope {
    pub fn enter() -> io::Result<Self> {
        if native_input_is_donated() || native_shutting_down() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "console input belongs to a live native child",
            ));
        }
        let session = current_session().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "interactive input and output require native consoles",
            )
        })?;
        let mut scope = Self {
            saved: Some(Snapshot::capture(Rc::clone(&session))),
            admitted: false,
            _main_thread: PhantomData,
        };
        session.pause_input()?;
        session.enter_editor_modes()?;
        // No actual read is submitted until the owner asks for input (including
        // bounded startup queries, which run before the first readline).
        session.resume_input()?;
        DONATIONS.with(|state| -> io::Result<()> {
            let mut state = state.borrow_mut();
            state.editors = state
                .editors
                .checked_add(1)
                .ok_or_else(|| io::Error::other("native editor scope count overflow"))?;
            Ok(())
        })?;
        scope.admitted = true;
        Ok(scope)
    }
    pub fn enable_protocols(&mut self) -> io::Result<()> {
        if native_input_is_donated() || native_shutting_down() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "console input belongs to a live native child",
            ));
        }
        set_protocols_active(true)
    }
    pub fn disable_protocols(&mut self) -> io::Result<()> {
        set_protocols_active(false)
    }
    pub fn finish(mut self) -> io::Result<()> {
        self.restore()
    }
    fn restore(&mut self) -> io::Result<()> {
        let Some(saved) = &self.saved else {
            return Ok(());
        };
        let result = saved.restore();
        if self.admitted {
            DONATIONS.with(|state| {
                let mut state = state.borrow_mut();
                state.editors = state
                    .editors
                    .checked_sub(1)
                    .expect("native editor scope underflow");
            });
            self.admitted = false;
        }
        result?;
        self.saved.take();
        Ok(())
    }
}
impl Drop for NativeEditorScope {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            crate::reader::native_input_failed(&error);
        }
    }
}

/// Stop editor transport/protocols before evaluator code can create children.
/// This is not a child lease: an ordinary function may still invoke `read`.
pub fn native_pause_for_evaluation() -> io::Result<()> {
    let Some(session) = current_session() else {
        return Ok(());
    };
    session.pause_input()?;
    Outputter::stdoutput().borrow_mut().flush()?;
    set_protocols_active(false)?;
    session.restore_modes()?;
    crate::screen::invalidate_native_screen();
    Ok(())
}
pub fn native_resume_editor_modes() -> io::Result<()> {
    if native_input_is_donated() || native_shutting_down() {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "console input still belongs to a live native child",
        ));
    }
    if let Some(session) = current_session() {
        session.pause_input()?;
        session.enter_editor_modes()?;
    }
    crate::screen::invalidate_native_screen();
    Ok(())
}

/// Stored in the native Job until its actual process/writer completion, not in a
/// foreground-wait stack frame that can return early on cooperative cancellation.
pub struct NativeChildDonation {
    active: bool,
    _main_thread: PhantomData<Rc<()>>,
}
impl NativeChildDonation {
    pub fn acquire() -> io::Result<Self> {
        let capture = DONATIONS.with(|state| -> io::Result<Option<(bool, usize)>> {
            let state = state.borrow();
            if state.editors == 0 || state.shutting_down {
                return Ok(None);
            }
            let next = state
                .depth
                .checked_add(1)
                .ok_or_else(|| io::Error::other("native console lease count overflow"))?;
            Ok(Some((state.depth == 0, next)))
        })?;
        let Some((first, next_depth)) = capture else {
            return Ok(Self {
                active: false,
                _main_thread: PhantomData,
            });
        };
        let session = current_session().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "native editor lost its console session",
            )
        })?;
        session.ensure_external_input_boundary()?;
        let saved = if first {
            let saved = Snapshot::capture(session);
            if let Err(error) = native_pause_for_evaluation() {
                if let Err(restore_error) = saved.restore() {
                    crate::reader::native_input_failed(&restore_error);
                }
                return Err(error);
            }
            Some(saved)
        } else {
            None
        };
        DONATIONS.with(|state| {
            let mut state = state.borrow_mut();
            if let Some(saved) = saved {
                state.saved = Some(saved);
            }
            state.depth = next_depth;
        });
        Ok(Self {
            active: true,
            _main_thread: PhantomData,
        })
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
}
impl Drop for NativeChildDonation {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let restore = DONATIONS.with(|state| {
            let mut state = state.borrow_mut();
            state.depth = state
                .depth
                .checked_sub(1)
                .expect("native console lease underflow");
            if state.depth != 0 {
                return None;
            }
            let saved = state.saved.take();
            if state.shutting_down { None } else { saved }
        });
        if let Some(saved) = restore {
            if let Err(error) = saved.restore() {
                crate::reader::native_input_failed(&error);
            }
        }
    }
}
