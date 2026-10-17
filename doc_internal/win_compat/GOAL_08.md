# GOAL_08 — Windows tool interoperability and native platform services

**Phase:** N8. **Status: planned / not started**.
Primary native target: `x86_64-pc-windows-msvc`; ARM64 is a separate track.
The gates below specify future acceptance work, not existing test targets or passing results.
This contract expands the approved roadmap at revision `2acbbad4b`.

## Objective

Run native Windows toolchains and deliberately supported script providers with secure,
provider-specific argument semantics, truthful native predicates and usable helper protocols.
A user must be able to distinguish literal argument delivery from explicit unsupported input;
fish must never turn a failed native launch into an opportunistic shell interpretation.
Completion adapters and native services delivered here enable the full asset closure in
[GOAL_09](GOAL_09.md), without requiring Unix utilities for mandatory shell behavior.

## Entry conditions and branch

- Verify [GOAL_03](GOAL_03.md) language/script execution and [GOAL_05](GOAL_05.md)
  process supervision, accounting and retained-handle ownership before adapter integration.
- Verify the N1 command classification policy and N2 stream/inheritance services through
  their implementation evidence; the presence of these planning files is not that evidence.
- Noninteractive provider, predicate and file-backed helper work may begin after N3/N5.
- Exit additionally requires [GOAL_06](GOAL_06.md): interactive native-child input handoff,
  console modes and fish-owned VT protocol restoration must work with the real providers.
- Suggested branch: `platform/windows/native-interop`; share established launch/state seams,
  rather than creating another process backend, evaluator or platform-framework rewrite.

## Owners and native invariants

- Extend command discovery/classification in `src/path/windows.rs`, execution in `src/exec.rs`
  and native process owners, with `src/env/` providing exported environment and cwd policy.
- Native predicates belong to `src/wutil/`, `src/fs.rs` and the owning `path`/`test` builtins;
  resource options belong to `src/builtins/ulimit.rs`, with N5 accounting services reused.
- Coordinate `share/functions/psub.fish`, native account/process/network providers and
  `share/functions/__fish_argcomplete_complete.fish` with their production services.
- Rationale: [processes](03-process-execution-job-control.md),
  [limits/events](04-signals-events-waits.md), [I/O](05-io-redirection-file-descriptors.md),
  [filesystem](07-filesystem-paths-wildcards.md), [assets](13-binaries-and-assets.md) and
  [Windows APIs](windows-api-reference.md). These analyses are not implementation evidence.
- Preserve strict identity conversion: no lossy path/environment aliases or lone-surrogate
  acceptance unless a separately tested reversible native-name codec has replaced rejection.

## Implementation delivery

1. Centralize deliberate `.cmd`/`.bat` and `.ps1` providers under N1 resolution rules.
   `.fish` uses fish. Specify `.com`, supported shebangs and unknown/binary/text formats;
   unsupported formats fail diagnostically, without optimistic interpreter launch retries.
   Resolve explicit interpreter paths and native PATH/PATHEXT collisions consistently.
2. Specify each provider's interpreter/version, switches, accepted argv domain, quoting,
   environment expansion and newline policy. Batch command syntax is not arbitrary argv.
   Disable autorun/profile behavior as appropriate; do not concatenate untrusted strings
   into `cmd` or PowerShell syntax without a proven provider-specific literal policy.
   Reject unrepresentable input before provider execution or its side effects, with a
   useful nonzero error. No generic `cmd /c`, PowerShell or `/bin/sh` fallback is allowed.
3. Preserve empty arguments, Unicode, quoting, cwd, exported environment and byte streams
   across native executables, npm-style shims and explicit interpreter launches. Keep raw
   DWORD status and N5 mapping: 0–255 unchanged, every other native exit maps to 1;
   cancellation provenance remains separate from an ordinary high-bit exit.
4. Exercise interactive child programs through N6 handoff: only one input owner reads,
   restore Win32 console modes and fish-owned paste/enhanced-key protocols before/after
   child input, failed launch and orderly cancellation. This is an exit requirement,
   not a reason to defer the independent noninteractive provider implementation.
5. Complete access, identity and device predicates with native ACL/token/reparse services.
   Distinguish denied access, read-only attributes, sharing failure and missing paths;
   an attribute check is not proof of ACL access or a guarantee of a future open.
   Do not invent UID/GID/mode bits or reinterpret all reparse points as Unix symlinks.
6. Close applicable time/process/account/network helper services and their option ledgers.
   Use native capability discovery; WMIC, `ps`, `getent` and Unix network utilities are not
   baseline dependencies. Missing optional tools produce useful capability diagnostics.
7. Inventory every `ulimit` option/category and query/set form. Implement only enforceable
   native quotas with documented scope, units, permissions, lifetime and failure semantics.
   Job Object process/memory/time limits do not establish all POSIX soft/hard/per-user limits.
   Unsupported categories fail explicitly and enter the semantic-difference ledger;
   a successful setter or an invented `unlimited` value is not acceptable enforcement.
8. Deliver file-backed `psub`: exclusive private temporary creation, exact data, a usable
   native consumer path and caller/job lifetime ownership. Preserve files while consumers
   need them; close handles and remove owned files on completion, failure and cancellation.
   Classify FIFO mode separately: Windows named pipes are not POSIX pathname FIFOs.
9. Supply tested child-specific adapters for completion protocols using fd 8/9 or sockets.
   Specify endpoint/environment negotiation, handle allowlists, child cooperation, framing,
   shutdown and errors. Arbitrary HANDLE inheritance is not a universal POSIX fd table.
   Unsupported protocol variants fail cleanly without leaking endpoints or executing a
   partially configured child. Queue completion to existing main-thread dispatch;
   callbacks/workers must not run parser code or outlive their cancel-and-drain owner.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no process snapshot/return-twice emulation and no post-fork backend.
  Reuse the evaluator and explicit native child creation, never fake successful fork state.
- **POSIX `exec`/`execve`:** no same-PID image replacement. Keep the `exec` decorator
  nonzero unsupported before argument side effects; spawn/wait/exit and CRT `_exec`
  are not replacements and must not be advertised as such.
- **POSIX `fg`/`bg`:** no terminal-pgroup transfer or stop/continue implementation.
  Keep both commands unsupported; no `SuspendThread`/`ResumeThread` or window-focus
  substitution. Native `&`, `jobs`, `wait` and `disown` from N5 remain supported contracts,
  not POSIX job control. Unsupported reserved commands never fall back to same-named tools.

## Mandatory automated tests

| Exact gate ID | Required type | Observable setup, actions and assertions |
|---|---|---|
| **N8-ARGV** | integration/e2e | Execute actual native, batch, PowerShell and fish script fixtures through native fish. Capture decoded arguments and compare empty/space/quote/trailing-backslash/Unicode values, leading dashes, `% ! ^ & \|` and each supported newline case. For inputs outside a provider's declared domain assert prelaunch rejection, useful status/diagnostic and no provider side-effect marker. Injection payloads target separate marker commands and prove those never execute. Cover cwd/environment/byte streams, explicit interpreters, extension/classification collisions and raw native statuses. |
| **N8-TOOLS** | real-tool e2e | Pin native Git, compiler, package-manager, Python and PowerShell versions plus console-tool fixtures; execute meaningful repository/build/package/script workflows. Put conflicting tools/interpreters earlier on PATH and install autorun/profile marker fixtures; assert deliberate resolver choice, no marker execution, no generic fallback and useful missing-dependency errors. Run actual interactive child input under console/ConPTY and assert input ownership and mode/protocol restoration after success, launch failure and cancellation. |
| **N8-POLICY** | unit/integration | Unit-test actual predicate/quota/provider policy owners; integrate real denied ACLs, read-only files, reparse targets and supported devices. Enforce each advertised quota with a real child and assert scope/units and failed/unsupported changes. Consume file-backed `psub` through real native tools; synchronize consumer lifetime, cancellation and cleanup. Negotiate real fd 8/9/socket adapters with cooperating children, assert literal payloads and unrelated-handle exclusion; reject unsupported Unix predicates, FIFO/descriptor variants and partial setup without leaks. |

### Execution and evidence requirements

- Implement discoverable concrete cases and commands for the three IDs when their suites
  land; do not present speculative Cargo targets as currently runnable acceptance commands.
- Run native MSVC debug and release artifacts, actual `fish.exe` and real required tools;
  unit mocks, source-text assertions, wiring checks and compile-only success are insufficient.
- Preserve applicable Unix language/backend suites and separate native/MSYS/Unix evidence.
  Required tool, privileged ACL/device, network or console fixtures must be provisioned;
  unavailability blocks closure, not an ignored/empty successful run.
- Launch with argument arrays, isolate user/config/temp roots and PATH, bound every wait,
  drain stdout/stderr concurrently and terminate/reap only owned work. Synchronize provider,
  consumer and cancellation races with explicit events/barriers, never arbitrary sleeps.
- Record revision, toolchain/SDK, target, feature profile, debug/release artifact references,
  exact commands/case names, expected discovery and executed/nonignored counts, tool versions,
  results and captured diagnostics/markers. CI fails on missing IDs, zero cases or hidden skips.
- Retain real runtime smoke evidence for an actual native toolchain, literal script arguments,
  a file-backed substitution and an interactive child, separately from automated results.

## Definition of done and handoff

Every numbered delivery and all three required gates pass, with no missing/ignored cases.
Update provider/native-semantics documentation, option/caller ledgers and owning CI counts.
Preserve positive prototype regressions and permanent/still-deferred rejection coverage;
replace obsolete script-provider rejection tests with literal-success and real failure cases,
not just a flipped expected status. Keep historical [GOAL](GOAL.md) evidence unchanged.
Remove obsolete prototype guards, unused adapter scaffolds and superseded launch paths.
Hand proven services/protocols and evidence to [GOAL_09](GOAL_09.md); complete asset adaptation
belongs there, but missing N8 argument security, predicates or helper protocols do not.
Optional ARM64 and additional distribution tracks do not alter this x64 completion contract.
