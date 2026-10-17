# GOAL_05 — Cancellation, asynchronous jobs, events and accounting

**Phase: N5**  
**Status: planned / not started**  
**Primary native target:** `x86_64-pc-windows-msvc`; ARM64 is a separate extension.

Acceptance IDs below specify future required behavior, not existing test names,
runnable targets or passing results. This follows the approved roadmap at commit
`2acbbad4b` and preserves the historical proof-of-concept [GOAL.md](GOAL.md).

## Objective

Deliver actual asynchronous native jobs, stable waits, exactly-once lifecycle
notifications, controlled cancellation and truthful native resource accounting.
Users can launch `&` pipelines, inspect jobs, wait and disown without enabling
POSIX terminal job control or silently killing unrelated host processes.
These services provide the lifecycle foundation for the interactive reader.

## Entry conditions and dependencies

- Verify [GOAL_02](GOAL_02.md) and [GOAL_03](GOAL_03.md) have passed their gates.
  Real retained process handles, asynchronous workers, debounce/wakeup/completion,
  cancellation-and-drain, streaming and evaluator completion already belong to N2.
  N5 exposes those services to users; it must not invent the first working backend.
- Retain N2 mixed-pipeline scheduling: parser work stays on its owning main thread,
  internal producers are scheduled without suspending consumers and then blocking.
- Preserve N3 language/events and capture completion independent of descendant-held
  writers. Do not regress foreground launch and raw native status from [GOAL.md](GOAL.md).
- Suggested branch: `platform/windows/process-supervision`. N4 may proceed
  independently; [GOAL_06](GOAL_06.md) requires both N4 and N5 to close.

## Owners and native invariants

- `src/exec_windows.rs`, `src/proc.rs`, `src/proc_status_windows.rs`,
  `src/job_group.rs` and `src/wait_handle.rs` own launch, process identity,
  containment, retained completion and job lifecycle state.
- `src/signal_windows.rs`, `src/event.rs`, `src/topic_monitor.rs`, `src/timer.rs`
  and the shared event/timer services own safe control-event reception and
  main-thread callback dispatch. `src/builtins/jobs.rs`, `src/builtins/wait.rs`
  and `src/builtins/disown.rs` own real command/option behavior.
- Native startup owns `--print-rusage-self`; process/time consumers own reported
  per-command, pipeline and shell counters. Inventory their actual call sites.
- Follow [process analysis](03-process-execution-job-control.md),
  [events/waits analysis](04-signals-events-waits.md),
  [runtime support](12-concurrency-and-runtime-support.md) and
  [Windows API reference](windows-api-reference.md).
- Retain raw DWORD exit values. Shell status maps 0–255 unchanged and every other
  ordinary native exit to 1. A high bit does not prove cancellation or a signal.
  Track cancellation request/provenance separately from observed child completion.

## Implementation delivery

1. **Model live and completed work.** Give jobs stable shell identity and keep
   native handles/generations so PID reuse cannot redirect waits or cancellation.
   Cache completion beyond active-list removal for `wait`, including multi-target
   waits and `wait -n`. Define unknown/duplicate targets, already-exited work and
   no-eligible-job behavior; completed pipeline status follows actual members.
2. **Dispatch exactly once.** Serialize process/job/exit events and timer callbacks
   through the evaluator's owning thread. Define registration, consumption and
   removal order so completion-before-registration, callback reentrancy and shutdown
   cannot lose or duplicate events. Release handles only after users and pending
   completions no longer need them; no busy polling or fake successful notifier.
3. **Make background input explicit.** Default each unredirected background stdin
   to a readable native `NUL` endpoint, reaching EOF instead of borrowing the
   reader's input. Honor explicit input redirection, including supplied pipes/files.
   Do not claim to prevent a hostile child reopening `CONIN$` or generically replace
   an already-running child's inherited stdin. Preserve pipeline backpressure.
4. **Specify cancellation before handlers.** Publish a transition table covering
   request issued, unsupported request, ignored request, acknowledged cancellation,
   normal exit and forced termination. Define exact status/pipestatus outcomes and
   event ordering independently of the raw DWORD mapping; retain raw observations.
   A request alone must not mark a live child completed or fabricate signal state.
5. **Receive console controls safely.** Unsafe callback contexts may only perform
   the documented bounded signal-safe handoff: no allocation, locks or parser work.
   Wake the normal runtime for evaluator cancellation and process actions. Distinguish
   Ctrl-C, targeted Ctrl-Break where genuinely supported, and explicit abrupt
   termination. Never silently escalate a failed/ignored cooperative request.
6. **Define child eligibility and cleanup.** Document console-group/attachment
   constraints and supported behavior for GUI, detached and foreign-console children.
   Cancel blocked evaluator/pipe work using N2 ownership and cancel/drain rules;
   preserve available output and unrelated jobs. Callback unregister/shutdown must
   finish before referenced state is freed; errors retain a truthful live-job record.
7. **Choose containment compatible with disown.** Use per-job Job Objects only
   where membership/lifecycle meets the declared policy. State whether ordinary jobs
   survive shell exit and guarantee the promised disowned-job survival. Never leave
   kill-on-close enabled for jobs promised to survive; membership is not generally
   detachable. Do not assign work to or terminate an enclosing CI/host Job Object.
   On assignment failure, terminate/reap the still-unassigned suspended child as well
   as owned members, close all handles, and report failure without orphaning it.
8. **Deliver observable lifecycle behavior.** Implement real `jobs`/`wait`/`disown`
   options and invalid-target diagnostics, shutdown warnings, process/job/exit
   handlers and their interaction with disowned/cached jobs. Define graceful shell
   exit versus explicit forced termination. A disowned child may remain an OS child;
   do not promise Unix reparenting or foreground reattachment.
9. **Account without invented fields.** Use `GetProcessTimes` and applicable Job
   Object counters with explicit elapsed/CPU units, collection intervals and nested
   child scope. Cover builtin-only work, concurrent pipelines and completed children
   without double counting. Implement `--print-rusage-self` using real shell counters;
   name unavailable POSIX categories explicitly instead of reporting fictitious zero
   measurements. Quotas/resource-limit option semantics remain N8, not fake rusage.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no snapshot/return-twice emulation or post-fork backend;
  execute shell code in the existing evaluator and create native children explicitly.
- **POSIX `exec`/`execve`:** no same-PID image replacement. Keep the `exec`
  decorator nonzero unsupported before argument side effects. Spawn/wait/exit and
  CRT `_exec` do not supply this contract.
- **POSIX `fg`/`bg`:** both remain unsupported, with no terminal-pgroup transfer,
  stop/continue emulation, `SuspendThread`/`ResumeThread` or window-focus substitute.
  Unsupported reserved commands must not fall through to same-named executables.
  Native `&`, jobs, wait and disown are explicitly delivered, not excluded.

## Mandatory automated acceptance

| Required ID | Type | Observable acceptance |
|---|---|---|
| **N5-JOBS** | multiprocess e2e | Actual fish launches background native/mixed pipelines; inspect jobs, multi-target/`-n` waits, already-exited cached results and PID reuse safety. Barrier-controlled completions prove exactly-once process/job/exit events. Disowned work survives shell exit under the declared policy. Exercise invalid targets, partial pipeline launch and containment-assignment failure without orphaning suspended children. |
| **N5-CANCEL** | integration/console e2e | Cancel real long native work and blocked pipe/evaluator work while an unrelated job stays live. Test request/ignored/acknowledged/forced outcomes, Ctrl-C versus eligible Ctrl-Break, and unrelated high DWORD exits. Assert raw/status/pipestatus and event provenance separately. GUI/detached/foreign-console fixtures receive only declared supported operations; no silent escalation or enclosing-host termination. |
| **N5-ACCOUNT** | unit/integration | Units exercise the actual owner status/accounting logic; real CPU child, builtin, concurrent pipeline, cached completed child and nested-child runs exercise collection. Assert units, nonnegative plausible counters, declared accumulation/scope, no double counting and explicit unavailable fields. Actual `--print-rusage-self` output/errors must not corrupt normal stdout/status. |

Add synchronized background-stdin EOF versus explicit-redirection checks and
completion/removal/shutdown races inside these gates. Verify real handles close
only after pending callbacks drain, and process ownership prevents PID-only kills.
Tests must not infer CPU usage from wall-clock sleeps or simulate completion by
calling an unrelated mock notifier. Required console/job fixtures block exit if absent.

## Verification and exit evidence

- Exercise actual native MSVC debug and release artifacts, retained applicable Unix
  jobs/wait/events/accounting suites and existing portable batch/pipeline tests.
- Use argument-array launches, isolated state and a harness outside the per-job
  containment under test. Bound waits and drain both output streams concurrently.
- Synchronize child/control/assignment stages, not sleeps. Cleanup terminates/reaps
  only owned work; verify survival before deliberately cleaning up disowned fixtures.
- CI discovers exactly the three mandatory IDs and all declared cases, rejecting
  missing IDs, zero runs, ignored cases and unavailable required fixtures as success.
- Record actual implementation names/commands, discovered/executed/nonignored counts,
  toolchain/SDK, target, source revision, results and artifacts when tests land.
  This planning contract asserts no runnable future Cargo target or passing result.
- Retain a real runtime smoke launching, inspecting and waiting on a background
  pipeline, cancelling an owned foreground fixture and observing disown survival.

## Definition of done and handoff

Every delivery/gate passes; callback ownership, statuses, exit policy, command/CLI
options and native accounting units are documented with updated callers and counts.
Preserve prototype positives and permanent/still-deferred rejection checks. Replace
obsolete background/job/cancellation rejection tests with positive and failure cases;
remove obsolete guards/placeholders, leaving historical GOAL evidence unchanged.
Hand [GOAL_06](GOAL_06.md) safe control/input ownership and job notifications;
[GOAL_07](GOAL_07.md) consumes the bounded completion services, and
[GOAL_08](GOAL_08.md) extends native providers/accounting-related platform options.
N11 audits these completed consumers; it does not own unimplemented waits or events.
