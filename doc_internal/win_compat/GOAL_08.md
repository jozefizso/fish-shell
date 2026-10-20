# GOAL_08 — Windows tool interoperability and native platform services

**Phase:** N8. **Status: all currently authorized host cases pass; disposable-account acceptance remains blocked**.
Primary native target: `x86_64-pc-windows-msvc`; ARM64 is a separate track.
The concrete gates below are registered in `tests/windows_n8.rs`; registration is not a passing result.
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

### Integrated provider and service contracts

- `src/path/windows_providers.rs` is the discovery, launch and executable-predicate owner.
  PATH order and PATHEXT select `.exe`, `.cmd`/`.bat`, `.ps1` and `.fish`; completion
  strips those same suffixes. DOS `.com`, unknown/extensionless files and implicit
  shebang interpretation fail explicitly. Invalid `.exe` images reach the native loader
  once; they never acquire a shell fallback.
- Batch uses the OS-reported system `cmd.exe` with `/d /s /v:off /c`. Every accepted
  argument is quoted. Quotes, percent expansion, CR/LF and trailing backslashes are
  outside its literal domain, including in the script path. Script and cwd require
  identity-equivalent ordinary local-drive names. The command and each inherited
  environment entry obey cmd's 8191 UTF-16-unit domain. Rejection precedes pipeline launch.
  Explicit OS-system `cmd.exe` is separately identity-checked and pinned: `/c` or
  `/k` accepts exactly one intentional command-language string, not arbitrary argv.
  `/d /s /v:off` is always present; supported additional switches are `/q /a /u /?`.
  Unknown switches or multiple command-tail arguments fail before launch. Other
  same-named executables do not acquire this interpreter policy.
  Batch script source encoding remains cmd's contract: fish does not silently
  change the console codepage. Native fixtures use ASCII command syntax and
  runtime Unicode path expansion, rather than embedding UTF-8 paths and assuming
  cmd interprets their source bytes as UTF-8.
- PowerShell means the Known Folder `ProgramFiles/PowerShell/7/pwsh.exe`, whose native
  file metadata must identify major 7, minor 3 or newer. `-NoLogo -NoProfile
  -EncodedCommand` runs a constant wrapper; a private base64 UTF-16/NUL-framed environment
  value carries script and argv data and is cleared before the script runs. All strict
  Unicode non-NUL argv, including empty strings and CR/LF, is supported within the
  native frame limit. There is no PATH/COMSPEC/profile/ExecutionPolicy fallback.
  Script, interpreter and cwd must have identity-proven ordinary drive/UNC spellings;
  extended-only names fail diagnostically. No authorization refusal is retried.
- Native application/cwd spellings prefer ordinary Win32 names only after N1
  validation and native identity proof. Extended-only native-image names retain
  their original spelling rather than acquiring a lossy alias. This avoids tools
  treating a `\\?\C:\...` local identity as a UNC share.
- Fish scripts use the retained startup image with `-N -- SCRIPT`; ordinary native
  images and explicitly invoked interpreters other than the system cmd policy use
  native argv serialization. Script/interpreter
  identity pins remain owned through execution and are released when the process exits;
  provider-only noninheritable leases survive shell exit without propagating to grandchildren.
  Existing N5 DWORD mapping and N6/N7 console donation remain the single launch owners.
  High native DWORD exits retain the existing N5 hexadecimal diagnostic; argv
  acceptance tests assert that diagnostic instead of requiring empty stderr.
  Actual counted console receipts latch cancellation on the existing evaluator
  JobGroup before callback rearm. That provenance survives callback dispatch and
  provider completion, including an ordinary exit DWORD of zero after a native
  confirmation/debugger interaction, so the interrupted command tail cannot resume.
  This never rewrites the child's recorded DWORD or infers cancellation from a
  high-bit exit; native input remains donated until the real provider completes.
  Foreground OperationContexts also retain their own counted-receipt baseline:
  cancellation remains true after a nested builtin `read` rearms its reader token,
  unwinding the interrupted function/list instead of executing its remaining tail.
  New callbacks and fresh-reader evaluations start at the current epoch; this is
  lifetime-scoped provenance, not a global sticky cancellation flag or an inference
  from builtin status 130. The shared ignored-control client regression retains
  the acknowledged native handle, compares its actual exit DWORD zero with status
  observed by the fresh reader, and forbids the interrupted command's old tail.
- Existing token/ACL access, read-only, sharing-error, native identity and reparse services
  remain authoritative; the N8 policy suite supplies actual denied ACL, hard-link,
  symbolic-link and NUL-device fixtures rather than invented POSIX mode/UID/GID values.
- `__fish_native_services` exposes local SAM users/groups, token groups/SIDs, explicit
  account/profile lookup, Toolhelp process observations, process image lookup, local
  adapters/addresses/hostname/hosts, process accounting, timestamp formatting (epoch
  seconds or `now`), logical drives and the registered desktop open verb. Identity
  fields reject unrepresentable line controls; descriptions alone are display-sanitized.
  Domain enumeration, numeric UID/GID and Unix utility option grammars fail explicitly.
  `open` passes each target separately to `ShellExecuteW`, not to a command shell.
- Native `ulimit` inventories every POSIX category in `ulimit_windows.rs` and rejects
  POSIX soft/hard/per-user forms. Only `--native-process-memory-kib`,
  `--native-job-memory-kib`, `--native-process-cpu-seconds` (USER CPU) and
  `--native-active-processes` are supported. Positive decimal setters are validated
  on a separate empty Job before atomically updating this parser's future-group
  defaults; query/reset uses `not-configured`, never an invented effective `unlimited`.
  Memory units are exactly 1024 bytes. Restrictions apply before preparation to future
  isolated child Jobs, not fish, enclosing Jobs or existing work. Kernel failures
  abort without weakening quotas. Completion children receive the same policy snapshot;
  the private non-evaluator substitution keeper is resource infrastructure, not a consumer.
- File-backed `psub` uses exclusive owner/SYSTEM-only private storage and framed exact
  bytes. A retained startup-image keeper owns cleanup, with acknowledged suspended-root
  Job leases retaining files for native/disowned consumers after caller or shell exit.
  Failure/cancellation drains owned operations; FIFO mode is explicitly unsupported.
  `__fish_native_fs make-temp-file` and `make-temp-directory` extend that same keeper
  protocol with exclusive fs-owned capabilities. Their owner is the outermost active
  function Job, so nested mktemp/set substitutions do not delete them prematurely.
  Normal return, failure and real console cancellation release the caller capability;
  acknowledged native consumers retain it through their actual Job lifetime. Matching
  uses original argv/environment before provider rewriting, directory descendants,
  redirected file handles and cwd—not arbitrary HANDLE inheritance.
  `remove PATH` and `remove-temp-directory PATH` explicitly release helper owners.
  `transfer-temp-file PATH` requires a previously acknowledged accepted native child
  Job (including an already-completed child), enabling disowned updater ownership
  without an unowned creation window. Helper files permit consumer unlink; psub keeps
  its original sharing policy. Outside a function, explicit cleanup is required.
  `read PATH` preserves binary bytes without `cat`; `atomic-write` observes the retained
  cancellation receipt before reads and immediately before publishing its update,
  so cancelled producer EOF cannot commit a partial function/cache definition.
- The deliberate argcomplete adapter supports native CPython 3.10–3.14, installed
  argcomplete 3.x and an opted-in `.py` application. A private allowlist negotiates
  two synchronous handles; the child bootstrap installs noninheritable CRT fds 8/9,
  acknowledges negotiation and runs the real application. UTF-8 newline-delimited
  candidates are limited to 64 KiB per record and 1 MiB total; COMP_POINT wire offsets
  are UTF-8 bytes translated at argcomplete's Unicode API boundary. Workers only move
  bytes; parser work stays on the evaluator thread. Missing/version-incompatible
  dependencies, Unix socket/generic-fd/launcher variants, partial negotiation,
  malformed records and cancellation fail diagnostically with owned cancel-and-drain.
  Both preflight proof bytes and the explicit UTF-8 candidate stream use LF-only framing;
  CPython's default Windows CRLF text translation is not part of the protocol.

The three concrete commands are `cargo test --test windows_n8 n8_argv -- --nocapture`,
`cargo test --test windows_n8 n8_tools -- --nocapture` and
`cargo test --test windows_n8 n8_policy -- --nocapture`. Each gate runs both required
profile-matched artifacts and retains per-family evidence under
`FISH_NATIVE_N8_EVIDENCE_ROOT` (default `target/n8-gate-results`).
Native debug/release evidence across the four toolchain/feature cells is recorded below;
neither an authorized-host subset nor a missing-fixture rejection closes the phase.

| Gate | Declared families | Required debug + release executions |
|---|---:|---:|
| N8-ARGV | 11 | 22 |
| N8-TOOLS | 20 | 40 |
| N8-POLICY | 49 | 98 |

Each full run retains its command log under `target/n7-gate-results/`.

| Toolchain / features | N8-ARGV | N8-TOOLS | N8-POLICY | Command log |
|---|---:|---:|---:|---|
| 1.95 / minimal | 22/22 | 38/40 | 96/98 | `n8-callable-alias-and-acknowledged-provider-control.txt` |
| 1.95 / default | 22/22 | 38/40 | 96/98 | `n8-current-default-acknowledged-controls-and-aliases.txt` |
| 1.85 / minimal | 22/22 | 38/40 | 96/98 | `n8-msrv-minimal-acknowledged-controls-and-aliases.txt` |
| 1.85 / default | 22/22 | 38/40 | 96/98 | `n8-msrv-default-acknowledged-controls-and-aliases.txt` |

Every cell discovered/executed all 160 cases with zero ignored. The four rejected
executions per cell are the same two debug/release pairs requiring the explicitly
declined disposable-account fixture: profile/AutoRun markers and profileless-account
SAM setup. Their guards reject before writing current-account profiles, HKCU or SAM.

The policy gate proves actual handle-object isolation rather than inferring
inheritance from reused numeric handle slots. Native resource cancellation uses
an acknowledged Win32 control callback and console-input/cancellation wait:
CPython's blocking `input()` waits on its SIGINT-only wake event and does not
provide the required CTRL_BREAK wake contract. The real consumer exits130,
the interrupted shell tail stays absent, and the recovered reader/resources
are asserted. Foreground control receipts remain sticky to their owning evaluator
epoch across nested-reader rearming without rewriting the native child's DWORD.

Batch cancellation confirmation now sends acknowledged native Y/Enter console key
records. Merely queuing VT pipe bytes did not establish that cmd consumed its
confirmation. Both the focused four-execution smoke and the full tools gate exercise
the native-record path without extending timeouts or matching localized prompt text.

The tools gate includes eight pinned Git/compiler/npm/Python/PowerShell workflows,
seven real-provider console cases, and five retained native input-ownership regressions.
The policy gate includes eight quota/predicate, 31 substitution/adapter and ten
service/helper families. These are discovery requirements, not observed passing counts.
Provider ownership fixtures explicitly put PowerShell native calls in a foreground
pipeline and assert each native exit, rather than depending on host application
classification to wait. The surviving-grandchild probe clears inheritance only on
standard I/O handles, so capture EOF cannot mask provider exit; generic inheritance
remains enabled for every other handle to detect leaked provider leases. Deliberate
extended-only test objects are removed using their exact extended names before the
ordinary evidence-copy phase.
Generated PowerShell fixtures declare UTF-8 with a BOM, including Unicode native
program paths and owned profile markers. Ownership scripts retain an explicit UTF-8
runtime record of actual interpreter path/version, script identity and PATHEXT,
and assert the installed interpreter's native identity plus supported runtime version.
This is a fixture encoding contract, not permission to recode user scripts or bypass
an interpreter's authorization or execution policy.
PowerShell ownership/native-descendant rows explicitly provision `PATHEXT=.EXE`
in their own launch environment and assert the effective `.EXE` capability after
interpreter startup. This is required by PowerShell's own native-command discovery;
the tested 7.6.7 bootstrap can otherwise produce a `.CPL`-only suffix list from an
absent PATHEXT. Provider-console native wrappers and nested psub script consumers
declare the same prerequisite. Generic empty-environment fixtures and negative rows
remain unchanged, and production fish never injects or repairs PATHEXT.

Required fixtures include writable `FISH_NATIVE_N1_UNC_ROOT`, native installed PowerShell,
ACL/reparse capability, and three explicit real Python environments:
`FISH_NATIVE_TEST_PYTHON` (supported CPython plus argcomplete 3.x),
`FISH_NATIVE_TEST_PYTHON_NO_ARGCOMPLETE`, and `FISH_NATIVE_TEST_PYTHON_UNSUPPORTED`.
The tools gate additionally requires absolute native `FISH_NATIVE_N8_GIT`, `_RUSTC`,
`_NODE`, `_NPM`, `_PYTHON`, `_PWSH` paths with matching `_VERSION` pins, the MSVC/SDK
linker/SDK executable directories in `FISH_NATIVE_N8_TOOL_PATH`, MSVC/SDK LIB search
list in `FISH_NATIVE_N8_LIB`, and a dedicated clean account for owned profile/AutoRun
markers. Set `FISH_NATIVE_N8_DEDICATED_ACCOUNT=1` only inside that disposable account;
the marker fixture rejects an unacknowledged account before writing HKCU or profiles.
Missing prerequisites block evidence; they never become successful skips.


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
