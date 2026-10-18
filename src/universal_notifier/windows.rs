//! Native state invalidation. Kernel notifications are hints, never state truth.
//!
//! Each reader owns independent ReadDirectoryChangesW registrations, including
//! an ancestor guard. An owned timer requests periodic absolute-path identity
//! rescans on the bounded N2 worker pool. Thus a renamed/deleted parent, lost
//! notification detail or disconnected share cannot strand a reader forever on
//! an old directory handle. Required watcher failures are returned, not replaced
//! by a null notifier. Consumers must reload/merge through their identity-aware
//! persistence transaction after a dirty hint, and publish only after success.
//! Incomplete ReadDirectoryChangesW completions (ERROR_MORE_DATA,
//! ERROR_NOTIFY_ENUM_DIR, zero bytes or malformed records) invalidate the whole
//! state even with a filename filter. Each read is re-armed before scheduling an
//! absolute-path identity scan; a failed re-arm remains an explicit error.
use crate::threads::{
    Debounce, NativeRuntime, NativeSource, NativeWaitRegistration, RegistrationId, ThreadPool,
    assert_is_main_thread,
};
use std::ffi::{OsStr, OsString, c_void};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::{Component, Path, PathBuf};
use std::ptr;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

#[path = "windows_io.rs"]
mod io_owner;
use io_owner::{DirectoryIo, lock};

// Fixed initialization, shared by every state watcher. A slow UNC server cannot
// create one unbounded worker/thread per watcher or per recovery attempt.
static RECOVERY_POOL: LazyLock<Arc<ThreadPool>> = LazyLock::new(|| ThreadPool::new(0, 2));
const RECOVERY_MILLISECONDS: i64 = 1000;

#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}
#[repr(C)]
#[derive(Default)]
struct FileInformation {
    attributes: u32,
    creation: FileTime,
    access: FileTime,
    write: FileTime,
    volume: u32,
    size_high: u32,
    size_low: u32,
    links: u32,
    index_high: u32,
    index_low: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        creation: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn GetFileInformationByHandle(file: RawHandle, information: *mut FileInformation) -> i32;
    fn CreateWaitableTimerW(security: *const c_void, manual: i32, name: *const u16) -> RawHandle;
    fn SetWaitableTimer(
        timer: RawHandle,
        due: *const i64,
        period: i32,
        callback: *const c_void,
        context: *const c_void,
        resume: i32,
    ) -> i32;
    fn CancelWaitableTimer(timer: RawHandle) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Identity {
    volume: u32,
    high: u32,
    low: u32,
}
struct Opened {
    path: PathBuf,
    identity: Identity,
    file: OwnedHandle,
    subtree: bool,
}
struct Snapshot {
    parent: Option<Opened>,
    guard: Option<Opened>,
}
struct Binding {
    path: PathBuf,
    identity: Identity,
    subtree: bool,
    io: DirectoryIo,
}
impl Binding {
    fn matches(&self, opened: &Opened) -> bool {
        self.path == opened.path
            && self.identity == opened.identity
            && self.subtree == opened.subtree
    }
}

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut value: Vec<_> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "directory watch path contains NUL",
        ));
    }
    value.push(0);
    Ok(value)
}
fn open_directory(path: &Path, subtree: bool) -> io::Result<Opened> {
    let name = wide(path.as_os_str())?;
    // FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES, all three sharing modes,
    // OPEN_EXISTING, FILE_FLAG_OVERLAPPED | FILE_FLAG_BACKUP_SEMANTICS.
    let raw = unsafe {
        CreateFileW(
            name.as_ptr(),
            0x81,
            7,
            ptr::null(),
            3,
            0x4200_0000,
            ptr::null_mut(),
        )
    };
    if raw == (-1isize) as RawHandle {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut info = FileInformation::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.attributes & 0x10 == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "state parent is not a directory",
        ));
    }
    Ok(Opened {
        path: path.to_owned(),
        file,
        subtree,
        identity: Identity {
            volume: info.volume,
            high: info.index_high,
            low: info.index_low,
        },
    })
}
fn missing(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(2 | 3))
}
fn scan(path: &Path) -> io::Result<Snapshot> {
    let parent = match open_directory(path, false) {
        Ok(value) => Some(value),
        Err(error) if missing(&error) => None,
        Err(error) => return Err(error),
    };
    // For missing chains, guard the closest existing ancestor recursively until
    // the requested parent exists. For a present parent, guard its immediate
    // parent against rename/replacement; periodic identity checks cover changes
    // to any higher ancestor (including drive/share remounts).
    let mut ancestor = path.parent();
    let guard = loop {
        let Some(candidate) = ancestor else {
            break None;
        };
        match open_directory(candidate, parent.is_none()) {
            Ok(value) => break Some(value),
            Err(error) if missing(&error) => ancestor = candidate.parent(),
            Err(error) => return Err(error),
        }
    };
    if parent.is_none() && guard.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "state parent has no available watchable ancestor",
        ));
    }
    Ok(Snapshot { parent, guard })
}

struct RecoveryTimer {
    handle: Arc<OwnedHandle>,
    wait: Option<NativeWaitRegistration>,
}
impl RecoveryTimer {
    fn new(runtime: &Arc<NativeRuntime>) -> io::Result<Self> {
        let raw = unsafe { CreateWaitableTimerW(ptr::null(), 1, ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut timer = Self {
            handle: Arc::new(unsafe { OwnedHandle::from_raw_handle(raw) }),
            wait: None,
        };
        timer.arm(runtime)?;
        Ok(timer)
    }
    fn arm(&mut self, runtime: &Arc<NativeRuntime>) -> io::Result<()> {
        if let Some(mut wait) = self.wait.take() {
            wait.remove_and_drain()?;
        }
        let due = -RECOVERY_MILLISECONDS * 10_000;
        if unsafe {
            SetWaitableTimer(
                self.handle.as_raw_handle(),
                &due,
                0,
                ptr::null(),
                ptr::null(),
                0,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        self.wait = Some(runtime.register_wait(Arc::clone(&self.handle))?);
        Ok(())
    }
    fn fired(&self) -> io::Result<bool> {
        match unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 0) } {
            0 => Ok(true),
            258 => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }
    fn shutdown(&mut self) -> io::Result<()> {
        let cancel = unsafe { CancelWaitableTimer(self.handle.as_raw_handle()) };
        let error = (cancel == 0).then(io::Error::last_os_error);
        if let Some(mut wait) = self.wait.take() {
            wait.remove_and_drain()?;
        }
        error.map_or(Ok(()), Err)
    }
}
impl Drop for RecoveryTimer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct State {
    parent: Option<Binding>,
    guard: Option<Binding>,
    source: NativeSource,
    timer: Option<RecoveryTimer>,
    recovery: Option<Debounce<io::Result<Snapshot>>>,
    scan_pending: bool,
    dirty: bool,
    closed: bool,
}
impl State {
    fn bind(
        slot: &mut Option<Binding>,
        opened: Option<Opened>,
        source: &NativeSource,
    ) -> io::Result<bool> {
        if let (Some(binding), Some(opened)) = (slot.as_ref(), opened.as_ref()) {
            if binding.matches(opened) {
                return Ok(false);
            }
        } else if slot.is_none() && opened.is_none() {
            return Ok(false);
        }
        // Disarm the old identity before releasing its native handle or buffer.
        let mut failure = None;
        if let Some(mut old) = slot.take() {
            if let Err(error) = old.io.shutdown() {
                failure = Some(error);
            }
        }
        if let Some(opened) = opened {
            let io = DirectoryIo::new(opened.file, opened.subtree, source.poster())?;
            *slot = Some(Binding {
                path: opened.path,
                identity: opened.identity,
                subtree: opened.subtree,
                io,
            });
        }
        failure.map_or(Ok(true), Err)
    }
    fn apply(&mut self, snapshot: Snapshot) -> io::Result<bool> {
        let parent = Self::bind(&mut self.parent, snapshot.parent, &self.source);
        let guard = Self::bind(&mut self.guard, snapshot.guard, &self.source);
        Ok(parent? | guard?)
    }
    fn shutdown(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let mut failure = None;
        for slot in [&mut self.parent, &mut self.guard] {
            if let Some(mut binding) = slot.take() {
                if let Err(error) = binding.io.shutdown() {
                    failure.get_or_insert(error);
                }
            }
        }
        if let Some(mut timer) = self.timer.take() {
            if let Err(error) = timer.shutdown() {
                failure.get_or_insert(error);
            }
        }
        if let Some(recovery) = self.recovery.take() {
            // A running scan has independent owned handles and cannot access
            // watcher storage. Closing its source rejects late publication;
            // the bounded worker drops its owned result when it returns.
            recovery.cancel_pending();
            drop(recovery);
        }
        if let Err(error) = self.source.remove_and_drain() {
            failure.get_or_insert(error);
        }
        failure.map_or(Ok(()), Err)
    }
}

/// Main-thread state invalidation owner. Every reader creates its own watcher;
/// notifications are not consumed from a shared named event or single reader.
/// The shared runtime wakeup may also signal other owners. Service its records
/// centrally, then call `check` on each watcher; `check` never drains that queue.
/// Only this owner's debounce/wait records are removed during result servicing
/// and cancellation. No worker or kernel callback invokes evaluator code.
pub struct NativeDirectoryWatcher {
    path: PathBuf,
    filter: Option<Vec<u16>>,
    runtime: Arc<NativeRuntime>,
    state: Mutex<State>,
}
impl NativeDirectoryWatcher {
    pub fn new(
        parent: &Path,
        filter: Option<OsString>,
        runtime: Arc<NativeRuntime>,
    ) -> io::Result<Self> {
        assert_is_main_thread();
        if !parent.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "state watch parent must be absolute",
            ));
        }
        let filter = filter
            .map(|name| {
                let path = Path::new(&name);
                let mut components = path.components();
                if !matches!(components.next(), Some(Component::Normal(_)))
                    || components.next().is_some()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "state watch filter must be one filename",
                    ));
                }
                let mut name = wide(&name)?;
                name.pop();
                Ok(name)
            })
            .transpose()?;
        // Open/identify both directories and submit their required reads here.
        // A redirector can still reject a queued read asynchronously; check()
        // reports that completion error rather than silently dropping a binding.
        let snapshot = scan(parent)?;
        let mut state = State {
            parent: None,
            guard: None,
            source: runtime.register_source()?,
            timer: Some(RecoveryTimer::new(&runtime)?),
            recovery: Some(Debounce::new(&RECOVERY_POOL, &runtime, Duration::ZERO)?),
            scan_pending: false,
            dirty: true,
            closed: false,
        };
        if let Err(error) = state.apply(snapshot) {
            let _ = state.shutdown();
            return Err(error);
        }
        Ok(Self {
            path: parent.to_owned(),
            filter,
            runtime,
            state: Mutex::new(state),
        })
    }

    pub fn notification_handle(&self) -> BorrowedHandle<'_> {
        self.runtime.notification_handle()
    }

    /// For a central router; Ready carries no state data and can be acknowledged
    /// centrally before calling check on all owners. Timer IDs change on re-arm.
    pub fn owns_registration(&self, id: RegistrationId) -> bool {
        let state = lock(&self.state);
        state.source.id() == id
            || state
                .recovery
                .as_ref()
                .is_some_and(|recovery| recovery.registration_id() == id)
            || state
                .timer
                .as_ref()
                .and_then(|timer| timer.wait.as_ref())
                .is_some_and(|wait| wait.id() == id)
    }

    /// Returns a dirty *hint*, including periodic full invalidation when kernel
    /// detail may have been lost. After Err the owner remains live and recovery
    /// stays armed; callers must diagnose the error, not claim synchronized state.
    /// Dirty is retained across errors for the next successful check.
    pub fn check(&self) -> io::Result<bool> {
        assert_is_main_thread();
        let mut state = lock(&self.state);
        if state.closed {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "state directory watcher is shut down",
            ));
        }
        let mut failure = None;
        let mut rescan = false;
        let result = state.recovery.as_mut().and_then(Debounce::take_result);
        if let Some(result) = result {
            state.scan_pending = false;
            match result.and_then(|snapshot| state.apply(snapshot)) {
                Ok(changed) => state.dirty |= changed,
                Err(error) => {
                    state.dirty = true;
                    failure = Some(error);
                }
            }
        }
        // Process each independent registration even if another has failed.
        for guard in [false, true] {
            let slot = if guard {
                &mut state.guard
            } else {
                &mut state.parent
            };
            let filter = if guard { None } else { self.filter.as_deref() };
            let completed = slot.as_mut().and_then(|binding| binding.io.take(filter));
            if let Some(completed) = completed {
                let result = completed.and_then(|dirty| {
                    // Includes incomplete SMB batches: arm before publishing
                    // dirty or requesting the absolute-path recovery scan.
                    slot.as_mut().expect("completed binding").io.arm()?;
                    Ok(dirty)
                });
                match result {
                    Ok(dirty) => {
                        state.dirty |= dirty;
                        rescan |= dirty;
                    }
                    Err(error) => {
                        // Network loss/invalid old directory: cancel/drain now,
                        // then retry absolute-path binding through owned recovery.
                        if let Some(mut binding) = slot.take() {
                            let _ = binding.io.shutdown();
                        }
                        state.dirty = true;
                        rescan = true;
                        failure.get_or_insert(error);
                    }
                }
            }
        }
        if let Some(timer) = state.timer.as_mut() {
            match timer.fired() {
                Ok(true) => {
                    // Re-arm before scheduling potentially slow network work.
                    if let Err(error) = timer.arm(&self.runtime) {
                        failure.get_or_insert(error);
                    }
                    state.dirty = true;
                    rescan = true;
                }
                Ok(false) => (),
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        if rescan && !state.scan_pending {
            let path = self.path.clone();
            state
                .recovery
                .as_ref()
                .expect("live recovery owner")
                .perform(move || scan(&path));
            state.scan_pending = true;
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(std::mem::take(&mut state.dirty))
    }

    /// Idempotent. Pending reads are cancelled and drained before buffers and
    /// directory handles are released; registered timer callbacks are joined.
    /// Running recovery scans own only independent handles, never watcher memory.
    pub fn shutdown(&self) -> io::Result<()> {
        lock(&self.state).shutdown()
    }
}
impl Drop for NativeDirectoryWatcher {
    fn drop(&mut self) {
        let _ = lock(&self.state).shutdown();
    }
}
