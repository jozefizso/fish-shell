//! Native completion codes are not POSIX wait statuses.

#[derive(Debug, Copy, Clone)]
enum Completion {
    Shell(u8),
    Native { raw: u32, failure: Option<u8> },
}

/// A synchronous shell result or an intact Windows process exit code.
#[derive(Default, Debug, Copy, Clone)]
pub struct ProcStatus(Option<Completion>);

impl ProcStatus {
    pub fn native(raw: u32) -> Self {
        Self(Some(Completion::Native { raw, failure: None }))
    }

    pub fn native_raw(&self) -> Option<u32> {
        match self.0 {
            Some(Completion::Native { raw, .. }) => Some(raw),
            _ => None,
        }
    }

    pub fn from_exit_code(code: i32) -> Self {
        Self(Some(Completion::Shell(
            u8::try_from(code).expect("Invalid shell exit code"),
        )))
    }

    /// The evaluator stopped after a native control request. This is a shell
    /// result, never an observed child signal or exit DWORD.
    pub fn interrupted() -> Self {
        Self::from_exit_code(130)
    }

    /// Preserve an observed native DWORD when a stage's own IO also fails.
    pub(crate) fn with_failure(self, code: i32) -> Self {
        if !self.is_empty() && !self.is_success() {
            return self;
        }
        let code = u8::try_from(code).expect("invalid shell failure code");
        match self.0 {
            Some(Completion::Native { raw, .. }) => Self(Some(Completion::Native {
                raw,
                failure: Some(code),
            })),
            _ => Self::from_exit_code(i32::from(code)),
        }
    }

    pub fn empty() -> Self {
        Self(None)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    pub fn normal_exited(&self) -> bool {
        self.0.is_some()
    }

    pub fn signal_exited(&self) -> bool {
        false
    }

    pub fn stopped(&self) -> bool {
        false
    }

    pub fn continued(&self) -> bool {
        false
    }

    pub fn signal_code(&self) -> i32 {
        panic!("A native completion is not a POSIX signal")
    }

    pub fn stop_signal(&self) -> i32 {
        panic!("Native batch processes cannot be signal-stopped")
    }

    pub fn exit_code(&self) -> u8 {
        match self.0.expect("An empty status has no exit code") {
            Completion::Shell(code) => code,
            Completion::Native { raw, failure } => {
                failure.unwrap_or_else(|| u8::try_from(raw).unwrap_or(1))
            }
        }
    }

    pub fn status_value(&self) -> i32 {
        i32::from(self.exit_code())
    }

    pub fn is_success(&self) -> bool {
        self.normal_exited() && self.exit_code() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::ProcStatus;

    #[test]
    fn native_exit_codes_keep_raw_bits_and_map_shell_status() {
        for (raw, expected) in [
            (0, 0),
            (7, 7),
            (255, 255),
            (130, 130),
            (256, 1),
            (0xC0000005, 1),
            (0xC000013A, 1),
            (0xFFFFFFFF, 1),
        ] {
            let status = ProcStatus::native(raw);
            assert_eq!(status.native_raw(), Some(raw));
            assert_eq!(status.status_value(), expected);
            assert_eq!(i32::from(status.exit_code()), expected);
            assert!(status.normal_exited());
            assert!(!status.signal_exited());
            assert!(!status.stopped());
            assert!(!status.continued());
            assert_eq!(status.is_success(), raw == 0);
        }
    }

    #[test]
    fn builtin_and_empty_statuses_are_not_native_exit_codes() {
        let builtin = ProcStatus::from_exit_code(7);
        assert_eq!(builtin.native_raw(), None);
        assert_eq!(builtin.status_value(), 7);
        assert!(builtin.normal_exited());
        let empty = ProcStatus::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.native_raw(), None);
        assert!(!empty.normal_exited());
        let interrupted = ProcStatus::interrupted();
        assert_eq!(interrupted.status_value(), 130);
        assert_eq!(interrupted.native_raw(), None);
        assert!(!interrupted.signal_exited());
        assert!(!empty.signal_exited());
    }

    #[test]
    fn io_failure_preserves_native_completion_and_prior_failure_precedence() {
        let successful_child = ProcStatus::native(0).with_failure(1);
        assert_eq!(successful_child.native_raw(), Some(0));
        assert_eq!(successful_child.status_value(), 1);
        assert!(!successful_child.is_success());
        assert_eq!(successful_child.with_failure(127).status_value(), 1);
        for raw in [7, 259, 0xC000013A, u32::MAX] {
            let child = ProcStatus::native(raw);
            let failed_io = child.with_failure(127);
            assert_eq!(failed_io.native_raw(), Some(raw));
            assert_eq!(failed_io.status_value(), child.status_value());
        }
        assert_eq!(ProcStatus::empty().with_failure(127).status_value(), 127);
    }
}
