//! Native quota policy, deliberately separate from POSIX soft/hard rlimits.
use super::prelude::*;
use crate::{err_fmt, native_limits::NativeLimits};
use fish_widestring::str2wcstring;

// Complete inventory of Unix ulimit selectors/aliases. No category is silently
// mapped to a non-equivalent Job limit (especially CPU, address space and UID counts).
pub const POSIX_OPTIONS: &[(&str, &str)] = &[
    ("-H", "--hard"), ("-S", "--soft"),
    ("-b", "--socket-buffers"), ("-c", "--core-size"),
    ("-d", "--data-size"), ("-e", "--nice"),
    ("-f", "--file-size"), ("-i", "--pending-signals"),
    ("-l", "--lock-size"), ("-m", "--resident-set-size"),
    ("-n", "--file-descriptor-count"), ("-q", "--queue-size"),
    ("-r", "--realtime-priority"), ("-s", "--stack-size"),
    ("-t", "--cpu-time"), ("-u", "--process-count"),
    ("-v", "--virtual-memory-size"), ("-w", "--swap-size"),
    ("-y", "--realtime-maxtime"), ("-K", "--kernel-queues"),
    ("-P", "--ptys"), ("-T", "--threads"),
];
const NATIVE_OPTIONS: [&wstr; 4] = [
    L!("--native-process-memory-kib"),
    L!("--native-job-memory-kib"),
    L!("--native-process-cpu-seconds"),
    L!("--native-active-processes"),
];
fn values(policy: &NativeLimits) -> [Option<u64>; 4] {
    [policy.process_memory_kib, policy.job_memory_kib,
     policy.process_cpu_seconds, policy.active_processes]
}
fn report(streams: &mut IoStreams, value: Option<u64>) {
    match value {
        Some(value) => streams.out.appendln(&str2wcstring(&value.to_string())),
        None => streams.out.appendln(L!("not-configured")),
    };
}
pub fn ulimit(parser: &mut Parser, streams: &mut IoStreams, args: &mut [&wstr]) -> BuiltinResult {
    let cmd = args[0];
    let operands = &args[1..];
    if operands == [L!("--help")] || operands == [L!("-h")] {
        streams.out.appendln(L!("ulimit --native-process-memory-kib [VALUE]\nulimit --native-job-memory-kib [VALUE]\nulimit --native-process-cpu-seconds [VALUE]\nulimit --native-active-processes [VALUE]\nulimit --all\nConfigured defaults for FUTURE native child groups of this parser only, lasting until reset or parser exit.\nMemory is committed memory in KiB (1024 bytes); CPU is per-process USER seconds.\nActive processes counts this isolated Job, not processes of a user.\nOmit VALUE to query configured policy, not effective ancestor limits.\nVALUE must be a positive decimal integer; zero is invalid.\nVALUE not-configured removes this default for future groups, not ancestor limits.\nNo existing group, root fish, or enclosing/host Job is changed.\nValidation uses an empty private Job with the current token; no extra privilege is requested.\nKernel policy/permission failures return nonzero without changing configured defaults.\nLaunch failures abort the child group, never retry without the configured quotas.\nPOSIX categories, soft/hard, unlimited and implicit file-size forms are unsupported."));
        return Ok(SUCCESS);
    }
    if operands == [L!("--all")] || operands == [L!("-a")] {
        let policy = parser.libdata().native_limits;
        for (name, value) in NATIVE_OPTIONS.iter().zip(values(&policy)) {
            streams.out.append(*name);
            streams.out.append(L!(" "));
            report(streams, value);
        }
        return Ok(SUCCESS);
    }
    let selected = operands.first().and_then(|arg| NATIVE_OPTIONS.iter().position(|name| name == arg));
    let Some(selected) = selected else {
        err_fmt!("POSIX ulimit categories and implicit file-size forms are unsupported on native Windows; use --help for explicit future-child native quotas")
            .cmd(cmd).finish(streams);
        return Err(STATUS_INVALID_ARGS);
    };
    if operands.len() == 1 {
        report(streams, values(&parser.libdata().native_limits)[selected]);
        return Ok(SUCCESS);
    }
    if operands.len() != 2 {
        err_fmt!("select exactly one native quota and at most one value")
            .cmd(cmd).finish(streams);
        return Err(STATUS_INVALID_ARGS);
    }
    let value = operands[1];
    let value = if value == "not-configured" {
        None
    } else {
        let number = value.chars().try_fold(0u64, |n, c| {
            if !c.is_ascii_digit() { return None; }
            n.checked_mul(10)?.checked_add(u64::from(c as u8 - b'0'))
        });
        let Some(number) = number.filter(|n| *n != 0 && !value.is_empty()) else {
            err_fmt!("native quota must be a positive decimal integer or not-configured; zero, POSIX soft/hard and unlimited are unsupported")
                .cmd(cmd).finish(streams);
            return Err(STATUS_INVALID_ARGS);
        };
        Some(number)
    };
    let mut candidate = parser.libdata().native_limits;
    match selected {
        0 => candidate.process_memory_kib = value,
        1 => candidate.job_memory_kib = value,
        2 => candidate.process_cpu_seconds = value,
        3 => candidate.active_processes = value,
        _ => unreachable!(),
    }
    if let Err(error) = candidate.validate() {
        err_fmt!("native quota rejected before changing configured policy: %s", str2wcstring(&error.to_string()))
            .cmd(cmd).finish(streams);
        return Err(STATUS_CMD_ERROR);
    }
    parser.libdata_mut().native_limits = candidate;
    Ok(SUCCESS)
}
