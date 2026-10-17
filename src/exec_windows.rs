//! Native launch and deferred internal scheduling for the shared process model.
use crate::builtins::{
    ErrorCode, STATUS_CMD_ERROR, STATUS_CMD_UNKNOWN, STATUS_INVALID_ARGS, STATUS_NOT_EXECUTABLE,
    builtin_run,
};
use crate::env::{EnvMode, Environment as _, READ_BYTE_LIMIT};
use crate::io::{
    IoBuffer, IoChain, IoStreams, NativeDeferredWriter, NativeEndpoint, OutputStream,
    native_input_pipe, native_output_pipe, native_synchronous_pipe,
};
use crate::parser::{Block, BlockType, Parser, ParserEnvSetMode};
use crate::path::{native_absolute_path, native_resolve_executable};
use crate::prelude::*;
use crate::proc::{Job, JobGroupRef, Pid, ProcStatus, Process, ProcessType};
use fish_common::ScopeGuard;
use fish_widestring::shell_text_to_osstring;
use std::io;
use std::sync::{Arc, atomic::Ordering};

#[path = "exec/windows_launch.rs"]
mod native_launch;
pub use native_launch::{
    NativeChild, NativeLaunchGroup, NativeLaunchPlan, initialize_native_image_identity,
};

fn diagnostic(message: impl std::fmt::Display) {
    let mut err = OutputStream::stderr();
    err.append(&format!("fish: {message}\n"));
    err.flush_and_check_error();
}

fn failure(message: impl std::fmt::Display, code: i32) -> ProcStatus {
    diagnostic(message);
    ProcStatus::from_exit_code(code)
}

fn with_process_assignments<R>(
    parser: &mut Parser,
    process: &Process,
    operation: impl FnOnce(&mut Parser) -> R,
) -> R {
    let block = (!process.variable_assignments.is_empty())
        .then(|| parser.push_block(Block::variable_assignment_block()));
    let parser = &mut **ScopeGuard::new(&mut *parser, |parser| {
        if let Some(block) = block {
            parser.pop_block(block);
        }
    });
    for assignment in &process.variable_assignments {
        parser.set_var(
            &assignment.variable_name,
            ParserEnvSetMode::new(EnvMode::LOCAL_EXPORTED),
            assignment.values.clone(),
        );
    }
    operation(parser)
}

fn run_internal(
    parser: &mut Parser,
    process: &Process,
    job: &Job,
    io: &IoChain,
) -> io::Result<ProcStatus> {
    with_process_assignments(parser, process, |parser| {
        match &process.typ {
            ProcessType::Builtin => {
                let mut out = io.output_stream(1)?;
                let mut err = io.output_stream(2)?;
                let mut streams = IoStreams::new(&mut out, &mut err, io);
                streams.job_group = job.group.clone();
                // Match the shared executor: enclosing block redirects are not direct
                // argument-source redirects, but a local non-close stdin action is.
                streams.stdin_is_directly_redirected = streams.stdin_file.is_some()
                    && (!process.is_first_in_job
                        || process
                            .redirection_specs()
                            .iter()
                            .any(|spec| spec.fd == 0 && !spec.is_close()));
                let mut argv: Vec<&wstr> = process.argv().iter().map(|s| s.as_utfstr()).collect();
                let status = builtin_run(parser, &mut argv, &mut streams);
                if let Some(error) = streams
                    .stdin_file
                    .as_ref()
                    .and_then(|input| input.take_error())
                {
                    streams
                        .err
                        .append(&format!("fish: cannot read input: {error}\n"));
                    streams.err.flush_and_check_error();
                    // builtin_run already owns output flushing and its empty
                    // preserve-status result. Only an actual input failure overrides it.
                    return Ok(if status.is_empty() || status.is_success() {
                        ProcStatus::from_exit_code(STATUS_CMD_ERROR)
                    } else {
                        status
                    });
                }
                Ok(status)
            }
            ProcessType::BlockNode(node) => Ok(parser
                .eval_node(node, io, job.group.as_ref(), BlockType::Top)
                .status),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native internal process type is unavailable",
            )),
        }
    })
}

enum DeferredOutput {
    Internal {
        buffer: IoBuffer,
        input: Arc<NativeEndpoint>,
    },
    Native {
        buffer: IoBuffer,
        target: Arc<NativeEndpoint>,
    },
}

struct NativeStage {
    io: Option<IoChain>,
    plan: Option<NativeLaunchPlan>,
    deferred: Option<DeferredOutput>,
    writer: Option<NativeDeferredWriter>,
    child: Option<Arc<NativeChild>>,
}

enum NativeExecutionFailure {
    Io(io::Error),
    Launch { index: usize, error: io::Error },
    Invalid(io::Error),
}

impl From<io::Error> for NativeExecutionFailure {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl NativeExecutionFailure {
    fn report(self, job: &Job) -> ProcStatus {
        match self {
            Self::Io(error) => failure(
                format_args!("cannot redirect or execute job: {error}"),
                STATUS_CMD_ERROR,
            ),
            Self::Invalid(error) => failure(error, STATUS_INVALID_ARGS),
            Self::Launch { index, error } => {
                let status = match error.kind() {
                    io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput => STATUS_INVALID_ARGS,
                    io::ErrorKind::NotFound => STATUS_CMD_UNKNOWN,
                    _ => STATUS_NOT_EXECUTABLE,
                };
                failure(
                    format_args!(
                        "cannot launch '{}': {error}",
                        job.processes()[index].actual_cmd
                    ),
                    status,
                )
            }
        }
    }
}

/// Native stages start before main-thread evaluation. An internal producer must
/// defer whenever any later internal consumer exists, including across relays.
fn execute_pipeline(
    parser: &mut Parser,
    job: &Job,
    block_io: &IoChain,
) -> Result<(), NativeExecutionFailure> {
    let processes = job.processes();
    let mut stages: Vec<_> = processes
        .iter()
        .map(|_| NativeStage {
            io: Some(block_io.clone()),
            plan: None,
            deferred: None,
            writer: None,
            child: None,
        })
        .collect();
    let last_internal = processes
        .iter()
        .rposition(|process| !matches!(process.typ, ProcessType::External));
    for index in 0..processes.len().saturating_sub(1) {
        let producer = &processes[index];
        let producer_external = matches!(producer.typ, ProcessType::External);
        let consumer_external = matches!(processes[index + 1].typ, ProcessType::External);
        let (read, write) = match (producer_external, consumer_external) {
            (true, true) => native_synchronous_pipe()?,
            (true, false) => native_input_pipe()?,
            (false, true) => {
                let (read, target) = native_output_pipe()?;
                if last_internal.is_some_and(|last| index < last) {
                    let buffer = IoBuffer::new_bounded(READ_BYTE_LIMIT.load(Ordering::Relaxed));
                    let output = NativeEndpoint::buffer(buffer.clone());
                    stages[index].deferred = Some(DeferredOutput::Native { buffer, target });
                    (read, output)
                } else {
                    (read, target)
                }
            }
            (false, false) => {
                let buffer = IoBuffer::new_bounded(READ_BYTE_LIMIT.load(Ordering::Relaxed));
                let input = NativeEndpoint::deferred_input();
                let output = NativeEndpoint::buffer(buffer.clone());
                stages[index].deferred = Some(DeferredOutput::Internal {
                    buffer,
                    input: Arc::clone(&input),
                });
                (input, output)
            }
        };
        stages[index]
            .io
            .as_mut()
            .unwrap()
            .set_endpoint(producer.pipe_write_fd, write, true);
        stages[index + 1]
            .io
            .as_mut()
            .unwrap()
            .set_endpoint(0, read, true);
    }
    // Finish every open/dup/close and native launch plan before child effects.
    for (stage, process) in stages.iter_mut().zip(processes) {
        let base = stage.io.take().unwrap();
        stage.io = Some(base.with_specs(process.redirection_specs(), parser.vars())?);
    }
    let has_external = processes
        .iter()
        .any(|process| matches!(process.typ, ProcessType::External));
    let mut group = if has_external {
        Some(NativeLaunchGroup::new(
            crate::threads::NativeRuntime::new()?
        )?)
    } else {
        None
    };
    for (index, (stage, process)) in stages.iter_mut().zip(processes).enumerate() {
        if !matches!(process.typ, ProcessType::External) {
            continue;
        }
        let io = stage.io.take().unwrap();
        stage.plan = Some(
            with_process_assignments(parser, process, |parser| {
                let executable = native_resolve_executable(&process.actual_cmd)?;
                let cwd = native_absolute_path(&parser.vars().get_working_directory()?)?;
                let environment = parser.vars().export_native_env()?;
                let argv = process
                    .argv()
                    .iter()
                    .map(|argument| {
                        shell_text_to_osstring(argument).map_err(|error| {
                            io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
                        })
                    })
                    .collect::<io::Result<Vec<_>>>()?;
                group
                    .as_ref()
                    .unwrap()
                    .prepare(&executable, &argv, &cwd, &environment, &io, true)
            })
            .map_err(|error| NativeExecutionFailure::Launch { index, error })?,
        );
        // Only the plan's explicit duplicates survive this parent map.
    }
    for (index, stage) in stages.iter_mut().enumerate().rev() {
        if let Some(plan) = stage.plan.take() {
            let child = group
                .as_mut()
                .unwrap()
                .spawn_prepared(plan)
                .map_err(|error| NativeExecutionFailure::Launch { index, error })?;
            let process = &processes[index];
            process.set_pid(Pid::from_native(child.pid()));
            parser
                .mut_wait_handles()
                .remove_by_pid(process.pid().unwrap());
            stage.child = Some(child);
        }
    }
    for (stage, process) in stages.iter_mut().zip(processes) {
        if matches!(process.typ, ProcessType::External) {
            continue;
        }
        let io = stage.io.take().unwrap();
        let status = run_internal(parser, process, job, &io)?;
        drop(io);
        process.status.set(status);
        process.completed.store(true);
        match stage.deferred.take() {
            Some(DeferredOutput::Internal { buffer, input }) => {
                input.complete_buffered_input(buffer.take_buffer())?;
            }
            Some(DeferredOutput::Native { buffer, target }) => {
                stage.writer = Some(NativeDeferredWriter::start(buffer.take_buffer(), target)?);
            }
            None => {}
        }
    }
    for (stage, process) in stages.iter_mut().zip(processes) {
        if let Some(writer) = stage.writer.take() {
            if writer.finish().is_err()
                && (process.status.get().is_empty() || process.status.get().is_success())
            {
                process
                    .status
                    .set(ProcStatus::from_exit_code(STATUS_CMD_ERROR));
            }
        }
        if let Some(child) = stage.child.take() {
            let raw = child.wait(None)?;
            if raw > 255 {
                diagnostic(format_args!(
                    "native process exited with status 0x{raw:08X}"
                ));
            }
            process.status.set(ProcStatus::native(raw));
            process.completed.store(true);
        }
    }
    if let Some(group) = group {
        group.finish()?;
    }
    Ok(())
}

pub fn exec_job(parser: &mut Parser, job: &Job, block_io: IoChain) -> bool {
    let unsupported = job.is_initially_background()
        || job.processes().iter().any(|process| {
            matches!(
                process.typ,
                ProcessType::Exec | ProcessType::Function { .. }
            )
        });
    let result = if unsupported {
        Err(NativeExecutionFailure::Invalid(io::Error::new(
            io::ErrorKind::Unsupported,
            "background jobs, exec and shell functions are unavailable",
        )))
    } else if job.processes().iter().any(|process| {
        process
            .argv()
            .iter()
            .any(|argument| argument.contains('\0'))
    }) {
        Err(NativeExecutionFailure::Invalid(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in command arguments is unsupported",
        )))
    } else {
        execute_pipeline(parser, job, &block_io)
    };
    if let Err(error) = result {
        let status = error.report(job);
        for process in job.processes() {
            if !process.completed.load() {
                process.status.set(status);
                process.completed.store(true);
            }
        }
    }
    job.mark_constructed();
    job.continue_job(parser, Some(&block_io));
    true
}

/// Command substitution is rejected by the evaluator before expansion. Keep an explicit
/// error at this shared expansion entry point as well; never evaluate the command.
pub fn exec_subshell_for_expand(
    _cmd: &wstr,
    _parser: &mut Parser,
    _job_group: Option<&JobGroupRef>,
    _outputs: &mut Vec<WString>,
) -> Result<(), ErrorCode> {
    diagnostic("command substitution is unsupported on native Windows");
    Err(STATUS_INVALID_ARGS)
}

pub fn exec_subshell(
    _cmd: &wstr,
    _parser: &mut Parser,
    _outputs: Option<&mut Vec<WString>>,
    _apply_exit_status: bool,
) -> Result<(), ErrorCode> {
    diagnostic("subshell execution is unsupported on native Windows");
    Err(STATUS_CMD_ERROR)
}
