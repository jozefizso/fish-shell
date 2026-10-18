# Concurrency, monitoring, diagnostics, and runtime support

## Current N2 implementation overlay

`src/threads/windows.rs` and `windows_workers.rs` now provide the native runtime; the following
audit's candidate mechanisms are historical, not a claim that Unix thread code runs on Windows.
See [N2 evidence](coverage.md#n2-verified-evidence-record).

- Main-thread source registration uses monotonic logical identity rather than reusable HANDLE values.
  Native callbacks enqueue owned completions; parser/evaluator mutation stays on the main thread.
  Queue/reset synchronization prevents a concurrent post from losing its wakeup.
- Native threadpool waits scale beyond the fixed Win32 wait-array limit. Safe generic registration
  validates Event, Process, Thread, Semaphore or Timer objects; mutexes, files, IOCP and unknown types
  are rejected rather than passed into a native wait mechanism with incompatible requirements.
- Remove-and-drain prevents further publication, cancels/unregisters pending native work and waits
  for in-flight callbacks before releasing their context. Operation-specific I/O cancellation owns
  OVERLAPPED storage through completion, separately from generic wait removal.
- Retained child process handles and immutable raw DWORD completion records outlive consumers;
  direct process waits do not steal unrelated queued completions.
- Real bounded workers and debounce preserve cancellation/generation ownership. An abandoned old
  token cannot publish merely because a newer request is active; completed-result delivery is main-thread work.

N4 watchers, N5 user-visible control/job events and N7 editor background work must reuse these
services. N2 does not implement those consumers or Unix signal masks/atfork on Windows.

N3 now consumes these services for actual nested capture, source/read and mixed function/block
pipelines. Main-thread generic/variable event dispatch, wildcard-selected debug categories,
native file sinks, argv trace hooks and real evaluation profiles are exercised; none claims
N5 control delivery or N6/N7 editor readiness. [N3 evidence](coverage.md#n3-verified-evidence-record)
records the tested consumers separately from N2's runtime foundations.

## Scope

`threads/`, `common.rs`, `global_safety.rs`, `operation_context.rs`, `portable_atomic.rs`, `stdx.rs`, `trace.rs`, `flog.rs`, `panic.rs`, `nix.rs`, `prelude.rs`, `print_help.rs`, `lib.rs`, and `src/tests/` supply worker pools, cancellation, debounce, global invariants, logging, panic handling, common platform probes, and crate organization. `fd_monitor` and `topic_monitor` are analyzed with I/O and signals but participate in this runtime.

## Native Linux/POSIX use

Rust threads, mutexes, condition variables, atomics, channels, scoped threads, and cancellation checks are portable. Native coupling includes:

- signal masks around thread creation and signal-safe wakeups;
- assumptions about post-`fork` execution, including “forked child” restrictions;
- `pthread_atfork` records the forked-child state; this is distinct from any build-script stack-size probes;
- logging through raw fds; the separate `fork_exec/flog_safe.rs` implements the post-fork-safe diagnostic path;
- panic handling that reads from a TTY fd;
- `isatty` wrapper and standard descriptor constants;
- `uname` release-string parsing for WSL detection (not `/proc` parsing);
- monotonic timing through Rust `Instant`;
- Linux `eventfd` in the descriptor monitor; topic-monitor semaphore/wakeup details belong to [signals and events](04-signals-events-waits.md).

## Windows equivalents

| Runtime need | Windows mechanism | Porting approach |
|---|---|---|
| Worker threads, mutexes, atomics | Rust standard library, backed by Windows synchronization | No custom Win32 implementation needed unless integrating native waits. |
| Main event wakeup | `CreateEventW`/`SetEvent`, IOCP completion packet | Replace signal-safe fd writer with a native waitable event or completion submission. |
| Thread signal mask | No equivalent | Console-control callbacks and process waits are explicit event sources; no inherited signal masks are required. |
| Forked-child restrictions | Not applicable with `CreateProcessW` | Remove/compile out post-fork state on Windows. Process setup occurs in the parent before `ResumeThread`. |
| Raw-fd logging | `WriteFile` to owned output handle; `OutputDebugStringW` optionally for debugger-only diagnostics | Keep one low-allocation logging path. Do not make debugger output the user-visible log sink. |
| TTY panic pause | Terminal backend plus waitable input | Preserve fast-fail behavior in CI; avoid blocking a console-control thread. |
| `uname`/WSL check | No native-Windows replacement needed | `is_windows_subsystem_for_linux` already returns false outside Linux; retain WSL detection only in the Linux backend. |
| Timers | Rust `Instant`, waitable/thread-pool timers | Preserve monotonic semantics. |

## Monitor integration

A native completion dispatcher should be the only Windows-specific concurrency service. It accepts registrations for:

- overlapped pipe/file-notification operations;
- process-handle waits;
- console-control and resize events;
- timers;
- internal worker completion/cancellation.

It posts small, owned events to fish's existing main-thread queues. Parser and reader code remain single-thread-oriented. Avoid callbacks directly entering parser code; Windows thread-pool callbacks may run concurrently and on arbitrary threads.

## Diagnostics

`flog` categories, trace formatting, and panic messages are portable. Replace the sink abstraction, not each call site. Windows error reporting should preserve both:

- `GetLastError` formatted through `FormatMessageW` for Win32 calls;
- `WSAGetLastError` for Winsock;
- CRT `errno` only for CRT calls.

Do not collapse all three into `errno`, because values and thread-local storage differ.

## Common/platform probes

Move these out of `common.rs` into explicit backends:

- terminal timestamp reliability;
- WSL version detection;
- shell mode capture;
- process/host identity;
- feature availability.

Native Windows should probe API/capability presence rather than OS version where possible. The minimum viable target can require Windows 10 1809+, which provides ConPTY, but normal VT mode is available earlier in Windows 10.

## Test support

`src/tests/` helpers initialize global shell state and should remain portable. Platform-specific unit tests need serialized access to process current directory, console modes, environment, and global handlers. Use temporary native paths rather than translating Unix fixture names that are illegal on Windows.

## Classification

Concurrency primitives and diagnostics logic: **portable**. Wakeups, raw I/O, signal/fork state, terminal panic interaction, and platform probes: **POSIX-backed**. Linux monitor primitives have existing conceptual Windows replacements.

## Per-module source evidence

Paths below are the implementation inventory, not proposed new modules.

| Component and source | Current Linux/POSIX dependency | Windows implementation candidate |
|---|---|---|
| [`threads/threads.rs`](../../src/threads/threads.rs), [`threads/mod.rs`](../../src/threads/mod.rs) | `init` installs `pthread_atfork`; `spawn` temporarily changes the calling thread's signal mask before `std::thread::Builder::spawn`. Thread IDs, pool queues, mutexes and condition variables otherwise use Rust. | Keep the Rust pool and thread-local IDs; omit the at-fork callback and Unix signal-mask dance. Dispatch console/process events to the main thread rather than running parser work in callbacks. |
| [`threads/debounce.rs`](../../src/threads/debounce.rs) | `Debounce` uses `Instant`, generation tokens and a mutex for latest-request processing; `FdEventSignaller` and `is_fd_readable` connect results to the main loop. | Retain coalescing/token logic; replace fd readiness with an event or completion notification. Preserve publish-result-before-notify and consume-notification-before-result ordering. Timeout abandons a result token, not the OS thread. |
| [`common.rs`](../../src/common.rs) | `shell_modes` returns Unix `Termios`; `has_working_tty_timestamps` controls screen heuristics; `is_windows_subsystem_for_linux` calls `uname` on Linux. Name validation, formatting and special-character selection are logic. | Terminal-mode state belongs to `GetConsoleMode`/`SetConsoleMode`; disable timestamp heuristics where unavailable. Do not substitute Windows version detection for a Linux-only WSL probe. Shared `fish_common` reexports are separately covered in [workspace crates](14-workspace-crates.md). |
| [`global_safety.rs`](../../src/global_safety.rs) | `RelaxedAtomicBool`, `AtomicRef`, and debug borrow/lock guards use Rust atomics/interior mutability; no Linux syscall. | Keep unchanged; preserve existing memory-order and main-thread invariants, rather than replace with Win32 atomics. Logging remains a downstream dependency. |
| [`operation_context.rs`](../../src/operation_context.rs) | Parser/environment references, expansion limits and cancellation callbacks are portable. `background_interruptible` consults `signal_check_cancel`; `get_bg_context` compares reader generation counts. | Retain the context and generation checks; supply cancellation state from console-control/process dispatch. No direct native API needed in the data structure. |
| [`portable_atomic.rs`](../../src/portable_atomic.rs) | Selects Rust `AtomicU64` or the `portable-atomic` fallback from `target_has_atomic`; not Linux-specific. | Retain target-capability selection. No Windows API replacement. |
| [`stdx.rs`](../../src/stdx.rs) | Only a test module: `test_fd_cloexec` checks `fcntl(F_GETFD)`/`FD_CLOEXEC` on a Rust-created file. | Replace the Unix test on Windows with handle-inheritance behavior checks (`GetHandleInformation`, plus actual child inheritance). It is not a runtime platform service. |
| [`trace.rs`](../../src/trace.rs) | Escapes argv, respects parser trace suppression and sends formatted text to `log_extra_to_flog_file`; no syscall of its own. | Keep formatting and suppression; migrate the common diagnostic sink. Trace quoting remains fish-language quoting, not Windows process-command-line quoting. |
| [`flog.rs`](../../src/flog.rs) | Category filtering and formatting are portable; `FLOG_FD` is an `AtomicI32` descriptor and `flog_impl` calls `nix::unistd::write`. | Use a pointer-width owned/borrowed `HANDLE` sink with `WriteFile`. Preserve disabled sinks, redirected output and one-record formatting; do not narrow a handle into `i32`. |
| [`panic.rs`](../../src/panic.rs) | `panic_handler` uses `isatty(STDIN_FILENO)`, `getpid`, `read_blocked` and `FISH_FAST_FAIL` to control an optional interactive debug pause and cleanup. | Use terminal detection through `GetConsoleMode`, `GetCurrentProcessId` and the terminal input abstraction; keep CI/noninteractive fast failure and main-thread-only exit cleanup. |
| [`nix.rs`](../../src/nix.rs) | A single `isatty(i32)` wrapper calls libc; the name does not denote a complete OS abstraction. | Use typed standard-stream handles and `GetConsoleMode`; a pipe to ConPTY is not a console handle, so VT-stream capability must be tracked separately. |
| [`prelude.rs`](../../src/prelude.rs), [`lib.rs`](../../src/lib.rs) | Reexports localization/wide-string/formatting helpers and declares modules/features. No native call, but unconditional Unix-backed modules prevent a native build. | Keep exports and compile the selected backend; no Win32 API needed for organization. |
| [`print_help.rs`](../../src/print_help.rs) | `print_help` uses Rust `Command::new("fish")` to launch `fish -c "__fish_print_help …"` and waits; it uses PATH, not necessarily the matching executable. | Rust `Command` already uses the Windows process backend, but a working native `fish.exe`, command discovery and help assets remain prerequisites. Decide matching-binary/PATH policy; embedded help does not automatically eliminate this subprocess. |
| [`tests/mod.rs`](../../src/tests/mod.rs), [`tests/prelude.rs`](../../src/tests/prelude.rs) | `test_init` changes process CWD, sets libc locales, initializes topic/thread/process state and resets signal handlers. `TestEnvironment` is an in-memory map; `TestParser` directory helpers use Rust filesystem calls. | Keep map/parser helpers; adapt initialization to native event/locale backends and serialize CWD/locale/global-handler mutation. See [test infrastructure](16-tests-benchmarks-ci.md). |

## Port verification scenarios

These are acceptance scenarios for a future backend, not executed native-Windows tests:

1. Submit overlapping debounced requests, allow an abandoned worker to finish late, and verify only the current token publishes a result; wakeups must not be lost during coalescing.
2. Exercise cancellation during background completion without entering parser state from a Windows callback thread.
3. Redirect diagnostics to a file and pipe, disable the sink, and pass pointer-width handle values without truncation; verify closed-child handles cannot keep pipes alive.
4. Trigger an interactive main-thread panic and a background-thread panic; with `FISH_FAST_FAIL` or redirected stdin, neither path may wait for console input.
5. Invoke `fish_indent --help` with native `fish.exe` available and absent from PATH; preserve a useful failure diagnostic.
6. Run test initialization in a Unicode temporary directory and verify serialization of process-global CWD, environment and locale changes.