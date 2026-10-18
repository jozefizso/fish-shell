//! Checked native CPU and resource accounting; no POSIX counters are synthesized.
//!
//! Process elapsed time uses the OS creation and exit/current FILETIME timestamps.
//! Job counters include all members (including nested descendants) attributed by
//! Windows to that Job Object. Launch jobs registered here must be disjoint: do
//! not register an enclosing host job or also add individual root process CPU.
//! Tracking ends at `TrackedJob::finish` (normally last root completion), not at
//! eventual descendant exit. Descendant work after that cutoff is excluded.

use std::collections::HashMap;
use std::ffi::c_void;
use std::io::{self, Write};
use std::mem::size_of;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, OwnedHandle, RawHandle};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuTimes {
    pub user: Duration,
    pub kernel: Duration,
}

impl CpuTimes {
    pub fn checked_add(self, other: Self) -> io::Result<Self> {
        Ok(Self {
            user: self
                .user
                .checked_add(other.user)
                .ok_or_else(counter_overflow)?,
            kernel: self
                .kernel
                .checked_add(other.kernel)
                .ok_or_else(counter_overflow)?,
        })
    }

    /// A decreasing native counter is an error, not fictitious zero usage.
    pub fn checked_sub(self, earlier: Self) -> io::Result<Self> {
        Ok(Self {
            user: self
                .user
                .checked_sub(earlier.user)
                .ok_or_else(counter_regressed)?,
            kernel: self
                .kernel
                .checked_sub(earlier.kernel)
                .ok_or_else(counter_regressed)?,
        })
    }
}

/// All timestamps are durations since the Windows epoch (1601-01-01 UTC).
#[derive(Clone, Copy, Debug)]
pub struct ProcessTimes {
    pub cpu: CpuTimes,
    pub created: Duration,
    pub exited: Option<Duration>,
    pub observed: Duration,
    pub elapsed: Duration,
}

/// Native operation counts and transferred bytes, not POSIX block counts.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IoCounters {
    pub read_operations: u64,
    pub write_operations: u64,
    pub other_operations: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub other_bytes: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct ProcessSnapshot {
    pub times: ProcessTimes,
    pub io: IoCounters,
    pub page_faults: u32,
    pub peak_working_set_bytes: usize,
    pub working_set_bytes: usize,
    pub peak_pagefile_bytes: usize,
}

/// A multi-query observation, not an atomic transaction across counter classes.
/// Windows does not provide a Job Object creation timestamp here, so no job
/// lifetime/elapsed duration is invented. Timer intervals use `Instant` instead.
#[derive(Clone, Copy, Debug)]
pub struct JobSnapshot {
    pub cpu: CpuTimes,
    pub io: IoCounters,
    pub total_processes: u32,
    pub active_processes: u32,
    /// Members terminated for job limit violations; not a signal/exit count.
    pub limit_terminated_processes: u32,
    pub page_faults: u32,
    /// Peak committed memory, not peak resident set.
    pub peak_process_memory_bytes: usize,
    pub peak_job_memory_bytes: usize,
    pub observed: Instant,
}

#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    fn ticks(&self) -> u64 {
        (u64::from(self.high) << 32) | u64::from(self.low)
    }
}

#[repr(C)]
#[derive(Default)]
struct ProcessMemoryCounters {
    cb: u32,
    page_faults: u32,
    peak_working_set: usize,
    working_set: usize,
    quota_peak_paged_pool: usize,
    quota_paged_pool: usize,
    quota_peak_nonpaged_pool: usize,
    quota_nonpaged_pool: usize,
    pagefile: usize,
    peak_pagefile: usize,
}

#[repr(C)]
#[derive(Default)]
struct JobAccounting {
    user: i64,
    kernel: i64,
    period_user: i64,
    period_kernel: i64,
    page_faults: u32,
    total_processes: u32,
    active_processes: u32,
    limit_terminated_processes: u32,
}

#[repr(C)]
#[derive(Default)]
struct JobAccountingAndIo {
    basic: JobAccounting,
    io: IoCounters,
}

#[repr(C)]
#[derive(Default)]
struct JobBasicLimits {
    process_user_time: i64,
    job_user_time: i64,
    flags: u32,
    minimum_working_set: usize,
    maximum_working_set: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
#[derive(Default)]
struct JobExtendedLimits {
    basic: JobBasicLimits,
    io: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory: usize,
    peak_job_memory: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> RawHandle;
    fn GetProcessTimes(
        process: RawHandle,
        created: *mut FileTime,
        exited: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn GetSystemTimePreciseAsFileTime(time: *mut FileTime);
    fn GetProcessIoCounters(process: RawHandle, counters: *mut IoCounters) -> i32;
    fn QueryInformationJobObject(
        job: RawHandle,
        class: i32,
        data: *mut c_void,
        size: u32,
        returned_size: *mut u32,
    ) -> i32;
}

#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(
        process: RawHandle,
        counters: *mut ProcessMemoryCounters,
        size: u32,
    ) -> i32;
}

fn counter_overflow() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "native accounting counter overflow",
    )
}

fn counter_regressed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "native accounting counter decreased",
    )
}

fn from_ticks(ticks: u64) -> Duration {
    // FILETIME and Job Object CPU counters use 100 ns units. Splitting avoids
    // overflowing even for the largest representable unsigned FILETIME.
    Duration::new(ticks / 10_000_000, ((ticks % 10_000_000) * 100) as u32)
}

fn from_signed_ticks(ticks: i64) -> io::Result<Duration> {
    u64::try_from(ticks)
        .map(from_ticks)
        .map_err(|_| counter_regressed())
}

fn elapsed(created: Duration, end: Duration) -> io::Result<Duration> {
    end.checked_sub(created).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "native process timestamp precedes creation",
        )
    })
}

pub fn process_times(process: BorrowedHandle<'_>) -> io::Result<ProcessTimes> {
    let mut created = FileTime::default();
    let mut exited = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    if unsafe {
        GetProcessTimes(
            process.as_raw_handle(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut now = FileTime::default();
    unsafe { GetSystemTimePreciseAsFileTime(&mut now) };
    let created = from_ticks(created.ticks());
    let exited = (exited.ticks() != 0).then(|| from_ticks(exited.ticks()));
    let observed = from_ticks(now.ticks());
    Ok(ProcessTimes {
        cpu: CpuTimes {
            user: from_ticks(user.ticks()),
            kernel: from_ticks(kernel.ticks()),
        },
        created,
        exited,
        observed,
        elapsed: elapsed(created, exited.unwrap_or(observed))?,
    })
}

fn current_process() -> BorrowedHandle<'static> {
    // The pseudo-handle is always valid in this process and must never be closed.
    unsafe { BorrowedHandle::borrow_raw(GetCurrentProcess()) }
}

pub fn current_process_times() -> io::Result<ProcessTimes> {
    process_times(current_process())
}

pub fn process_snapshot(process: BorrowedHandle<'_>) -> io::Result<ProcessSnapshot> {
    let times = process_times(process)?;
    let mut io = IoCounters::default();
    if unsafe { GetProcessIoCounters(process.as_raw_handle(), &mut io) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut memory = ProcessMemoryCounters {
        cb: size_of::<ProcessMemoryCounters>() as u32,
        ..ProcessMemoryCounters::default()
    };
    if unsafe {
        GetProcessMemoryInfo(
            process.as_raw_handle(),
            &mut memory,
            size_of::<ProcessMemoryCounters>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(ProcessSnapshot {
        times,
        io,
        page_faults: memory.page_faults,
        peak_working_set_bytes: memory.peak_working_set,
        working_set_bytes: memory.working_set,
        peak_pagefile_bytes: memory.peak_pagefile,
    })
}

pub fn current_process_snapshot() -> io::Result<ProcessSnapshot> {
    process_snapshot(current_process())
}

fn query_job<T: Default>(job: BorrowedHandle<'_>, class: i32) -> io::Result<T> {
    let mut data = T::default();
    let mut returned = 0;
    if unsafe {
        QueryInformationJobObject(
            job.as_raw_handle(),
            class,
            (&mut data as *mut T).cast(),
            size_of::<T>() as u32,
            &mut returned,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if returned as usize != size_of::<T>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete native job accounting snapshot",
        ));
    }
    Ok(data)
}

fn job_cpu(basic: &JobAccounting) -> io::Result<CpuTimes> {
    Ok(CpuTimes {
        user: from_signed_ticks(basic.user)?,
        kernel: from_signed_ticks(basic.kernel)?,
    })
}

pub fn job_cpu_times(job: BorrowedHandle<'_>) -> io::Result<CpuTimes> {
    job_cpu(&query_job::<JobAccounting>(
        job, 1, /* JobObjectBasicAccountingInformation */
    )?)
}

pub fn job_snapshot(job: BorrowedHandle<'_>) -> io::Result<JobSnapshot> {
    let accounting = query_job::<JobAccountingAndIo>(
        job, 8, /* JobObjectBasicAndIoAccountingInformation */
    )?;
    let limits =
        query_job::<JobExtendedLimits>(job, 9 /* JobObjectExtendedLimitInformation */)?;
    Ok(JobSnapshot {
        cpu: job_cpu(&accounting.basic)?,
        io: accounting.io,
        total_processes: accounting.basic.total_processes,
        active_processes: accounting.basic.active_processes,
        limit_terminated_processes: accounting.basic.limit_terminated_processes,
        page_faults: accounting.basic.page_faults,
        peak_process_memory_bytes: limits.peak_process_memory,
        peak_job_memory_bytes: limits.peak_job_memory,
        observed: Instant::now(),
    })
}

#[derive(Default)]
struct Registry {
    completed: CpuTimes,
    live: HashMap<usize, Arc<OwnedHandle>>,
    // One sticky error, not a permanently growing completed-PID/error map.
    failure: Option<String>,
}

fn registry() -> io::Result<MutexGuard<'static, Registry>> {
    static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));
    REGISTRY
        .lock()
        .map_err(|_| io::Error::other("native accounting registry poisoned"))
}

/// Register exactly once per disjoint launch job, before resuming its roots.
/// Distinct duplicated handles to the same Job Object must not be registered.
/// Clones of the same Arc/handle are detected and rejected.
pub fn register_job(job: Arc<OwnedHandle>) -> io::Result<TrackedJob> {
    job_cpu_times(job.as_handle())?;
    let key = job.as_raw_handle() as usize;
    let mut registry = registry()?;
    if registry.live.contains_key(&key) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "native job already tracked",
        ));
    }
    registry.live.insert(key, job);
    Ok(TrackedJob {
        key: Some(key),
        finished: None,
    })
}

/// RAII accounting registration. A successful finish caches just this owner's
/// snapshot, removes the live handle, and adds its CPU to the aggregate once.
/// Dropping without finish takes the same cutoff; query errors remain visible
/// to future timer snapshots instead of silently undercounting.
#[must_use = "retain this owner until the native job accounting cutoff"]
pub struct TrackedJob {
    key: Option<usize>,
    finished: Option<JobSnapshot>,
}

impl TrackedJob {
    /// Idempotent. A failed query keeps the live registration for an explicit retry.
    pub fn finish(&mut self) -> io::Result<JobSnapshot> {
        if let Some(snapshot) = self.finished {
            return Ok(snapshot);
        }
        let key = self
            .key
            .ok_or_else(|| io::Error::other("native job tracking already released"))?;
        let mut registry = registry()?;
        let job = registry
            .live
            .get(&key)
            .ok_or_else(|| io::Error::other("native job registration missing"))?;
        let snapshot = job_snapshot(job.as_handle())?;
        let completed = registry.completed.checked_add(snapshot.cpu)?;
        registry.live.remove(&key);
        registry.completed = completed;
        self.key = None;
        self.finished = Some(snapshot);
        Ok(snapshot)
    }
}

impl Drop for TrackedJob {
    fn drop(&mut self) {
        if self.key.is_none() {
            return;
        }
        if let Err(error) = self.finish() {
            if let Ok(mut registry) = registry() {
                if let Some(key) = self.key.take() {
                    registry.live.remove(&key);
                }
                if registry.failure.is_none() {
                    registry.failure =
                        Some(format!("native job accounting cutoff failed: {error}"));
                }
            }
            // Drop cannot return an error. Retain it above and diagnose now.
            if let Ok(mut stderr) = crate::io::NativeOutput::stderr() {
                let _ = writeln!(stderr, "fish: native job accounting cutoff failed: {error}");
            }
        }
    }
}

/// Cumulative CPU of completed cutoff snapshots plus currently registered jobs.
/// The registry lock serializes sampling with finish; root times are never added.
/// Concurrent jobs all contribute to an interval, including unrelated background
/// work running during it. Disowned jobs remain in scope only until their cutoff.
pub fn tracked_children_cpu() -> io::Result<CpuTimes> {
    let registry = registry()?;
    if let Some(error) = &registry.failure {
        return Err(io::Error::other(error.clone()));
    }
    let mut total = registry.completed;
    for job in registry.live.values() {
        total = total.checked_add(job_cpu_times(job.as_handle())?)?;
    }
    Ok(total)
}

/// This is not getrusage: native categories are named with their actual units.
/// POSIX-only values cannot be inferred from Windows operation/page-fault counts.
pub fn write_self_usage(output: &mut impl Write) -> io::Result<()> {
    let snapshot = current_process_snapshot()?;
    writeln!(
        output,
        "Resource usage (shell process only; native Windows counters):"
    )?;
    writeln!(
        output,
        "  elapsed: {:.6} seconds",
        snapshot.times.elapsed.as_secs_f64()
    )?;
    writeln!(
        output,
        "  user CPU: {:.6} seconds",
        snapshot.times.cpu.user.as_secs_f64()
    )?;
    writeln!(
        output,
        "  kernel CPU: {:.6} seconds",
        snapshot.times.cpu.kernel.as_secs_f64()
    )?;
    writeln!(
        output,
        "  peak working set: {} bytes",
        snapshot.peak_working_set_bytes
    )?;
    writeln!(
        output,
        "  working set: {} bytes",
        snapshot.working_set_bytes
    )?;
    writeln!(
        output,
        "  peak pagefile usage: {} bytes",
        snapshot.peak_pagefile_bytes
    )?;
    writeln!(output, "  page faults (combined): {}", snapshot.page_faults)?;
    writeln!(
        output,
        "  I/O read: {} operations, {} bytes",
        snapshot.io.read_operations, snapshot.io.read_bytes
    )?;
    writeln!(
        output,
        "  I/O write: {} operations, {} bytes",
        snapshot.io.write_operations, snapshot.io.write_bytes
    )?;
    writeln!(
        output,
        "  I/O other: {} operations, {} bytes",
        snapshot.io.other_operations, snapshot.io.other_bytes
    )?;
    writeln!(
        output,
        "  unavailable: POSIX minor/major fault split, swaps, block I/O, IPC messages, signals, voluntary/involuntary context switches, shared/unshared memory integrals"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_units_cover_boundaries_without_overflow() {
        assert_eq!(from_ticks(1), Duration::from_nanos(100));
        assert_eq!(from_ticks(9_999_999), Duration::new(0, 999_999_900));
        assert_eq!(from_ticks(10_000_000), Duration::from_secs(1));
        assert_eq!(from_ticks(u64::MAX).as_nanos(), u128::from(u64::MAX) * 100);
        assert!(from_signed_ticks(-1).is_err());
    }

    #[test]
    fn elapsed_uses_creation_and_actual_endpoint() {
        let creation = from_ticks(100_000_000);
        let exit = from_ticks(170_000_001);
        assert_eq!(elapsed(creation, exit).unwrap(), Duration::new(7, 100));
        assert!(elapsed(exit, creation).is_err());
    }

    #[test]
    fn completed_and_live_cpu_accumulate_once_across_cutoff() {
        let completed = CpuTimes {
            user: Duration::from_secs(3),
            kernel: Duration::from_secs(1),
        };
        let first = CpuTimes {
            user: Duration::from_secs(5),
            kernel: Duration::from_secs(2),
        };
        let second = CpuTimes {
            user: Duration::from_secs(7),
            kernel: Duration::from_secs(4),
        };
        let before = completed
            .checked_add(first)
            .unwrap()
            .checked_add(second)
            .unwrap();
        let after_completed = completed.checked_add(first).unwrap();
        let after = after_completed.checked_add(second).unwrap();
        assert_eq!(before, after);
        assert_eq!(
            after.checked_sub(completed).unwrap(),
            first.checked_add(second).unwrap()
        );
        assert!(first.checked_sub(second).is_err());
        let maximum = CpuTimes {
            user: Duration::MAX,
            kernel: Duration::ZERO,
        };
        assert!(
            maximum
                .checked_add(CpuTimes {
                    user: Duration::from_nanos(1),
                    kernel: Duration::ZERO
                })
                .is_err()
        );
    }
}
