// Implementation of the disown builtin.

use super::prelude::*;
use crate::builtins::Error;
use crate::builtins::HelpOnlyCmdOpts;
use crate::io::IoStreams;
use crate::parser::Parser;
use crate::proc::Job;
#[cfg(unix)]
use crate::proc::add_disowned_job;
use crate::{err_fmt, err_str};
use fish_widestring::wstr;
#[cfg(unix)]
use nix::sys::signal::{Signal, killpg};

/// Helper for builtin_disown.
#[cfg(unix)]
fn disown_job(_parser: &mut Parser, cmd: &wstr, streams: &mut IoStreams, j: &Job) -> BuiltinResult {
    // Nothing to do if already disowned.
    if j.flags().disown_requested {
        return Ok(SUCCESS);
    }

    // Stopped disowned jobs must be manually signaled; explain how to do so.
    let pgid = j.pgid();
    if j.is_stopped() {
        if let Some(pgid) = pgid {
            _ = killpg(pgid.as_nullable_pid(), Some(Signal::SIGCONT));
        }
        err_fmt!(
            "job %d ('%s') was stopped and has been signalled to continue.",
            j.job_id(),
            j.command()
        )
        .cmd(cmd)
        .finish(streams);
    }

    // We cannot directly remove the job from the jobs() list as `disown` might be called
    // within the context of a subjob which will cause the parent job to crash in exec_job().
    // Instead, we set a flag and the parser removes the job from the jobs list later.
    j.flags_mut().disown_requested = true;
    add_disowned_job(j);
    Ok(SUCCESS)
}

#[cfg(windows)]
fn disown_job(parser: &mut Parser, cmd: &wstr, streams: &mut IoStreams, j: &Job) -> BuiltinResult {
    crate::proc::native_disown_job(parser, j).map_err(|error| {
        err_fmt!("Failed to disown job: %s", error.to_string())
            .cmd(cmd)
            .finish(streams);
        STATUS_CMD_ERROR
    })?;
    Ok(SUCCESS)
}

/// Builtin for removing jobs from the job list.
pub fn disown(parser: &mut Parser, streams: &mut IoStreams, args: &mut [&wstr]) -> BuiltinResult {
    let opts = HelpOnlyCmdOpts::parse(args, parser, streams)?;

    let cmd = args[0];
    if opts.print_help {
        return builtin_print_help(parser, streams, cmd);
    }

    let mut retval: BuiltinResult;
    if opts.optind == args.len() {
        // Select last constructed job (ie first job in the job queue) that is possible to disown.
        // Stopped jobs can be disowned (they will be continued).
        // Foreground jobs can be disowned.
        // Even jobs that aren't under job control can be disowned!
        let mut job = None;
        for j in parser.jobs() {
            if j.is_constructed() && !j.is_completed() && !j.flags().disown_requested {
                job = Some(j.clone());
                break;
            }
        }

        if let Some(job) = job {
            retval = disown_job(parser, cmd, streams, &job);
        } else {
            err_str!(Error::NO_SUITABLE_JOBS).cmd(cmd).finish(streams);
            retval = Err(STATUS_CMD_ERROR);
        }
    } else {
        retval = Ok(SUCCESS);

        // If one argument is not a valid pid (i.e. integer >= 0), fail without disowning anything,
        // but still print errors for all of them.
        // Non-existent jobs aren't an error, but information about them is useful.
        let mut jobs: Vec<_> = args[opts.optind..]
            .iter()
            .filter_map(|arg| {
                let pid = match parse_pid(streams, cmd, arg) {
                    Ok(pid) => pid,
                    Err(code) => {
                        retval = Err(code);
                        return None;
                    }
                };
                parser.job_get_from_pid(pid).or_else(|| {
                    // Valid identifier but no such job
                    err_fmt!(Error::COULD_NOT_FIND_JOB, pid)
                        .cmd(cmd)
                        .finish(streams);
                    None
                })
            })
            .collect();

        retval?;

        // One PID/JID may be repeated or multiple PIDs may refer to the same job;
        // include the job only once.
        jobs.sort_unstable_by_key(|job| job.job_id());
        jobs.dedup_by_key(|job| job.job_id());

        // Disown all target jobs.
        for j in jobs {
            disown_job(parser, cmd, streams, &j)?;
        }
    }

    retval
}
