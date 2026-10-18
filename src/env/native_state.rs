//! Main-thread persistence ownership and checked universal-variable acknowledgement.
use super::*;
use crate::event::Event;
use crate::threads::{NativeCompletionPayload, NativeRuntime};
use crate::universal_notifier::NativeDirectoryWatcher;
use fish_widestring::shell_text_to_osstring;
use std::{
    cell::RefCell,
    collections::HashSet,
    io,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

thread_local! {
    // The existing Copy status API carries a category; its caller consumes the
    // corresponding owned error into its actual (possibly redirected) stream.
    static STORAGE_ERROR: RefCell<Option<io::Error>> = const { RefCell::new(None) };
}

pub(super) fn set_storage_error(error: io::Error) {
    STORAGE_ERROR.with(|slot| *slot.borrow_mut() = Some(error));
}

pub(super) fn take_storage_error() -> Option<io::Error> {
    STORAGE_ERROR.with(|slot| slot.borrow_mut().take())
}

struct NativeState {
    runtime: Arc<NativeRuntime>,
    universal_watch: NativeDirectoryWatcher,
    history_watch: NativeDirectoryWatcher,
    events: Mutex<Vec<Event>>,
    // Failed local mutations have not run their consumer effects or variable
    // events. Keep keys, not values: a retry must report the committed state.
    deferred_mutations: Mutex<HashSet<WString>>,
}

static STATE: LazyLock<Mutex<Option<Arc<NativeState>>>> = LazyLock::new(|| Mutex::new(None));

fn state() -> Option<Arc<NativeState>> {
    STATE.lock().unwrap().as_ref().map(Arc::clone)
}

struct NativeStateGuard(std::marker::PhantomData<std::rc::Rc<()>>);

impl Drop for NativeStateGuard {
    fn drop(&mut self) {
        let Some(owner) = STATE.lock().unwrap().take() else {
            return;
        };
        // The parser has left scope. Persist pending data without invoking
        // handlers, then drain every watcher even if saving failed.
        let save = {
            let mut universals = uvars();
            let result = universals.sync_checked();
            if result.is_ok() {
                UVARS_LOCALLY_MODIFIED.store(false);
            }
            result
        };
        if let Err(error) = save {
            crate::flog::flog!(
                error,
                "Unable to synchronize universal variables at shutdown:",
                error
            );
        }
        // Automatic history persistence logs failures and retains uncommitted
        // changes without replacing the final command's exit status.
        crate::history::save_all();
        if let Err(error) = owner.shutdown() {
            crate::flog::flog!(error, "Unable to drain native state notifications:", error);
        }
    }
}

pub(super) fn initialized() -> bool {
    STATE.lock().unwrap().is_some()
}

impl NativeState {
    fn queue(&self, events: Vec<Event>) {
        self.events.lock().unwrap().extend(events);
    }

    fn shutdown(&self) -> io::Result<()> {
        let universal = self.universal_watch.shutdown();
        let history = self.history_watch.shutdown();
        universal.and(history)
    }

    fn changed(&self) -> io::Result<bool> {
        // The runtime belongs only to these state sources. Directory owners
        // retain their completion data; queue records are owned wakeup hints.
        for completion in self.runtime.drain() {
            if !self
                .universal_watch
                .owns_registration(completion.registration)
                && !self
                    .history_watch
                    .owns_registration(completion.registration)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unknown native state completion source",
                ));
            }
            if let NativeCompletionPayload::Error(error) = completion.payload {
                return Err(error);
            }
        }
        let universal = self.universal_watch.check();
        let history = self.history_watch.check();
        // Both watchers are serviced even if one has lost its directory.
        let universal = universal?;
        if history? {
            crate::history::invalidate_native_snapshots();
        }
        Ok(universal)
    }
}

impl EnvStack {
    /// Keep cleanup earlier than initialization's parser so every early return
    /// drops the parser first and then cancels/drains persistent state ownership.
    pub fn native_state_guard(&self) -> impl Drop + use<> {
        crate::threads::assert_is_main_thread();
        NativeStateGuard(std::marker::PhantomData)
    }

    pub fn native_state_initialized(&self) -> bool {
        initialized()
    }

    /// Initialize one native shell's real persistent state after importing its
    /// environment. Basic initialize_native deliberately remains side-effect-free
    /// with respect to config/history, for tools and isolated library consumers.
    pub fn initialize_native_state(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        if initialized() {
            return Ok(());
        }
        crate::path::initialize_native_paths(self)?;
        crate::env::config_paths::initialize_native_variables(self)?;
        let config = crate::path::path_get_config();
        let data = crate::path::path_get_data();
        if !config.ok || !data.ok {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "native state directories are unavailable",
            ));
        }
        let config = PathBuf::from(shell_text_to_osstring(config.path)?);
        let data = PathBuf::from(shell_text_to_osstring(data.path)?);
        let runtime = NativeRuntime::new()?;
        // Arm before the initial read, so a replacement during initialization
        // cannot fall into a load-then-watch notification gap.
        let universal_watch = NativeDirectoryWatcher::new(
            &config,
            Some("fish_variables".into()),
            Arc::clone(&runtime),
        )?;
        let history_watch = NativeDirectoryWatcher::new(&data, None, Arc::clone(&runtime))?;
        let exported_universals = {
            let mut universals = uvars();
            if !universals.initialized() {
                // No user handlers exist yet. Initial special-variable effects
                // are applied once from the complete environment below.
                let _ = universals.initialize_checked()?;
            }
            universals
                .get_table()
                .iter()
                .filter(|(_, value)| value.exports())
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect::<Vec<_>>()
        };
        // An inherited exported universal must not become a global shadow in
        // this shell. Compare exported strings, as in Unix initialization, but
        // do all environment lookups and removals outside the universal lock.
        let global_exported = EnvSetMode::new_at_early_startup(EnvMode::GLOBAL_EXPORTED);
        for (name, universal) in exported_universals {
            if self
                .getf(&name, EnvMode::GLOBAL_EXPORTED)
                .is_some_and(|global| global.as_string() == universal.as_string())
            {
                self.remove(&name, global_exported);
            }
        }
        let owner = Arc::new(NativeState {
            runtime,
            universal_watch,
            history_watch,
            events: Mutex::new(Vec::new()),
            deferred_mutations: Mutex::new(HashSet::new()),
        });
        *STATE.lock().unwrap() = Some(Arc::clone(&owner));
        if let Err(error) = crate::env_dispatch::env_dispatch_init(self) {
            STATE.lock().unwrap().take();
            let _ = owner.shutdown();
            return Err(error);
        }
        Ok(())
    }

    /// Synchronize under the universal lock, then dispatch callbacks only after
    /// releasing it. A failed transaction leaves both its modified keys and the
    /// dirty flag intact; a later mutation cannot be cleared by an earlier save.
    pub fn universal_sync_checked(
        &self,
        always: bool,
        is_repainting: bool,
    ) -> io::Result<Vec<Event>> {
        self.universal_sync_checked_impl(always, is_repainting, None)
    }

    fn universal_sync_checked_impl(
        &self,
        always: bool,
        is_repainting: bool,
        current_mutation: Option<&wstr>,
    ) -> io::Result<Vec<Event>> {
        if !always && !UVARS_LOCALLY_MODIFIED.load() {
            return Ok(Vec::new());
        }
        let owner = state();
        let (callbacks, deferred) = {
            let mut universals = uvars();
            if !universals.initialized() {
                return Ok(Vec::new());
            }
            let (_, callbacks) = match universals.sync_checked() {
                Ok(result) => result,
                Err(error) => {
                    if let (Some(owner), Some(key)) = (&owner, current_mutation) {
                        owner
                            .deferred_mutations
                            .lock()
                            .unwrap()
                            .insert(key.to_owned());
                    }
                    return Err(error);
                }
            };
            UVARS_LOCALLY_MODIFIED.store(false);
            let mut callbacks = callbacks.unwrap_or_default();
            // The successful current caller owns its ordinary effect/event,
            // even when this key previously failed. Replay only other keys.
            if let Some(key) = current_mutation {
                callbacks.retain(|callback| callback.key.as_utfstr() != key);
            }
            let deferred = if let Some(owner) = &owner {
                let mut pending = owner.deferred_mutations.lock().unwrap();
                if let Some(key) = current_mutation {
                    pending.remove(key);
                }
                // A storage callback already owns notification for its key.
                for callback in &callbacks {
                    pending.remove(&callback.key);
                }
                pending
                    .drain()
                    .map(|name| {
                        let erased = !universals.get_table().contains_key(&name);
                        (name, erased)
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            (callbacks, deferred)
        };
        let mut events = Vec::with_capacity(callbacks.len() + deferred.len());
        for (name, erased) in callbacks
            .into_iter()
            .map(|callback| (callback.key, callback.val.is_none()))
            .chain(deferred)
        {
            env_dispatch_var_change(
                VarChangeMilieu {
                    is_repainting,
                    global_or_universal: true,
                },
                &name,
                self,
            );
            events.push(if erased {
                Event::variable_erase(name)
            } else {
                Event::variable_set(name)
            });
        }
        Ok(events)
    }

    pub(super) fn commit_native_universal_mutation(&self, key: &wstr) -> io::Result<()> {
        let owner =
            state().ok_or_else(|| io::Error::other("native state owner is not initialized"))?;
        owner.queue(self.universal_sync_checked_impl(false, false, Some(key))?);
        Ok(())
    }

    /// Service storage invalidations at evaluator safe points. The caller fires
    /// these owned events on the parser thread, with no environment/watch lock held.
    pub fn service_native_state(&self) -> io::Result<Vec<Event>> {
        crate::threads::assert_is_main_thread();
        let Some(owner) = state() else {
            return Ok(Vec::new());
        };
        let changed = owner.changed()?;
        owner.queue(self.universal_sync_checked(changed, false)?);
        let events = std::mem::take(&mut *owner.events.lock().unwrap());
        Ok(events)
    }

    /// The reader can wait on this owner's runtime notification without
    /// interpreting a HANDLE as a Unix fd or creating another state event loop.
    pub fn native_state_runtime(&self) -> Option<Arc<NativeRuntime>> {
        state().map(|owner| Arc::clone(&owner.runtime))
    }

    pub fn shutdown_native_state(&self) -> io::Result<()> {
        crate::threads::assert_is_main_thread();
        let sync = self.universal_sync_checked(false, false);
        let owner = STATE.lock().unwrap().take();
        let close = owner.map_or(Ok(()), |owner| owner.shutdown());
        sync.map(|_| ()).and(close)
    }
}
