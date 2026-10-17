//! The native batch runtime leaves console control events to Windows' default handler.
//! No Unix signal handlers, signal masks, or topic-monitor waiters are installed.
use crate::prelude::*;
use std::num::NonZeroI32;
use std::sync::atomic::{AtomicI32, Ordering};

/// Shared evaluator cancellation identity, never a Windows process exit code.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RawSignal(NonZeroI32);

impl RawSignal {
    pub const fn new(code: i32) -> Self {
        match NonZeroI32::new(code) {
            Some(code) => Self(code),
            None => panic!("Invalid zero signal value"),
        }
    }

    pub fn code(&self) -> i32 {
        self.0.get()
    }

    pub fn name(&self) -> &'static wstr {
        L!("Unsupported signal")
    }

    pub fn desc(&self) -> &'static wstr {
        L!("POSIX signals are unsupported in native Windows batch mode")
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
        usize::try_from(value.code()).expect("Negative signal code")
    }
}

impl From<RawSignal> for NonZeroI32 {
    fn from(value: RawSignal) -> Self {
        value.0
    }
}

// This is evaluator cancellation state, not an installed OS signal handler. Normal native
// Ctrl-C handling terminates the process and does not return to the evaluator.
static CANCELLATION_SIGNAL: AtomicI32 = AtomicI32::new(0);

pub fn signal_check_cancel() -> i32 {
    CANCELLATION_SIGNAL.load(Ordering::Relaxed)
}

pub fn signal_clear_cancel() {
    CANCELLATION_SIGNAL.store(0, Ordering::Relaxed);
}
