//! Owned reader work contexts. No parser or console owner crosses this boundary.
use crate::{
    complete::completion_revision,
    env::{EnvMode, EnvStack, EnvVar, Environment},
    history::History,
    operation_context::{OperationContext, EXPANSION_LIMIT_BACKGROUND},
    prelude::*,
};
use std::{collections::BTreeMap, sync::{Arc, atomic::{AtomicU64, Ordering}}};

/// Immutable variables: unlike EnvStack::snapshot(), globals are not shared.
#[derive(PartialEq)]
struct Variables(BTreeMap<WString, Option<EnvVar>>);
impl Variables {
    fn capture(vars: &dyn Environment) -> Self {
        let mut values: BTreeMap<_, _> = vars.get_names(EnvMode::default()).into_iter()
            .map(|name| {
                // GET_HISTORY deliberately returns empty on background EnvScoped.
                // Preserve that policy without reentering the main-thread reader.
                let value = if Self::reader_electric(&name) {
                    Some(EnvVar::new_from_name_vec(&name, vec![]))
                } else {
                    vars.get(&name)
                };
                (name, value)
            }).collect();
        // get_names(default) does not enumerate exported electric PWD. Retain it
        // explicitly for filesystem consumers and synchronous editor-context checks.
        if !values.contains_key(L!("PWD")) {
            values.insert(L!("PWD").to_owned(), vars.get(L!("PWD")));
        }
        Self(values)
    }
    fn reader_electric(name: &wstr) -> bool {
        name == L!("history")
    }
    fn matches(&self, vars: &dyn Environment) -> bool {
        let names = vars.get_names(EnvMode::default());
        let has_pwd = names.iter().any(|name| name == L!("PWD"));
        if names.len() + usize::from(!has_pwd) != self.0.len()
            || (!has_pwd && self.0.get(L!("PWD")) != Some(&vars.get(L!("PWD")))) {
            return false;
        }
        names.iter().all(|name| {
            if Self::reader_electric(name) { return true; }
            self.0.get(name) == Some(&vars.get(name))
        })
    }
}
impl Environment for Variables {
    fn getf(&self, name: &wstr, mode: EnvMode) -> Option<EnvVar> {
        let value = self.0.get(name)?.as_ref()?;
        if mode.export.is_some_and(|export| export != value.exports())
            || mode.pathvar.is_some_and(|pathvar| pathvar != value.is_pathvar()) {
            return None;
        }
        Some(value.clone())
    }
    fn get_names(&self, mode: EnvMode) -> Vec<WString> {
        self.0.keys().filter(|name| {
            (mode.export.is_none() && mode.pathvar.is_none()) || self.getf(name, mode).is_some()
        }).cloned().collect()
    }
}

/// Exact immutable input shared by highlighting, suggestions and history work.
/// The retained native cwd keeps relative I/O independent of later process cwd.
pub(super) struct ContextSnapshot {
    pub text: WString,
    pub cursor: usize,
    pub bind_mode: WString,
    pub history: Arc<History>,
    source_history: Arc<History>,
    variables: Variables,
    history_revision: u64,
    completion_revision: u64,
    function_revision: u64,
    abbreviation_revision: u64,
    #[cfg(windows)]
    cwd: Option<Arc<crate::path::NativeCwd>>,
}
impl Environment for ContextSnapshot {
    fn getf(&self, name: &wstr, mode: EnvMode) -> Option<EnvVar> { self.variables.getf(name, mode) }
    fn get_names(&self, mode: EnvMode) -> Vec<WString> { self.variables.get_names(mode) }
    #[cfg(windows)]
    fn get_working_directory(&self) -> std::io::Result<WString> {
        match &self.cwd {
            Some(cwd) => cwd.physical_path(),
            None => self.variables.get_working_directory(),
        }
    }
}
impl ContextSnapshot {
    fn matches(&self, vars: &EnvStack, text: &wstr, cursor: usize, history: &Arc<History>) -> bool {
        self.text == text && self.cursor == cursor
            && Arc::ptr_eq(&self.source_history, history)
            && self.history_revision == history.revision()
            && self.completion_revision == completion_revision()
            && self.function_revision == crate::function::function_revision()
            && self.abbreviation_revision == crate::abbrs::abbreviation_revision()
            && self.variables.matches(vars)
            && self.cwd_matches(vars)
    }
    #[cfg(windows)]
    fn cwd_matches(&self, vars: &EnvStack) -> bool {
        match (&self.cwd, vars.native_cwd()) {
            (Some(old), Some(new)) => Arc::ptr_eq(old, &new),
            (None, None) => true,
            _ => false,
        }
    }
    #[cfg(not(windows))]
    fn cwd_matches(&self, _vars: &EnvStack) -> bool { true }
}

/// Reader-local lifetime + context generation; equality cannot alias nested readers.
#[derive(Clone)]
pub(super) struct ContextStamp {
    epoch: Arc<AtomicU64>,
    generation: u64,
    pub snapshot: Arc<ContextSnapshot>,
}
impl PartialEq for ContextStamp {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.epoch, &other.epoch) && self.generation == other.generation
    }
}
impl Eq for ContextStamp {}
impl ContextStamp {
    /// Safe on workers; checking cancellation does not lock history or call a parser.
    pub fn is_current(&self) -> bool {
        self.epoch.load(Ordering::Acquire) == self.generation
            && self.snapshot.history_revision == self.snapshot.source_history.revision()
            && self.snapshot.completion_revision == completion_revision()
            && self.snapshot.function_revision == crate::function::function_revision()
            && self.snapshot.abbreviation_revision == crate::abbrs::abbreviation_revision()
    }

    /// A foreground evaluator may legitimately load definitions and update provider
    /// caches during completion. Those effects are part of this same computation,
    /// unlike concurrent changes to the editor being completed.
    /// Call this before refresh(), which would advance the context for those effects.
    pub fn allows_synchronous_publication(
        &self,
        vars: &EnvStack,
        text: &wstr,
        cursor: usize,
        history: &Arc<History>,
    ) -> bool {
        self.epoch.load(Ordering::Acquire) == self.generation
            && self.snapshot.text == text
            && self.snapshot.cursor == cursor
            && self.snapshot.bind_mode == vars.get(L!("fish_bind_mode")).map(|v| v.as_string()).unwrap_or_default()
            && self.snapshot.cwd_matches(vars)
            && self.snapshot.get(L!("PWD")) == vars.get(L!("PWD"))
            && Arc::ptr_eq(&self.snapshot.source_history, history)
            && self.snapshot.history_revision == history.revision()
    }

    /// Already-published prefix suggestions may survive an incremental text edit,
    /// but never changes to the environment in which they were validated.
    /// This is NOT the acceptance check for newly completed worker results.
    pub fn same_environment(&self, other: &Self) -> bool {
        let left = &self.snapshot;
        let right = &other.snapshot;
        #[cfg(windows)]
        let same_cwd = match (&left.cwd, &right.cwd) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        };
        #[cfg(unix)]
        let same_cwd = true;
        same_cwd
            && left.variables == right.variables
            && left.bind_mode == right.bind_mode
            && Arc::ptr_eq(&left.source_history, &right.source_history)
            && left.history_revision == right.history_revision
            && left.completion_revision == right.completion_revision
            && left.function_revision == right.function_revision
            && left.abbreviation_revision == right.abbreviation_revision
    }
    pub fn operation_context(&self) -> OperationContext<'_> {
        let stamp = self.clone();
        OperationContext::background_with_cancel_checker(
            self.snapshot.as_ref(), Box::new(move || !stamp.is_current()), EXPANSION_LIMIT_BACKGROUND,
        )
    }
}

/// Main-thread owner. Dropping/invalidation cancels even when text later returns to its old value.
#[derive(Default)]
pub(super) struct ReaderContext {
    epoch: Arc<AtomicU64>,
    current: Option<ContextStamp>,
    _main_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl ReaderContext {
    /// Refresh before scheduling AND before publication (including needs_load side effects).
    /// Reuses the exact same Arc for highlight/suggestion requests when nothing changed.
    pub fn refresh(&mut self, vars: &EnvStack, text: &wstr, cursor: usize, history: &Arc<History>) -> ContextStamp {
        crate::threads::assert_is_main_thread();
        if let Some(current) = &self.current {
            if current.is_current() && current.snapshot.matches(vars, text, cursor, history) {
                return current.clone();
            }
        }
        self.invalidate();
        let variables = Variables::capture(vars);
        let bind_mode = variables.get(L!("fish_bind_mode")).map(|v| v.as_string()).unwrap_or_default();
        #[cfg(windows)]
        let frozen_history = history.reader_snapshot();
        #[cfg(unix)]
        let frozen_history = Arc::clone(history);
        let history_revision = frozen_history.revision();
        let stamp = ContextStamp {
            epoch: Arc::clone(&self.epoch),
            generation: self.epoch.load(Ordering::Acquire),
            snapshot: Arc::new(ContextSnapshot {
                text: text.to_owned(), cursor, bind_mode, variables,
                history: frozen_history, source_history: Arc::clone(history), history_revision,
                completion_revision: completion_revision(),
                function_revision: crate::function::function_revision(),
                abbreviation_revision: crate::abbrs::abbreviation_revision(),
                #[cfg(windows)]
                cwd: vars.native_cwd(),
            }),
        };
        self.current = Some(stamp.clone());
        stamp
    }
    /// Call on nested-reader push/pop and after evaluator callbacks that may change definitions.
    pub fn invalidate(&mut self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        self.current = None;
    }
}
impl Drop for ReaderContext {
    fn drop(&mut self) { self.invalidate(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{env::EnvSetMode, history::{HistoryId, MemoryHistoryId}};
    use std::sync::mpsc;

    #[test]
    fn capture_never_evaluates_reader_electrics_and_observes_status_changes() {
        struct ElectricProbe(i32);
        impl Environment for ElectricProbe {
            fn get_names(&self, _: EnvMode) -> Vec<WString> {
                vec![L!("history").to_owned(), L!("status").to_owned(), L!("__fish_help_dir").to_owned()]
            }
            fn getf(&self, name: &wstr, _: EnvMode) -> Option<EnvVar> {
                assert_ne!(name, L!("history"), "context capture reentered reader history");
                if name == L!("__fish_help_dir") { return None; }
                Some(EnvVar::new_from_name(name, WString::from(self.0.to_string().as_str())))
            }
        }
        let captured = Variables::capture(&ElectricProbe(0));
        assert!(captured.get(L!("history")).unwrap().is_empty());
        assert!(captured.matches(&ElectricProbe(0)));
        assert!(!captured.matches(&ElectricProbe(1)));
        assert!(captured.get(L!("__fish_help_dir")).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn publication_barrier_rejects_each_context_change() {
        if !crate::threads::isolated_test(concat!(module_path!(), "::publication_barrier_rejects_each_context_change")) {
            return;
        }
        let directory = fish_tempfile::new_dir().unwrap();
        let other = fish_tempfile::new_dir().unwrap();
        for path in [directory.path(), other.path()] {
            std::fs::write(path.join("n7-owned-file"), b"actual").unwrap();
        }
        let vars = EnvStack::new();
        vars.initialize_native_from(std::iter::empty(), directory.path()).unwrap();
        assert!(vars.get_names(EnvMode::default()).contains(&L!("__fish_help_dir").to_owned()));
        assert!(vars.get(L!("__fish_help_dir")).is_none());
        let history = History::new(HistoryId::Memory(MemoryHistoryId::PrivateMode));
        history.add_commandline(L!("echo history-before").to_owned());
        let mut owner = ReaderContext::default();
        let first = owner.refresh(&vars, L!("echo n7-"), 8, &history);
        let reused = owner.refresh(&vars, L!("echo n7-"), 8, &history);
        assert!(Arc::ptr_eq(&first.snapshot, &reused.snapshot));
        assert_eq!(first.snapshot.get(L!("PWD")), vars.get(L!("PWD")));
        assert!(first.allows_synchronous_publication(&vars, L!("echo n7-"), 8, &history));
        let published = std::thread::spawn(move || {
            let mut ctx = first.operation_context();
            let (completions, _) = crate::complete::complete(
                &first.snapshot.text, None, crate::complete::CompletionRequestOptions::Autosuggestion, &mut ctx,
            );
            assert!(!completions.is_empty());
            drop(ctx);
            first
        }).join().unwrap();
        let live = owner.refresh(&vars, L!("echo n7-"), 8, &history);
        assert!(published == live && published.is_current(),
            "unset enumerated electric variables rejected real unchanged completion work");
        for change in 0..10 {
            let old = owner.refresh(&vars, L!("echo n7-"), 8, &history);
            let frozen = Arc::clone(&old.snapshot);
            let prior = old.clone();
            let (started_tx, started_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let mut ctx = old.operation_context();
                let (completions, _) = crate::complete::complete(
                    &old.snapshot.text,
                    None,
                    crate::complete::CompletionRequestOptions::Autosuggestion,
                    &mut ctx,
                );
                assert!(!completions.is_empty(), "real filesystem completion did not run");
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
                ctx.check_cancel()
            });
            started_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            match change {
                0 => {
                    let current = owner.refresh(&vars, L!("echo changed"), 12, &history);
                    assert!(prior.same_environment(&current));
                    assert!(prior != current);
                }
                1 => {
                    let current = owner.refresh(&vars, L!("echo n7-"), 5, &history);
                    assert!(prior.same_environment(&current));
                    assert!(prior != current);
                }
                2 => {
                    vars.set_one(L!("fish_bind_mode"), EnvSetMode::new_at_early_startup(EnvMode::GLOBAL), L!("insert").to_owned());
                    owner.refresh(&vars, L!("echo n7-"), 8, &history);
                    assert_ne!(frozen.bind_mode, L!("insert"));
                }
                3 => {
                    vars.set_one(L!("PATH"), EnvSetMode::new_at_early_startup(EnvMode::GLOBAL), L!("changed-path").to_owned());
                    owner.refresh(&vars, L!("echo n7-"), 8, &history);
                    assert_ne!(frozen.get(L!("PATH")), vars.get(L!("PATH")));
                }
                4 => {
                    let path = crate::path::native_path_text(other.path()).unwrap();
                    vars.set_native_cwd(Arc::new(crate::path::NativeCwd::open(&path, &path, false).unwrap()));
                    owner.refresh(&vars, L!("echo n7-"), 8, &history);
                    assert_ne!(frozen.get_working_directory().unwrap(), vars.get_working_directory().unwrap());
                }
                5 => {
                    history.add_commandline(L!("echo history-after").to_owned());
                    assert!(!frozen.history.get_history().contains(&L!("echo history-after").to_owned()));
                }
                6 => { history.remove(L!("echo history-before")); }
                7 => {
                    let switched = History::new(HistoryId::Memory(MemoryHistoryId::BuiltinRead));
                    owner.refresh(&vars, L!("echo n7-"), 8, &switched);
                }
                8 => {
                    crate::complete::complete_add_wrapper(L!("n7outer").to_owned(), L!("echo").to_owned());
                }
                9 => { owner.invalidate(); }
                _ => unreachable!(),
            }
            if change > 1 {
                let current = owner.refresh(&vars, L!("echo n7-"), 8, &history);
                if change != 7 && change != 9 {
                    assert!(!prior.same_environment(&current));
                }
            }
            release_tx.send(()).unwrap();
            assert!(worker.join().unwrap(), "obsolete context {change} was publishable");
        }
        let old = owner.refresh(&vars, L!("echo n7-"), 8, &history);
        drop(owner);
        assert!(!old.is_current());
    }

    #[cfg(windows)]
    #[test]
    fn definition_changes_reject_old_actual_command_classification() {
        use crate::{highlight::{highlight_shell, HighlightRole}, io::IoChain, parser::{Parser, CancelBehavior}};
        if !crate::threads::isolated_test(concat!(module_path!(), "::definition_changes_reject_old_actual_command_classification")) {
            return;
        }
        crate::common::PROGRAM_NAME.set(L!("fish")).unwrap();
        let directory = fish_tempfile::new_dir().unwrap();
        let vars = EnvStack::new();
        vars.initialize_native_from(std::iter::empty(), directory.path()).unwrap();
        let mut parser = Parser::new(vars, CancelBehavior::default()).unwrap();
        let history = History::new(HistoryId::Memory(MemoryHistoryId::PrivateMode));
        let mut owner = ReaderContext::default();
        for (text, definition, before, after) in [
            (L!("n7_function_context"), L!("function n7_function_context; end"), HighlightRole::Error, HighlightRole::Function),
            (L!("n7_abbr_context"), L!("abbr --add n7_abbr_context echo"), HighlightRole::Error, HighlightRole::Command),
            (L!("n7_function_context"), L!("functions --erase n7_function_context"), HighlightRole::Function, HighlightRole::Error),
            (L!("n7_abbr_context"), L!("abbr --erase n7_abbr_context"), HighlightRole::Command, HighlightRole::Error),
        ] {
            let old = owner.refresh(parser.vars(), text, text.len(), &history);
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let colors = {
                    let mut ctx = old.operation_context();
                    let mut colors = vec![];
                    highlight_shell(&old.snapshot.text, &mut colors, &mut ctx, true, None).unwrap();
                    colors
                };
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
                (old, colors)
            });
            entered_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            let result = parser.eval(definition, &IoChain::new());
            assert_eq!(result.status.status_value(), 0);
            assert!(!result.break_expand);
            release_tx.send(()).unwrap();
            let (old, colors) = worker.join().unwrap();
            assert_eq!(colors[0].foreground, before);
            assert!(!old.is_current(), "obsolete classified command remained publishable");
            assert!(old.allows_synchronous_publication(parser.vars(), text, text.len(), &history),
                "foreground evaluation's own definition updates were rejected");
            let current = owner.refresh(parser.vars(), text, text.len(), &history);
            assert!(!old.same_environment(&current));
            let colors = std::thread::spawn(move || {
                let mut ctx = current.operation_context();
                let mut colors = vec![];
                highlight_shell(&current.snapshot.text, &mut colors, &mut ctx, true, None).unwrap();
                colors
            }).join().unwrap();
            assert_eq!(colors[0].foreground, after);
        }
    }
}
