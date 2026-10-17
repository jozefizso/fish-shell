# Process execution and job control

## Scope

[`exec.rs`](../../src/exec.rs), [`proc.rs`](../../src/proc.rs), [`job_group.rs`](../../src/job_group.rs), [`tty_handoff.rs`](../../src/tty_handoff.rs), [`fork_exec/`](../../src/fork_exec/), and [`wait_handle.rs`](../../src/wait_handle.rs) launch commands, build pipelines, track jobs, transfer terminal ownership, collect status, implement foreground/background behavior, and support `exec`, `jobs`, `fg`, `bg`, and `disown`. This is a source audit and proposed native-Windows contract, not an executed port or compile report.

## Native Linux/POSIX use

This is the largest native-porting boundary.

- `fork` plus async-signal-safe child setup applies redirections, signal masks, process groups, and terminal ownership before `execve`.
- `posix_spawn` is used when its restrictions permit.
- `setpgid`, `getpgrp`, `tcgetpgrp`, and `tcsetpgrp` provide shell job-control groups and terminal foreground ownership.
- `waitpid` status provides normal exit, signal death, stop, and continue transitions.
- `kill`/`killpg` deliver signals to processes and jobs.
- `SIGCHLD` drives asynchronous status collection.
- `/proc/<pid>/stat` supplies Linux jiffies for process-use estimates; absence returns zero.
- external commands receive an `argv`/`envp` pair of NUL-terminated byte strings.
- a shebang-less safe text file may be retried with `/bin/sh` after `ENOEXEC`.

WSL-specific retries work around Linux syscall behavior under WSL; they do not help a native port.

### Component evidence and port scenarios

| Component and concrete source evidence | Linux/POSIX dependence; Windows candidate or gap | Verification scenario (not executed) |
|---|---|---|
| [`exec_job`, `exec_external_command`, `fork_child_for_process`](../../src/exec.rs) | Pipelines and `PgroupPolicy::{Inherit,Join,Lead}` coordinate real PIDs and pgroups; `fork_child_for_process` calls `setpgid` from both parent and child to close a race. Replace child creation with `CreateProcessW`; retain logical job membership independently of Job Objects. | Mixed external/builtin/function pipeline; first/middle/last launch failure; shell remains usable and unrelated children survive. |
| [`can_use_posix_spawn_for_job`](../../src/exec.rs), [`PosixSpawner`](../../src/fork_exec/spawn.rs) | `posix_spawn` sets signal defaults/mask, pgroups and close/dup2 actions. It is excluded when claiming the terminal or when a self-fd redirection needs CLOEXEC cleared. Windows needs explicit handles and console policy, not a mechanical spawn-function replacement. | Command whose redirected target equals the newly opened descriptor; foreground program reads immediately at startup. |
| [`execute_fork`, `child_setup_process`, `execute_setpgid`](../../src/fork_exec/postfork.rs), [`blocked_signals_for_job`, `PATH_BSHELL`](../../src/fork_exec/mod.rs), [`flog_impl_async_safe`](../../src/fork_exec/flog_safe.rs) | Fork retry, async-signal-safe `dup2`/close/CLOEXEC work, terminal claim and signal reset are Unix-specific. Background jobs without job control block SIGINT/SIGQUIT. Stack-based diagnostic formatting avoids post-fork allocation. No Windows fork stage exists; map launch errors before resuming the new child and define background Ctrl-event behavior. | Resource-exhausted launch, redirection failure, diagnostic output with stderr redirected, Ctrl-C during launch; no handles or suspended processes leaked. |
| [`signal_safe_launch_process`, `is_thompson_shell_script`, `launch_process_nofork`, `internal_exec`](../../src/exec.rs) | `execve` accepts byte argv/envp; ENOEXEC text fallback excludes `.fish`, checks payload, and retries `PATH_BSHELL`. `exec` replaces the fish image. Windows has neither kernel shebang dispatch nor same-PID replacement; interpreter selection and absence of `/bin/sh` must be explicit. | Empty arguments, embedded quotes/backslashes, non-ASCII executable/environment; `.fish`, shebang scripts, shebang-less text and binary files; parent observes changed PID for emulated `exec`. |
| [`create_output_stream_for_builtin`, `run_internal_process`, `exec_subshell_for_expand`](../../src/exec.rs), [`InternalProc::mark_exited`](../../src/proc.rs) | Builtins/functions are not necessarily OS child processes: buffered writes use an execution thread pool, propagate write failures, and post `Topic::InternalExit`. Command substitution has separate output limits/splitting. Preserve these in-memory semantics with Windows write endpoints rather than manufacturing child handles. | Output larger than a pipe buffer, downstream early close, nested substitutions, empty output that still truncates a redirected file; no deadlock. |
| [`ProcessType`, `Process`, `Job`, `ProcStatus`, `process_mark_finished_children`, `handle_child_status`](../../src/proc.rs) | External, builtin, function, block and `exec` processes share status handling; Unix wait bits encode stop/continue/signal death, `waitpid(WNOHANG\|WUNTRACED\|WCONTINUED)` runs after topic generation changes. Windows process completion is waitable but has no corresponding stopped/continued encoding. Keep native DWORD exit codes distinct from fish-compatible statuses. | Fast exit before wait registration, simultaneous pipeline exits, nonzero/high-bit Windows exit code, builtin error, `$status` and `$pipestatus` including negation. |
| [`JobGroup`, `JobId::acquire`, `cancel_with_signal`](../../src/job_group.rs) | Job IDs and first-cancellation-wins atomics are portable; stored `Termios` and `pgid` are not. Use console-mode state and native launch-group metadata, not PID-as-job-ID or Job Object-as-pgroup. | Nested functions/subshells share intended cancellation context; recycled OS PID does not cancel or identify a later logical job incorrectly. |
| [`TtyHandoff`](../../src/tty_handoff.rs) | Foreground pgroup ownership and saved termios require `tcgetpgrp`/`tcsetpgrp` and terminal mode operations; the same file precomputes VT keyboard/protocol enable/disable bytes. Windows console modes and VT transport cover mode/protocol restoration, but offer no foreground-pgroup ownership. WSL retries remain Unix workarounds. | Child changes console mode; launch fails; Ctrl-C arrives during handoff; keyboard protocols restore without writing to an invalid/disconnected terminal. |
| [`job_reap`, `hup_jobs`, `add_disowned_job`, `reap_disowned_pids`, `generate_process_exit_events`, `generate_job_exit_events`](../../src/proc.rs) | Unix cleanup reaps even disowned children to avoid zombies, sends HUP/CONT on shutdown, and dispatches language events and job summaries. Windows needs handle-release/accounting and explicit shutdown policy, not zombie reaping or synthetic OS HUP delivery. | Disown then shell exit; no unintended Job Object kill-on-close; process/job/caller exit handlers fire once and completed jobs do not print duplicate summaries. |
| [`WaitHandle`, `WaitHandleStore`](../../src/wait_handle.rs), [`save_wait_handle_for_completed_job`](../../src/proc.rs) | These are fish-language status records (`Rc`, completion cell, bounded LRU keyed by PID), **not kernel wait handles**. The store preserves background status after job removal. No direct Linux dependency in the cache itself except its `Pid` type. Add native ownership elsewhere or explicitly redesign it. | `wait` after a background job has been cleaned up, wait by name/PID, PID reuse, cache eviction, repeated wait and cancellation. |
| [`proc_get_jiffies`, `proc_update_jiffies`, `clock_ticks_to_seconds`, `HAVE_PROC_STAT`](../../src/proc.rs) | `/proc/<pid>/stat` and `sysconf(_SC_CLK_TCK)` are Linux-style accounting; source returns zero when procfs is unavailable. `GetProcessTimes` can supply per-process CPU time while handles remain owned. | Missing procfs on Unix; child exits between accounting samples; native CPU estimates use correct 100-ns-to-seconds conversion. |


## Windows API mapping

| Fish requirement | Windows API | Equivalence and gap |
|---|---|---|
| Spawn executable with explicit arguments/environment/CWD | `CreateProcessW` with UTF-16 command line, environment block, and working-directory path | Direct equivalent at a lower semantic level. Windows passes one command-line string, not `argv`; quoting must follow the target runtime's parser. |
| Select inherited redirection handles | `STARTUPINFOEXW`, `UpdateProcThreadAttribute(PROC_THREAD_ATTRIBUTE_HANDLE_LIST)`, `STARTF_USESTDHANDLES` | Explicitly list intended inheritable child handles and use the required `bInheritHandles` setting. The whitelist does not make arbitrary handles inheritable; create per-launch inheritable duplicates without leaking unrelated handles. |
| Create a job/pipeline group | `CreateJobObjectW`, `AssignProcessToJobObject`; optionally `CREATE_NEW_PROCESS_GROUP` | Job Objects support lifetime and limits, but are not POSIX process groups. Console process groups only affect control-event delivery and do not provide wait/status aggregation. |
| Start children atomically before releasing a pipeline | `CREATE_SUSPENDED`, configure Job Object, then `ResumeThread` | Useful to prevent escape before assignment. Nested-job restrictions and breakaway flags must be handled. |
| Wait/reap without `SIGCHLD` | Process handles with `WaitForSingleObject`, registered waits, or a dedicated wait thread | Process handles are waitable and retain exit state; there is no zombie/reap requirement. Keep handles until status is recorded. |
| Obtain exit status | `GetExitCodeProcess` | No stopped/continued/signal encoding. Maintain synthetic transition state. |
| Send Ctrl-C/Ctrl-Break to a console group | `GenerateConsoleCtrlEvent` with `CREATE_NEW_PROCESS_GROUP` | Limited to processes sharing a console. `CTRL_C_EVENT` targeting is restricted; ConPTY behavior must be tested. |
| Force termination | `TerminateProcess` or terminate a Job Object | Abrupt; no cleanup handlers. Map only to uncatchable termination, not routine `SIGTERM`. |
| Per-process timing | `GetProcessTimes`; `QueryInformationJobObject` for aggregate accounting | Good replacement for `/proc` jiffies, with different units. |
| Replace shell via `execve` | No exact equivalent | Spawn with inherited console/handles, wait, then `ExitProcess`; PID and parentage change. |

## Recommended backend

Create a Windows `Process` record that owns:

- process handle and PID;
- primary-thread handle until resumed;
- logical fish job ID;
- optional Job Object;
- console process-group ID;
- synthetic state (`Running`, `Stopped` only if emulated, `Exited`, `Terminated`);
- native exit code and mapped fish status.

Launch a whole pipeline in phases:

1. Create all pipe handles and the exact inheritance lists.
2. Create each process suspended with its standard handles and environment.
3. Assign each process to its intended pipeline Job Object, if that grouping policy is used, before user code runs.
4. Close parent-side inherited handle copies.
5. Resume all primary threads.
6. Register process handles with a wait service.

A process may fail before the full pipeline exists, including failure to assign a newly created suspended process to the intended Job Object. Cleanup must terminate every process created for that failed pipeline: use the pipeline's Job Object for assigned members and the retained process handle for any unassigned child. Never terminate unrelated jobs or a shared enclosing Job Object. Close every thread/process/pipe handle after cancellation and completion bookkeeping.

## Job-control policy

Windows cannot reproduce POSIX stopped jobs. There is no safe supported equivalent to `SIGTSTP`/`SIGCONT` for arbitrary console applications. `SuspendThread` is per-thread, races with thread creation, and can deadlock while locks are held; it must not implement shell suspension.

Recommended native contract:

- foreground/background execution and waiting: supported;
- Ctrl-C/Ctrl-Break forwarding: supported where console topology permits;
- `jobs`, `wait`, job IDs, pipeline status: supported;
- stopped/continued states, Ctrl-Z suspension, `bg` of a stopped job: explicitly unsupported until a safe mechanism exists;
- `fg`: brings a tracked background job into fish's wait/input-forwarding policy, not a window-focus operation;
- `disown`: suppress fish ownership/notifications while preserving process lifetime; a process cannot simply be removed from an assigned Job Object, so initial Job Object/breakaway/kill-on-close policy must permit this contract.

## Command-line construction

`CreateProcessW` does not accept an argv array. Use a dedicated encoder for programs following `CommandLineToArgvW`/MSVCRT rules. Batch files require a separate, carefully tested `cmd.exe` encoder. Never concatenate untrusted arguments into `cmd.exe /c` using the C-runtime quoting algorithm.

Executable resolution must be centralized with completion. Decide explicit-path behavior, executable-extension/`PATHEXT` support and batch interpreter selection. Supplying an explicit `lpApplicationName` avoids accidentally adopting `CreateProcessW`'s implicit current-directory/system-directory search. `PATHEXT` expansion is shell policy, not a feature automatically supplied by `CreateProcessW`; do not silently import `cmd.exe` search ordering.

## Classification

**POSIX-backed with major semantic gaps.** This backend is the critical path for a useful native port.