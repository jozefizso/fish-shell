//! Windows implementation of the existing worker/debounce ownership model.
//! No signal masks or post-fork state are involved. All tasks share a bounded
//! pool, and results are values serviced by the main thread, not parser calls.

use super::{
    NativeCompletionPayload, NativeEvent, NativePoster, NativeRuntime, NativeSource,
    RegistrationId, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT, WaitForSingleObject,
    assert_is_main_thread, lock, wait_milliseconds,
};
use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::num::NonZeroU64;
use std::os::windows::io::{AsRawHandle, BorrowedHandle};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

type WorkItem = Box<dyn FnOnce() + Send + 'static>;
const IO_WAIT_FOR_WORK_DURATION: Duration = Duration::from_millis(500);

/// Spawn an explicitly requested detached thread, without Unix signal masking.
/// Arbitrary queued work should use ThreadPool instead.
pub fn spawn<F: FnOnce() + Send + 'static>(callback: F) -> bool {
    match std::thread::Builder::new().spawn(callback) {
        Ok(handle) => {
            drop(handle);
            true
        }
        Err(error) => {
            eprintln!("rust thread spawn failure: {error}");
            false
        }
    }
}

#[derive(Default)]
struct ThreadPoolProtected {
    request_queue: VecDeque<WorkItem>,
    total_threads: usize,
    waiting_threads: usize,
}

/// On-demand, bounded workers with the same soft-minimum idle policy as Unix.
pub struct ThreadPool {
    shared: Mutex<ThreadPoolProtected>,
    cond_var: Condvar,
    soft_min_threads: usize,
    max_threads: usize,
}

impl fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadPool")
            .field("min_threads", &self.soft_min_threads)
            .field("max_threads", &self.max_threads)
            .finish()
    }
}

impl ThreadPool {
    pub fn new(soft_min_threads: usize, max_threads: usize) -> Arc<Self> {
        assert!(max_threads > 0, "a worker pool needs at least one worker");
        assert!(
            soft_min_threads <= max_threads,
            "worker minimum exceeds maximum"
        );
        Arc::new(Self {
            shared: Mutex::new(ThreadPoolProtected::default()),
            cond_var: Condvar::new(),
            soft_min_threads,
            max_threads,
        })
    }

    /// Queue real background work and return the pre-enqueue worker count.
    /// If the very first worker cannot start, reject the task rather than claim
    /// success and leave it stranded forever. Existing workers can service a
    /// task even if an attempted additional worker cannot be started.
    pub fn try_perform<F>(self: &Arc<Self>, handler: F) -> io::Result<usize>
    where
        F: FnOnce() + Send + 'static,
    {
        let work: WorkItem = Box::new(handler);
        let mut data = lock(&self.shared);
        let previous_count = data.total_threads;
        data.request_queue.push_back(work);
        if data.waiting_threads >= data.request_queue.len() {
            self.cond_var.notify_one();
        } else if data.total_threads < self.max_threads {
            data.total_threads += 1;
            let pool = Arc::clone(self);
            // Keep the mutex until spawn succeeds/fails so an exiting worker
            // cannot make a failed spawn strand the newly accepted request.
            match std::thread::Builder::new()
                .name("fish-worker".into())
                .spawn(move || pool.run_worker())
            {
                Ok(handle) => drop(handle),
                Err(error) => {
                    data.total_threads -= 1;
                    if data.total_threads == 0 {
                        let rejected = data.request_queue.pop_back();
                        drop(data);
                        drop(rejected);
                        return Err(error);
                    }
                }
            }
        }
        Ok(previous_count)
    }

    /// Preserve the existing infallible worker interface. Resource exhaustion
    /// with no possible servicing worker is an explicit failure, not lost work.
    pub fn perform<F>(self: &Arc<Self>, handler: F) -> usize
    where
        F: FnOnce() + Send + 'static,
    {
        self.try_perform(handler)
            .expect("could not start a worker thread")
    }

    /// Run only the producer on a worker. The client owns the source and applies
    /// the resulting User payload from NativeRuntime::service_main or drain.
    pub fn perform_with_completion<R, F>(
        self: &Arc<Self>,
        poster: NativePoster,
        handler: F,
    ) -> io::Result<usize>
    where
        R: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
    {
        self.try_perform(move || {
            let result = handler();
            poster.post(NativeCompletionPayload::User(Box::new(result)));
        })
    }

    fn run_worker(&self) {
        while let Some(work) = self.dequeue_work_or_commit_to_exit() {
            // A user panic must not strand the rest of the accepted queue or
            // leak the pool's worker count. The normal panic hook still runs.
            let _ = catch_unwind(AssertUnwindSafe(work));
        }
    }

    fn dequeue_work_or_commit_to_exit(&self) -> Option<WorkItem> {
        let mut data = lock(&self.shared);
        if data.request_queue.is_empty() && data.total_threads == self.soft_min_threads {
            data.waiting_threads += 1;
            data = self
                .cond_var
                .wait_timeout(data, IO_WAIT_FOR_WORK_DURATION)
                .unwrap_or_else(|poison| poison.into_inner())
                .0;
            data.waiting_threads -= 1;
        }
        let work = data.request_queue.pop_front();
        if work.is_none() {
            // Commit to exit while holding the enqueue mutex. A subsequent
            // enqueue sees the reduced count and can start a replacement.
            data.total_threads -= 1;
        }
        work
    }
}

/// A Sync wrapper whose value is only accessed from the registered main thread.
pub struct MainThread<T> {
    data: T,
}

// Like the Unix owner: T can be transferred, but shared access is main-only.
unsafe impl<T: Send + 'static> Sync for MainThread<T> {}

impl<T> MainThread<T> {
    pub const fn new(value: T) -> Self {
        Self { data: value }
    }

    pub fn get(&self) -> &T {
        assert_is_main_thread();
        &self.data
    }
}

struct DebounceRequest<R> {
    sequence: NonZeroU64,
    handler: Box<dyn FnOnce() -> R + Send + 'static>,
}

struct DebounceData<R> {
    next_req: Option<DebounceRequest<R>>,
    active_token: Option<NonZeroU64>,
    next_token: NonZeroU64,
    next_sequence: NonZeroU64,
    start_time: Instant,
    result: Option<R>,
    latest_result_sequence: u64,
    cancelled_through: u64,
    closed: bool,
}

struct DebounceOwner<R> {
    data: Arc<Mutex<DebounceData<R>>>,
    source: NativeSource,
    ready: Arc<NativeEvent>,
}

impl<R> Drop for DebounceOwner<R> {
    fn drop(&mut self) {
        let (pending, result) = {
            let mut data = lock(&self.data);
            data.closed = true;
            data.active_token = None;
            (data.next_req.take(), data.result.take())
        };
        // No worker owns this owner or its NativeSource. Source close therefore
        // happens as soon as the last client drops, even with a busy handler.
        drop(self.source.remove_and_drain());
        drop(pending);
        drop(result);
    }
}

/// Execute one background handler with at most one queued replacement. Timeout
/// abandons the active token, not the actual OS thread: replacements still obey
/// the shared worker bound. Old handlers cannot steal a new token's request or
/// overwrite a newer delivered result. Dropping/cancelling invalidates results
/// without attempting unsafe forced thread cancellation.
pub struct Debounce<R> {
    pool: Arc<ThreadPool>,
    timeout: Duration,
    owner: Arc<DebounceOwner<R>>,
}

impl<R> Clone for Debounce<R> {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
            timeout: self.timeout,
            owner: Arc::clone(&self.owner),
        }
    }
}

impl<R: Send + 'static> Debounce<R> {
    pub fn new(
        pool: &Arc<ThreadPool>,
        runtime: &Arc<NativeRuntime>,
        timeout: Duration,
    ) -> io::Result<Self> {
        Ok(Self {
            pool: Arc::clone(pool),
            timeout,
            owner: Arc::new(DebounceOwner {
                data: Arc::new(Mutex::new(DebounceData {
                    next_req: None,
                    active_token: None,
                    next_token: NonZeroU64::new(1).unwrap(),
                    next_sequence: NonZeroU64::new(1).unwrap(),
                    start_time: Instant::now(),
                    result: None,
                    latest_result_sequence: 0,
                    cancelled_through: 0,
                    closed: false,
                })),
                source: runtime.register_source()?,
                ready: Arc::new(NativeEvent::new()?),
            }),
        })
    }

    pub fn registration_id(&self) -> RegistrationId {
        self.owner.source.id()
    }

    /// Notification for this debouncer alone, independent of other runtime
    /// records. Result publication and reset both hold the result mutex.
    pub fn notification_handle(&self) -> BorrowedHandle<'_> {
        self.owner.ready.handle()
    }

    pub fn perform<F>(&self, handler: F) -> NonZeroU64
    where
        F: FnOnce() -> R + Send + 'static,
    {
        let handler = Box::new(handler);
        let (token, spawn_worker, replaced) = {
            let mut data = lock(&self.owner.data);
            let sequence = data.next_sequence;
            data.next_sequence = sequence
                .checked_add(1)
                .expect("debounce request identity exhausted");
            let replaced = data.next_req.replace(DebounceRequest { sequence, handler });
            if data.active_token.is_some()
                && !self.timeout.is_zero()
                && data.start_time.elapsed() > self.timeout
            {
                data.active_token = None;
            }
            let spawn_worker = data.active_token.is_none();
            if spawn_worker {
                data.active_token = Some(data.next_token);
                data.next_token = data
                    .next_token
                    .checked_add(1)
                    .expect("debounce token identity exhausted");
                data.start_time = Instant::now();
            }
            (
                data.active_token.expect("debounce token is active"),
                spawn_worker,
                replaced,
            )
        };
        // A replaced handler may own arbitrary resources: never drop it locked.
        drop(replaced);
        if spawn_worker {
            let data = Arc::clone(&self.owner.data);
            let poster = self.owner.source.poster();
            let ready = Arc::clone(&self.owner.ready);
            if let Err(error) =
                self.pool.try_perform(
                    move || {
                        while Self::run_next(&data, &poster, &ready, token) {}
                    },
                )
            {
                let rejected = {
                    let mut data = lock(&self.owner.data);
                    if data.active_token == Some(token) {
                        data.active_token = None;
                        data.next_req.take()
                    } else {
                        None
                    }
                };
                drop(rejected);
                panic!("could not start debounce worker: {error}");
            }
        }
        token
    }

    fn run_next(
        shared: &Mutex<DebounceData<R>>,
        poster: &NativePoster,
        ready: &NativeEvent,
        token: NonZeroU64,
    ) -> bool {
        let request = {
            let mut data = lock(shared);
            if data.closed || data.active_token != Some(token) {
                return false;
            }
            let Some(request) = data.next_req.take() else {
                data.active_token = None;
                return false;
            };
            data.start_time = Instant::now();
            request
        };
        let result = catch_unwind(AssertUnwindSafe(request.handler));
        if let Ok(result) = result {
            let mut data = lock(shared);
            if data.closed
                || data.active_token != Some(token)
                || request.sequence.get() <= data.cancelled_through
                || request.sequence.get() < data.latest_result_sequence
            {
                drop(data);
                drop(result);
            } else {
                data.latest_result_sequence = request.sequence.get();
                let displaced = data.result.replace(result);
                // Publish/store/reset ordering is protected by the result lock.
                // The poster owns no DebounceOwner and cannot unregister itself.
                poster.post_ready_coalesced();
                ready.set();
                drop(data);
                drop(displaced);
            }
        }
        true
    }

    /// Invalidate pending/running results cooperatively. A running handler owns
    /// its resources until it returns, but can no longer publish a completion.
    /// New perform calls remain supported after cancellation.
    pub fn cancel_pending(&self) {
        let (pending, result, records) = {
            let mut data = lock(&self.owner.data);
            data.cancelled_through = data.next_sequence.get() - 1;
            data.active_token = None;
            let pending = data.next_req.take();
            let result = data.result.take();
            let records = self
                .owner
                .source
                .runtime
                .take_source_records(self.registration_id(), false);
            self.owner.ready.reset();
            (pending, result, records)
        };
        drop(pending);
        drop(result);
        drop(records);
    }

    /// The caller applies the owned result on the main thread, like Unix reader
    /// servicing. Clearing this source cannot consume another client's records.
    pub fn take_result(&mut self) -> Option<R> {
        assert_is_main_thread();
        let (result, records) = {
            let mut data = lock(&self.owner.data);
            let records = self
                .owner
                .source
                .runtime
                .take_source_records(self.registration_id(), false);
            let result = data.result.take();
            self.owner.ready.reset();
            (result, records)
        };
        drop(records);
        result
    }

    pub fn take_result_with_timeout(&mut self, timeout: Duration) -> Option<R> {
        assert_is_main_thread();
        let start = Instant::now();
        loop {
            if let Some(result) = self.take_result() {
                return Some(result);
            }
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return None;
            }
            match unsafe {
                WaitForSingleObject(
                    self.notification_handle().as_raw_handle(),
                    wait_milliseconds(Some(remaining)),
                )
            } {
                WAIT_OBJECT_0 => (),
                WAIT_TIMEOUT => {
                    if start.elapsed() >= timeout {
                        return self.take_result();
                    }
                }
                WAIT_FAILED => panic!(
                    "debounce notification wait failed: {}",
                    io::Error::last_os_error()
                ),
                value => panic!("unexpected debounce wait result {value}"),
            }
        }
    }
}
