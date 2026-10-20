//! Configured defaults for future native child launch groups of one parser.
//!
//! These are not POSIX rlimits, effective ancestor limits, or limits on fish itself.
//! None means not configured; zero is invalid. Clearing a field removes only this
//! parser's default for subsequent groups. Existing groups retain their limits.
//! Defaults live for this parser's lifetime; descendants inherit enforced Job
//! restrictions, not this parser's configured-default inventory. Setters validate
//! a separate empty Job using the current token and change no policy on failure.
//! No extra privilege is requested: kernel policy/permission failures are errors.
//! Launch applies a snapshot before any child is prepared, and failure must abort
//! that launch rather than retrying with weaker restrictions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeLimits {
    /// Committed memory per process, in units of exactly 1024 bytes.
    pub process_memory_kib: Option<u64>,
    /// Aggregate committed memory of the group's processes, in 1024-byte units.
    pub job_memory_kib: Option<u64>,
    /// Per-process USER CPU seconds, not elapsed or combined user/kernel time.
    pub process_cpu_seconds: Option<u64>,
    /// Concurrent processes in this isolated Job, not a per-user process count.
    pub active_processes: Option<u64>,
}
