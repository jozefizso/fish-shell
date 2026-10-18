//! Main-thread lifecycle publication over retained native process/thread waits.
//! Native callbacks publish readiness only. Even under-construction owners are
//! observed before reentrant evaluation can wait on the shared notification.
use super::{Job, ProcStatus, Statuses};
use crate::event::{self, Event};
use crate::exec::{NativeBreakOutcome, record_stage_io_error};
use crate::io::{IoChain, OutputStream};
use crate::parser::{Block, Parser};
use crate::prelude::*;
use crate::threads::NativeRuntime;
use std::io;
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::rc::Rc;
use std::sync::Arc;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitForMultipleObjects(count: u32, handles: *const RawHandle, all: i32, timeout: u32)
    -> u32;
}

#[derive(Default)]
pub struct NativeJobs {
    runtime: Option<Arc<NativeRuntime>>,
    // Disown removes user selection/notifications, not pending native IO. Existing
    // wait references still receive real completion; no new cache entry is made.
    disowned: Vec<Rc<Job>>,
}

pub fn native_runtime(parser: &mut Parser) -> io::Result<Arc<NativeRuntime>> {
    let jobs = &mut parser.libdata_mut().native_jobs;
    if jobs.runtime.is_none() {
        jobs.runtime = Some(NativeRuntime::new()?);
    }
    Ok(Arc::clone(jobs.runtime.as_ref().unwrap()))
}

pub fn native_runtime_if_initialized(parser: &Parser) -> Option<Arc<NativeRuntime>> {
    parser
        .libdata()
        .native_jobs
        .runtime
        .as_ref()
        .map(Arc::clone)
}

fn diagnostic(message: impl std::fmt::Display) {
    let mut output = OutputStream::stderr();
    output.append(&format!("fish: {message}\n"));
    output.flush_and_check_error();
}

fn observe_job(job: &Job) -> io::Result<bool> {
    let mut changed = false;
    for process in job.processes() {
        if !process.is_completed() {
            if let Some(child) = process.native_child.get() {
                let observed = child.status_if_exited();
                // A failed observation must not leave a sticky callback record
                // spinning wait-any, or manufacture completion of a live child.
                if observed.is_err() {
                    child.drain_registration()?;
                }
                if let Some(raw) = observed? {
                    child.drain_registration()?;
                    let previous = process.status();
                    let mut status = ProcStatus::native(raw);
                    if !previous.is_empty() {
                        status = status.with_failure(previous.status_value());
                    }
                    process.status.set(status);
                    if raw > 255 {
                        diagnostic(format_args!(
                            "native process exited with status 0x{raw:08X}"
                        ));
                    }
                    let input = process.native_input.borrow_mut().take();
                    if let Some(error) = input.as_ref().and_then(|input| input.input_error()) {
                        record_stage_io_error(process, io::Error::new(error.kind(), error));
                    }
                    drop(input);
                    process.completed.store(true);
                    changed = true;
                }
            }
        }
        let writer_finished = process
            .native_writer
            .borrow()
            .as_ref()
            .map(|writer| writer.is_finished())
            .transpose()?
            .unwrap_or(false);
        if writer_finished {
            let writer = process.native_writer.borrow_mut().take().unwrap();
            if let Err(error) = writer.finish() {
                record_stage_io_error(process, error);
            }
            process.completed.store(true);
            changed = true;
        }
        if process.is_completed() {
            if let Some(wait) = process.wait_handle() {
                if !wait.is_completed() {
                    wait.set_status_and_complete(process.status().status_value());
                }
            }
        }
    }
    if job.is_constructed() && job.is_completed() {
        let group = job.native_group.borrow_mut().take();
        if let Some(group) = group {
            changed = true;
            if let Err(error) = group.finish() {
                job.native_failure
                    .set(Some(crate::builtins::STATUS_CMD_ERROR));
                diagnostic(format_args!(
                    "cannot finish native launch ownership: {error}"
                ));
            }
        }
        let donation = job.native_terminal.borrow_mut().take();
        changed |= donation.is_some();
        drop(donation);
    }
    Ok(changed)
}

fn observe_all(parser: &Parser) -> io::Result<bool> {
    let mut error = None;
    let mut changed = false;
    for job in parser
        .jobs()
        .iter()
        .chain(&parser.libdata().native_jobs.disowned)
    {
        match observe_job(job) {
            Ok(progress) => changed |= progress,
            Err(failure) => {
                error.get_or_insert(failure);
            }
        }
    }
    match error {
        Some(error) => Err(error),
        None => Ok(changed),
    }
}

/// No dequeue/reset here: every signaled process/thread owner selectively retires
/// its own registration. One event scales to any number of native job members.
pub fn native_wait_for_change(parser: &mut Parser) -> io::Result<()> {
    if parser.libdata().exit_current_script {
        return Ok(());
    }
    if observe_all(parser)? {
        job_reap(parser, false, None);
        return Ok(());
    }
    let Some(runtime) = native_runtime_if_initialized(parser) else {
        return Ok(());
    };
    let cancellation = crate::signal::console_control_cancellation();
    let handles = [
        runtime.notification_handle().as_raw_handle(),
        cancellation
            .as_ref()
            .map_or(std::ptr::null_mut(), |token| token.handle().as_raw_handle()),
    ];
    let count = if cancellation.is_some() { 2 } else { 1 };
    match unsafe { WaitForMultipleObjects(count, handles.as_ptr(), 0, u32::MAX) } {
        0 => {
            job_reap(parser, false, None);
            Ok(())
        }
        1 if cancellation.is_some() => Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "native job wait interrupted by console control",
        )),
        u32::MAX => Err(io::Error::last_os_error()),
        result => Err(io::Error::other(format!(
            "unexpected native job wait result {result}"
        ))),
    }
}

pub fn proc_wait_any(parser: &mut Parser) {
    if let Err(error) = native_wait_for_change(parser) {
        diagnostic(error);
    }
}

pub fn native_disown_job(parser: &mut Parser, job: &Job) -> io::Result<()> {
    if job.flags().disown_requested {
        return Ok(());
    }
    if let Some(group) = job.native_group.borrow_mut().as_mut() {
        group.accept()?;
    }
    job.flags_mut().disown_requested = true;
    for process in job.external_procs() {
        if let Some(pid) = process.pid() {
            parser.mut_wait_handles().remove_by_pid(pid);
        }
    }
    Ok(())
}

fn summarize(parser: &mut Parser, job: &Job) {
    let mut command = L!("fish_job_summary").to_owned();
    for argument in [
        job.job_id().to_wstring(),
        L!("0").to_owned(),
        job.command().to_owned(),
        L!("ENDED").to_owned(),
        WString::new(),
    ] {
        command.push(' ');
        command.push_utfstr(&fish_common::escape(&argument));
    }
    let block = parser.push_block(Block::event_block(Event::generic(
        L!("fish_job_summary").to_owned(),
    )));
    let statuses = parser.last_statuses();
    parser.eval(&command, &IoChain::new());
    parser.set_last_statuses(statuses);
    parser.pop_block(block);
}

pub fn job_reap(parser: &mut Parser, interactive: bool, _block_io: Option<&IoChain>) -> bool {
    // Observation must precede the reentrancy guard: a callback may itself wait
    // while an outer constructed or under-construction job has a queued exit.
    if let Err(error) = observe_all(parser) {
        diagnostic(error);
    }
    if parser.scope().is_cleaning_procs {
        return false;
    }
    let _cleaning = parser.push_scope(|scope| scope.is_cleaning_procs = true);
    let mut events = Vec::new();
    let mut completed = Vec::new();
    let mut detached = Vec::new();
    let mut cache = Vec::new();
    for job in parser.jobs() {
        if !job.is_constructed() || job.flags().disown_requested {
            continue;
        }
        for process in job.external_procs() {
            if !process.is_completed() || process.posted_proc_exit.load() {
                continue;
            }
            // Failed preparation/launch has no OS process to publish or cache.
            let Some(child) = process.native_child.get() else {
                continue;
            };
            let wait = if !job.is_foreground() {
                process.make_wait_handle(job.internal_job_id)
            } else {
                process.wait_handle()
            };
            if let Some(wait) = wait {
                if !wait.is_completed() {
                    wait.set_status_and_complete(process.status().status_value());
                }
                if !job.is_foreground() {
                    cache.push(wait);
                }
            }
            process.posted_proc_exit.store(true);
            if !job.from_event_handler() || !job.is_foreground() {
                events.push((
                    Rc::clone(job),
                    Event::process_exit(Arc::clone(child), process.status().status_value()),
                ));
            }
        }
    }
    for wait in cache {
        parser.mut_wait_handles().add(wait);
    }
    parser.jobs_mut().retain(|job| {
        if !job.is_constructed() {
            return true;
        }
        if job.flags().disown_requested {
            detached.push(Rc::clone(job));
            return false;
        }
        if job.is_completed() {
            completed.push(Rc::clone(job));
            return false;
        }
        true
    });
    parser.libdata_mut().native_jobs.disowned.extend(detached);
    for job in &completed {
        if let Some(child) = job
            .external_procs()
            .last()
            .and_then(|process| process.native_child.get())
        {
            let status = job
                .statuses()
                .expect("completed native job has a status")
                .status;
            let mut completion = None;
            for process in job.external_procs() {
                if let Some(wait) = process.wait_handle() {
                    let completion = completion.get_or_insert_with(|| {
                        Arc::new(crate::wait_handle::NativeJobCompletion {
                            status,
                            process: Arc::clone(child),
                        })
                    });
                    wait.mark_native_job_complete(Arc::clone(completion));
                }
            }
            if job.posts_job_exit_events() && (!job.from_event_handler() || !job.is_foreground()) {
                events.push((
                    Rc::clone(job),
                    Event::job_exit(Arc::clone(child), job.internal_job_id, status),
                ));
            }
        }
        events.push((
            Rc::clone(job),
            Event::caller_exit(job.internal_job_id, job.job_id()),
        ));
    }
    parser
        .libdata_mut()
        .native_jobs
        .disowned
        .retain(|job| !job.is_completed());
    let mut printed = false;
    for job in &completed {
        if interactive
            && !job.is_foreground()
            && !job.skip_notification()
            && job.flags().is_group_root
        {
            summarize(parser, job);
            printed = true;
        }
    }
    for (job, event) in events {
        if !job.flags().disown_requested {
            if crate::signal::signal_check_cancel() != 0 {
                event::enqueue(event);
            } else {
                event::fire(parser, event);
            }
        }
    }
    printed
}

fn interrupt_group(parser: &Parser, current: &Job, release_foreground: bool) {
    // This is an explicitly declared native cooperative Ctrl-Break request,
    // prompted by evaluator interruption. A sent request never implies an ACK.
    for job in parser
        .jobs()
        .iter()
        .filter(|job| std::ptr::eq(job.group(), current.group()))
    {
        for process in job.external_procs() {
            let Some(child) = process.native_child.get() else {
                continue;
            };
            if child.last_ctrl_break_attempt().is_some() {
                continue;
            }
            match child.request_ctrl_break() {
                Ok(NativeBreakOutcome::Requested | NativeBreakOutcome::AlreadyExited(_)) => (),
                Ok(NativeBreakOutcome::UnsupportedConsole) => diagnostic(format_args!(
                    "process {} does not share fish's console; cooperative cancellation is unsupported and it may still be running",
                    child.pid()
                )),
                Err(error) => diagnostic(format_args!(
                    "cannot request Ctrl-Break for process {}: {error}; no forced termination was issued",
                    child.pid()
                )),
            }
        }
    }
    if release_foreground {
        current.group().set_is_foreground(false);
    }
}

impl Job {
    /// Stable identity of this owned native launch group, not a POSIX pgid or
    /// a transferable Windows HANDLE. IDs are never recycled within the shell.
    pub fn native_group_id(&self) -> Option<u64> {
        self.has_external_proc().then_some(self.internal_job_id.0)
    }

    pub fn native_cancellation_requested(&self) -> bool {
        self.external_procs().any(|process| {
            !process.is_completed()
                && process
                    .native_child
                    .get()
                    .is_some_and(|child| child.ctrl_break_was_requested())
        })
    }

    pub fn continue_job(&self, parser: &mut Parser, block_io: Option<&IoChain>) {
        if parser.libdata().exit_current_script {
            return;
        }
        // A native command inside a background function must still finish
        // synchronously on the evaluator thread, matching upstream semantics.
        if self.is_initially_background() {
            if crate::signal::signal_check_cancel() == 0 {
                parser.set_last_statuses(Statuses::just(0));
            } else {
                parser.set_last_statuses(Statuses::just(130));
            }
            parser.libdata_mut().status_count += 1;
            return;
        }
        let console_owned = self
            .native_terminal
            .borrow()
            .as_ref()
            .is_some_and(|lease| lease.is_active());
        let mut cancelled = false;
        loop {
            job_reap(parser, false, block_io);
            // Exit requested by an unrelated job's callback must not wait for
            // this child or overwrite the callback's explicit shell status.
            if parser.libdata().exit_current_script {
                return;
            }
            if self.is_completed() {
                break;
            }
            if let Err(error) = native_wait_for_change(parser) {
                if error.kind() == io::ErrorKind::Interrupted {
                    interrupt_group(parser, self, !console_owned);
                    if console_owned {
                        cancelled = true;
                        let controls = crate::signal::pending_console_controls();
                        if controls.close != 0 {
                            crate::reader::native_console_closed();
                            crate::tty_handoff::native_begin_shutdown();
                        }
                        // Receipt counts survive acknowledgement. Handlers run
                        // with a fresh token, while this wait retains provenance.
                        if let Err(error) = crate::signal::rearm_console_controls() {
                            diagnostic(error);
                            parser.set_last_statuses(Statuses::just(1));
                            parser.libdata_mut().status_count += 1;
                            return;
                        }
                        event::fire_delayed(parser);
                        if parser.libdata().exit_current_script {
                            return;
                        }
                        if controls.close == 0 {
                            continue;
                        }
                    }
                    parser.set_last_statuses(Statuses::just(130));
                } else {
                    diagnostic(error);
                    self.group().set_is_foreground(false);
                    parser.set_last_statuses(Statuses::just(1));
                }
                parser.libdata_mut().status_count += 1;
                return;
            }
        }
        if cancelled {
            parser.set_last_statuses(Statuses::just(130));
            parser.libdata_mut().status_count += 1;
            return;
        }
        if let Some(statuses) = self.statuses() {
            parser.set_last_statuses(statuses);
            parser.libdata_mut().status_count += 1;
        } else if self.flags().negate {
            // `set name (substitution)` deliberately preserves the substitution
            // status. Negation still applies when no process supplies a status,
            // matching the shared evaluator's Unix short-circuit path.
            let mut statuses = parser.last_statuses();
            statuses.status = if statuses.status == 0 { 1 } else { 0 };
            parser.set_last_statuses(statuses);
        }
    }
}

/// Called after shell-exit handlers, while the console/state/runtime owners are
/// still alive. Accepted processes survive; only shell-side pumps/waits drain.
pub fn native_shutdown_jobs(parser: &mut Parser) {
    crate::tty_handoff::native_begin_shutdown();
    job_reap(parser, false, None);
    let live = parser
        .jobs()
        .iter()
        .filter(|job| job.is_constructed() && !job.is_completed() && !job.flags().disown_requested)
        .count();
    if live != 0 {
        diagnostic(format_args!(
            "leaving {live} native job(s) running; use wait or disown to make ownership explicit"
        ));
    }
    parser.jobs_mut().clear();
    parser.libdata_mut().native_jobs.disowned.clear();
}
