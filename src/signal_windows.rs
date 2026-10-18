//! Owned native console controls, not POSIX signals or native process exit codes.
//!
//! The Windows callback only counts receipts, acquires a bounded atomic handle
//! lease and sets events. Evaluator callbacks run later through `event` on the
//! main thread. A receipt requests cancellation; it never observes child exit.
//!
//! Ctrl-C and Ctrl-Break each request evaluator interruption and wake native
//! waits; receipt never forwards a console broadcast or escalates termination.
//! Targeted child Ctrl-Break eligibility/policy belongs to the process owner.
//! Console close is only a best-effort notification: Windows can terminate the
//! shell on callback return, so it cannot promise graceful cleanup or delivery
//! of a user function. Logoff/shutdown and POSIX signal names are unsupported.
use crate::prelude::*;
use crate::threads::NativeCancellation;
use std::ffi::c_void;
use std::io;
use std::num::NonZeroI32;
use std::os::windows::io::{
    AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle,
};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

const WAIT_OBJECT_0: u32 = 0;
#[cfg(test)]
const WAIT_TIMEOUT: u32 = 258;
const INFINITE: u32 = u32::MAX;
const CLOSED: usize = 1 << (usize::BITS - 1);
const CALLBACK_COUNT_MASK: usize = (1 << 16) - 1;
const CALLBACK_GENERATION_STEP: usize = 1 << 16;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const c_void,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> RawHandle;
    fn SetEvent(event: RawHandle) -> i32;
    fn ResetEvent(event: RawHandle) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}

/// Internal control identity. These nonzero codes are neither Windows control
/// DWORDs (CTRL_C_EVENT is zero) nor POSIX signal numbers.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RawSignal(NonZeroI32);

impl RawSignal {
    pub const CTRL_C: Self = Self::new(1);
    pub const CTRL_BREAK: Self = Self::new(2);
    pub const CTRL_CLOSE: Self = Self::new(3);

    pub const fn new(code: i32) -> Self {
        match NonZeroI32::new(code) {
            Some(code) => Self(code),
            None => panic!("Invalid zero control value"),
        }
    }

    pub fn code(&self) -> i32 {
        self.0.get()
    }

    pub fn name(&self) -> &'static wstr {
        match *self {
            Self::CTRL_C => L!("CTRL_C"),
            Self::CTRL_BREAK => L!("CTRL_BREAK"),
            Self::CTRL_CLOSE => L!("CTRL_CLOSE"),
            _ => L!("Unsupported native control"),
        }
    }

    pub fn desc(&self) -> &'static wstr {
        match *self {
            Self::CTRL_C => L!("Console interrupt cancellation request"),
            Self::CTRL_BREAK => L!("Console break cancellation request"),
            Self::CTRL_CLOSE => L!("Console closing; best-effort shutdown notification"),
            _ => L!("POSIX signals are unsupported in native Windows mode"),
        }
    }

    /// Only native control names are accepted, case-insensitively, optionally
    /// with the Windows `_EVENT` suffix. No POSIX aliases or numeric signals.
    pub fn parse(name: &wstr) -> Option<Self> {
        [
            (Self::CTRL_C, L!("CTRL_C_EVENT")),
            (Self::CTRL_BREAK, L!("CTRL_BREAK_EVENT")),
            (Self::CTRL_CLOSE, L!("CTRL_CLOSE_EVENT")),
        ]
        .into_iter()
        .find_map(|(control, event_name)| {
            let matches = |candidate: &wstr| {
                candidate.len() == name.len()
                    && candidate
                        .chars()
                        .zip(name.chars())
                        .all(|(a, b)| a.eq_ignore_ascii_case(&b))
            };
            (matches(control.name()) || matches(event_name)).then_some(control)
        })
    }
}

impl PartialEq<i32> for RawSignal {
    fn eq(&self, other: &i32) -> bool {
        self.code() == *other
    }
}

impl From<RawSignal> for i32 {
    fn from(value: RawSignal) -> Self {
        value.code()
    }
}

impl From<RawSignal> for usize {
    fn from(value: RawSignal) -> Self {
        usize::try_from(value.code()).expect("Negative control code")
    }
}

impl From<RawSignal> for NonZeroI32 {
    fn from(value: RawSignal) -> Self {
        value.0
    }
}

/// Per-kind receipt epochs, retained independently of cancellation acknowledgement
/// and event dispatch. Counts wrap only after 2^64 receipts of the same kind.
/// Cross-kind ordering is deliberately not represented.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ConsoleControlEpoch {
    pub ctrl_c: u64,
    pub ctrl_break: u64,
    pub close: u64,
}

impl ConsoleControlEpoch {
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }

    fn difference(self, earlier: Self) -> Self {
        Self {
            ctrl_c: self.ctrl_c.wrapping_sub(earlier.ctrl_c),
            ctrl_break: self.ctrl_break.wrapping_sub(earlier.ctrl_break),
            close: self.close.wrapping_sub(earlier.close),
        }
    }

    /// One entry per supported kind; counts are not coalesced into a single event.
    pub fn controls(self) -> [(RawSignal, u64); 3] {
        [
            (RawSignal::CTRL_C, self.ctrl_c),
            (RawSignal::CTRL_BREAK, self.ctrl_break),
            (RawSignal::CTRL_CLOSE, self.close),
        ]
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

struct ControlEvent(OwnedHandle);

impl ControlEvent {
    fn new() -> io::Result<Self> {
        let handle = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(handle) }))
    }

    fn set(&self) -> io::Result<()> {
        if unsafe { SetEvent(self.0.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn reset(&self) -> io::Result<()> {
        if unsafe { ResetEvent(self.0.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn wait(&self) -> io::Result<()> {
        match unsafe { WaitForSingleObject(self.0.as_raw_handle(), INFINITE) } {
            WAIT_OBJECT_0 => Ok(()),
            u32::MAX => Err(io::Error::last_os_error()),
            result => Err(io::Error::other(format!(
                "unexpected console wait result {result}"
            ))),
        }
    }
}

/// All callback-accessed storage is static. Handles have an explicit, separately
/// drained owner; no callback ever follows an Arc, mutex, or parser pointer.
struct ControlHandoff {
    active: AtomicBool,
    gate: AtomicUsize,
    received: [AtomicU64; 3],
    acknowledged: [AtomicU64; 3],
    wake: AtomicPtr<c_void>,
    cancellation: AtomicPtr<c_void>,
    drained: AtomicPtr<c_void>,
    failed: AtomicBool,
}

impl ControlHandoff {
    const fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            gate: AtomicUsize::new(CLOSED),
            received: [const { AtomicU64::new(0) }; 3],
            acknowledged: [const { AtomicU64::new(0) }; 3],
            wake: AtomicPtr::new(ptr::null_mut()),
            cancellation: AtomicPtr::new(ptr::null_mut()),
            drained: AtomicPtr::new(ptr::null_mut()),
            failed: AtomicBool::new(false),
        }
    }

    fn epoch(counters: &[AtomicU64; 3]) -> ConsoleControlEpoch {
        ConsoleControlEpoch {
            ctrl_c: counters[0].load(Ordering::Acquire),
            ctrl_break: counters[1].load(Ordering::Acquire),
            close: counters[2].load(Ordering::Acquire),
        }
    }

    fn pending(&self) -> ConsoleControlEpoch {
        // Read acknowledgements first: a concurrent rearm must not appear newer
        // than the receipt snapshot and produce a wrapping, fictitious delta.
        let acknowledged = Self::epoch(&self.acknowledged);
        Self::epoch(&self.received).difference(acknowledged)
    }

    fn acknowledge(&self, epoch: ConsoleControlEpoch) {
        for (counter, (_, count)) in self.acknowledged.iter().zip(epoch.controls()) {
            counter.store(count, Ordering::Release);
        }
    }

    /// One CAS, never a retry/spin loop. A failed acquisition means either a
    /// concurrent callback has set/will set the permanent cancellation token, or
    /// the normal-thread rearm/shutdown barrier is responsible for reconciliation.
    fn try_enter(&self, gate: usize) -> bool {
        gate & CLOSED == 0
            && gate & CALLBACK_COUNT_MASK != CALLBACK_COUNT_MASK
            && self
                .gate
                .compare_exchange(gate, gate + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }

    fn open(&self) {
        // Change generation on every publication so a callback paused before
        // its single CAS cannot acquire a replacement token through an ABA.
        let generation = (self.gate.load(Ordering::Acquire) & !CALLBACK_COUNT_MASK)
            .wrapping_add(CALLBACK_GENERATION_STEP)
            & !CLOSED;
        self.gate.store(generation, Ordering::Release);
    }

    fn leave(&self) {
        if self.gate.fetch_sub(1, Ordering::AcqRel) & (CLOSED | CALLBACK_COUNT_MASK) == CLOSED | 1 {
            // Final handle use by the final admitted callback. The waiter cannot
            // return before SetEvent has referenced/signaled this kernel object.
            self.set_callback_event(&self.drained);
        }
    }

    fn set_callback_event(&self, event: &AtomicPtr<c_void>) {
        if unsafe { SetEvent(event.load(Ordering::Acquire)) } == 0 {
            self.failed.store(true, Ordering::Release);
        }
    }

    fn receive(&self, control: u32) -> i32 {
        let index = match control {
            0..=2 => control as usize,
            // Logoff/shutdown are not promised by the console API for GUI DLL
            // consumers. Leave unsupported controls to subsequent/default handlers.
            _ => return 0,
        };
        if !self.active.load(Ordering::Acquire) {
            return 0;
        }
        self.received[index].fetch_add(1, Ordering::Release);
        let gate = self.gate.load(Ordering::Acquire);
        self.finish_receipt(gate);
        // C and Break are handled cancellation requests, not process termination.
        // For Close, Windows may terminate us as soon as this callback returns;
        // recording/waking is best effort, not a veto or a graceful-exit promise.
        1
    }

    fn finish_receipt(&self, gate: usize) {
        if self.try_enter(gate) {
            if !self.pending().is_empty() {
                self.set_callback_event(&self.cancellation);
            }
            self.set_callback_event(&self.wake);
            self.leave();
        }
    }

    fn close_and_drain(&self, drained: &ControlEvent) -> io::Result<()> {
        drained.reset()?;
        let previous = self.gate.fetch_or(CLOSED, Ordering::AcqRel);
        if previous & CALLBACK_COUNT_MASK != 0 {
            drained.wait()?;
        }
        Ok(())
    }
}

static HANDOFF: ControlHandoff = ControlHandoff::new();
static OWNER: Mutex<Option<Arc<ConsoleControlState>>> = Mutex::new(None);
static DELIVERED: Mutex<ConsoleControlEpoch> = Mutex::new(ConsoleControlEpoch {
    ctrl_c: 0,
    ctrl_break: 0,
    close: 0,
});

struct ConsoleControlState {
    wake: ControlEvent,
    drained: ControlEvent,
    cancellation: Mutex<NativeCancellation>,
}

impl ConsoleControlState {
    fn reconcile(&self, cancellation: &NativeCancellation) -> io::Result<()> {
        if !HANDOFF.pending().is_empty() {
            cancellation.cancel();
        }
        if cancellation.is_cancelled() || console_control_epoch() != *lock(&DELIVERED) {
            self.wake.set()?;
        }
        self.check_health()
    }

    fn check_health(&self) -> io::Result<()> {
        if HANDOFF.failed.load(Ordering::Acquire) {
            Err(io::Error::other("console control SetEvent handoff failed"))
        } else {
            Ok(())
        }
    }
}

/// Startup/shutdown owner. Keep alive until native work and main-thread event
/// dispatch have drained. No console attachment changes are supported while
/// installed: AttachConsole/AllocConsole/FreeConsole reset Windows' handler list.
///
/// Drop unregisters and drains; an unregistration/drain failure is fatal rather
/// than freeing callback-referenced handles. Use `shutdown` to handle errors.
pub struct ConsoleControlGuard {
    state: Arc<ConsoleControlState>,
    registered: bool,
    closed: bool,
}

pub fn initialize_console_controls() -> io::Result<ConsoleControlGuard> {
    let mut owner = lock(&OWNER);
    if owner.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "console controls already initialized",
        ));
    }
    let state = Arc::new(ConsoleControlState {
        wake: ControlEvent::new()?,
        drained: ControlEvent::new()?,
        cancellation: Mutex::new(NativeCancellation::new()?),
    });
    let initial = console_control_epoch();
    HANDOFF.acknowledge(initial);
    *lock(&DELIVERED) = initial;
    HANDOFF.failed.store(false, Ordering::Release);
    HANDOFF
        .wake
        .store(state.wake.0.as_raw_handle(), Ordering::Release);
    HANDOFF
        .drained
        .store(state.drained.0.as_raw_handle(), Ordering::Release);
    HANDOFF.cancellation.store(
        lock(&state.cancellation).handle().as_raw_handle(),
        Ordering::Release,
    );
    HANDOFF.active.store(true, Ordering::Release);
    if unsafe { SetConsoleCtrlHandler(Some(console_control_handler), 1) } == 0 {
        HANDOFF.active.store(false, Ordering::Release);
        let error = io::Error::last_os_error();
        HANDOFF.wake.store(ptr::null_mut(), Ordering::Release);
        HANDOFF
            .cancellation
            .store(ptr::null_mut(), Ordering::Release);
        HANDOFF.drained.store(ptr::null_mut(), Ordering::Release);
        return Err(error);
    }
    HANDOFF.open();
    *owner = Some(Arc::clone(&state));
    drop(owner);
    let mut guard = ConsoleControlGuard {
        state,
        registered: true,
        closed: false,
    };
    // CREATE_NEW_PROCESS_GROUP launchers may pass an inherited ignore-Ctrl-C
    // attribute. Installing a handler alone does not clear it. This process
    // explicitly adopts console interrupts after its handler/gate are ready.
    if unsafe { SetConsoleCtrlHandler(None, 0) } == 0 {
        let error = io::Error::last_os_error();
        guard.shutdown()?;
        return Err(error);
    }
    // Controls arriving between registration and gate publication are counted.
    let reconciled = guard.state.reconcile(&lock(&guard.state.cancellation));
    if let Err(error) = reconciled {
        guard.shutdown()?;
        return Err(error);
    }
    Ok(guard)
}

impl ConsoleControlGuard {
    /// Borrowed real manual-reset notification, suitable for a native wait set.
    /// It remains set while cancellation is unacknowledged; do not reset it.
    pub fn wake_handle(&self) -> BorrowedHandle<'_> {
        self.state.wake.0.as_handle()
    }

    /// Permanent token for this cancellation epoch. Capture anew after rearm;
    /// an older clone intentionally remains cancelled forever.
    pub fn cancellation(&self) -> NativeCancellation {
        lock(&self.state.cancellation).clone()
    }

    pub fn check_health(&self) -> io::Result<()> {
        self.state.check_health()
    }

    /// Acknowledge only controls already present at entry, replacing rather than
    /// resetting the permanent token. A racing receipt is retained and signals
    /// the new token. Native child completion/status is never changed here.
    pub fn rearm(&self) -> io::Result<()> {
        if self.closed || !self.registered {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "console controls shut down",
            ));
        }
        rearm_console_controls()
    }

    pub fn shutdown(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        let mut owner = lock(&OWNER);
        // On failure retain the registered owner and all callback handles.
        if self.registered {
            if unsafe { SetConsoleCtrlHandler(Some(console_control_handler), 0) } == 0 {
                return Err(io::Error::last_os_error());
            }
            self.registered = false;
        }
        HANDOFF.active.store(false, Ordering::Release);
        HANDOFF.close_and_drain(&self.state.drained)?;
        HANDOFF.wake.store(ptr::null_mut(), Ordering::Release);
        HANDOFF
            .cancellation
            .store(ptr::null_mut(), Ordering::Release);
        HANDOFF.drained.store(ptr::null_mut(), Ordering::Release);
        *owner = None;
        self.closed = true;
        self.check_health()
    }
}

impl Drop for ConsoleControlGuard {
    fn drop(&mut self) {
        if self.shutdown().is_err() {
            std::process::abort();
        }
    }
}

unsafe extern "system" fn console_control_handler(control: u32) -> i32 {
    HANDOFF.receive(control)
}

pub fn console_control_epoch() -> ConsoleControlEpoch {
    ControlHandoff::epoch(&HANDOFF.received)
}

pub fn pending_console_controls() -> ConsoleControlEpoch {
    HANDOFF.pending()
}

/// Clone usable directly by NativeChild::wait/NativeRuntime::wait. None means
/// startup has not installed the owner; it is not a successful fake notifier.
pub fn console_control_cancellation() -> Option<NativeCancellation> {
    lock(&OWNER)
        .as_ref()
        .map(|owner| lock(&owner.cancellation).clone())
}

pub fn signal_check_cancel() -> i32 {
    let pending = pending_console_controls();
    if pending.close != 0 {
        RawSignal::CTRL_CLOSE.code()
    } else if pending.ctrl_break != 0 {
        RawSignal::CTRL_BREAK.code()
    } else if pending.ctrl_c != 0 {
        RawSignal::CTRL_C.code()
    } else {
        0
    }
}

/// Checked normal-thread rearm. Allocation failure preserves the old token and
/// unacknowledged requests. Synchronization is never performed in the callback.
pub fn rearm_console_controls() -> io::Result<()> {
    let owner = lock(&OWNER);
    let Some(owner) = owner.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "console controls not initialized",
        ));
    };
    if !HANDOFF.active.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "console controls shutting down",
        ));
    }
    let cutoff = console_control_epoch();
    let mut current = lock(&owner.cancellation);
    if HANDOFF.pending().is_empty() && !current.is_cancelled() {
        return owner.check_health();
    }
    let next = NativeCancellation::new()?;
    HANDOFF.close_and_drain(&owner.drained)?;
    // A receipt can precede admission. Retire every outgoing waiter's epoch
    // before acknowledging it, even if that callback has not signaled a handle.
    current.cancel();
    HANDOFF.acknowledge(cutoff);
    HANDOFF
        .cancellation
        .store(next.handle().as_raw_handle(), Ordering::Release);
    *current = next;
    HANDOFF.open();
    owner.reconcile(&current)
}

/// Shared evaluator compatibility entry point. A failed rearm does not clear
/// cancellation. Startup/command boundaries should use the checked owner API.
pub fn signal_clear_cancel() {
    if let Err(error) = rearm_console_controls() {
        if error.kind() != io::ErrorKind::NotConnected {
            eprintln!("fish: cannot rearm console controls: {error}");
        }
    }
}

/// Transfer counted receipts to the event subsystem, on its owning main thread.
/// Acknowledgement/rearm never consumes these counts, and dispatch never clears
/// cancellation. The wake reset/recheck cannot erase a racing callback receipt.
pub(crate) fn take_pending_console_controls() -> io::Result<ConsoleControlEpoch> {
    crate::threads::assert_is_main_thread();
    let owner = lock(&OWNER);
    let mut delivered = lock(&DELIVERED);
    let now = console_control_epoch();
    let previous = *delivered;
    let pending = now.difference(*delivered);
    *delivered = now;
    drop(delivered);
    if let Some(owner) = owner.as_ref() {
        let result = owner
            .wake
            .reset()
            .and_then(|()| owner.reconcile(&lock(&owner.cancellation)));
        if let Err(error) = result {
            *lock(&DELIVERED) = previous;
            return Err(error);
        }
    }
    Ok(pending)
}

/// Shared wait-builtin interruption detector. Rearm cannot erase its history.
pub struct SigChecker {
    epoch: ConsoleControlEpoch,
}

impl SigChecker {
    pub fn new_sighupintterm() -> Self {
        Self {
            epoch: console_control_epoch(),
        }
    }

    pub fn check(&mut self) -> bool {
        let epoch = console_control_epoch();
        let changed = epoch != self.epoch;
        self.epoch = epoch;
        changed
    }

    /// Block on the current real, permanent cancellation event, including an
    /// already-pending request. Returns an error without an installed owner;
    /// never polls or allocates a fake wake source. Only `check` tests freshness.
    pub fn wait(&self) -> io::Result<()> {
        let cancellation = console_control_cancellation().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "console controls not initialized",
            )
        })?;
        if console_control_epoch() != self.epoch {
            return Ok(());
        }
        match unsafe { WaitForSingleObject(cancellation.handle().as_raw_handle(), INFINITE) } {
            WAIT_OBJECT_0 => Ok(()),
            u32::MAX => Err(io::Error::last_os_error()),
            result => Err(io::Error::other(format!(
                "unexpected console wait result {result}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_native_controls_not_posix_signals() {
        assert_eq!(RawSignal::parse(L!("ctrl_c")), Some(RawSignal::CTRL_C));
        assert_eq!(
            RawSignal::parse(L!("CTRL_BREAK_EVENT")),
            Some(RawSignal::CTRL_BREAK)
        );
        assert_eq!(
            RawSignal::parse(L!("CTRL_CLOSE")),
            Some(RawSignal::CTRL_CLOSE)
        );
        for unsupported in [
            L!("SIGINT"),
            L!("INT"),
            L!("SIGTERM"),
            L!("2"),
            L!("CTRL_LOGOFF"),
        ] {
            assert_eq!(RawSignal::parse(unsupported), None);
        }
    }

    #[test]
    fn handoff_counts_receipts_and_preserves_permanent_tokens() {
        let handoff = ControlHandoff::new();
        let wake = ControlEvent::new().unwrap();
        let drained = ControlEvent::new().unwrap();
        let first = NativeCancellation::new().unwrap();
        handoff
            .wake
            .store(wake.0.as_raw_handle(), Ordering::Release);
        handoff
            .drained
            .store(drained.0.as_raw_handle(), Ordering::Release);
        handoff
            .cancellation
            .store(first.handle().as_raw_handle(), Ordering::Release);
        handoff.active.store(true, Ordering::Release);
        handoff.gate.store(0, Ordering::Release);
        assert_eq!(handoff.receive(0), 1);
        assert_eq!(handoff.receive(0), 1);
        assert!(first.is_cancelled());
        assert_eq!(ControlHandoff::epoch(&handoff.received).ctrl_c, 2);
        assert_eq!(
            unsafe { WaitForSingleObject(wake.0.as_raw_handle(), 0) },
            WAIT_OBJECT_0
        );
        let cutoff = ControlHandoff::epoch(&handoff.received);
        handoff.close_and_drain(&drained).unwrap();
        // Deterministically place a receipt inside the closed rearm interval.
        assert_eq!(handoff.receive(1), 1);
        let next = NativeCancellation::new().unwrap();
        handoff.acknowledge(cutoff);
        handoff
            .cancellation
            .store(next.handle().as_raw_handle(), Ordering::Release);
        handoff.open();
        if !handoff.pending().is_empty() {
            next.cancel();
        }
        assert!(first.is_cancelled());
        assert!(next.is_cancelled());
        assert_eq!(
            handoff.pending(),
            ConsoleControlEpoch {
                ctrl_break: 1,
                ..Default::default()
            }
        );
        handoff.close_and_drain(&drained).unwrap();
        handoff.active.store(false, Ordering::Release);
        assert_eq!(handoff.receive(0), 0);
        assert_eq!(handoff.receive(5), 0);
    }

    #[test]
    fn closed_gate_waits_for_admitted_callback_handle_lease() {
        let handoff = Arc::new(ControlHandoff::new());
        let drained = Arc::new(ControlEvent::new().unwrap());
        handoff
            .drained
            .store(drained.0.as_raw_handle(), Ordering::Release);
        handoff.gate.store(0, Ordering::Release);
        assert!(handoff.try_enter(handoff.gate.load(Ordering::Acquire)));
        // Close after admission but before leave, then test the actual kernel
        // wake rather than a separate mock notifier or timing-based sleep.
        drained.reset().unwrap();
        handoff.gate.fetch_or(CLOSED, Ordering::AcqRel);
        assert!(!handoff.try_enter(handoff.gate.load(Ordering::Acquire)));
        assert_eq!(
            unsafe { WaitForSingleObject(drained.0.as_raw_handle(), 0) },
            WAIT_TIMEOUT
        );
        let callback = Arc::clone(&handoff);
        let thread = std::thread::spawn(move || callback.leave());
        drained.wait().unwrap();
        thread.join().unwrap();
        assert!(!handoff.failed.load(Ordering::Acquire));
    }

    #[test]
    #[serial_test::serial(native_console_owner)]
    fn registered_owner_rearms_and_unregisters_real_events() {
        let mut guard = initialize_console_controls().unwrap();
        assert_eq!(
            initialize_console_controls().err().unwrap().kind(),
            io::ErrorKind::AlreadyExists
        );
        let first = guard.cancellation();
        let mut checker = SigChecker::new_sighupintterm();
        assert!(!first.is_cancelled());
        assert_eq!(unsafe { console_control_handler(0) }, 1);
        assert!(first.is_cancelled());
        assert!(checker.check());
        assert!(!checker.check());
        assert_eq!(signal_check_cancel(), RawSignal::CTRL_C.code());
        guard.rearm().unwrap();
        assert!(first.is_cancelled());
        assert!(!guard.cancellation().is_cancelled());
        assert_eq!(signal_check_cancel(), 0);
        assert!(!checker.check());
        guard.check_health().unwrap();
        guard.shutdown().unwrap();
        assert!(console_control_cancellation().is_none());
        let stopped = console_control_epoch();
        assert_eq!(unsafe { console_control_handler(1) }, 0);
        assert_eq!(console_control_epoch(), stopped);
        assert_eq!(
            guard.rearm().unwrap_err().kind(),
            io::ErrorKind::NotConnected
        );
        guard.shutdown().unwrap();
    }

    #[test]
    #[serial_test::serial(native_console_owner)]
    fn retiring_receipt_epoch_cancels_old_not_replacement() {
        if !crate::threads::isolated_test(concat!(
            module_path!(),
            "::retiring_receipt_epoch_cancels_old_not_replacement"
        )) {
            return;
        }
        for pause_after_gate_load in [false, true] {
            let mut guard = initialize_console_controls().unwrap();
            let outgoing = guard.cancellation();
            // The callback has published a real receipt but has not yet used
            // any owned handle. Continue the same bounded wake stage after rearm.
            HANDOFF.received[0].fetch_add(1, Ordering::Release);
            let captured = pause_after_gate_load.then(|| HANDOFF.gate.load(Ordering::Acquire));
            guard.rearm().unwrap();
            assert!(
                outgoing.is_cancelled(),
                "retired waiters lost their cancellation receipt"
            );
            let replacement = guard.cancellation();
            let gate = captured.unwrap_or_else(|| HANDOFF.gate.load(Ordering::Acquire));
            HANDOFF.finish_receipt(gate);
            assert!(
                !replacement.is_cancelled(),
                "acknowledged receipt canceled replacement work"
            );
            assert!(pending_console_controls().is_empty());
            assert_eq!(take_pending_console_controls().unwrap().ctrl_c, 1);
            assert!(take_pending_console_controls().unwrap().is_empty());
            guard.shutdown().unwrap();
        }
    }
}
