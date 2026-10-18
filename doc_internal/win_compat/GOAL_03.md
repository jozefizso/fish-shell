# GOAL_03 — Full batch language, functions, scripts and portable builtins

**Phase:** N3. **Status: implemented and verified at source commit `861a9bdc6`.**
Native debug/release, minimum-Rust executables and retained Unix verification are recorded below.
The primary native target is `x86_64-pc-windows-msvc`; ARM64 requires separate native architecture evidence.

## Objective and completion boundary

Deliver the complete portable noninteractive fish language on Windows, using the existing grammar and evaluator.
Actual native fish must execute command strings, files and stdin; support functions, substitutions and mixed
pipelines; run portable builtins and PCRE2-32 regex; and provide real formatter, diagnostic and profiling behavior.
Users can write meaningful batch scripts without waiting for a graphical/terminal editor or persistent startup policy.
This enables [N4 startup/state](GOAL_04.md) and [N5 supervision](GOAL_05.md), not a second shell implementation.

## Entry conditions and dependencies

- Verify [N2](GOAL_02.md) is complete at the integration revision, including real streaming, deferred producers,
  bounded capture completion, input buffering, workers/wakeups and retained process handles.
- Consume [N1](GOAL_01.md) filesystem/discovery/predicate semantics, checked text boundaries and supported-system matrix.
  Linked plans do not assert that these prerequisites are already implemented in every checkout.
- Preserve [GOAL.md](GOAL.md) positive native behavior and permanent/still-deferred rejection coverage.
- Suggested branch: `platform/windows/batch-parity`; share one integration owner for evaluator/input/backend seams.
- File and stdin execution retain explicit `-N` until N4 implements ordinary configuration startup.
  Noninteractive source reading must not depend on the [N6 editor](GOAL_06.md).

## Owning components and invariants

Owners include native executable startup, parser/evaluator/expansion/function/event modules, builtin registry,
shared input, `src/re.rs`, PCRE2 integration, `fish_indent`, trace and parser profiling.
N3 owns the noninteractive reader seam used by `source` (`reader_read`/`read_ni`), extracted or reused without
requiring interactive initialization. Native owner paths are module references, not promises about checkout contents.
See [parser/evaluator analysis](01-parser-and-evaluator.md),
[expansion analysis](02-expansion-completion-highlighting.md), [builtin analysis](10-builtins.md),
[text/regex analysis](11-text-regex-localization.md), [functions/autoload analysis](09-history-functions-autoload.md),
[binaries/assets analysis](13-binaries-and-assets.md) and [Windows API reference](windows-api-reference.md).

- Restore portable algorithms and existing fish semantics, not a parallel grammar, regex engine or shell fallback.
- Keep Windows paths inside fish quoting/escaping rules; filenames do not authorize a second command language.
- Run parser/evaluator and queued handlers on the main thread; worker callbacks must not enter parser state.
- Use N2 endpoint/capture ownership, ordered redirection and deferred internal producer scheduling.
  Do not suspend consumers and then block a main-thread function or block producing into their pipe.
- PCRE2 must be the same 32-bit-code-unit engine with matching MSVC architecture/runtime dependencies.
  Preserve fish Unicode offsets, captures, errors and replacements; successful linking alone is not acceptance.

## Implementation delivery

1. **Restore full portable syntax and control flow.**
   - Support while, switch/case, negation, functions/return, break/continue, scopes, assignments and nested evaluation,
     alongside existing if/for/begin/boolean chains. Preserve recursion, status and parse/runtime error behavior.
   - Reuse existing expansion and parser cases, including lists, quoting, empty values and scope boundaries.
     Portable local/global assignment belongs here; universal persistence is N4, not a silent global fallback.
   - Restore scope/return/cancellation context after errors and nested eval/source/function execution.
2. **Enable bounded substitution and functions/blocks in pipelines.**
   - Use N2 capture and separated buffers for exact splitting, trailing-newline, empty/no-value and status semantics.
     Support nested substitution, NUL-delimited operations and defined output/read limits without unbounded buffering.
   - Preserve cancellation, broken downstream and write/read failures; unwind scope and I/O state on all exits.
   - Schedule functions and blocks on the main thread through existing deferred/internal producer behavior.
     Concurrently schedulable mixed stages must stream without deadlock; preserve existing bounded
     internal-to-internal buffering instead of demanding concurrent parser execution.
   - Finish capture at evaluation completion even when a live descendant holds stdout open: cancel/drain pending reads,
     retain already-available output at the defined boundary and release operation storage only after draining.
     Descendant EOF is not the completion condition; document this boundary for nested and interrupted capture.
3. **Implement scripts, stdin, source and eval through the noninteractive reader seam.**
   - Support script-file and stdin execution, source/`.` and eval, preserving argv, filename, line, scope and return context.
   - Specify UTF-8/BOM/CRLF handling and EOF behavior using the existing language/text contract; do not use CRT text
     conversion that changes byte-sensitive input or reinterpret Windows paths as a new grammar.
   - Restore the noninteractive `reader_read`/`read_ni` path independently of N6 terminal/editor initialization.
   - Provide useful source/syntax/runtime diagnostics with correct originating file/line and status.
     Reject invalid inputs before their affected side effects according to existing parser behavior.
   - Require `-N` for file/stdin entry until N4 startup lands; do not quietly run partial config or claim normal startup.
4. **Restore portable builtin coverage and regex.**
   - Inventory and implement complete portable families with their existing options/aliases: string (every subcommand),
     argparse, contains, count, random, loop helpers, builtin, `:`, `[`, portable status queries and existing core builtins.
   - Use N1 filesystem predicates and discovery; preserve invalid-option/argument/status behavior and stdin selection.
     Test regex groups, Unicode offsets, named captures, replacement and malformed patterns on actual PCRE2-32.
   - Implement noninteractive read with real input buffering, delimiter/NUL handling, limits and EOF semantics;
     successive read commands must not lose bytes to overread or seek on a pipe.
   - Keep active-terminal, process and platform-specific options assigned explicitly to their later phase in the ledger,
     rather than faking portable success. Ordinary portable options cannot be deferred as platform-inapplicable.
5. **Restore in-memory functions and events.**
   - Support definitions/invocation, copies, erase and tombstones with correct function metadata and scope restoration.
   - Implement generic/variable events, emit and block, including queued/blocked delivery and safe handler execution.
     Queue from callbacks and dispatch on the main thread; nested handlers must preserve context and event ordering.
   - [N4](GOAL_04.md) owns disk autoload/startup/save policy, N5 job events and N6 reader events; none replaces
     the in-memory language/event behavior required here.
6. **Close owned executable CLI and formatter behavior.**
   - Inventory actual native argument parsers and every formatter flag, preserving `--` and argv boundaries.
   - Implement fish `-c`/`--command`, repeated commands, script/stdin/argv modes, `-n`/`--no-execute`,
     `-f`/`--features` and `-v`/`--version`, including selection/ordering, option errors and stable results.
     Parse-only mode must diagnose syntax without command side effects, not silently execute requested code.
   - Implement `-d`/`--debug`, `-o`/`--debug-output`, `--print-debug-categories` and `-p`/`--profile` with real
     category selection, native sinks, write-error handling and no interference with command stdout/status.
   - For legacy `-D`/`--debug-stack-frames`, implement real behavior or document an explicit retired/unavailable policy;
     do not advertise the upstream ignored flag as a working stack-debug feature.
   - Restore native standalone/builtin fish_indent behavior and all non-help formatter options: stream/file input,
     formatting, overwrite/check modes, argument errors, diagnostics and file-write failure behavior.
   - Enable declarative completion/abbreviation metadata needed by startup without claiming active editor operations.
     Keep N4 init/login/no-config/private/startup-profile flags, N5 rusage, N6 interactive/key-reader flags and N9
     help delivery explicitly owned in the CLI ledger. `-N` retains its supported no-config meaning during this phase.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no faithful snapshot/return-twice, fork emulation or post-fork Windows backend.
  Run fish code in the existing evaluator and explicitly create children; do not return fake fork success.
- **POSIX `exec`/`execve`:** no same-PID replacement. Keep the exec decorator nonzero unsupported before
  argument side effects or launching a child. Spawn/wait/exit and CRT `_exec` do not fulfill this contract.
- **POSIX `fg`/`bg`:** no controlling-terminal pgroup transfer or POSIX stop/continue. Keep commands unsupported;
  no SuspendThread/ResumeThread or window-focus substitution and no same-named external fallback for reserved
  unsupported commands, including exec. Native `&`, jobs, wait and disown are N5 work, not these excluded contracts.

## Mandatory automated tests

Exactly these three required gate IDs must be discoverable and nonignored; expand them with implementation-owned subcases.

| ID | Type | Setup, actions and observable assertions |
|---|---|---|
| **N3-LANGUAGE** | Unit/e2e | Migrate existing portable language cases and run actual native fish for recursion, every control-flow form, scope/assignment, nested evaluation, function copies/erase/tombstones and return/error propagation. Exercise generic/variable emit/block handler ordering and safe context restoration. Exercise actual regex owners for Unicode offsets/groups/replacements/errors and every string subcommand plus remaining portable builtin options/aliases, invalid inputs and stdin behavior. Assert exact values, diagnostics, status and absence of forbidden effects, not just parse/compile success. |
| **N3-CAPTURE** | Integration/e2e | Execute nested substitution and functions/blocks in all mixed pipeline directions with real external helpers and data beyond pipe capacity. Assert empty versus no values, splitting/newlines/status, NUL-delimited operations, read/output limits, successive reads, broken downstream, cancellation and scope restoration. Synchronize a helper that exits after spawning a descendant holding stdout; substitution must return available output while that descendant is demonstrably alive and held until harness release. Do not wait for descendant EOF or use sleeps to infer it. Assert bounded finish, cancelled/drained I/O and owned cleanup. |
| **N3-SCRIPTS** | E2e | Run actual fish and fish_indent with Unicode/space filenames, UTF-8/BOM/CRLF fixtures, stdin EOF, source argv/return/filename-line diagnostics and eval nesting. Exercise repeated command execution, argv/`--`, syntax-check/features/version and invalid options; parse-only/syntax errors must prevent their affected marker effects. Run formatter stream/overwrite/check modes and trace/debug/profile/category output to real files, including denied/unwritable sinks and legacy debug-flag policy. Assert exact files/status/diagnostics, no command stdout/status interference, isolated source scope and no accidental startup without the required no-config mode. |

Unit tests exercise owning production code; integration uses actual files/pipes/processes; e2e uses native executables.
No source-text, mock-wiring, echo substitute or bare-not-throw acceptance is sufficient.
Keep a per-option/subcommand mapping under these IDs so portable coverage cannot disappear behind a small smoke subset.

## Verification and execution requirements

- Exercise actual native MSVC debug and release fish/formatter artifacts and PCRE2 behavior, plus applicable retained
  Unix unit/integration and portable language suites. Record a real script/substitution/formatter runtime smoke as well.
- Launch with native argument arrays, isolated state roots/cwd/environment and bounded waits; drain stdout/stderr
  concurrently and terminate/reap only owned work. Never use a POSIX host shell to manufacture native acceptance.
- Use barriers/events for live-descendant, cancellation and event-delivery races. The harness owns a bounded release
  and cleanup path even when an assertion fails; unavailable required fixtures block acceptance instead of skipping.
- When suites land, record exact runnable test names/commands, toolchain/SDK/dependency versions, target/revision,
  required discovery and executed/nonignored counts, results and debug/release artifact references.
  The implemented Cargo target is `windows_n3`; its underscore test names emit the required hyphenated IDs.
- CI fails on missing IDs, empty execution, ignored mandatory cases and unmapped required option/subcommand coverage.
- Preserve prototype positives and permanent/still-deferred rejections. Replace superseded syntax/substitution/script/
  formatter rejection tests with real positive/failure cases, updating CI registry/counts rather than merely flipping status.
  Keep GOAL.md historical evidence unchanged.

## Definition of done and handoff

All delivery items and three gates pass with actual native debug/release and retained applicable Unix evidence.
Update affected evaluator/builtin/input/CLI callers and docs; remove obsolete portable-feature guards and placeholders.
Every required option/subcommand has behavioral assertions; no portable capability is hidden as platform-inapplicable.
Permanent exclusions and still-deferred startup/interactive/platform rejection remain enforced without argument side effects.
Hand [N4](GOAL_04.md) a real source/noninteractive reader seam, functions/events, declarative metadata and CLI ledger;
hand [N5](GOAL_05.md) full evaluator behavior atop N2 runtime ownership. [N6](GOAL_06.md) adds the interactive reader,
[N8](GOAL_08.md) adds native script providers/platform services and [N9](GOAL_09.md) closes help/shipped assets.
Those later owners do not excuse missing N3 batch language, portable options, formatter or script execution behavior.

## Verified implementation and handoff

Branch: `platform/windows/batch-parity`. Source: `861a9bdc6`. The approved delivery scope above
is implemented, not a replacement interpreter or a reduced prototype. Detailed option mapping,
commands, counts and artifact identities are in the [N3 coverage record](coverage.md#n3-verified-evidence-record).

Run from an x64 MSVC developer environment with SDK `10.0.26100.0` selected:

```powershell
$env:PCRE2_SYS_STATIC = '1'
cargo build --locked --target x86_64-pc-windows-msvc --no-default-features --bin fish --bin fish_indent --bin windows_native_probe
cargo build --locked --target x86_64-pc-windows-msvc --no-default-features --release --bin fish --bin fish_indent
$env:FISH_NATIVE_TEST_BIN = (Resolve-Path target/x86_64-pc-windows-msvc/debug/fish.exe).Path
$env:FISH_NATIVE_TEST_RELEASE_BIN = (Resolve-Path target/x86_64-pc-windows-msvc/release/fish.exe).Path
cargo test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n3 -- --list
cargo test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n3 -- --nocapture
```

Discovery is exactly `N3_LANGUAGE`, `N3_CAPTURE`, `N3_SCRIPTS`. Each debug/release harness
executed all three against both executable profiles, including all eight mapped behavioral families.
The actual native CI gate body ran locally with its missing/ignored/empty-case checks enabled.
Minimum Rust `1.85.0` fish/formatter artifacts also passed N1–N3 against the stable release artifact.
Retained Unix verification passed 184 library tests and all eleven selected portable check files,
with zero skips. This is local execution evidence, not a claim of a remote CI run.

The real debug/release smoke executed a Unicode/space UTF-8 BOM/CRLF script with functions,
switch, a block pipeline, nested expansion, UTF-32 named captures, Unicode argv and a real profile
file; standalone formatting produced the expected nested indentation. Development artifacts used
Rust `1.95.0`, MSVC `14.51.36231`, SDK `10.0.26100.0` and `--no-default-features`.
Default-feature packaging, other Windows/volume architectures and interactive GUI certification are not inferred.

N4 receives the actual noninteractive reader/source seam, in-memory functions/events and declarative
completion/abbreviation metadata. Until N4 supplies durable universal state and utility-free bootstrap,
normal startup still requires `-N`; autoload is intentionally limited to the embedded `_validate_int`
core dependency, including user overrides and erase tombstones. General disk/embedded autoload,
`funcsave`, history and startup flags remain N4, not silent global/universal substitutes.

Capture completion remains evaluation-boundary based even with live descendants. Deferred internal
producers have their own bounded retention budget, before a downstream collector can remove delimiters.
A genuine deferred retention fault aborts substitution expansion; an ordinary native exit 122 does not.
Later captures start from a fresh fault-generation snapshot. Reserved private-use Unicode retains
fish's existing encoded-byte representation, including Unix's internal-count limitation, while literal
Unicode and arbitrary redirected bytes remain distinct. Batch ANSI follows configured styles without
claiming terminal capability. `-D`/`--debug-stack-frames` is explicitly retired/nonzero.

Fork, same-PID exec and POSIX fg/bg remain the permanent exclusions above. Native user jobs,
console editing, evaluated completion, additional script providers, help and catalogs retain N5–N9 ownership.
