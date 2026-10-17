//! Synchronous execution for the native Windows batch capability set.
//! This dispatches the shared process model and evaluator; it is not a second evaluator.
use crate::builtins::{
    ErrorCode, STATUS_CMD_ERROR, STATUS_CMD_UNKNOWN, STATUS_INVALID_ARGS, STATUS_NOT_EXECUTABLE,
    builtin_run,
};
use crate::env::{EnvMode, Environment as _};
use crate::io::{IoChain, IoStreams, OutputStream, flush_standard_streams};
use crate::parser::{Block, BlockType, Parser, ParserEnvSetMode};
use crate::path::{native_absolute_path, native_resolve_executable};
use crate::prelude::*;
use crate::proc::{Job, JobGroupRef, Pid, ProcStatus, Process, ProcessType};
use fish_common::ScopeGuard;
use fish_widestring::shell_text_to_osstring;
use std::io;
use std::process::{Command, Stdio};

fn diagnostic(message: impl std::fmt::Display) {
    let mut err = OutputStream::stderr();
    err.append(&format!("fish: {message}\n"));
    // The caller is already returning a failure if diagnostics themselves cannot be written.
    err.flush_and_check_error();
}

fn failure(message: impl std::fmt::Display, code: i32) -> ProcStatus {
    diagnostic(message);
    ProcStatus::from_exit_code(code)
}

/// Run exactly one foreground process and record its actual synchronous completion.
/// The existing evaluator owns expansion, scope, control flow, and job construction.
pub fn exec_job(parser: &mut Parser, job: &Job, block_io: IoChain) -> bool {
    if job.processes().len() != 1 || job.is_initially_background() {
        let status = failure(
            "pipelines and background jobs are unsupported on native Windows",
            STATUS_INVALID_ARGS,
        );
        for process in job.processes() {
            process.status.set(status);
            process.completed.store(true);
        }
        job.mark_constructed();
        job.continue_job(parser, Some(&block_io));
        return true;
    }

    let process = &job.processes()[0];
    let status = (|| {
        if matches!(
            process.typ,
            ProcessType::Exec | ProcessType::Function { .. }
        ) {
            failure(
                "exec and shell functions are unsupported on native Windows",
                STATUS_INVALID_ARGS,
            )
        } else if process
            .argv()
            .iter()
            .any(|arg| arg.chars().any(|c| c == '\0'))
        {
            failure(
                "NUL in command arguments is unsupported",
                STATUS_INVALID_ARGS,
            )
        } else {
            let block_io =
                match block_io.with_input_specs(process.redirection_specs(), parser.vars()) {
                    Ok(io) => io,
                    Err(error) => {
                        return failure(
                            format_args!("cannot redirect input: {error}"),
                            STATUS_CMD_ERROR,
                        );
                    }
                };
            let block = if process.variable_assignments.is_empty() {
                None
            } else {
                Some(parser.push_block(Block::variable_assignment_block()))
            };
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
            match &process.typ {
                ProcessType::Builtin => {
                    let mut out = OutputStream::stdout();
                    let mut err = OutputStream::stderr();
                    let mut streams = IoStreams::new(&mut out, &mut err, &block_io);
                    streams.job_group = job.group.clone();
                    let mut argv: Vec<&wstr> =
                        process.argv().iter().map(|s| s.as_utfstr()).collect();
                    builtin_run(parser, &mut argv, &mut streams)
                }
                ProcessType::BlockNode(node) => {
                    parser
                        .eval_node(node, &block_io, job.group.as_ref(), BlockType::Top)
                        .status
                }
                ProcessType::External => run_external(parser, process, &block_io),
                ProcessType::Exec | ProcessType::Function { .. } => unreachable!(),
            }
        }
    })();

    process.status.set(status);
    process.completed.store(true);
    job.mark_constructed();
    job.continue_job(parser, Some(&block_io));
    true
}

fn run_external(parser: &mut Parser, process: &Process, io_chain: &IoChain) -> ProcStatus {
    let result =
        (|| -> io::Result<u32> {
            let executable = native_resolve_executable(&process.actual_cmd)?;
            let cwd = native_absolute_path(&parser.vars().get_working_directory()?)?;
            let environment = parser.vars().export_native_env()?;
            let stdin = match io_chain.input_file() {
                Some(input) => Stdio::from(input.try_clone()?),
                None => Stdio::inherit(),
            };
            let mut command = Command::new(executable);
            for argument in process.argv().iter().skip(1) {
                command.arg(shell_text_to_osstring(argument).map_err(|error| {
                    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
                })?);
            }
            command
                .env_clear()
                .envs(environment)
                .current_dir(cwd)
                .stdin(stdin)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            flush_standard_streams()?;
            let mut child = command.spawn()?;
            process.set_pid(Pid::from_native(child.id()));
            parser
                .mut_wait_handles()
                .remove_by_pid(process.pid().unwrap());
            let status = match child.wait() {
                Ok(status) => status,
                Err(error) => {
                    // Do not abandon a test-owned/running child if waiting itself fails.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
            };
            // On Windows code() retains the DWORD bits in its i32 return value.
            Ok(status
                .code()
                .expect("Windows child completion has an exit code") as u32)
        })();
    match result {
        Ok(raw) => {
            if raw > 255 {
                diagnostic(format_args!(
                    "native process exited with status 0x{raw:08X}"
                ));
            }
            ProcStatus::native(raw)
        }
        Err(error) => {
            let code = if error.kind() == io::ErrorKind::NotFound {
                STATUS_CMD_UNKNOWN
            } else {
                STATUS_NOT_EXECUTABLE
            };
            failure(
                format_args!("cannot launch '{}': {error}", process.actual_cmd),
                code,
            )
        }
    }
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
