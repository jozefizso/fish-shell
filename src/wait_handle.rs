use crate::prelude::*;
use crate::proc::{InternalJobId, Pid};
use std::cell::Cell;
use std::rc::Rc;

/// The bits of a job necessary to support 'wait' and '--on-process-exit'.
/// This may outlive the job.
pub struct WaitHandle {
    /// The pid of this process.
    pub pid: Pid,

    /// The internal job ID of the job which contained this process.
    pub internal_job_id: InternalJobId,

    /// The "base name" of this process.
    /// For example if the process is "/bin/sleep" then this will be 'sleep'.
    pub base_name: WString,

    /// The status, if completed; None if not completed.
    status: Cell<Option<i32>>,

    /// Keep the Windows process object (and therefore its PID) alive until this
    /// cached completion is consumed or evicted; never retarget a reused PID.
    #[cfg(windows)]
    native_child: std::sync::Arc<crate::exec::NativeChild>,
    #[cfg(windows)]
    native_job: std::cell::OnceCell<std::sync::Arc<NativeJobCompletion>>,
}

/// A whole native pipeline completion, shared by its member wait records.
#[cfg(windows)]
pub struct NativeJobCompletion {
    pub status: i32,
    pub(crate) process: std::sync::Arc<crate::exec::NativeChild>,
}

#[cfg(windows)]
impl NativeJobCompletion {
    pub fn pid(&self) -> Pid {
        Pid::from_native(self.process.pid())
    }
}

impl WaitHandle {
    /// Return true if this wait handle is completed.
    pub fn is_completed(&self) -> bool {
        self.status.get().is_some()
    }
    pub fn set_status_and_complete(&self, status: i32) {
        assert!(!self.is_completed(), "wait handle already completed");
        self.status.set(Some(status));
    }

    /// Return the status, or None if not yet completed.
    pub fn status(&self) -> Option<i32> {
        self.status.get()
    }

    #[cfg(windows)]
    pub fn native_job_completion(&self) -> Option<std::sync::Arc<NativeJobCompletion>> {
        self.native_job.get().cloned()
    }

    #[cfg(windows)]
    pub(crate) fn mark_native_job_complete(&self, completion: std::sync::Arc<NativeJobCompletion>) {
        assert!(
            self.native_job.set(completion).is_ok(),
            "native job completion published twice"
        );
    }

    #[cfg(windows)]
    pub(crate) fn native_process(&self) -> &std::sync::Arc<crate::exec::NativeChild> {
        &self.native_child
    }
}

impl WaitHandle {
    /// Construct from a pid, job ID, and base name.
    pub fn new(
        pid: Pid,
        internal_job_id: InternalJobId,
        base_name: WString,
        #[cfg(windows)] native_child: std::sync::Arc<crate::exec::NativeChild>,
    ) -> WaitHandleRef {
        Rc::new(WaitHandle {
            pid,
            internal_job_id,
            base_name,
            status: Default::default(),
            #[cfg(windows)]
            native_child,
            #[cfg(windows)]
            native_job: std::cell::OnceCell::new(),
        })
    }
}

pub type WaitHandleRef = Rc<WaitHandle>;

const WAIT_HANDLE_STORE_DEFAULT_LIMIT: usize = 1024;

/// Support for storing a list of wait handles, with a max limit set at initialization.
/// Note this class is not safe for concurrent access.
pub struct WaitHandleStore {
    // Map from pid to wait handles.
    cache: lru::LruCache<Pid, WaitHandleRef>,
}

impl Default for WaitHandleStore {
    /// Construct with the default capacity.
    fn default() -> Self {
        Self::new_with_capacity(WAIT_HANDLE_STORE_DEFAULT_LIMIT)
    }
}

impl WaitHandleStore {
    pub fn new_with_capacity(capacity: usize) -> Self {
        let capacity = std::num::NonZeroUsize::new(capacity).unwrap();
        WaitHandleStore {
            cache: lru::LruCache::new(capacity),
        }
    }

    /// Add a wait handle to the store. This may remove the oldest handle, if our limit is exceeded.
    /// It may also remove any existing handle with that pid.
    pub fn add(&mut self, wh: WaitHandleRef) {
        self.cache.put(wh.pid, wh);
    }

    /// Return the wait handle for a pid, or None if there is none.
    /// This is a fast lookup.
    pub fn get_by_pid(&self, pid: Pid) -> Option<WaitHandleRef> {
        self.cache.peek(&pid).cloned()
    }

    /// Remove a given wait handle, if present in this store.
    pub fn remove(&mut self, wh: &WaitHandleRef) {
        // Note: this differs from remove_by_pid because we verify that the handle is the same.
        if let Some(key) = self.cache.peek(&wh.pid) {
            if Rc::ptr_eq(key, wh) {
                self.cache.pop(&wh.pid);
            }
        }
    }

    /// Remove the wait handle for a pid, if present in this store.
    pub fn remove_by_pid(&mut self, pid: Pid) {
        self.cache.pop(&pid);
    }

    /// Iterate over wait handles.
    pub fn iter(&self) -> impl Iterator<Item = &WaitHandleRef> {
        self.cache.iter().map(|(_, wh)| wh)
    }

    /// Copy out the list of all wait handles, returning the most-recently-used first.
    pub fn get_list(&self) -> Vec<WaitHandleRef> {
        self.cache.iter().map(|(_, wh)| wh.clone()).collect()
    }

    /// Convenience to return the size, for testing.
    pub fn size(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{WaitHandle, WaitHandleStore};
    use crate::prelude::*;
    use crate::proc::{InternalJobId, Pid};

    #[test]
    fn test_wait_handles() {
        #[cfg(windows)]
        if let Ok(status) = std::env::var("FISH_NATIVE_WAIT_UNIT_EXIT") {
            std::process::exit(status.parse().expect("controlled native fixture status"));
        }
        let mut cache = WaitHandleStore::new_with_capacity(4);
        #[cfg(windows)]
        let (group, children) = {
            use std::ffi::OsString;
            let image = crate::wutil::native_current_exe_path().unwrap();
            let cwd = std::env::current_dir().unwrap();
            let mut environment = [
                (
                    OsString::from("SystemRoot"),
                    std::env::var_os("SystemRoot").expect("native test requires SystemRoot"),
                ),
                (
                    OsString::from("FISH_NATIVE_WAIT_UNIT_EXIT"),
                    OsString::new(),
                ),
            ];
            let case = concat!(module_path!(), "::test_wait_handles")
                .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
                .unwrap();
            let mut group =
                crate::exec::NativeLaunchGroup::new(crate::threads::NativeRuntime::new().unwrap())
                    .unwrap();
            let children: Vec<_> = (1..=5)
                .map(|status| {
                    environment[1].1 = OsString::from(status.to_string());
                    let argv = [
                        image.as_os_str().to_owned(),
                        OsString::from("--exact"),
                        OsString::from(case),
                        OsString::from("--nocapture"),
                    ];
                    let child = group
                        .spawn(
                            &image,
                            &argv,
                            &cwd,
                            &environment,
                            &crate::io::IoChain::new(),
                            false,
                        )
                        .unwrap();
                    let observed = child.wait(None).unwrap();
                    assert_eq!(observed, status);
                    (child, observed as i32)
                })
                .collect();
            (group, children)
        };
        let pid = |index: usize| {
            #[cfg(unix)]
            let pid = Pid::new(index as i32 + 1);
            #[cfg(windows)]
            let pid = Pid::from_native(children[index].0.pid());
            pid
        };
        let handle = |index: usize| {
            let handle = WaitHandle::new(
                pid(index),
                InternalJobId::default(),
                L!("process").to_owned(),
                #[cfg(windows)]
                std::sync::Arc::clone(&children[index].0),
            );
            #[cfg(unix)]
            let status = index as i32 + 1;
            #[cfg(windows)]
            let status = children[index].1;
            handle.set_status_and_complete(status);
            handle
        };
        let first = handle(0);
        let replacement = handle(0);
        cache.add(first.clone());
        cache.add(replacement.clone());
        cache.remove(&first);
        // Consuming an older observer must not erase the newer cached record.
        assert_eq!(
            cache.get_by_pid(pid(0)).and_then(|wait| wait.status()),
            Some(1)
        );
        cache.remove(&replacement);
        assert!(cache.get_by_pid(pid(0)).is_none());
        for index in 0..5 {
            cache.add(handle(index));
        }
        // Capacity eviction is observable to wait/late-handler selection; the
        // cache's incidental iterator order and entry representation are not.
        assert!(cache.get_by_pid(pid(0)).is_none());
        for index in 1..5 {
            assert_eq!(
                cache.get_by_pid(pid(index)).and_then(|wait| wait.status()),
                Some(index as i32 + 1)
            );
        }
        #[cfg(windows)]
        group.finish().unwrap();
    }
}
