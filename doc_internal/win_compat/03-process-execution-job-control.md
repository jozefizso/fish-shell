# Process execution and job control

## Current N4 image/startup boundary

`wutil/windows.rs::native_current_exe_path` centralizes trusted OS-reported current-image
identity for executable-relative assets/current-shell consumers, including literal extended
install roots. Its namespace restoration is not accepted for ordinary user paths or a
PATH-selected shell, and does not change N2's handle-based launch/bridge contracts. Normal
startup and real login state are N4; the implemented N5 lifecycle contract is described below.
Fork, same-PID exec and POSIX fg/bg remain permanently excluded. See the
[N4 record](coverage.md#n4-implementation-and-evidence-record)
for the completed native/minimum-Rust/retained matrix.

## Current N2/N3 implementation overlay

`exec_windows.rs` and `exec/windows_launch.rs` now implement prepared native pipeline launch.
The following original audit/candidate scenarios are not the current support ledger; consult
[N2 evidence](coverage.md#n2-verified-evidence-record) and [N3 batch consumers](GOAL_03.md).

All native plans and ordered file opens are prepared before stage launch. `CreateProcessW`
receives explicit application/argv/environment/cwd and `STARTUPINFOEX` handle allowlists;
no global environment, cwd or standard-handle swapping is used. Children are created suspended,
assigned to the operation-owned Job Object, registered with retained process ownership and resumed.
Partial failure terminates/reaps only owned descendants and drains completion ownership.
Acceptance clears per-job kill-on-close policy; normal completion also releases normally surviving descendants.

The executable is pinned using execution/read-attributes access and read-only sharing through
launch. Same-image numbered-fd bridging compares full native file identity, not basename or copied
contents. The initial loader-to-startup path-resolution interval is not a handle-based image-launch
guarantee. Child physical cwd may use an extended namespace spelling; identity, not prefix spelling,
is the filesystem contract.

Native/builtin/function/block pipelines and raw DWORD completion retention are delivered.
Function scope and separated substitution helpers are shared with Unix, while Windows launch
retains its native process and endpoint ownership. Deferred retention faults carry independent
provenance; an ordinary native exit 122 is not an expansion-breaking I/O failure.
User-visible background jobs, control cancellation/events and accounting now have the N5 implementation below.
Faithful fork, same-PID exec and POSIX fg/bg remain permanently excluded.

## Current N5 native lifecycle contract

[`exec_windows.rs`](../../src/exec_windows.rs), [`proc_windows.rs`](../../src/proc_windows.rs),
[`exec/windows_launch.rs`](../../src/exec/windows_launch.rs) and the shared `jobs`, `wait`,
`disown`, function/event and wait-cache consumers implement native lifecycle ownership.
This is a source-based contract, not a claim that every N5 gate has passed. Final artifact,
case and revision evidence belongs in the [N5 implementation and evidence record](coverage.md#n5-implementation-and-evidence-record).
N1–N4 evidence above remains historical and unchanged.

### Progress, input and identity

- Native `&` returns without waiting for its roots, and native/mixed pipelines can continue
  while later commands run. Functions and blocks still evaluate synchronously on the main
  evaluator thread; a native command inside such a function waits normally. `&` does not
  create a concurrent parser or promise arbitrary function evaluation in a worker.
- Internal producers before later internal consumers use bounded deferred retention.
  A background builtin feeding a native consumer also retains its already-produced output
  and drains asynchronously, so a blocked consumer does not prevent a later command from
  releasing it. A final function/block producer instead streams while its synchronous
  evaluator is active. These are distinct scheduling contracts, not universal buffering.
- An initially background pipeline without explicit/inherited stdin redirection gets the
  native `\\.\NUL` input device, not a borrowed console reader. Explicit stdin actions take
  precedence; pipeline inputs and ordered redirections retain their normal endpoint semantics.
- A Windows PID is an unsigned `u32`, not a signed Unix PID or an identity reopened later.
  `NativeChild` retains the process HANDLE through live tracking, cached wait records and
  queued events. That pins the process object and prevents PID reuse while those references
  are held ([Microsoft explanation](https://devblogs.microsoft.com/oldnewthing/20110107-00/?p=11803/)).
  Logical fish job IDs and the nonrecycled native group identifier are separate from PID,
  console process-group number and Job Object HANDLE.

### Lookup, completion and language events

`jobs` implements normal listing, `--command/-c`, `--group/-g`, `--last/-l`,
`--pid/-p` and `--quiet/--query/-q`, with job-ID/PID selection. The native Group column
is a shell-local launch-group identity, not a POSIX pgid. Native listing does not invent a
`/proc` CPU percentage or stopped state; a successful cooperative request can label a still
running job as Ctrl-Break-requested.

`wait` selects all waitable background processes or PID/basename matches, including bounded
cached completed records, and supports `--any/-n`. Successful waiting returns success rather
than the child's exit code; cancellation returns 130. Consumed completed records leave the
cache (at most one for `--any`); live references still own their actual completion.
`disown` accepts the most recent suitable job or explicit PID selectors, deduplicates jobs,
and validates malformed selectors before changing ownership. Help delivery remains N9,
not an N5 claim that every shared help path is delivered.

Tracked process completion publishes the real mapped process status exactly once.
`JobExit` carries the final job status, including pipeline/negation semantics; it is not
hard-coded zero. `CallerExit` retains its historical zero-status event contract.
Late process/job handler registration replays a retained cached completion only to the
newly registered handler. Targeted one-shot handlers are claimed before reentrant evaluation.
Queued and event-blocked lifecycle events retain their process lease independently of job
removal or bounded cache eviction, until dispatch/removal releases them. Callbacks execute
on the main thread, never on native wait/control callback threads.

Disown removes future jobs/wait lookup, cached entries and future lifecycle notification
production, not an existing observer's acquired reference. Already-acquired wait observers
can finish with real completion; already-retained queued events keep their own lease.
Disowned jobs retain shell-side I/O/completion ownership until completion or shell shutdown,
without repopulating the public wait cache.

### Acceptance, rollback and shell exit

Every pipeline owns a separate Job Object. Prepared native children are created suspended,
assigned and registered before each root resumes. Acceptance clears
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` while work is still live. Ordinary accepted and disowned
work therefore survive graceful **and abrupt** shell-process exit under this fish-owned
policy; independently imposed host jobs, console shutdown or external termination can still
affect them. Graceful exit warns about ordinary running jobs; disown suppresses that warning.
It does not imply detach from Windows Job Object membership or POSIX HUP/CONT delivery.

Before acceptance, partial-launch failure rolls back only this pipeline's owned roots and
descendants. Assignment failure includes the retained, still-unassigned suspended child;
cleanup must never terminate a shared enclosing host job. A host job with UI limits alone
is not evidence that assignment to an unrestricted target job must fail: the
[`AssignProcessToJobObject` restrictions](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject)
include limits on the target job and nested-job/breakaway policy.

Shell-owned pipe pumps, deferred writers, waits and descriptor-bridge services live only
while fish is alive. Process survival is not a promise that these shell-hosted services
remain available after graceful drain or abrupt death. See [control/status transitions
and accounting cutoffs](04-signals-events-waits.md#current-n5-control-and-accounting-contract).

## Scope

[`exec.rs`](../../src/exec.rs), [`proc.rs`](../../src/proc.rs), [`job_group.rs`](../../src/job_group.rs), [`tty_handoff.rs`](../../src/tty_handoff.rs), [`fork_exec/`](../../src/fork_exec/), and [`wait_handle.rs`](../../src/wait_handle.rs) supply the shared/Unix process model; the Windows owners are described above. The original audit and candidate scenarios below remain source analysis, not additional executed evidence.

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
| [`signal_safe_launch_process`, `is_thompson_shell_script`, `launch_process_nofork`, `internal_exec`](../../src/exec.rs) | `execve` accepts byte argv/envp; ENOEXEC text fallback excludes `.fish`, checks payload, and retries `PATH_BSHELL`. `exec` replaces the fish image. Windows has neither kernel shebang dispatch nor same-PID replacement; interpreter selection and absence of `/bin/sh` must be explicit. | Empty arguments, embedded quotes/backslashes, non-ASCII executable/environment; `.fish`, shebang scripts, shebang-less text and binary files; reserved `exec` rejection with no fallback, not changed-PID emulation. |
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
| Protect each root until ownership is installed | `CREATE_SUSPENDED`, assign/register root, then `ResumeThread` | Prevent execution before that root's assignment/registration; not an all-roots atomic start. Nested-job restrictions and breakaway flags must be handled. |
| Wait/reap without `SIGCHLD` | Process handles with `WaitForSingleObject`, registered waits, or a dedicated wait thread | Process handles are waitable and retain exit state; there is no zombie/reap requirement. Keep handles until status is recorded. |
| Obtain exit status | `GetExitCodeProcess` after real completion observation | Preserve raw DWORD and mapped shell status; no synthetic stopped/continued/signal encoding. Cancellation provenance is separate. |
| Request native Ctrl-Break for an eligible console root | `GenerateConsoleCtrlEvent` with `CREATE_NEW_PROCESS_GROUP` | Same-console eligibility required; no group-targeted Ctrl-C equivalence or acknowledgement/exit guarantee. |
| Force owned rollback | `TerminateProcess` or `TerminateJobObject` | Abrupt explicit cleanup followed by real waits, not routine `SIGTERM` and not fallback for an ignored cooperative request. |
| Per-process timing | `GetProcessTimes`; `QueryInformationJobObject` for aggregate accounting | Good replacement for `/proc` jiffies, with different units. |
| Replace shell via `execve` | No exact equivalent; permanently EXCLUDED | Spawn/wait/exit changes PID and parentage and is not offered as replacement. |

## Implemented backend ownership

The N2/N5 Windows owners described above retain process and primary-thread handles,
logical job membership, a per-launch Job Object, console-group metadata and separate raw
native/mapped shell status. There is no emulated stopped state.

Preparation resolves all ordered opens/dup/close actions before native stage effects.
Each root is created suspended, assigned and registered, then resumed; the scheduler starts
concurrent native stages before waiting or evaluating their internal peers. This is not a
promise that all roots stay suspended until the entire pipeline has been created.

Failure to assign a newly created suspended root must release that root using its retained
process handle, as well as roll back already-assigned members through this pipeline's own
Job Object. Normal accepted destruction releases monitoring rather than escalating to kill.

## Job-control policy

Windows cannot reproduce POSIX stopped jobs. There is no safe supported equivalent to `SIGTSTP`/`SIGCONT` for arbitrary console applications. `SuspendThread` is per-thread, races with thread creation, and can deadlock while locks are held; it must not implement shell suspension.

The native contract is:

- foreground/native background execution, `jobs`, `wait`, pipeline statuses and `disown` are implemented as above;
- evaluator cancellation can request explicit native Ctrl-Break only for eligible same-console
  `CREATE_NEW_PROCESS_GROUP` roots; request, unsupported topology and observed exit stay distinct;
- fork, same-PID `exec`, POSIX `fg`/`bg`, stopped/continued states and Ctrl-Z job suspension
  are permanently **EXCLUDED**, not deferred pending a suspension mechanism;
- reserved `exec`, `fg` and `bg` remain rejected with no same-named external fallback.
  No reattachment, foreground-pgroup transfer, window-focus or Windows job-control emulation
  is advertised.

## Command-line construction

`CreateProcessW` does not accept an argv array. Use a dedicated encoder for programs following `CommandLineToArgvW`/MSVCRT rules. Batch files require a separate, carefully tested `cmd.exe` encoder. Never concatenate untrusted arguments into `cmd.exe /c` using the C-runtime quoting algorithm.

Executable resolution must be centralized with completion. Decide explicit-path behavior, executable-extension/`PATHEXT` support and batch interpreter selection. Supplying an explicit `lpApplicationName` avoids accidentally adopting `CreateProcessW`'s implicit current-directory/system-directory search. `PATHEXT` expansion is shell policy, not a feature automatically supplied by `CreateProcessW`; do not silently import `cmd.exe` search ordering.

## Classification

**POSIX-backed with major semantic gaps.** This backend is the critical path for a useful native port.