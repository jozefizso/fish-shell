# Native Windows compatibility analysis

This directory maps fish components to their Linux/POSIX dependencies and native Windows implementations or candidate APIs. The implemented prototype and primary roadmap target `x86_64-pc-windows-msvc`; ARM64 is a separate verification track. WSL, Cygwin and MSYS2 are compatibility environments, not the native runtime backend described here.

## Current status and planning entry points

A native `x86_64-pc-windows-msvc` batch runtime is implemented. **N1 filesystem, cwd, discovery and glob delivery is verified for the recorded host/volume/feature matrix.** It retains the existing evaluator and real native streams/environment/children; it is not yet a full interactive Windows shell. See the explicit SMB glob capability limit below.

- [GOAL.md](GOAL.md) records the completed prototype contract and its historical native/Unix verification evidence; that record is not rewritten by the N1 integration.
- [ROADMAP.md](ROADMAP.md) defines the eleven dependency-ordered phases, semantic decisions and mandatory acceptance gates.
- The standalone contracts [N1](GOAL_01.md), [N2](GOAL_02.md), [N3](GOAL_03.md), [N4](GOAL_04.md), [N5](GOAL_05.md), [N6](GOAL_06.md), [N7](GOAL_07.md), [N8](GOAL_08.md), [N9](GOAL_09.md), [N10](GOAL_10.md) and [N11](GOAL_11.md) retain their approved scopes.
- The [current coverage/ownership ledger](coverage.md#scope-state-vocabulary-and-ownership) enumerates native/runtime modules, all existing workspace crates, asset families and **every executable CLI/builtin subcommand/option** with a source-based state, owning phase and required native semantics. Registry inclusion is not passing evidence; compiled-out Unix code is unavailable, not supported.
- The [N1 evidence record](coverage.md#n1-verified-evidence-record) is the stable place to add integration revision, exact commands/counts, native debug/release artifacts, applicable Unix results and actual smoke observations after the gates.

The component analyses and old file counts describe the original pre-prototype audit. Current-source overlays in [filesystem analysis](07-filesystem-paths-wildcards.md) and [builtin analysis](10-builtins.md) distinguish integrating implementations from historical candidate APIs. Historical counts never establish current support, and proposed future scenarios are not passing tests.

Extend the existing owning Windows backends while retaining shared fish language/editor logic. Do not create a second interpreter or promise faithful POSIX fork, same-PID exec or fg/bg job control.

## Current native boundary and host evidence

The current native registry contains `[`, `cd`, `command`, `path`, `pwd`, `realpath`, `test`,
`type` plus prototype `echo`, `exit`, `false`, `math`, `printf`, `set`, `true`. The N1 additions
are implemented with option-specific restrictions; this does not enable deferred Unix options,
function/autoload/highlight consumers or help. Deferred reserved commands remain rejected rather
than falling through to unrelated executables. The native executable currently requires
`fish -N -c <code> [argv...]`; normal config/script/stdin/interactive modes and standalone
formatter/key-reader behavior are not made available by their Unix parsers.

Ordinary namespace, metadata and native permission options belong to N1. Full portable batch
language/builtins and formatter flags belong to N3; startup/persistence to N4; jobs/accounting to
N5; interactive/key-reader flags to N6; advanced editor UX to N7; platform-only identity/device/
quota options and additional script providers to N8; help/assets/localization to N9; release to
N10. N11 reconciles completed behavior, not forgotten implementation.

The exercised host is **Windows `10.0.26300` x64**, local **C:/D: NTFS**,
`LongPathsEnabled=1`, `DeveloperMode=1`, a non-elevated token and a writable
`\\localhost\D$` share. Native debug/release N1 harnesses each passed all three required gates
against both artifacts, with no ignored cases. Rust **1.95.0** and minimum **1.85.0** executable
artifacts were exercised; the explicit SDK verification used **10.0.26100.0** and MSVC
**14.51.36231**. Only `--no-default-features` development builds are certified here.

UNC metadata/identity/cwd passed. **The tested SMB endpoint rejects directory case queries
with Win32 error 50, so UNC glob traversal explicitly fails rather than guessing case policy.**
Local NTFS traversal passed; this is not a certification for FAT/ReFS, remote SMB servers,
other Windows builds, ARM64 or an interactive terminal GUI.

Owned fd-0 `< file` / `<? file` input, including block input, supports real NUL-delimited
filesystem builtin input without changing argument precedence. Output/dup/close redirects,
pipelines, capture and full runtime ownership remain N2. `cd -` belongs to the shipped directory
history functions, not an added Windows-only builtin convention; loader/helper integration
remains N4/N9.

The [detailed evidence record](coverage.md#n1-verified-evidence-record) identifies source revision,
artifact hashes and pre-commit version metadata, exact discovery/execution counts, 76 native
library tests, preserved prototype regressions, 181 Unix library tests and real native/Unix
smokes. The actual native workflow gate body also passed locally; no remote CI run is claimed.
Required fixture absence still **hard-fails**, never skips.

## Portability classifications

- **Portable:** pure Rust or fish-language logic; expected to compile after its dependencies are abstracted.
- **POSIX-backed:** behavior is portable in concept, but the implementation directly uses Unix APIs.
- **Linux-specialized:** Linux has an optimized or richer implementation; another implementation or fallback already exists.
- **Semantic gap:** Windows has related APIs but no exact equivalent. The user-visible contract needs an explicit decision.


“Equivalent” below means a candidate implementation primitive, not guaranteed behavioral parity. Proposed policies and future port-verification scenarios are analysis, not implemented Windows support or executed runtime tests. Linux-specific mechanisms are distinguished from POSIX facilities also used on other supported Unix systems.
## Component analyses

1. [Parser and evaluator](01-parser-and-evaluator.md)
2. [Expansion, command discovery, completion, and highlighting](02-expansion-completion-highlighting.md)
3. [Process execution and job control](03-process-execution-job-control.md)
4. [Signals, events, waits, and resource limits](04-signals-events-waits.md)
5. [I/O, file descriptors, pipes, and redirection](05-io-redirection-file-descriptors.md)
6. [Terminal, reader, screen, and input](06-terminal-reader-screen-input.md)
7. [Filesystem, paths, directories, and wildcards](07-filesystem-paths-wildcards.md)
8. [Environment, configuration paths, and universal variables](08-environment-and-universal-variables.md)
9. [History, functions, abbreviations, and autoloading](09-history-functions-autoload.md)
10. [Builtins](10-builtins.md)
11. [Text, regex, numeric parsing, and localization](11-text-regex-localization.md)
12. [Concurrency, monitoring, diagnostics, and runtime support](12-concurrency-and-runtime-support.md)
13. [Binaries and shipped fish assets](13-binaries-and-assets.md)
14. [Workspace crates](14-workspace-crates.md)
15. [Build, installation, packaging, and documentation](15-build-packaging-documentation.md)
16. [Tests, benchmarks, and CI](16-tests-benchmarks-ci.md)
17. [Windows API reference](windows-api-reference.md)
18. [Complete component coverage inventory](coverage.md)

## Original source audit coverage matrix

The original audit assigns top-level runtime modules to owning analyses, including child modules by directory. The [current runtime ledger](coverage.md#current-runtime-and-native-module-inventory) now separately records every new native backend, selected/compiled-out service and remaining phase ownership; the table below is historical.

| Source component | Owning analysis |
|---|---|
| `src/ast.rs`, `tokenizer.rs`, `parser.rs`, `parser_keywords.rs`, `parse_constants.rs`, `parse_tree.rs`, `parse_util.rs`, `parse_execution.rs`, `tinyexpr.rs` | [Parser and evaluator](01-parser-and-evaluator.md) |
| `src/expand.rs`, `wildcard.rs`, `complete.rs`, `autoload.rs`, `highlight/` | [Expansion and completion](02-expansion-completion-highlighting.md) |
| `src/exec.rs`, `proc.rs`, `job_group.rs`, `tty_handoff.rs`, `fork_exec/`, `wait_handle.rs` | [Execution and job control](03-process-execution-job-control.md) |
| `src/signal.rs`, `event.rs`, `topic_monitor.rs`, `timer.rs` | [Signals and events](04-signals-events-waits.md) |
| `src/io.rs`, `fds.rs`, `fd_monitor.rs`, `fd_readable_set.rs`, `redirection.rs`, `null_terminated_array.rs` | [I/O and descriptors](05-io-redirection-file-descriptors.md) |
| `src/reader/`, `input/`, `editable_line.rs`, `key.rs`, `kill.rs`, `pager.rs`, `screen.rs`, `terminal.rs`, `termsize.rs`, `text_face.rs` | [Terminal and reader](06-terminal-reader-screen-input.md) |
| `src/fs.rs`, `path.rs`, `wutil/` | [Filesystem and paths](07-filesystem-paths-wildcards.md) |
| `src/env/`, `env_dispatch.rs`, `env_universal_common.rs`, `universal_notifier/` | [Environment and universal variables](08-environment-and-universal-variables.md) |
| `src/history/`, `function.rs`, `abbrs.rs` | [History and functions](09-history-functions-autoload.md) |
| `src/builtins/` | [Builtins](10-builtins.md) |
| `src/re.rs`, `locale.rs`, `localization/` | [Text and localization](11-text-regex-localization.md) |
| `src/threads/`, `common.rs`, `global_safety.rs`, `operation_context.rs`, `portable_atomic.rs`, `stdx.rs`, `trace.rs`, `flog.rs`, `panic.rs`, `nix.rs`, `prelude.rs`, `print_help.rs`, `lib.rs`, `tests/` | [Runtime support](12-concurrency-and-runtime-support.md) |
| `src/bin/`, `share/`, `etc/` | [Binaries and assets](13-binaries-and-assets.md) |
| `crates/` | [Workspace crates](14-workspace-crates.md) |
| `build.rs`, `Cargo.toml`, `.cargo/`, `cmake/`, `build_tools/`, `contrib/`, `osx/`, `updatecli.d/`, makefiles, packaging metadata, `doc_src/`, `doc_internal/`, `localization/`, and root project/security/license metadata | [Build and documentation](15-build-packaging-documentation.md) |
| `tests/`, `benchmarks/`, `.github/`, `.builds/`, `docker/`, `vagrants/` | [Tests and CI](16-tests-benchmarks-ci.md) |

Some directories appear in two rows because runtime test helpers or localization data belong both to runtime ownership and to their build/test pipeline. Each analysis states that boundary.

The [historical coverage inventory](coverage.md#historical-pre-prototype-audit-preserved) retains the original 174 runtime Rust files, 21 workspace crates and 2,247-file audit baseline. Current-source ownership is recorded separately above it; neither set of counts is a native-support proof. Functions, completions, prompts, themes, catalogs and fixtures remain behavior families until their owning phases supply real per-asset execution evidence.

## Recommended next architecture

Evolve the existing owning services instead of requiring a wholesale new platform framework:

- Process creation/completion: `src/exec_windows.rs`, `src/proc.rs`, `src/proc_status_windows.rs` and job/wait services; preserve the explicit exec/job-control exclusions.
- I/O: `src/io_windows.rs` and `src/fds_windows.rs`; typed handles, logical descriptor graphs, pipelines, inheritance and cancellation ownership.
- Terminal: the shared reader/input/screen models with native console/VT transport and explicit foreground input policy.
- Filesystem: `src/path/windows.rs`, `src/wutil/` and shared filesystem services; native identity, paths, permissions, locking and notifications.
- Identity/configuration: native environment and known-folder services without fabricated Unix credentials or stale process-global exports.
- Locale: shared numeric/text semantics and native locale/catalog discovery at the existing owning boundaries.

Keep portable algorithms shared and restore consumers phase by phase with real services. Extract common interfaces when demonstrated by callers; a new `platform::*` namespace is not itself a delivery.

## Original analysis audit verification

The original analysis-only audit completed these documentation checks:

- Rendered all 19 Markdown documents, including 41 tables, with `markdown-it-py` using CommonMark plus tables.
- Checked local Markdown/source link targets and table column counts; no missing targets or malformed table rows remain.
- Matched all 174 then-current runtime Rust files to direct source links in their owning analyses.
- Matched all 21 then-current workspace crates and the complete 2,247-file audit inventory to analysis ownership, with no missing or extra source paths at that baseline.
- Checked the critical Windows inheritance, console-control, file-replacement and directory-notification contracts against Microsoft documentation.

These historical checks establish original documentation structure and ownership. Current N1 native/runtime evidence is recorded in the coverage ledger; later phases remain unverified until their implementations and exact execution evidence land. GOAL.md remains the unchanged prototype history.