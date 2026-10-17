# Goal: native Windows fish proof of concept

## Objective and completion boundary

Deliver a native `x86_64-pc-windows-msvc` console executable that evaluates a deliberately small subset of real fish code and runs sequential foreground native executables. Reuse fish's parser, evaluator, variable model and selected builtin implementations; do not implement a second shell language or substitute command execution through another shell.

The first runnable milestone is builtin-only `fish.exe -N -c ...`. The completed proof of concept must additionally launch a real native child, pass arguments/environment/current directory/standard streams correctly, wait for completion and report its status.

This goal implements the five workstreams from the reviewed proposal. The fifth workstream is split into two phases so builtin evaluation and external process execution have separate, testable deliveries. It does **not** promise full Windows feature parity. ARM64, interactive use and additional shell capabilities require separately approved scope.

**Status:** implementation not started. Every phase and test below is required future work, not an assertion that native support or passing tests already exist. Test IDs are acceptance specifications, not existing test names.

## Explicitly out-of-scope contracts

These exclusions apply to the entire goal, not merely an early phase. They concern faithful POSIX behavior for arbitrary programs using supported native Windows facilities without a compatibility runtime.

| Contract | Explicit exclusion | Required behavior of this prototype |
|---|---|---|
| **POSIX `fork()`** | No snapshot-and-return-twice implementation of the running fish process, fork emulation, or post-fork child backend. | Run supported builtins/control flow in fish and create external processes explicitly. There must be no fake successful fork operation. |
| **POSIX `exec` / `execve()`** | No process-image replacement preserving fish's PID and process relationships. Spawn/wait/exit emulation of the `exec` decorator is also outside this goal. The CRT `_exec` family is not a same-PID solution. | Reject the `exec` decorator with a nonzero status and an unsupported-operation diagnostic before evaluating its arguments or starting a child. Ordinary external commands remain supported. |
| **POSIX `fg` / `bg` terminal job control** | No controlling-terminal foreground-pgroup transfer, automatic `SIGTTIN`/`SIGTTOU` stops, or arbitrary stopped-job continuation using `SIGTSTP`/`SIGCONT`. No `SuspendThread` substitute. | Reject `fg` and `bg` as unsupported; do not dispatch them to another executable or report successful continuation. Logical foreground execution means waiting for one child, not implementing Unix job control. |

Background execution itself is also deferred for this prototype. That is a scope choice, not a claim that Windows cannot run background processes. Likewise, pipes, an interactive shell and logical job tracking are implementable later without preserving the three excluded contracts.

See [process execution and job control](03-process-execution-job-control.md) and the [Windows API reference](windows-api-reference.md) for the semantic differences.

## Supported prototype contract

- Run from a native Windows host using `fish.exe -N -c <fish-code> [arguments...]`; `--no-config` is the long spelling of `-N`.
- Preserve fish tokenization, quoting, list/variable expansion, local/global scopes, sequential statements, `begin`/`end`, `if`/`else`, `for`, and boolean command chaining for supported commands.
- Support the builtin operations required by the acceptance cases: `echo`, `printf`, `set` (local/global assignment, export, query and erase), `math`, `true`, `false` and `exit`. Other options or builtin commands must be explicitly excluded unless implemented and tested. In particular, `set -U` must not silently become a global assignment.
- Launch executables by explicit absolute `.exe` path, with conventional Windows argument decoding. Preserve empty arguments, spaces, quotes, trailing backslashes and valid Unicode. Do not use a generic `cmd.exe /c` or PowerShell fallback.
- Preserve fish export visibility when constructing a child's environment, and use a real absolute current directory consistent with `$PWD`. No `cd` builtin is required in this goal; the harness supplies the initial cwd.
- Read/write real inherited standard streams synchronously. Console text supports valid Unicode; redirected files and pipes retain the supported byte stream without CRT newline rewriting. Using pipes in the test harness does not imply support for fish pipeline syntax.
- Run supported code without configuration scripts, interactive terminal setup, history/universal-variable persistence, Python tools, coreutils, a POSIX shell, WSL, Cygwin or an MSYS runtime.
- Use the normal Windows console Ctrl-C termination behavior initially. Graceful fish-specific cancellation, forwarding and cleanup-on-control-event guarantees are not part of this goal. Document this limitation rather than claiming POSIX signal support.

### Other deferred capabilities

The following must not be advertised as implemented: interactive reader/editor and rendering; programmable completion/autoloaded user functions; on-disk function/config sourcing; glob traversal; pipelines; command substitution; file or arbitrary numbered-fd redirection; background jobs; history/universal-variable storage and notification; arbitrary POSIX signals; `ulimit`; automatic PATH/PATHEXT executable search; `.bat`/`.cmd`/PowerShell script dispatch; manpage/localization tooling; the standalone `fish_indent`/`fish_key_reader` tools; Windows installers and terminal-profile integration.

Native path support initially covers ordinary local absolute drive paths, including spaces and valid Unicode. Drive-relative paths, UNC/extended namespaces, long-path guarantees and unpaired UTF-16 surrogates are deferred. Unsupported identity-bearing input must be rejected without lossy substitution; it must not alias another file or environment value.

## Verification rules for every phase

1. A phase is complete only when its delivery and **all** mandatory automated tests pass on native Windows. Compilation, a screenshot, a manually typed example or an empty/fully skipped test suite is insufficient.
2. Record the exact test command, toolchain/target, executed case IDs/count, result and artifact location. Test names and runnable commands must be documented when each suite is implemented; the runner must fail if a required case is undiscovered or skipped.
3. Follow existing colocated Rust unit-test conventions. Use integration tests for real filesystem/handle/process behavior and e2e tests for the executable. Reuse portable existing language cases where they cover the contract. Do not add source-text, wiring, mock-echo or bare-not-throw tests.
4. Before the full executable is available, tests must link the actual owning production component through the selected native compilation boundary. A tiny test driver is acceptable; a copied implementation, stub shell or unrelated executable is not. Full `fish.exe` compilation is an explicit Phase 5 gate, not a claimed Phase 1 result.
5. Launch subprocesses with argument arrays from a native Rust harness, or Python `subprocess` with `shell=False`. Do not depend on littlecheck's POSIX host-shell directives, `tests/test_driver.py`'s Unix initialization, pexpect or tmux to establish native acceptance.
6. Use isolated temporary directories and child environments. Bound subprocess waits, drain both output streams, and terminate/reap only test-owned processes on failure. Serialize tests that mutate process-global cwd/environment/locale state, or isolate that mutation in child processes. No sleeps as a substitute for synchronization.
7. Preserve relevant Unix behavior. Shared-code changes must pass their applicable existing Unix unit/integration suites and the portable language acceptance cases. Do not disable those tests to make Windows green.
8. Add a distinct native-MSVC CI lane in Phase 1 and grow its required tests with each phase. Existing MSYS/Cygwin CI remains separate evidence. Later CI success cannot retroactively substitute for missing phase tests.

## Phase 1 — Native build support and compilation boundaries

**Depends on:** none.

**Owners:** [root manifest](../../Cargo.toml), [root build script](../../build.rs), [build-helper](../../crates/build-helper/src/lib.rs), [library module ownership](../../src/lib.rs), [builtin modules](../../src/builtins/mod.rs) and [registration](../../src/builtins/shared/misc.rs).

### Delivery

- Make build-host helpers native-Windows-safe, including the observed Unix `OsStrExt` dependency in `fish-build-helper`.
- Replace the native Cargo build's direct `.sh` version-generator invocation with host-portable version acquisition, preserving package-version fallback when Git metadata/tools are absent. Keep existing Unix/release callers working.
- Establish explicit target/dependency/module boundaries for the prototype, including builtin registration and their transitive helpers. A new binary linked against the unchanged full library does not satisfy this requirement.
- Keep native build output/version probes separate from Unix-only capability probes. Retained native libraries, including PCRE2-32 if retained, must match MSVC architecture/runtime. Exclude a dependency only together with every consumer outside the supported slice; merely not calling its functions does not stop Rust compiling it.
- Provide native Cargo test entry points and a Windows CI lane for the build-support components. Do not require root `fish.exe` to compile before Phases 2–4 have supplied its real services.

### Mandatory automated tests

| ID | Type | Behavior and required assertions |
|---|---|---|
| W1-VERSION | Unit | Verify version selection with matching Git version, a revision requiring the package-version prefix, and unavailable Git/source-archive metadata. Assert the expected version value and fallback/error distinction, not implementation text. |
| W1-HOST | Integration | Execute the real native build-support/version path in an isolated source/archive fixture under a space/Unicode directory, with no `sh`, `awk` or Git available. Assert the package-version fallback and successful native build-support execution; inspect the resulting version rather than accepting compile success alone. |

**Exit gate:** the build-support production code and W1 tests run under MSVC without a POSIX host tool. The native compilation/test boundaries are explicit; no full-shell build claim is made yet. Carry the selected graph forward and close it in Phase 5.

## Phase 2 — Native strings and essential shared runtime

**Depends on:** Phase 1.

**Owners:** [widestring](../../crates/widestring/src/lib.rs), [common](../../crates/common/src/lib.rs), [util](../../crates/util/src/lib.rs), [thread initialization](../../src/threads/threads.rs), [locale](../../src/locale.rs), and any retained [tempfile](../../crates/tempfile/src/lib.rs) functionality.

### Delivery

- Retain UTF-32 fish text and the existing byte-stream codec; implement a separate Windows `OsStr`/UTF-16 conversion boundary instead of treating Windows names as Unix bytes.
- Separate portable escaping, formatting and guards from Unix-only imports/types. Establish real main-thread identity without `pthread_atfork` or inherited Unix signal masks.
- Keep Win32/CRT error domains distinct and capture native errors before subsequent calls overwrite them. Native errors must not be decoded as Unix errno.
- Give the selected runtime deterministic, noninteractive locale initialization without POSIX-only locale categories. Localization/manpage generation remains excluded.
- If tempfile remains in the selected dependency graph, replace `mkdtemp` and support exclusive creation/cleanup on Windows; otherwise remove that dependency from the native slice. No success-returning fallback for unavailable operations.

### Mandatory automated tests

| ID | Type | Behavior and required assertions |
|---|---|---|
| W2-TEXT | Unit | Round-trip empty, ASCII, BMP, non-BMP and private-use characters across fish/native OS-string conversion. Confirm supplementary characters are not counted as two fish characters and native private-use text is not reinterpreted as a Unix byte escape. |
| W2-INVALID | Unit | Supply unpaired UTF-16 input and embedded NUL at an OS boundary. Assert explicit rejection without replacement-character aliasing; separately retain supported arbitrary-byte round trips for redirected data. |
| W2-RUNTIME | Integration | Run the actual runtime initialization in an isolated native test process. Verify main-thread versus worker identity and propagation of a real missing-file/access error in its correct domain. If tempfile is retained, force a name collision, preserve the existing file and verify eventual temporary cleanup. |

**Exit gate:** retained foundation crates and W2 tests pass natively. Text, identity and error behavior have real implementations; passing pure string tests alone does not establish runtime initialization.

## Phase 3 — Synchronous standard-stream transport

**Depends on:** Phases 1–2.

**Owners:** [I/O streams](../../src/io.rs), [descriptor wrappers](../../src/fds.rs), [common I/O helpers](../../crates/common/src/lib.rs), [logging](../../src/flog.rs) and [panic handling](../../src/panic.rs).

### Delivery

- Supply native synchronous input/output endpoints for builtin streams, diagnostics and logging. Prefer `std::io`; use typed native handles only where required. Never cast a pointer-width `HANDLE` to a Unix `RawFd`.
- Preserve byte-exact redirected output, partial-write/error handling and flush ordering. Limit console text to the declared valid-Unicode contract; do not silently discard unavailable output or enable CRT text-mode rewriting.
- Preserve borrowed-versus-owned endpoint lifetime. Exclude readiness monitors, pipe capture and arbitrary child fd inheritance instead of pretending CRT descriptors implement them.
- Provide a noninteractive panic/error path that does not wait for input or enter reader/terminal initialization.

### Mandatory automated tests

| ID | Type | Behavior and required assertions |
|---|---|---|
| W3-BYTES | Integration | Exercise the real transport through native redirected files/pipes. Assert exact bytes containing NUL, LF, CRLF and non-UTF-8 data; prove there is no implicit newline/encoding conversion on these endpoints. |
| W3-ERRORS | Integration | Close a reader and write through the production endpoint; also exercise closed/missing standard handles in an isolated child. Assert the documented error/status, not silent success, a hang or an unhandled assertion. |
| W3-LIFETIME | Integration | Borrow, use and drop a standard-stream wrapper, then write through its original owner. Assert the original stream still works. Close all owned copies of a test pipe writer and assert the reader observes EOF within a bounded wait. |
| W3-CONSOLE | Integration | Use an isolated native console test process to write valid Unicode text through the production console endpoint and observe the console's text content. Assert the expected text; separately assert that unsupported non-UTF-8 console output returns an error rather than claiming byte-transparent console support. This is a console-I/O test, not an interactive editor/ConPTY test. |

**Exit gate:** W3 tests prove real Windows I/O and lifetime behavior. These tests may use native pipes as transport fixtures; they do not enable fish pipelines or redirection syntax.

## Phase 4 — Environment, explicit paths and cwd

**Depends on:** Phases 1–3.

**Owners:** [environment startup](../../src/env/environment.rs), [variable representation](../../src/env/var.rs), [scope/export implementation](../../src/env/impl/environment.rs), [dispatch](../../src/env_dispatch.rs), [paths](../../src/path.rs), [path utilities](../../src/wutil/mod.rs) and [parser directory state](../../src/parser.rs).

### Delivery

- Provide a minimal native initializer rather than calling the full existing `env_init(..., true)` and assuming it skips passwd, directories, termsize, locale/input dispatch and persistence setup.
- Preserve local/global scope behavior, lists and export/unexport precedence. Keep fish-local variable names case-sensitive. Normalize inherited `Path` spellings to one `PATH` variable during import, preferring an exact uppercase inherited `PATH` if a malformed environment contains several variants; erasing it must not resurrect a stale inherited spelling. At native export, canonical `PATH` wins over other case-equivalent spellings. Reject ambiguous case-equivalent exported names for other keys rather than exporting duplicates.
- Handle native PATH list rendering/import/export without splitting drive-letter colons. Do not globally reinterpret unrelated colon-delimited values such as `LANGUAGE` as Windows path lists.
- Maintain a real cwd using native paths and the shared environment model, without a retained Unix directory fd or `fchdir` imitation. Resolve supported absolute executable paths without `access(X_OK)` or an execute-bit test.
- Build child environment key/value data from fish's exported view, not stale process-global environment state. Do not invent numeric Unix EUID/GID/umask values. Do not create user-state directories or initialize persistent universal variables.

### Mandatory automated tests

| ID | Type | Behavior and required assertions |
|---|---|---|
| W4-SCOPES | Unit | Assert local/global shadowing, scope pop, export/unexport/erase, empty value versus empty list and child-export selection. Include a local unexported value shadowing an exported global value. |
| W4-ENV | Unit | Round-trip a PATH containing multiple drive-qualified directories, spaces and Unicode; apply the specified `Path`/`PATH` policy and reject other ambiguous exported-name collisions. Erase imported PATH and assert no stale alternate spelling is exported. Assert exact values and no duplicate case-equivalent native names. |
| W4-PATHS | Integration | In isolated native processes, initialize from a local space/Unicode cwd and resolve an absolute `.exe` path. Assert `$PWD`-model identity, existence/error distinctions and preservation of drive roots. Verify initialization leaves fresh config/data/history fixture directories unchanged and rejects unsupported path forms without launching anything. |

**Exit gate:** W4 tests pass against the actual environment/path services, including their native initialization. No persistent-state or terminal service is required to construct the supported execution context.

## Phase 5 — First runnable fish: batch evaluator and selected builtins

**Depends on:** Phases 1–4.

**Owners:** [executable startup](../../src/bin/fish.rs), [parser](../../src/parser.rs), [evaluator](../../src/parse_execution.rs), [expansion](../../src/expand.rs), [execution dispatch](../../src/exec.rs) and [selected builtins](../../src/builtins/mod.rs).

### Delivery

- Build and link the actual native `fish.exe` selected by `cargo build --target x86_64-pc-windows-msvc --bin fish --no-default-features`. Make the narrow Windows capability set explicit; do not present the Unix default feature set as supported.
- Enter batch initialization before unconditional signal/topic/terminal/reader work. Preserve required runtime invariants rather than replacing initialization with no-ops. `-N` is an invocation contract, not the mechanism that removes Unix compilation dependencies.
- Connect the existing parser/evaluator, variable services, synchronous streams and selected builtin operations. Builtin completion/status must be real and synchronous; no fake `InternalExit` event or discarded output.
- Reject unsupported builtins/options and AST/expansion forms before the affected job is expanded or executed. In particular, pipes, redirections, substitutions, `&`, `exec`, `fg`, `bg` and `set -U` must not fall through to an external shell or be silently ignored. Already completed earlier supported jobs need not be rolled back.
- Disable config sourcing, autoload, interactive startup, persistence and optional help/localization tools through real compilation/initialization boundaries. Calling `fish.exe` without `-N -c` must give a clear unsupported-mode diagnostic rather than hang or enter a partial editor.

### Mandatory automated tests

| ID | Type | Behavior and required assertions |
|---|---|---|
| W5-LANGUAGE | E2E | Run native `fish.exe -N -c` scripts covering quoted/empty/list values, local/global scope, `begin`, both conditional branches, `for`, boolean chaining, arithmetic and explicit exit. Assert exact stdout/stderr, ordering and status; include a syntax error and arithmetic error with useful nonzero diagnostics. |
| W5-NO-CONFIG | E2E | Put config scripts that create marker files into isolated HOME/config roots. Run supported batch evaluation with `-N` and assert correct output, no markers and no new persistent state. Run with no command and with an interactive flag and assert prompt-free unsupported-mode failure. |
| W5-REJECT | E2E | Exercise every excluded syntax family and the three excluded contracts' reachable commands. Use a supported side-effecting builtin in a command substitution inside an unsupported job and prove its variable mutation/output never occurs. Assert nonzero diagnostic status; neither discarded syntax nor a successful no-op satisfies the test. |

**Exit gate:** the native Cargo build links and all W5 tests execute the real `fish.exe`, without a compatibility runtime. This is the first green compile-and-run milestone, not completion of the whole goal.

## Phase 6 — Native child execution and completed proof of concept

**Depends on:** Phases 1–5.

**Owners:** [execution](../../src/exec.rs), [process/status records](../../src/proc.rs), [job groups](../../src/job_group.rs), [command resolution](../../src/path.rs), [evaluator dispatch](../../src/parse_execution.rs), [native CI](../../.github/workflows/test.yml) and the native test harness.

### Delivery

- Launch one foreground native executable at a time, initially by explicit absolute `.exe` path. Prefer `std::process::Command` with separate arguments, the fish-exported environment and explicit cwd. Inherit only the supported standard streams; no implicit command interpreter.
- Flush shell output before child launch, wait on real child completion, record completion once, update `$status` and one-element `$pipestatus`, and release resources. Replace the relevant Unix topic/wait consumer as well as the launch producer; no remaining dependency on `SIGCHLD`, `waitpid` or POSIX wait-status macros in this path.
- Use a distinct native exit representation retaining the full `u32` code. **Prototype status policy:** native codes `0..255` map unchanged to shell status; all other native exit codes map to shell status `1`, while the raw code remains available in the process record/diagnostic. Apply the same mapping to `$pipestatus` and implicit shell exit. Never report an arbitrary high-bit code as a Unix signal or let it trigger `ProcStatus`'s existing range assertion.
- Retain only synchronous services needed for this contract. Job Objects, IOCP, ConPTY and generalized process supervision are not prerequisites.
- Complete the native CI lane for debug and release execution. Run the resulting executable from an isolated space/Unicode directory outside the source tree with no POSIX tools/runtime on PATH. Keep existing Unix CI gates intact.

### Mandatory automated tests

| ID | Type | Behavior and required assertions |
|---|---|---|
| W6-CHILD | E2E | Have fish start the same native `fish.exe` by absolute path. The child must observe exported values but not unexported/erased values, the real cwd and exact empty/space/quote/backslash/Unicode arguments. Assert exactly the expected child output, exit `7`, parent `$status`, one-element `$pipestatus` and host-observed shell exit `7`. |
| W6-STREAMS | Integration/E2E | Launch a small real native probe through fish. It reads inherited stdin to EOF, writes distinguishable stdout/stderr records and exits. Assert exact input/output bytes, both streams, builtin-before-child output ordering and bounded EOF after all writers close. Probe code performs real I/O; it must not mock the fish backend. |
| W6-STATUS | Unit + E2E | Check the mapping with native codes `0`, `7`, `255`, `256`, `0xC0000005` and `0xFFFFFFFF`. A native probe must actually exit with the requested codes. Assert retained raw code, exact shell/pipe/host status under the declared policy, and no fabricated Unix signal state. |
| W6-FAILURE | E2E | Attempt a missing executable and a non-executable file with a `.exe` suffix, then run a supported builtin. Assert launch diagnostics/nonzero status, continued shell usability and the later builtin's real result. Repeated launches must still reach EOF and complete; no stale job status or leaked child-owned stream may keep the harness blocked. |
| W6-BOUNDARY | E2E | Repeat unsupported `exec`, `fg`, `bg`, pipeline, redirection, substitution and background cases using a native probe that creates a marker if run. Also attempt `.bat`, `.cmd` and `.ps1` paths containing marker-producing commands. Assert the markers never appear. Invalid/unsupported arguments or paths must not be lossy-converted or passed to a fallback shell. |
| W6-CLEAN-RUN | E2E | Run the language, child, stream and status cases against debug and release artifacts outside the checkout, with isolated user-state roots and no compatibility runtime/tools. Assert behavior and absence of config/history side effects, not just successful process startup. |

**Exit gate:** all mandatory W1–W6 cases pass on native Windows, all retained shared-code Unix gates pass, and the CI records the exact artifact/toolchain and executed case results. Native compilation alone, builtin-only execution or an MSYS test run does not satisfy completion.

## Definition of done and implementation records

- All six phases have passed their deliveries and mandatory tests; no required case is skipped or replaced with a mock/stub.
- Native debug and release `fish.exe` evaluate the supported fish subset and complete real sequential child execution under the declared argument/environment/cwd/stream/status contract.
- The three POSIX contracts remain explicitly unsupported. Deferred capabilities have clear nonzero diagnostics when reachable, without side effects from the rejected job.
- Preserve existing Unix contracts and update affected documentation/test commands to match the implemented capability set. Do not silently broaden the Windows support claim to all features, architectures or console modes.
- Record phase completion evidence here as work lands: test command, target/toolchain, executed IDs/count, result and CI/artifact reference. Until then, every phase remains not started.

The exploratory analyses in this directory describe possible later-port architecture; they do not expand this goal. Use [the component inventory](coverage.md), [workspace/build analysis](14-workspace-crates.md), [build tooling analysis](15-build-packaging-documentation.md) and [test infrastructure analysis](16-tests-benchmarks-ci.md) as source evidence. None is a substitute for executing the phase tests.
