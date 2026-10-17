# GOAL_02 — Streaming I/O, handle graphs and runtime ownership

**Phase:** N2. **Status: implemented and verified on the recorded native host.**
The automated gates exist in `tests/windows_n2.rs`; the evidence and platform limits are
recorded in [coverage](coverage.md#n2-verified-evidence-record).
The primary target is native `x86_64-pc-windows-msvc`; ARM64 is a separate architecture extension.

## Objective and completion boundary

Deliver real streaming pipelines, ordered redirection and bounded capture infrastructure on native Windows.
Concurrently schedulable pipeline stages must exchange data before producers finish, without deadlock,
unbounded buffering, leaked inherited handles or dependence on a descendant eventually closing a writer.
Retain existing bounded internal-to-internal buffering when main-thread evaluator scheduling requires it.
Land shared workers, wakeups, cancellation and retained process ownership now, before watchers or user-visible jobs.
This makes [N3 batch language and substitution](GOAL_03.md) possible without inventing a second I/O runtime.

## Entry conditions and dependencies

- Verify [N1](GOAL_01.md) has closed on the integration revision: native path/open/cwd behavior, discovery,
  identity codecs and supported-system matrix must be real prerequisites, not assumptions from these plans.
- Preserve [GOAL.md](GOAL.md) positive native foreground/stdio/export behavior and permanent rejection checks.
- Suggested branch: `platform/windows/streaming-io`; coordinate one shared owner for I/O, launch and completion services.
- Define interfaces consumed by N3 capture, [N4 watchers](GOAL_04.md), [N5 jobs](GOAL_05.md)
  and [N7 background completion](GOAL_07.md) before enabling those clients.

## Owning components and design invariants

Owners are `src/io_windows.rs`, `src/fds_windows.rs`, `src/redirection.rs`, `src/exec_windows.rs`,
`src/proc.rs`, `src/threads/`, native worker/monitor/wakeup services and shared input adapters.
See [I/O and redirection analysis](05-io-redirection-file-descriptors.md),
[process execution analysis](03-process-execution-job-control.md),
[concurrency/runtime analysis](12-concurrency-and-runtime-support.md),
[signals/events/waits analysis](04-signals-events-waits.md) and [Windows API reference](windows-api-reference.md).

- Extend existing stream/evaluator/backend contracts, not a new shell or wholesale platform framework.
- Logical fish descriptor numbers are not native handle values. Never cast pointer-width HANDLE to `RawFd`.
- Distinguish owned, borrowed, duplicated and closed endpoints, with direction and inheritance policy.
  Dropping a borrowed builtin stream must not close fish stdin or change ownership accidentally.
- Parser/evaluator work stays on the main thread. Native callbacks post owned completion work, never run the parser.
- Process handles and completion registrations remain live until their consumers drain; a cached language wait status
  is not a substitute for the live process handle used by runtime services.
- Console Unicode output and redirected byte streams have different boundaries; no CRT newline rewriting of bytes.

## Implementation delivery

1. **Replace the empty I/O-chain representation.**
   - Represent real typed file, pipe, console and closed endpoints with owned lifetimes and a logical descriptor map.
   - Lower ordered open/dup/close actions without using native handle values as logical descriptor identifiers.
   - Implement input, try-input, overwrite, append, noclobber, merge, explicit close and block redirections.
     Use N1 cwd/path policy; preserve distinct outcomes for missing input, denied opens and closed versus empty stdin.
   - Implement atomic native noclobber creation and append semantics. Preserve redirection ordering, repeated targets,
     self-dup and aliases whose source is later closed; failed planning/opening must not leave child effects behind.
   - Document native-null syntax and treatment; keep shell virtual-device handling separate from real path identity.
2. **Restore the shared asynchronous runtime.**
   - Restore worker/debounce functionality using existing ownership/cancellation semantics, without Unix masks or atfork.
   - Provide completion/wakeup registration, coalescing and consume/reset rules without lost concurrent posts.
   - Use stable nonrecycled registration identity/generations; recycled HANDLE values must not deliver to a new owner.
   - Define remove-and-drain: stop new callbacks, cancel pending operations, wait for in-flight users, then free state.
     Callbacks enqueue work; main-thread consumers apply evaluator state changes.
   - Retain real child process handles and immutable completion records until all registered consumers finish.
     Keep language-level wait-result retention/consumption separate from resource lifetime.
   - Add minimal interruptible waits now. Later watcher/job/editor consumers must reuse this foundation,
     not be the first implementation of completion, debounce, wakeups or process ownership.
3. **Select cancellable, scalable endpoint mechanisms.**
   - Use overlapped-capable pipes where readiness/cancellation is needed; synchronous anonymous pipes are not overlapped.
   - Explicitly own buffers, OVERLAPPED storage, operation identity and completion state until cancellation is drained.
   - Choose bounded pumps or completion mechanisms for the actual endpoint type, not a universal fixed-size wait array.
     More live registrations than `WaitForMultipleObjects` capacity must remain supported.
   - Handle partial reads/writes, data/EOF/error completion and pending-operation cancellation without use-after-free,
     busy waiting, unbounded per-stream accumulation or treating every file/console/socket as one fd namespace.
4. **Schedule real pipelines without serial waits or producer deadlocks.**
   - Connect external→external, builtin→external, external→builtin and builtin→builtin stages using real streams.
   - Start required producers/consumers before waiting for a stage; do not finish one external stage before launching next.
   - Reuse deferred/internal producer scheduling and existing buffering semantics for evaluator-owned stages.
     Never suspend consumers while a main-thread internal producer blocks writing into their full pipe.
   - Prepare explicit child handle inheritance lists and only intended inheritable duplicates; keep internal monitors,
     unrelated files, other launches' pipes and parent-only endpoints out of children.
   - Close unused writer/read ends at their ownership boundary so EOF and broken-downstream status arrive correctly.
     Function/block mixed pipelines are delivered and tested in N3, not falsely claimed by this phase.
5. **Implement capture and shared input infrastructure.**
   - Preserve bounded separated-buffer semantics, explicit empty elements and byte provenance for N3 consumers.
   - On evaluation completion, finish capture without waiting for all descendant-held writers to reach EOF.
     Cancel/drain pending reads, retain already-available output at a defined completion boundary and release operations safely.
   - Define completion-versus-cancellation ordering and test interrupted/partial capture without losing available output.
     This is infrastructure; enabling language command substitution belongs to N3.
   - Buffer input correctly without seeking pipes or overreading bytes needed by the next `read` command.
     Retain byte-exact binary redirected data, NULs and incremental valid-Unicode console output across split writes.
6. **Respect numbered-descriptor limits and failure ownership.**
   - Preserve syntax and internal logical maps; inherit extra descriptors only through a documented, tested child-specific bridge.
   - Diagnose unsupported extra-fd contracts before child launch/marker effects, not by silently dropping the redirect.
   - Own rollback for partial open, pipe creation, registration and spawn failure; retain/terminate/reap only owned children
     as required by the failed operation and close every endpoint/registration exactly once after pending work drains.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no faithful snapshot/return-twice, fork emulation or post-fork Windows backend.
  Execute fish code in the existing evaluator and create children explicitly; no fake successful fork.
- **POSIX `exec`/`execve`:** no same-PID image replacement. Keep the `exec` decorator nonzero unsupported
  before evaluating arguments or launching children; spawn/wait/exit and CRT `_exec` are not replacements.
- **POSIX `fg`/`bg`:** no terminal-pgroup transfer, stop/continue or controlling-terminal job control.
  Keep these commands unsupported, never substitute SuspendThread/ResumeThread or window focus, and never invoke
  a same-named external fallback for unsupported reserved commands, including `exec`.
  Native `&`, jobs, wait and disown in [N5](GOAL_05.md) are supported-roadmap work, not these excluded contracts.

## Mandatory automated tests

Exactly these three required gate IDs must be discoverable and nonignored; implementation subcases belong under them.

| ID | Type | Setup, actions and observable assertions |
|---|---|---|
| **N2-STREAM** | Integration/e2e | Run actual native fish and real helpers through external→external, builtin→external, external→builtin and builtin→builtin pipelines. Send simultaneous stdout/stderr larger than pipe capacity, exact binary/NUL data and slow-consumer traffic; assert bytes, EOF/status and bounded memory. A synchronized external producer waits for a consumer acknowledgement before it may finish: receipt must occur while the producer is alive, proving streaming rather than complete-output spooling on this concurrent path. Verify bounded internal-to-internal buffering follows existing fish scheduling rather than demanding concurrent parser execution. Assert consumer scheduling does not deadlock internal producers. |
| **N2-REDIRECT** | Unit/integration | Exercise actual redirection planner/map owners and real files/handles for input/try-input, overwrite, append/noclobber, ordered dup/close, repeated/self dup, merged streams, closed endpoints, block redirects and explicit native-null handling. Compare `>file 2>&1` with `2>&1 >file`, including failure/status and resulting data. Test supported extra-fd bridges with a real cooperating child; unsupported bridges must fail before its marker side effect. Assert invalid/overflow fd and denied/missing target failures are distinct and release resources. |
| **N2-LIFETIME** | Integration | Synchronize partial spawn/open/registration failures, abandoned readers, early exits, interrupted capture and concurrent launches. Exercise remove/reset/completion races, recycled-handle protection and more registrations than a single wait-array capacity. Hold a descendant writer live while evaluation completes; assert bounded capture finish, retained available output, cancelled/drained pending reads and no wait for descendant EOF. Assert every owned child/handle/registration is cleaned, no unintended child inheritance and no completion-after-free. |

Unit tests must call actual owning production code; integration uses actual pipes/files/process handles, not wiring mocks.
E2e must launch native fish, not an echo substitute. Failure injection may trigger real owning paths but cannot
replace the cleanup/stream implementation under test. Function/block language pipeline coverage is an additional N3 obligation.

## Verification and execution requirements

- Exercise actual native MSVC debug and release artifacts and applicable existing Unix suites for shared changes.
  Record a real runtime streaming/redirection smoke in addition to automated gates.
- Launch helpers with argument arrays and isolated cwd/environment/user-state roots. Bound waits, concurrently drain
  stdout/stderr, and terminate/reap only owned processes; never interfere with enclosing host or unrelated descendants.
- Use event/barrier acknowledgements for scheduling, shutdown and handle-reuse races; sleeps do not establish causality.
  Bound retained-writer fixtures independently of EOF and arrange harness-owned descendant release/cleanup.
- Provision required filesystem/process fixtures explicitly; unavailable required infrastructure blocks the phase.
- When suites land, record exact implementation names/commands, toolchain/SDK, target/revision, expected discovery,
  executed/nonignored counts, results and debug/release artifact references. These proposed IDs are not runnable commands.
- CI fails on missing IDs, empty suites, ignored mandatory cases or unavailable required fixtures.
- Preserve positive prototype behavior and permanent/still-deferred rejections. Replace obsolete pipe/redirect rejection
  tests with substantive positive/failure coverage; migrate CI registry/counts and leave GOAL.md historical evidence intact.

## Definition of done and handoff

Every delivery and all three gates pass; no compile-only or fully skipped acceptance claim is permitted.
Update affected stream/launch/input callers, lifecycle documentation and coverage; remove empty I/O placeholders,
obsolete guards and temporary compatibility paths as real consumers land. Keep all permanent exclusions enforced.
Hand [N3](GOAL_03.md) capture/input/deferred-producer contracts with defined limits, completion boundaries and cleanup.
Hand [N4](GOAL_04.md) real worker/debounce/wakeup services and [N5](GOAL_05.md) retained process/completion ownership.
N5 owns user-visible jobs, richer cancellation/events/accounting, not missing N2 runtime foundations;
[N6](GOAL_06.md) owns interactive input lifecycle and [N7](GOAL_07.md) reuses the same background completion services.

## Delivered interfaces and exercised verification

- `src/io/windows_descriptors.rs` owns typed logical bindings and ordered per-job overlays;
  `src/io/windows_pipe.rs` owns cancellable overlapped endpoints and capture;
  `src/io/windows_deferred.rs` owns bounded internal-to-native deferred pumps.
  Shared separated-buffer storage is in `src/io_buffer.rs`, without replacing Unix I/O ownership.
- `src/exec/windows_launch.rs` prepares every child plan before launch, pins executable identity,
  supplies an explicit inheritance allowlist and retains native processes/registrations.
  `src/native_fd_bridge.rs` carries extra logical descriptors only to the identical running fish image.
- `src/threads/windows.rs` and `windows_workers.rs` deliver scalable native waits, main-thread
  completion queues, cancellation/drain, workers and debounce. N5 still owns user-visible job/control events.
- Finite capture/internal-buffer policies bound data and element metadata. Capture finish cancels/drains
  pending reads, snapshots available pipe bytes once, and does not wait for descendant EOF.
  N3 must reuse this boundary when enabling substitution and mixed function/block pipelines.
- Required Rust test names are `N2_STREAM`, `N2_REDIRECT`, `N2_LIFETIME`, emitting the three
  hyphenated gate IDs for both executable profiles. Discovery must find exactly these three nonignored tests.
- Native execution: `cargo +stable test --locked --target x86_64-pc-windows-msvc
  --no-default-features --test windows_n1 --test windows_n2 -- --nocapture`, with explicit
  `FISH_NATIVE_TEST_BIN`, `FISH_NATIVE_TEST_RELEASE_BIN` and provisioned `FISH_NATIVE_N1_UNC_ROOT`.
  The final minimum-Rust executable plus stable release run passed 3 N1 and 3 N2 tests,
  zero ignored/filtered; each gate exercised both artifacts.
- Shared-code regression: WSL Ubuntu `cargo test --locked --no-default-features --lib`
  passed all 181 tests, zero ignored/filtered, after the final shared cleanup.
- Actual native smoke reproduced and fixed an internal→native relay→internal deadlock,
  imported-fd alias overread, empty-status panic and inherited-unavailable output silently succeeding.
  Explicit builtin output close remains the shared discard policy; unavailable inherited output fails on write.
  See component overlays for ownership limits and the evidence record for complete commands/artifacts.
