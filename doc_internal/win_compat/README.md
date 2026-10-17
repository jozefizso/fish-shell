# Native Windows compatibility analysis

This directory maps fish components to their Linux/POSIX dependencies and native Windows implementations or candidate APIs. The implemented prototype and primary roadmap target `x86_64-pc-windows-msvc`; ARM64 is a separate verification track. WSL, Cygwin and MSYS2 are compatibility environments, not the native runtime backend described here.

## Current status and planning entry points

A native `x86_64-pc-windows-msvc` batch proof of concept is implemented. It uses the existing evaluator, selected builtins, real native streams/environment/cwd and sequential native child processes. It is not yet a full interactive Windows shell.

- [GOAL.md](GOAL.md) records the completed prototype contract and its native/Unix verification evidence.
- [ROADMAP.md](ROADMAP.md) defines the next eleven dependency-ordered phases, native semantic decisions and mandatory acceptance gates for full Windows functionality.

The component analyses and coverage counts below describe the original pre-prototype source audit. They explain portability boundaries and candidate APIs, not current implementation status. Use GOAL.md for the implemented baseline and ROADMAP.md for remaining work; proposed future tests are not passing results.

Extend the existing owning Windows backends while retaining shared fish language/editor logic. Do not create a second interpreter or promise faithful POSIX fork, same-PID exec or fg/bg job control.

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

The original audit assigns top-level runtime modules to owning analyses, including child modules by directory. New native modules and remaining implementation ownership are covered by GOAL.md and ROADMAP.md; N1 refreshes the exact inventory.

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

The [coverage inventory](coverage.md) records the original 174 runtime Rust files, 21 workspace crates and 2,247-file audit baseline. These are historical ownership counts, not current native-support counts. Fish functions, completion definitions, prompts, themes, catalogs and fixtures are analyzed as behavior families; ROADMAP.md requires complete per-asset and per-option closure before release.

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
- Matched all 174 current runtime Rust files to direct source links in their owning analyses.
- Matched all 21 current workspace crates and the complete 2,247-file tracked project inventory to analysis ownership, with no missing or extra source paths.
- Checked the critical Windows inheritance, console-control, file-replacement and directory-notification contracts against Microsoft documentation.

These historical checks establish documentation structure and original component coverage, not runtime compatibility. Subsequent native build/runtime evidence is recorded in GOAL.md. ROADMAP.md's future gates remain prospective until their implementations and native tests land.