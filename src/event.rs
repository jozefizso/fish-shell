//! Functions for handling event triggers
//!
//! Because most of these functions can be called by signal handler, it is important to make it well
//! defined when these functions produce output or perform memory allocations, since such functions
//! may not be safely called by signal handlers.

use crate::{
    job_group::MaybeJobId,
    parser::Parser,
    prelude::*,
    proc::{InternalJobId, Pid},
};

use crate::{
    flog::flog,
    io::{IoChain, IoStreams},
    parser::{Block, LoopStatus},
    signal::{RawSignal, signal_check_cancel},
};
#[cfg(unix)]
use crate::{reader::reader_update_termsize, signal::signal_handle};
use fish_common::{ScopeGuard, escape};
use fish_widestring::str2wcstring;
use std::sync::atomic::AtomicU32;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub enum EventType {
    Any,
    Signal,
    Variable,
    ProcessExit,
    JobExit,
    CallerExit,
    Generic,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum EventDescription {
    /// Matches any event type (not always any event, as the function name may limit the choice as
    /// well).
    Any,
    /// A POSIX signal on Unix, or a received native console control on Windows.
    Signal { signal: RawSignal },
    /// An event triggered by a variable update.
    Variable { name: WString },
    /// An event triggered by a process exit.
    ProcessExit {
        /// Process ID. Use [`None`] to match any pid.
        pid: Option<Pid>,
    },
    /// An event triggered by a job exit.
    JobExit {
        /// pid requested by the event, or [`None`] for all.
        pid: Option<Pid>,
        /// `internal_job_id` of the job to match.
        /// If this is 0, we match either all jobs (`pid == ANY_PID`) or no jobs (otherwise).
        internal_job_id: InternalJobId,
    },
    /// An event triggered by a job exit, triggering the 'caller'-style events only.
    CallerExit {
        /// Internal job ID.
        caller_id: InternalJobId,
    },
    /// A generic event.
    Generic {
        /// The parameter describing this generic event.
        param: WString,
    },
}

impl EventDescription {
    fn str_param1(&self) -> Option<&wstr> {
        match self {
            EventDescription::Signal { .. } => None,
            EventDescription::Any
            | EventDescription::ProcessExit { .. }
            | EventDescription::JobExit { .. }
            | EventDescription::CallerExit { .. } => None,
            EventDescription::Variable { name } => Some(name),
            EventDescription::Generic { param } => Some(param),
        }
    }

    fn name(&self) -> &'static wstr {
        match self {
            EventDescription::Any => L!("any"),
            EventDescription::Signal { .. } => L!("signal"),
            EventDescription::Variable { .. } => L!("variable"),
            EventDescription::ProcessExit { .. } => L!("process-exit"),
            EventDescription::JobExit { .. } => L!("job-exit"),
            EventDescription::CallerExit { .. } => L!("caller-exit"),
            EventDescription::Generic { .. } => L!("generic"),
        }
    }

    fn matches_filter(&self, filter: &wstr) -> bool {
        if filter.is_empty() {
            return true;
        }

        match self {
            EventDescription::Any => false,
            EventDescription::ProcessExit { .. }
            | EventDescription::JobExit { .. }
            | EventDescription::CallerExit { .. }
                if filter == "exit" =>
            {
                true
            }
            _ => filter == self.name(),
        }
    }
}

impl From<&EventDescription> for EventType {
    fn from(desc: &EventDescription) -> Self {
        match desc {
            EventDescription::Any => EventType::Any,
            EventDescription::Signal { .. } => EventType::Signal,
            EventDescription::Variable { .. } => EventType::Variable,
            EventDescription::ProcessExit { .. } => EventType::ProcessExit,
            EventDescription::JobExit { .. } => EventType::JobExit,
            EventDescription::CallerExit { .. } => EventType::CallerExit,
            EventDescription::Generic { .. } => EventType::Generic,
        }
    }
}

#[derive(Debug)]
pub struct EventHandler {
    /// Properties of the event to match.
    pub desc: EventDescription,
    /// Name of the function to invoke.
    pub function_name: WString,
    /// A flag set when an event handler is removed from the global list.
    /// Once set, this is never cleared.
    pub removed: AtomicBool,
    /// A flag set when an event handler is first fired.
    pub fired: AtomicBool,
}

impl EventHandler {
    pub fn new(desc: EventDescription, name: Option<WString>) -> Self {
        Self {
            desc,
            function_name: name.unwrap_or_default(),
            removed: AtomicBool::new(false),
            fired: AtomicBool::new(false),
        }
    }

    /// Return true if a handler is "one shot": it fires at most once.
    fn is_one_shot(&self) -> bool {
        match self.desc {
            EventDescription::ProcessExit { pid } => pid.is_some(),
            EventDescription::JobExit { pid, .. } => pid.is_some(),
            EventDescription::CallerExit { .. } => true,
            EventDescription::Signal { .. } => false,
            EventDescription::Variable { .. }
            | EventDescription::Generic { .. }
            | EventDescription::Any => false,
        }
    }

    /// Tests if this event handler matches an event that has occurred.
    fn matches(&self, event: &Event) -> bool {
        match (&self.desc, &event.desc) {
            (EventDescription::Any, _) => true,
            (
                EventDescription::Signal { signal },
                EventDescription::Signal { signal: ev_signal },
            ) => signal == ev_signal,
            (EventDescription::Variable { name }, EventDescription::Variable { name: ev_name }) => {
                name == ev_name
            }
            (
                EventDescription::ProcessExit { pid },
                EventDescription::ProcessExit { pid: ev_pid },
            ) => pid.is_none() || pid == ev_pid,
            (
                EventDescription::JobExit {
                    pid,
                    internal_job_id,
                },
                EventDescription::JobExit {
                    internal_job_id: ev_internal_job_id,
                    ..
                },
            ) => pid.is_none() || internal_job_id == ev_internal_job_id,
            (
                EventDescription::CallerExit { caller_id },
                EventDescription::CallerExit {
                    caller_id: ev_caller_id,
                },
            ) => caller_id == ev_caller_id,
            (
                EventDescription::Generic { param },
                EventDescription::Generic { param: ev_param },
            ) => param == ev_param,
            (_, _) => false,
        }
    }
}
type EventHandlerList = Vec<Arc<EventHandler>>;

#[derive(Clone)]
pub struct Event {
    desc: EventDescription,
    arguments: Vec<WString>,
    // A queued native lifecycle event must pin its PID independently of job
    // removal and bounded wait-cache eviction. This is lifetime, not value data.
    #[cfg(windows)]
    _native_process: Option<Arc<crate::exec::NativeChild>>,
}

impl std::fmt::Debug for Event {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Event")
            .field("desc", &self.desc)
            .field("arguments", &self.arguments)
            .finish()
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.desc == other.desc && self.arguments == other.arguments
    }
}

impl Event {
    fn new(desc: EventDescription, arguments: Vec<WString>) -> Self {
        Self {
            desc,
            arguments,
            #[cfg(windows)]
            _native_process: None,
        }
    }

    fn process_exit_value(pid: Pid, status: i32) -> Self {
        Self::new(
            EventDescription::ProcessExit { pid: Some(pid) },
            vec![
                "PROCESS_EXIT".into(),
                pid.to_string().into(),
                status.to_string().into(),
            ],
        )
    }

    /// The current process implicitly pins its own PID until it actually exits.
    pub fn shell_exit(status: i32) -> Self {
        #[cfg(windows)]
        let pid = Pid::from_native(std::process::id());
        #[cfg(not(windows))]
        let pid = Pid::new(
            std::process::id()
                .try_into()
                .expect("process ID fits pid_t"),
        );
        Self::process_exit_value(pid, status)
    }

    pub fn generic(desc: WString) -> Self {
        Self::new(EventDescription::Generic { param: desc }, vec![])
    }

    pub fn variable_erase(name: WString) -> Self {
        Self::new(
            EventDescription::Variable { name: name.clone() },
            vec!["VARIABLE".into(), "ERASE".into(), name],
        )
    }

    pub fn variable_set(name: WString) -> Self {
        Self::new(
            EventDescription::Variable { name: name.clone() },
            vec!["VARIABLE".into(), "SET".into(), name],
        )
    }

    pub fn process_exit(
        #[cfg(not(windows))] pid: Pid,
        #[cfg(windows)] process: Arc<crate::exec::NativeChild>,
        status: i32,
    ) -> Self {
        #[cfg(windows)]
        let pid = Pid::from_native(process.pid());
        let event = Self::process_exit_value(pid, status);
        #[cfg(windows)]
        let event = Self {
            _native_process: Some(process),
            ..event
        };
        event
    }

    pub fn job_exit(
        #[cfg(not(windows))] pid: Pid,
        #[cfg(windows)] process: Arc<crate::exec::NativeChild>,
        jid: InternalJobId,
        #[cfg(windows)] status: i32,
    ) -> Self {
        #[cfg(windows)]
        let pid = Pid::from_native(process.pid());
        #[cfg(not(windows))]
        let status = 0; // Historical Unix job-exit argument.
        let event = Self::new(
            EventDescription::JobExit {
                pid: Some(pid),
                internal_job_id: jid,
            },
            vec![
                "JOB_EXIT".into(),
                pid.to_string().into(),
                status.to_string().into(),
            ],
        );
        #[cfg(windows)]
        let event = Self {
            _native_process: Some(process),
            ..event
        };
        event
    }

    pub fn caller_exit(internal_job_id: InternalJobId, job_id: MaybeJobId) -> Self {
        Self::new(
            EventDescription::CallerExit {
                caller_id: internal_job_id,
            },
            vec!["JOB_EXIT".into(), job_id.to_wstring(), "0".into()], // Historical caller argument.
        )
    }

    /// A native console-control receipt on Windows; a POSIX signal on Unix.
    pub fn signal(signal: RawSignal) -> Self {
        Self::new(
            EventDescription::Signal { signal },
            vec![signal.name().into()],
        )
    }

    /// Test if specified event is blocked.
    fn is_blocked(&self, parser: &Parser) -> bool {
        for block in parser.blocks_iter_rev() {
            if block.event_blocks {
                return true;
            }
        }

        parser.global_event_blocks != 0
    }
}

pub fn get_desc(parser: &Parser, evt: &Event) -> WString {
    let s = match &evt.desc {
        EventDescription::Signal { signal } => {
            format!("signal handler for {} ({})", signal.name(), signal.desc(),)
        }
        EventDescription::Variable { name } => format!("handler for variable '{name}'"),
        EventDescription::ProcessExit { pid: None } => "exit handler for any process".to_owned(),
        EventDescription::ProcessExit { pid: Some(pid) } => {
            format!("exit handler for process {pid}")
        }
        EventDescription::JobExit { pid, .. } => {
            if let Some(pid) = pid {
                #[cfg(unix)]
                if let Some(job) = parser.job_get_from_pid(*pid) {
                    format!("exit handler for job {}, '{}'", job.job_id(), job.command())
                } else {
                    format!("exit handler for job with pid {pid}")
                }
                #[cfg(windows)]
                format!("exit handler for job with pid {pid}")
            } else {
                "exit handler for any job".to_owned()
            }
        }
        EventDescription::CallerExit { .. } => {
            "exit handler for command substitution caller".to_owned()
        }
        EventDescription::Generic { param } => format!("handler for generic event '{param}'"),
        EventDescription::Any => unreachable!(),
    };

    str2wcstring(&s)
}

mod dispatch {
    use super::*;

    #[cfg(unix)]
    /// All the signals we are interested in are in the 1-32 range (with 32 being the typical SIGRTMAX),
    /// but we can expand it to 64 just to be safe. All code checks if a signal value is within bounds
    /// before handling it.
    const SIGNAL_COUNT: usize = 64;
    #[cfg(windows)]
    const SIGNAL_COUNT: usize = 4;

    #[cfg(unix)]
    struct PendingSignals {
        /// A counter that is incremented each time a pending signal is received.
        counter: AtomicU32,
        /// List of pending signals.
        received: [AtomicBool; SIGNAL_COUNT],
        /// The last counter visible in `acquire_pending()`.
        /// This is not accessed from a signal handler.
        last_counter: Mutex<u32>,
    }

    #[cfg(unix)]
    impl PendingSignals {
        /// Mark a signal as pending. This may be called from a signal handler. We expect only one
        /// signal handler to execute at once. Also note that these may be coalesced.
        pub fn mark(&self, sig: libc::c_int) {
            if let Some(received) = self.received.get(usize::try_from(sig).unwrap()) {
                received.store(true, Ordering::Relaxed);
                self.counter.fetch_add(1, Ordering::Relaxed);
            }
        }

        /// Return the list of signals that were set as the bits in a u64, clearing them.
        pub fn acquire_pending(&self) -> u64 {
            let mut current = self
                .last_counter
                .lock()
                .expect("mutex should not be poisoned");

            // Check the counter first. If it hasn't changed, no signals have been received.
            let count = self.counter.load(Ordering::Acquire);
            if count == *current {
                return 0;
            }

            // The signal count has changed. Store the new counter and fetch all set signals.
            *current = count;
            let mut result = 0;
            for (i, received) in self.received.iter().enumerate() {
                if received.load(Ordering::Relaxed) {
                    result |= 1_u64 << i;
                    received.store(false, Ordering::Relaxed);
                }
            }

            result
        }
    }

    #[cfg(unix)]
    // Required until inline const is stabilized.
    #[allow(clippy::declare_interior_mutable_const)]
    const ATOMIC_BOOL_FALSE: AtomicBool = AtomicBool::new(false);
    #[allow(clippy::declare_interior_mutable_const)]
    const ATOMIC_U32_0: AtomicU32 = AtomicU32::new(0);

    #[cfg(unix)]
    static PENDING_SIGNALS: PendingSignals = PendingSignals {
        counter: AtomicU32::new(0),
        received: [ATOMIC_BOOL_FALSE; SIGNAL_COUNT],
        last_counter: Mutex::new(0),
    };

    /// List of event handlers. **While this is locked to allow safely accessing/modifying the vector,
    /// note that it does NOT provide exclusive access to the [`EventHandler`] objects which are shared
    /// references (in an `Arc<T>`).**
    static EVENT_HANDLERS: Mutex<EventHandlerList> = Mutex::new(Vec::new());

    /// Tracks the number of registered event handlers for each signal.
    /// This is inspected by a signal handler. We assume no values in here overflow.
    static OBSERVED_SIGNALS: [AtomicU32; SIGNAL_COUNT] = [ATOMIC_U32_0; SIGNAL_COUNT];

    /// List of events that have been sent but have not yet been delivered because they are blocked.
    ///
    /// This was part of profile_item_t accessed as parser.libdata().blocked_events and has been
    /// temporarily moved here. There was no mutex around this in the cpp code. TODO: Move it back.
    struct EventDelivery {
        event: Event,
        only_handler: Option<Arc<EventHandler>>,
    }

    impl From<Event> for EventDelivery {
        fn from(event: Event) -> Self {
            Self {
                event,
                only_handler: None,
            }
        }
    }

    static BLOCKED_EVENTS: Mutex<Vec<EventDelivery>> = Mutex::new(Vec::new());

    fn inc_signal_observed(sig: RawSignal) {
        if let Some(sig) = OBSERVED_SIGNALS.get(usize::from(sig)) {
            sig.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn dec_signal_observed(sig: RawSignal) {
        if let Some(sig) = OBSERVED_SIGNALS.get(usize::from(sig)) {
            sig.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Returns whether an event listener is registered for the given signal. This is safe to call from
    /// a signal handler.
    pub fn is_signal_observed(sig: libc::c_int) -> bool {
        // Unix calls this from a signal handler; native controls do not need to.
        usize::try_from(sig)
            .ok()
            .and_then(|index| OBSERVED_SIGNALS.get(index))
            .is_some_and(|s| s.load(Ordering::Relaxed) > 0)
    }

    /// Add an event handler.
    pub fn add_handler(eh: EventHandler) -> Arc<EventHandler> {
        if let EventDescription::Signal { signal } = eh.desc {
            #[cfg(unix)]
            signal_handle(signal);
            inc_signal_observed(signal);
        }

        let handler = Arc::new(eh);
        EVENT_HANDLERS
            .lock()
            .expect("event handler list should not be poisoned")
            .push(Arc::clone(&handler));
        handler
    }

    /// Remove handlers where `pred` returns true. Simultaneously update our `signal_observed` array.
    fn remove_handlers_if(pred: impl Fn(&EventHandler) -> bool) -> usize {
        let mut handlers = EVENT_HANDLERS
            .lock()
            .expect("event handler list should not be poisoned");

        let mut removed = 0;
        for i in (0..handlers.len()).rev() {
            let handler = &handlers[i];
            if pred(handler) {
                handler.removed.store(true, Ordering::Relaxed);
                if let EventDescription::Signal { signal } = handler.desc {
                    dec_signal_observed(signal);
                }
                handlers.remove(i);
                removed += 1;
            }
        }

        removed
    }

    /// Remove all events for the given function name.
    pub fn remove_function_handlers(name: &wstr) -> usize {
        remove_handlers_if(|h| h.function_name == name)
    }

    /// Return all event handlers for the given function.
    pub fn get_function_handlers(name: &wstr) -> EventHandlerList {
        EVENT_HANDLERS
            .lock()
            .expect("event handler list should not be poisoned")
            .iter()
            .filter(|h| h.function_name == name)
            .cloned()
            .collect()
    }

    /// Perform the specified event. Since almost all event firings will not be matched by even a single
    /// event handler, we make sure to optimize the 'no matches' path. This means that nothing is
    /// allocated/initialized unless needed.
    fn fire_internal(parser: &mut Parser, event: &Event, only_handler: Option<&Arc<EventHandler>>) {
        // Suppress fish_trace during events.
        let _saved = parser.push_scope(|s| {
            s.is_event = true;
            s.suppress_fish_trace = true;
        });

        // Capture the event handlers that match this event.
        let fire: Vec<_> = EVENT_HANDLERS
            .lock()
            .expect("event handler list should not be poisoned")
            .iter()
            .filter(|h| h.matches(event) && only_handler.is_none_or(|only| Arc::ptr_eq(h, only)))
            .cloned()
            .collect();

        // Iterate over our list of matching events. Fire the ones that are still present.
        let mut fired_one_shot = false;
        for handler in fire {
            // A previous handler may have erased this one.
            if handler.removed.load(Ordering::Relaxed) {
                continue;
            }

            // Claim one-shot delivery before reentrant evaluation. A callback
            // may register another already-completed handler or reap another job.
            let one_shot = handler.is_one_shot();
            if one_shot && handler.fired.swap(true, Ordering::Relaxed) {
                continue;
            }
            if !one_shot {
                handler.fired.store(true, Ordering::Relaxed);
            }
            fired_one_shot |= one_shot;

            // Construct a buffer to evaluate, starting with the function name and then all the
            // arguments.
            let mut buffer = escape(&handler.function_name);
            for arg in &event.arguments {
                buffer.push(' ');
                buffer.push_utfstr(&escape(arg));
            }

            // Event handlers are not part of the main flow of code, so they are marked as
            // non-interactive.
            let _non_interactive = parser.push_scope(|s| s.is_interactive = false);
            let saved_statuses = parser.last_statuses();
            let saved_returning = std::mem::replace(&mut parser.libdata_mut().returning, false);
            let saved_exit_current_script =
                std::mem::replace(&mut parser.libdata_mut().exit_current_script, false);
            let saved_loop_status =
                std::mem::replace(&mut parser.libdata_mut().loop_status, LoopStatus::Normals);
            let parser = &mut **ScopeGuard::new(&mut *parser, |parser| {
                let requested_exit = parser.libdata().exit_current_script;
                if !requested_exit {
                    parser.set_last_statuses(saved_statuses);
                }
                parser.libdata_mut().returning = saved_returning;
                parser.libdata_mut().exit_current_script =
                    saved_exit_current_script || requested_exit;
                parser.libdata_mut().loop_status = saved_loop_status;
            });

            flog!(
                event,
                "Firing event '",
                event.desc.str_param1().unwrap_or(L!("")),
                "' to handler '",
                handler.function_name,
                "'"
            );

            let b = parser.push_block(Block::event_block(event.clone()));
            parser.eval(&buffer, &IoChain::new());
            parser.pop_block(b);

            if parser.libdata().exit_current_script {
                break;
            }
        }

        if fired_one_shot {
            remove_handlers_if(|h| h.fired.load(Ordering::Relaxed) && h.is_one_shot());
        }
    }

    /// Fire all delayed events attached to the given parser.
    pub fn fire_delayed(parser: &mut Parser) {
        crate::threads::assert_is_main_thread();
        // Do not invoke new event handlers from within event handlers.
        if parser.scope().is_event {
            return;
        }

        // Do not invoke new event handlers if we are unwinding (#6649).
        if signal_check_cancel() != 0 || parser.libdata().exit_current_script {
            return;
        }

        // We unfortunately can't keep this locked until we're done with it because the SIGWINCH handler
        // code might call back into here and we would delay processing of the events, leading to a test
        // failure under CI.
        let mut to_send = std::mem::take(&mut *BLOCKED_EVENTS.lock().expect("Mutex poisoned!"));

        // Append all signal events to to_send.
        // 'signals' contains a bit set for each signal that has been received.
        #[cfg(unix)]
        {
            let mut signals: u64 = PENDING_SIGNALS.acquire_pending();
            while signals != 0 {
                let sig = signals.trailing_zeros() as i32;
                signals &= !(1_u64 << sig);
                let sig = RawSignal::new(sig);

                // HACK: The only variables we change in response to a *signal* are $COLUMNS and $LINES.
                // Do that now.
                if sig == libc::SIGWINCH {
                    reader_update_termsize(parser);
                }
                to_send.push(Event::signal(sig).into());
            }
        }
        #[cfg(windows)]
        match crate::signal::take_pending_console_controls() {
            Ok(pending) => {
                for (control, count) in pending.controls() {
                    for _ in 0..count {
                        to_send.push(Event::signal(control).into());
                    }
                }
            }
            Err(error) => eprintln!("fish: cannot dispatch console controls: {error}"),
        }

        // Never retain the queue lock while invoking a handler: a handler can
        // emit another event, which is queued until main-thread dispatch resumes.
        while !to_send.is_empty() {
            let mut delivered = false;
            #[cfg(windows)]
            let interrupted = || signal_check_cancel() != 0;
            #[cfg(unix)]
            let interrupted = || false;
            for delivery in to_send {
                if delivery.event.is_blocked(parser)
                    || parser.libdata().exit_current_script
                    || interrupted()
                {
                    BLOCKED_EVENTS
                        .lock()
                        .expect("Mutex poisoned")
                        .push(delivery);
                } else {
                    fire_internal(parser, &delivery.event, delivery.only_handler.as_ref());
                    delivered = true;
                }
            }
            if !delivered {
                break;
            }
            to_send = std::mem::take(&mut *BLOCKED_EVENTS.lock().expect("Mutex poisoned"));
        }
    }

    /// Queue a callback-produced event without entering parser state.
    pub fn enqueue(event: Event) {
        BLOCKED_EVENTS
            .lock()
            .expect("Mutex poisoned!")
            .push(event.into());
    }

    /// Enqueue a signal event. Invoked from a signal handler.
    #[cfg(unix)]
    pub fn enqueue_signal(signal: libc::c_int) {
        // Beware, we are in a signal handler
        PENDING_SIGNALS.mark(signal);
    }

    /// Fire the specified event event, executing it on `parser`.
    pub fn fire(parser: &mut Parser, event: Event) {
        crate::threads::assert_is_main_thread();
        // Fire events triggered by signals.
        fire_delayed(parser);

        if event.is_blocked(parser) || parser.libdata().exit_current_script {
            BLOCKED_EVENTS
                .lock()
                .expect("Mutex poisoned!")
                .push(event.into());
        } else {
            fire_internal(parser, &event, None);
        }
    }

    /// Replay only a newly registered lifecycle handler. This is not a new
    /// process/job transition and must not notify existing catch-all handlers.
    pub(crate) fn fire_to_handler(parser: &mut Parser, event: Event, handler: Arc<EventHandler>) {
        crate::threads::assert_is_main_thread();
        fire_delayed(parser);
        if event.is_blocked(parser) || parser.libdata().exit_current_script {
            BLOCKED_EVENTS
                .lock()
                .expect("Mutex poisoned!")
                .push(EventDelivery {
                    event,
                    only_handler: Some(handler),
                });
        } else {
            fire_internal(parser, &event, Some(&handler));
        }
    }

    pub const EVENT_FILTER_NAMES: [&wstr; 7] = [
        L!("signal"),
        L!("variable"),
        L!("exit"),
        L!("process-exit"),
        L!("job-exit"),
        L!("caller-exit"),
        L!("generic"),
    ];

    /// Print all events. If type_filter is not empty, only output events with that type.
    pub fn print(streams: &mut IoStreams, type_filter: &wstr) {
        let mut tmp = EVENT_HANDLERS
            .lock()
            .expect("event handler list should not be poisoned")
            .clone();

        tmp.sort_by(|e1, e2| e1.desc.cmp(&e2.desc));

        let mut last_type = std::mem::discriminant(&EventDescription::Any);
        for evt in tmp {
            // If we have a filter, skip events that don't match.
            if !evt.desc.matches_filter(type_filter) {
                continue;
            }

            // Print a "Event $TYPE" header for each event type.
            // This compares only the event *type*, not the entire event,
            // so we don't compare variable events for different variables as different.
            //
            // This assumes EventDescription::Any is not a valid value for an event to have
            // - it's marked "unreachable!()" below!
            if last_type != std::mem::discriminant(&evt.desc) {
                if last_type != std::mem::discriminant(&EventDescription::Any) {
                    streams.out.append(L!("\n"));
                }

                last_type = std::mem::discriminant(&evt.desc);
                streams.out.append(&sprintf!("Event %s\n", evt.desc.name()));
            }

            match &evt.desc {
                EventDescription::Signal { signal } => {
                    let name: WString = signal.name().into();
                    streams
                        .out
                        .append(&sprintf!("%s %s\n", name, evt.function_name));
                }
                EventDescription::ProcessExit { .. } | EventDescription::JobExit { .. } => {}
                EventDescription::CallerExit { .. } => {
                    streams
                        .out
                        .append(&sprintf!("caller-exit %s\n", evt.function_name));
                }
                EventDescription::Variable { name: param }
                | EventDescription::Generic { param } => {
                    streams
                        .out
                        .append(&sprintf!("%s %s\n", param, evt.function_name));
                }
                EventDescription::Any => unreachable!(),
            }
        }
    }

    /// Fire a generic event with the specified name.
    pub fn fire_generic(parser: &mut Parser, name: WString, arguments: Vec<WString>) {
        fire(
            parser,
            Event::new(EventDescription::Generic { param: name }, arguments),
        );
    }
}

pub use dispatch::*;
