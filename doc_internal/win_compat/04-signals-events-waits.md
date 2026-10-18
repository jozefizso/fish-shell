# Signals, events, waits, accounting, and resource limits

## Current native N3/N4 overlay

Generic/variable events execute on the main evaluator thread. N4's `env/native_state.rs`
queues universal callbacks/effects outside environment locks and retains failed-mutation keys for
deferred replay after a successful retry. Directory changes are invalidation hints delivered by
`universal_notifier/windows{,_io}.rs` through N2 runtime ownership, not Unix signals or fabricated
child events. Cancel/drain protects notification storage. Native control/job/shell lifecycle
events and accounting now use the N5 owners below; quotas/Unix-only identity policy remain N8. The original
candidate audit below is historical; [N4 combined evidence](coverage.md#n4-implementation-and-evidence-record)
records the completed native/minimum-Rust/retained matrix.

## Current N5 control and accounting contract

[`signal_windows.rs`](../../src/signal_windows.rs), [`proc_windows.rs`](../../src/proc_windows.rs),
[`proc_status_windows.rs`](../../src/proc_status_windows.rs), [`native_accounting.rs`](../../src/native_accounting.rs)
and [`timer_windows.rs`](../../src/timer_windows.rs) implement these native contracts.
This describes current source, not completed acceptance evidence. The
[N5 implementation and evidence record](coverage.md#n5-implementation-and-evidence-record)
owns final revisions/artifacts/results; N1–N4 records remain unchanged.

### Callback handoff, epochs and dispatch

`SetConsoleCtrlHandler` receives native Ctrl-C, Ctrl-Break and best-effort console-close
notifications. The callback uses static atomics, a bounded single-CAS admission gate and
`SetEvent`; it does not allocate, acquire a mutex, follow an `Arc`/parser pointer, spin,
evaluate functions or dispatch language events. Normal-thread owners retain all callback
HANDLEs, unregister and drain admitted callbacks before freeing them. A failed checked
handoff/rearm is an error, not a fabricated successful cancellation.

Per-kind receipt epochs count controls independently; cross-kind ordering is not promised.
Receipt, cancellation acknowledgement and main-thread event delivery are separate.
Rearm closes/drains the outgoing generation, cancels its old token permanently, acknowledges
the cutoff and publishes a new token. A stale callback cannot cancel the replacement token;
a genuinely newer receipt is reconciled into it. Existing waiters holding the retired token
remain canceled, and rearming does not discard undelivered event receipts. Main-thread
dispatch transfers counts to the event subsystem, respecting blocked/reentrant delivery.
Generic/variable/process/job/caller/shell-exit functions also remain main-thread evaluator work.
Specific late lifecycle handlers and their deferred process leases are detailed in
[process ownership](03-process-execution-job-control.md#lookup-completion-and-language-events).

Native control names are `CTRL_C`, `CTRL_BREAK`, `CTRL_CLOSE` (also their `_EVENT`
spellings), not a POSIX signal-number/disposition table. Registration does not make receipt
an acknowledgement by a child. Console-close may be followed immediately by Windows
termination and cannot guarantee parser/history cleanup or shell-exit handlers;
logoff/shutdown notifications and arbitrary Unix signals are not supported deliveries.

### Cancellation and completion transitions

Evaluator cancellation wakes native waits and returns shell interruption status 130.
On the main thread, the process owner explicitly requests native Ctrl-Break for eligible
retained `CREATE_NEW_PROCESS_GROUP` roots sharing fish's current console. It does not
broadcast Ctrl-C, claim POSIX SIGINT/SIGTERM, or silently escalate when a child ignores a
request. GUI, detached and foreign-console roots cannot use this cooperative operation;
they remain live unless they independently exit or are explicitly terminated elsewhere.
No console attachment change is supported while the handler owner is installed.

| Transition/observation | What is known | Status and ownership consequence |
|---|---|---|
| Shell receives Ctrl-C/Ctrl-Break | Cancellation requested; no child response observed | Wake evaluator/native waits; interrupted shell evaluation is 130, not child exit or signal death. |
| Eligible root's Ctrl-Break API request succeeds | Native request issued, not acknowledged | Root may remain running; jobs can report requested state. No completion/event is invented. |
| GUI/detached/foreign-console topology, or request API failure | Request unsupported or failed | Diagnose explicitly; keep live ownership and real waits; no forced fallback. |
| Root ignores a request | Still running at observation | Retain the request provenance and live process; no timeout-to-kill escalation. |
| Root exits after a request | HANDLE signaled and `GetExitCodeProcess` returned a DWORD | Record actual completion; temporal sequence alone is not proof the control caused the exit. |
| Unrelated/high native DWORD exit | Actual raw exit, including all high bits | Preserve raw DWORD; map 0–255 unchanged and every other DWORD to shell 1. Never infer cancellation or a Unix signal. |
| Evaluator stops but child has not exited | Shell cancellation only | Shell result 130 is separate from the child's still-live/native completion status and eventual events. |
| Explicit forced rollback of an unaccepted owned group | `TerminateJobObject`/retained-root termination requested, then real waits | Abrupt owned-process cleanup, not cooperative acknowledgement; never target enclosing/foreign jobs. Observed DWORD remains the completion source. |

Native raw completion is retained even if the stage's own I/O fails; that failure has
separate provenance and can make an otherwise successful stage fail. `$status` and
`$pipestatus` are shell-facing results, not a lossless encoding of native DWORDs.
Process events carry mapped real completion; JobExit carries actual final job status,
while CallerExit preserves its historical zero argument. Cancellation alone generates
neither a child completion nor a synthetic stopped/continued/signal transition.

### Native accounting units and observation scope

`GetProcessTimes` supplies fish/root process user and kernel CPU, creation and exit/current
timestamps. FILETIME and Job Object CPU values are **100-nanosecond units**:
10,000,000 ticks per second, not Unix clock ticks or milliseconds. Job Object basic and
I/O accounting covers OS-attributed members/descendants of each disjoint owned launch job.
Root process times are not added again to that job aggregate.

The registry sums completed cutoff snapshots plus live registered job CPU exactly once.
Completion takes a final snapshot when all tracked roots are observed complete and internal
stage completion permits owner finish; explicit monitoring detach/release takes its own
cutoff. Disown itself changes lookup/notification policy, not that accounting cutoff.
Descendant CPU up to that observation is included; later surviving-descendant work is
excluded even if Windows still has the Job Object. This is not POSIX wait/rusage lifetime
accounting or per-command isolation. Concurrent background jobs contribute to a timed
interval. Fish and individual job counters are sampled sequentially, not atomically across
the system; checked overflow/decreasing counters/query failures are reported, not zeroed.

The `time` decorator uses monotonic `Instant` elapsed time plus real fish and tracked-child
user/kernel CPU deltas, and writes its report to stderr. `--print-rusage-self` also writes to
stderr and reports **shell process only**: elapsed/user/kernel seconds, current/peak working
set bytes, peak pagefile bytes, combined page faults, and read/write/other I/O operation and
byte counts. It explicitly marks unavailable POSIX minor/major fault split, swaps, block I/O,
IPC messages, signals, voluntary/involuntary context switches and shared/unshared memory
integrals. Native I/O operations are not POSIX blocks and pagefile usage is not a swap count.
These measurements do not implement quotas; resource-limit policy remains N8.

## Scope and evidence boundary

This source audit covers [`signal.rs`](../../src/signal.rs), [`event.rs`](../../src/event.rs), [`topic_monitor.rs`](../../src/topic_monitor.rs), [`timer.rs`](../../src/timer.rs), and [`builtins/ulimit.rs`](../../src/builtins/ulimit.rs), with implemented native owners above. Process lifecycle and language wait records are detailed in [03](03-process-execution-job-control.md); descriptor wakeups are in [05](05-io-redirection-file-descriptors.md). The original per-component candidate/scenario table below is historical, not current support or extra executed evidence.

`timer.rs` implements the `time` decorator's elapsed/CPU accounting, not timer scheduling, a timer thread, or timer-driven event delivery. `SIGALRM` in the signal handler is a deliberate no-op used for signal-torture testing, not evidence of a scheduler.

## Per-component mapping

| Component and source symbols | Native Linux/POSIX use | Native Windows candidate and semantic gap | Verification scenarios (not executed) |
|---|---|---|---|
| [`fish_signal_handler`, `reraise_if_forked_child`](../../src/signal.rs) | Preserves errno; detects forked child by PID and resets/re-raises its signal; observed signals enqueue events. SIGINT sets cancellation unless observed, HUP/TERM request deferred reader exit unless observed, WINCH invalidates size, CHLD posts a topic. | `SetConsoleCtrlHandler` for console Ctrl-C/Ctrl-Break; callback only publishes cancellation/exit state and wakes main-thread dispatch. No fork re-raise branch is needed. Close/logoff/shutdown notifications have time limits and cannot promise arbitrary parser/history work before termination. | Ctrl-C while reading/evaluating/waiting, observed versus unobserved cancellation, terminal disconnected during callback, concurrent callback and shutdown. |
| [`signal_set_handlers`, `set_interactive_handlers`, `signal_set_handlers_once`](../../src/signal.rs) | `sigaction`/`SA_SIGINFO`, CHLD `SA_RESTART`, ignores PIPE/QUIT, interactive TSTP/TTOU behavior; TTIN is not simply ignored. Inherited ignored HUP is preserved. TSAN feature injects CHLD as a workaround. | Console handler registration is not a Unix disposition table; handler state and inheritance on process creation differ. Broken pipe is an I/O error; foreground/background console reads do not produce Unix TTIN/TTOU job stops. | Broken downstream pipe leaves fish alive; nested shell/control-handler inheritance; background child and parent do not both consume or forward cancellation twice. |
| [`signal_reset_handlers`, `SIGNALS_TO_DEFAULT`, `signal_handle`, `signal_unblock_all`](../../src/signal.rs), [`blocked_signals_for_job`](../../src/fork_exec/mod.rs) | `sigaction`, `sigset_t`, `sigprocmask` reset child dispositions/masks; HUP ignored by `nohup` remains ignored; background jobs without job control block INT/QUIT. | No general Win32 signal-mask or disposition-reset API. Create children with deliberate console group/handle inheritance; expose only actual or explicitly synthetic notifications. | Spawn under a customized parent handler, ignored Unix HUP behavior on Unix, Ctrl events for background/foreground children on Windows. |
| [`RawSignal`, `SIGNAL_TABLE`, `SigChecker`](../../src/signal.rs), [`SIGNAL_COUNT`](../../src/event.rs) | Parses numeric/name forms and translates platform-specific numbers; Linux adds PWR/STKFLT entries. Fixed event signal count, generation-based cancellation checking and the event bitmap are part of the representation. | Windows exceptions/NTSTATUS codes are not Unix signals. Keep a separate supported logical event namespace; `CTRL_BREAK_EVENT` does not establish a portable Unix `SIGBREAK` constant (not in this source table). No arbitrary `kill(pid, sig)` equivalent. | Numeric/name parsing, unsupported signal diagnostics, code/description reporting for native child exit and exceptions without falsely claiming OS signal delivery. |
| [`EventDescription`, `EventHandler`, `fire`, `fire_generic`, `fire_internal`](../../src/event.rs) | Signal, variable, process-exit, job-exit, caller-exit and generic event categories. Most dispatch is parser/in-memory work: **no direct Linux syscall needed** for variable/generic events. Signal registration calls `signal_handle`; exit production depends on process backend. | Retain language event API with native sources feeding it. `GetCurrentProcessId` covers `%self`; process-handle completion covers tracked-child exits, not arbitrary global PID lifecycle monitoring. | `emit` arguments/status preservation; variable event; process/job/caller exit exactly once; event-handler registration/removal during dispatch. |
| [`PendingSignals::{mark,acquire_pending}`, `OBSERVED_SIGNALS`, `fire_delayed`](../../src/event.rs) | Arrays indexed by raw signal numbers; pending notifications coalesce into a `u64` bitset. Delayed dispatch avoids reentry during event handlers/unwinding, respects event blocking, and updates COLUMNS/LINES on WINCH. | Keep delayed main-thread execution and coalescing, but change native event encoding deliberately. Console callbacks may execute concurrently; do not assume the source's single-signal-handler-at-a-time expectation automatically holds on Windows. | Bursts while events are blocked; unblocking order; callback racing with pending-bit drain; resize updates and no parser execution inside callback. |
| [`Topic`, `GenerationsList`, `TopicMonitor::{post,check}`, `BinarySemaphore`](../../src/topic_monitor.rs) | Topics are HUP/INT/TERM, CHLD and internal exit. Atomics coalesce posts; one reader blocks on a binary semaphore and other waiters coordinate through mutex/condition variable. Linux tries unnamed `sem_init`/`sem_post`/`sem_wait`; failure and other Unix platforms use a self-pipe. | Auto-reset event or carefully synchronized semaphore/event backend using `CreateEventW`/`SetEvent`/waits. Preserve generation snapshots and the wakeup-commit handshake: replacing only the sleep primitive must not lose posts or release too many waiters. Rust mutex/Condvar logic itself is portable. | Post before/during/after wait commitment, many simultaneous waiters, mixed child/internal/cancel topics, repeated posts coalescing and no missed wakeups. |
| [`TimerSnapshot::take`, `TimerSnapshot::get_delta`, `push_timer`, `PrintElapsedOnDrop`](../../src/timer.rs) | `getrusage(RUSAGE_SELF)` and `RUSAGE_CHILDREN` yield user/system CPU; `Instant` gives monotonic wall time. Drop writes the `time` report directly to stderr because this is a decorator, not a builtin stream. | Preserve `Instant`; use `GetProcessTimes` for fish and accumulated completed-child process times, optionally Job Object accounting with explicitly defined descendant scope. Windows has no drop-in cumulative `RUSAGE_CHILDREN`; retain accounting before closing process handles and avoid double-counting descendants/jobs. No waitable timer is required by this module. | Time a pure builtin, external CPU work, parallel pipeline and nested child; compare wall versus user/kernel totals; stable units, no negative deltas or duplicate child accounting; redirected stderr. |
| [`limits`, `getrlimit`, `setrlimit`, `set_limit`, `RESOURCE_ARR`, `ulimit`](../../src/builtins/ulimit.rs) | `getrlimit`/`setrlimit`, soft/hard pairs and resource multipliers. Unavailable resources use `-1` and produce “Resource limit not available on this operating system”. Linux-only and BSD-only categories are explicitly gated. | Capability-based interface; Job Object limits are partial approximations, not POSIX limits inherited with the same scope. Do not return unlimited/success when no enforcement exists. See category policy below. | Enumerate all limits; unavailable option, soft/hard increase/decrease, permissions, child enforcement and shell-versus-child scope; no unsupported category enters Unix resource conversion. |

## Windows event-source mapping

| POSIX source | Candidate | Limits |
|---|---|---|
| SIGINT | `SetConsoleCtrlHandler` / `CTRL_C_EVENT` | Console topology matters; detached/GUI applications need not receive it. Registering a handler is not identical to controlling terminal ownership. |
| Ctrl-Break | `CTRL_BREAK_EVENT`, `GenerateConsoleCtrlEvent` | A separate native control event. `CREATE_NEW_PROCESS_GROUP` enables group targeting; Ctrl-C cannot be targeted to a nonzero group in the same way. |
| SIGCHLD | Process handles, registered waits or a wait thread | Completion only; no Unix stopped/continued state or zombie reaping. Queue a language status update rather than run parser callbacks on the wait thread. |
| SIGHUP, SIGTERM, USR1/USR2 and other arbitrary signals | No general equivalent | Define explicit shell shutdown/internal-event behavior; child graceful termination is application/protocol-specific. `TerminateProcess`/`TerminateJobObject` is abrupt, not catchable SIGTERM. |
| SIGWINCH | Console `WINDOW_BUFFER_SIZE_EVENT`, size queries/host integration | `ENABLE_WINDOW_INPUT`/console event consumption must coexist with the terminal transport. ConPTY resize is controlled by the host (`ResizePseudoConsole`), not a child-side SIGWINCH guarantee. Compare visible viewport size, not blindly buffer dimensions. |
| self-pipe/eventfd wakeup | Event object or native completion packet | An event HANDLE cannot be inserted into Unix `poll`; integrate with the [I/O backend](05-io-redirection-file-descriptors.md). |

## Resource-limit categories and policy

The complete source category set is CORE, DATA, FSIZE, MEMLOCK, NOFILE, STACK, CPU, NPROC, SIGPENDING, MSGQUEUE, RTPRIO, RTTIME, RSS, AS, SBSIZE, NICE, KQUEUES, SWAP, NPTS and NTHR. The latter BSD categories must not disappear from the audit just because Linux does not enable them.

- **CPU:** Job Object per-process/per-job user-time limits are candidates, but termination/notification, scope and soft/hard semantics differ from `RLIMIT_CPU` and SIGXCPU.
- **NPROC / NTHR:** active-process limits can constrain a Windows job; POSIX per-user process/thread counting is not equivalent. No general Job Object thread-count equivalent for NTHR.
- **DATA / AS / RSS / SWAP:** process/job committed-memory limits, working-set policies and virtual-address limits are distinct concepts. `SetInformationJobObject` process/job memory limits and working-set APIs cannot be advertised as faithful DATA/AS/RSS/SWAP replacements.
- **MEMLOCK:** `SetProcessWorkingSetSize` and `VirtualLock` do not impose the inherited per-user/process locked-memory quota that this interface exposes. Privilege and working-set policy require a separate contract.
- **FSIZE / NOFILE / STACK / CORE:** no general inherited Win32 equivalent enforcing the POSIX limit for arbitrary native children. CRT fd limits do not cap native HANDLEs; stack reserve is selected per executable/thread creation; WER/minidumps are not `RLIMIT_CORE`.
- **SIGPENDING / MSGQUEUE / RTPRIO / RTTIME / NICE / SBSIZE / KQUEUES / NPTS:** no faithful general Win32 equivalent. Priority classes are not Unix nice/realtime resource ceilings; named pipes, Winsock buffers, IOCP and ConPTY counts do not provide those quotas.

Unsupported categories should reuse explicit unavailability diagnostics rather than invent enforcement. Document whether a supported candidate applies to fish itself, future children, one pipeline, all descendants, or a shared user; disown and preexisting Job Object membership constrain the design.

## Classification

Language event dispatch and generation bookkeeping are **portable logic with platform sources**. Native control reception, retained-handle child waits and measured accounting now use the N5 owners above, without POSIX signal/stop emulation. Resource limits retain **POSIX semantic gaps** for N8. Linux unnamed semaphores and extra RLIMIT categories are **Linux-specialized**. `time` is elapsed/CPU accounting, not a scheduling subsystem.
