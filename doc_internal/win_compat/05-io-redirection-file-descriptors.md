# I/O, file descriptors, pipes, and redirection

## Current N4 persistence/notification consumers

`fs/windows.rs` implements native state transactions separately from user redirection streams:
stable parent `.fish_lock`/LockFileEx, checked identities, flushed sibling staging and checked
replacement/create-new. Existing-target DACLs are installed at staging creation before bytes;
failure cannot acknowledge a save. Public funcsave uses `builtins/native_fs.rs` atomic writes.
`universal_notifier/windows_io.rs` owns OVERLAPPED directory requests until cancellation/drain,
posting through N2 without pretending a HANDLE is a Unix fd. `read -U` now assigns real
durable universals; prompt/terminal read remains N6. See the
[N4 record](coverage.md#n4-implementation-and-evidence-record) for final combined results.

## Current N2/N3 implementation overlay

The sections below this overlay retain the original pre-prototype audit and proposed scenarios.
Current verified implementation/evidence is recorded in [GOAL_02](GOAL_02.md) and
[N2 coverage](coverage.md#n2-verified-evidence-record), extended by [N3](GOAL_03.md).

- `io_windows.rs` and `io/windows_descriptors.rs` bind logical numbers to typed borrowed/owned
  file, console, pipe, memory and closed endpoints. Ordered overlays implement input/try-input,
  overwrite/append/atomic noclobber, dup/self-dup/close and block redirection without global std-handle mutation.
- Explicitly closed builtin output is a discard stream, matching the shared Unix evaluator.
  Missing inherited native output is different: unused streams succeed; attempting a write fails.
  An external child receives an actually closed native standard handle, not an invented writable sink.
- `fds_windows.rs` shares pending input/cursor state across aliases and imported descriptor aliases.
  Incremental console decoding and successive pipe readers do not discard overread bytes.
- External stages are prepared before launch and started before waiting. Internal producers use bounded
  separated buffers whenever a later main-thread internal consumer exists, including across native relays.
  `windows_deferred.rs` pumps those buffers with owned cancellable native I/O and closes its writer before EOF is needed.
- `windows_pipe.rs` owns overlapped state/buffers through completion and cancellation drain.
  Capture finish stops rearming, cancels/drains the active read and drains one finite available-byte snapshot.
  It retains completed bytes without waiting for a live descendant-held writer to close.
  Finite native buffers charge element metadata as well as data; explicit zero limit remains unbounded policy.
- Redirection-only null syntax is `/dev/null`, case-insensitive `NUL`, `NUL:` and `\\.\NUL`.
  This does not alias ordinary filesystem identities; noclobber against the null device fails as existing.
- `native_fd_bridge.rs` transfers extra descriptors only to the same actual running executable identity.
  The private versioned environment payload validates handles/direction/ownership, preserves aliases and is
  removed from ordinary import/export. Arbitrary native executables with extra fds fail before child effects.
  Internal monitoring handles and unrelated launch endpoints are never in the child allowlist.

N3 exercises function/block pipelines, nested substitutions and noninteractive `read` against
these endpoints. Partial input survives internal/native handoff and failed materialization without
consuming the caller's cursor; setup commits only after pipes and idle workers are prepared.
Native readers peek chunks, not one byte per kernel call. Descendant-held capture tests isolate
stderr ownership so a harness waiting for stderr EOF cannot masquerade as evaluator deadlock.
Sockets are not implicitly supported by these file/pipe/console endpoint mechanisms.

## Current N5 background I/O lifetime

Native `&` and mixed pipelines now retain I/O ownership without waiting for every native
root before later commands. A background builtin feeding a native consumer uses bounded
deferred output and asynchronous draining, including when it is the last internal stage.
The final function/block producer instead streams while its main-thread evaluator remains
active; functions/blocks are not evaluated concurrently on a worker. Existing deferral
before later internal consumers remains necessary across native relays.

Without explicit/inherited stdin redirection, initially background work opens trusted native
`\\.\NUL` input. Explicit stdin, pipeline edges and ordered fd actions take precedence.
Cancellation wakes evaluator/I/O waits, not an invented child exit. Existing OVERLAPPED
storage and buffers stay owned through cancellation/drain.

Disown suppresses future job selection/cache/notifications, not pending native I/O or an
already-acquired completion observer. Accepted ordinary/disowned processes survive fish's
job-handle closure, but pipe pumps, deferred writers, descriptor bridges and wait services
are shell-hosted: they end when fish shuts down or dies. Survival does not promise a daemon
that maintains these streams. See [N5 lifecycle](03-process-execution-job-control.md#current-n5-native-lifecycle-contract)
and the [final evidence location](coverage.md#n5-implementation-and-evidence-record);
this source contract does not assert all gates passed or change N2/N3 evidence.

## Scope

[`io.rs`](../../src/io.rs), [`fds.rs`](../../src/fds.rs), [`fd_monitor.rs`](../../src/fd_monitor.rs), [`fd_readable_set.rs`](../../src/fd_readable_set.rs), [`redirection.rs`](../../src/redirection.rs), and [`null_terminated_array.rs`](../../src/null_terminated_array.rs) own fish's descriptor model, pipe creation, buffered builtin output, redirection planning, readiness monitoring, and `argv`/`envp` construction. This is source-grounded design analysis, not an executed port.

## Native Linux/POSIX use

- `RawFd`, `OwnedFd`, and `BorrowedFd` are pervasive public types.
- `pipe`/`pipe2`, `dup2`, `close`, `fcntl(F_GETFD/F_SETFD/F_GETFL/F_SETFL)`, `FD_CLOEXEC`, and `O_NONBLOCK` implement pipeline topology and inheritance safety.
- `select`/`poll` monitor terminals, pipes, notifier descriptors, and worker wakeups in one namespace.
- Linux uses `eventfd` for cheap coalesced notification; other Unix systems use a self-pipe.
- redirection assumes arbitrary nonnegative descriptor numbers and `/dev/null` conventions.
- `null_terminated_array.rs` constructs C `argv` and `envp` pointer arrays for `execve`.

### Component and subcomponent evidence

| Source component/symbol | Linux/POSIX dependency or explicit none | Windows mapping and gap | Verification scenario (not executed) |
|---|---|---|---|
| [`make_autoclose_pipes`, `heightenize_fd`, `FIRST_HIGH_FD`, `set_cloexec`](../../src/fds.rs) | Linux `pipe2(O_CLOEXEC)`; other Unix paths use `pipe` then CLOEXEC. Internal descriptors are moved above the user range (10+) with `F_DUPFD_CLOEXEC`. | `CreatePipe`/overlapped named pipes and owned HANDLEs; logical descriptor namespace still must avoid colliding with internal endpoints. `DuplicateHandle` is not integer-fd assignment. | Pipe creation/duplication failure; child cannot inherit an internal monitor pipe; explicit user fd 3/9/10 does not alias internal resources. |
| [`open_cloexec`, `wopen_cloexec`, `open_dir`, `wopen_dir`, `BEST_O_SEARCH`, `exec_close`, `make_fd_{nonblocking,blocking}`](../../src/fds.rs) | `open(O_CLOEXEC)`, directory flags, EINTR/cancellation handling, `close`, `fcntl`. Linux uses O_PATH for searchable directory handles, distinct from readable directory streams. | `CreateFileW`, `CloseHandle`, operation-specific async state; directory handles require `FILE_FLAG_BACKUP_SEMANTICS`. No Windows `fchdir`, generic EINTR, O_PATH or universal nonblocking flag. | Directory can be traversed without listing rights; cancellation while opening input; failed open/close does not leak or retry an unrelated recycled resource. |
| [`BorrowedFdFile`](../../src/fds.rs) | `ManuallyDrop<File>` borrows an existing fd; `Clone` **does not duplicate** ownership. Standard input is fd 0. | Borrowed native endpoint with explicit lifetime; do not turn every clone into an owning `OwnedHandle` or duplicate file position accidentally. | Shared reads consume one stream; dropping a builtin wrapper does not close shell input; explicitly closed stdin differs from empty input. |
| [`IoData`, `IoClose`, `IoFd`, `IoFile`, `IoPipe`, `IoChain::{io_for_fd,append_from_specs}`](../../src/io.rs) | Arc-owned file/pipe resources versus logical fd targets; last matching redirection wins; file specs open relative to PWD with mode 0666 (subject to umask). `TryInput` substitutes `/dev/null` on failure. | Logical descriptor table over owned/borrowed handles; `CreateFileW` sharing/security/creation policy must be explicit. ACL defaults are not Unix umask; preserve ordering independently of native handle values. | `>a 2>&1` versus `2>&1 >a`, repeated redirects, close/duplicate chain, relative PWD, noclobber race and `<?` missing input. |
| [`RedirectionSpec`, `RedirectionMode::oflags`, `Dup2List`, `dup2_list_resolve_chain`, `fd_for_target_fd`](../../src/redirection.rs) | Syntax/planning itself is in-memory, but encodes RawFd, O_* flags and ordered dup2/close actions. Modes include Overwrite, Append, Input, **TryInput, NoClob**, Fd. | Retain syntax and ordered endpoint resolution; replace Unix flag lowering with native open dispositions. Child arbitrary numbered descriptors have no universal native protocol. | Numeric overflow/invalid targets; redirected source later closed; self-dup and fd ordering; builtin and external standard streams agree. |
| [`SeparatedBuffer`, `BufferElement`, `SeparationType`](../../src/io.rs) | **No Linux syscall** in storage/size-limit/discard and explicit-versus-inferred element boundaries. These are command-substitution semantics, not ordinary newline-only strings. | Reuse in-memory logic; a handle backend must not flatten away explicit empty elements or insert/remove NUL/newline delimiters. | `string split0`/collect in substitution, empty element, missing final newline, read limit exceeded. |
| [`IoBufferfill::{create_opts,finish}`, `IoBuffer::{read_once,complete_and_take_buffer}`, `begin_filling`](../../src/io.rs) | Nonblocking read pipe monitored in background; reads retry EINTR and stop on EAGAIN/EOF. Finish takes the endpoint back and drains currently available data, avoiding indefinite wait if a background grandchild retains the writer. | Overlapped reads or a cancellable reader service; cancel/drain pending I/O before releasing OVERLAPPED storage or handles. Preserve partial output and bounded completion even if another process retains a writer. | Output beyond pipe capacity, retained grandchild writer, simultaneous direct builtin-buffer writes, EOF/removal/cancellation races. |
| [`OutputStream`, `FdOutputStream`, `StringOutputStream`, `BufferedOutputStream`, `IoStreams`](../../src/io.rs) | String/null/buffered output is portable; fd output encodes fish text and writes bytes, maps errors/status. IoStreams carries redirected/piped flags, borrowed input, terminal queries and job group. | Native write/read endpoints behind existing stream semantics; retain byte-exact files/pipes and use terminal encoding only at console boundary. Do not enable CRT text-mode CRLF translation on arbitrary pipelines. | NUL bytes, Unicode and invalid-byte roundtrip policy; broken pipe/write failure status; direct versus indirect stdin redirection; terminal detection after redirection. |
| [`FdEventSignaller::{post,try_consume,read_fd}`](../../src/fd_monitor.rs) | `have_eventfd` path uses eventfd(EFD_CLOEXEC\|EFD_NONBLOCK), otherwise nonblocking self-pipe; coalesced, async-signal-safe wakeups. This is separate from topic_monitor's Linux unnamed semaphore. | Event object/completion packet; consume/reset protocol must not lose concurrent posts. Native HANDLE is not a Unix pollable fd. | Many posts coalesce; full wakeup pipe/event overflow counterpart; post races reset; no child inheritance. |
| [`FdMonitor`, `FdMonitorItemId`, `BackgroundFdMonitor::run`, `remove_item`, `with_fd`](../../src/fd_monitor.rs) | Lazy background thread waits on readable/HUP fds. Nonrecycled IDs plus snapshot/generation handshake prevent stale callbacks/descriptor-reuse ABA; remove waits for the active poll/select generation before caller can close. | Completion registration with nonrecycled logical IDs and safe removal/cancellation. HANDLE values also recycle; pending completions must not target a new item or freed callback. | Add/remove while blocked, reuse handle value, callback closes endpoint, termination while no work, final buffered output retained. |
| [`FdReadableSet`, `Timeout`, `is_fd_readable`, `poll_fd_readable`](../../src/fd_readable_set.rs) | Linux/non-Apple uses `poll`; Apple uses `select` because of device support. Readability-only, timeout/infinite/zero-time probes, EINTR visible to caller. | Dispatcher over process/event waits and overlapped pipe reads; Winsock `select`/`WSAPoll` only cover sockets, not a substitute for shell stdin/pipes. | Pipe data/EOF/HUP, console input, zero timeout, cancellation during indefinite wait, more endpoints than `MAXIMUM_WAIT_OBJECTS`. |
| [`NullTerminatedArray`, `OwningNullTerminatedArray`](../../src/null_terminated_array.rs) | **No Linux syscall** in construction; pinned owned CStrings and trailing null pointer satisfy Unix byte argv/envp ABI. | Keep Unix ABI utility where still needed; Windows process boundary requires UTF-16 command line/environment block. A Unix pointer array cannot simply be widened and passed to `CreateProcessW`. | Zero/empty arguments, stable pointer lifetime, empty environment, exported case collisions, command-line quoting roundtrip through actual native target runtimes. |


## Windows equivalents

| POSIX operation | Windows API | Porting consequence |
|---|---|---|
| Owned fd | Owned `HANDLE` with `CloseHandle`; sockets use `SOCKET` | A handle and socket are distinct namespaces. Do not model both as an integer fd. |
| Anonymous pipe | `CreatePipe` | Synchronous anonymous pipes work for simple child stdio but do not support overlapped I/O. For scalable asynchronous monitoring, create uniquely named pipes with `CreateNamedPipeW(FILE_FLAG_OVERLAPPED)`. |
| `dup`/`dup2` | `DuplicateHandle` | Duplicates kernel handles but cannot assign an arbitrary child-side integer. Standard handles are installed through `STARTUPINFO`; other handles need an application protocol or CRT bridge. |
| `CLOEXEC` | Non-inheritable handles plus explicit `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` | Keep shell-owned handles non-inheritable; prepare intended inheritable duplicates and use the required launch inheritance setting with a whitelist. Listing a non-inheritable handle does not make inheritance valid. |
| Blocking/nonblocking fd mode | Overlapped I/O; pipe mode flags; `SetNamedPipeHandleState` | There is no universal handle-level `O_NONBLOCK`. The operation and handle creation mode determine async behavior. |
| `read`/`write` | `ReadFile`/`WriteFile`; console-specific APIs when appropriate | Byte-stream semantics work for files and pipes. Console handles require VT/Unicode policy. |
| `poll`/`select` | Waitable events, overlapped operation events, thread-pool waits, IOCP | `WaitForMultipleObjects` has a small fixed maximum and cannot wait on all file handles. Use a dispatcher combining process/event waits with overlapped pipe completion. |
| `/dev/null` | Open `NUL` using `CreateFileW` | Hide behind `platform::io::null_device()`. |
| `argv`/`envp` arrays | UTF-16 command-line string and double-NUL-terminated environment block | Replace `OwningNullTerminatedArray` at the process boundary; retain it only for Unix. |

## Required abstraction

Use tagged native resources rather than a fake universal fd:

```text
NativeIo
  File(handle)
  Pipe(handle, sync-or-overlapped)
  Console(handle, input-or-output)
  Socket(socket)
  Closed
```

Maintain a separate logical descriptor table for fish syntax. Entries map logical numbers to `NativeIo` plus read/write direction and inheritance policy. Builtins consume logical streams; external-process launch resolves only the child's standard streams and explicitly supported inherited handles.

`FdOutputStream` and `IoBufferfill` should depend on `ReadEndpoint`/`WriteEndpoint` traits or enums. This avoids forcing Windows handles through the MSVCRT fd layer and prevents truncation on 64-bit Windows.

## Readiness architecture

The existing `FdMonitor` assumes every source can become a readable fd. A proposed Windows backend can use a completion queue (the following is design, not existing functionality):

- overlapped named-pipe reads report through IOCP or per-operation events;
- process completion uses registered waits that post to the queue;
- console-control and internal changes call `SetEvent` or post a completion packet;
- filesystem notifications use overlapped `ReadDirectoryChangesW`;
- deadlines, if needed by consumers, use wait timeouts or native timers; `timer.rs` itself is CPU/wall accounting, not a scheduler.

A dedicated dispatcher thread is the conservative design. It can translate native completions into fish callbacks without imposing `WaitForMultipleObjects`' object-count limit.

## Redirection semantics

File redirection maps to `CreateFileW` creation dispositions:

- `>`: `CREATE_ALWAYS` (create or truncate);
- `>>`: `OPEN_ALWAYS` and seek/atomic append policy;
- `<`: `OPEN_EXISTING` with read access;
- `>?` (noclobber): `CREATE_NEW`, failing atomically if the target exists; do not check existence then truncate;
- `<?` (try-input): attempt `OPEN_EXISTING`, substituting a read endpoint for `NUL` on failure as the current code substitutes `/dev/null`;
- fd duplication: duplicate the logical endpoint;
- fd close: map to `Closed`, passing an invalid/closed standard handle only where child behavior is understood.

Windows append writes are not automatically equivalent to POSIX `O_APPEND` for all handle configurations. Use append-only `FILE_APPEND_DATA` rather than granting ordinary offset-write access where possible; test concurrent writers and remote filesystems. A seek-to-end followed by `WriteFile` is not an atomic append operation. Specify `FILE_SHARE_READ`/`FILE_SHARE_WRITE`/`FILE_SHARE_DELETE` deliberately: default exclusive opens would break common Unix redirection/rename behavior.

## Semantic risks

- CRT programs, Win32 programs, and managed programs interpret inherited handles differently.
- `CreatePipe` handles cannot be retrofitted for overlapped I/O.
- Closing one duplicated handle does not invalidate other duplicates, matching Unix open-file descriptions only partially.
- Sockets cannot be treated as ordinary file handles for every operation.
- Arbitrary `3>&1` inheritance cannot be promised to unrelated native applications.

## Classification

**POSIX-backed foundational component.** Linux `eventfd` is an optimization; the main port requires a new handle and completion model.