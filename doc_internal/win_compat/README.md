# Native Windows compatibility analysis

This directory maps every fish component to the Linux/POSIX facilities it uses and to a plausible native Windows implementation. The target is a native `x86_64-pc-windows-msvc` or `aarch64-pc-windows-msvc` build: WSL, Cygwin, and MSYS2 are compatibility environments and are not the target described here.

## Status baseline

The source does not provide a complete native Windows runtime backend. Foundational crates directly import Unix-only APIs: [`build-helper`](../../crates/build-helper/src/lib.rs) imports Unix `OsStrExt`, [`widestring`](../../crates/widestring/src/lib.rs) imports Unix `OsStr` byte conversions, and [`tempfile`](../../crates/tempfile/src/lib.rs) calls `nix::unistd::mkdtemp`. These are source-identified native-build blockers, not a fresh compiler result. The Windows CI configuration uses MSYS2, not a native MSVC runtime backend. Existing Windows-specific branches do not implement Win32 process, console, path, or handle semantics.

A native port should preserve fish language behavior while introducing explicit platform interfaces. It should not spread `cfg(windows)` through parser and UI code.

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

## Source coverage matrix

Every top-level runtime module is assigned an owning analysis. Child modules are included by directory.

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

The [coverage inventory](coverage.md) enumerates all 174 runtime Rust files, all 21 workspace crates and every tracked repository family. Its ownership counts cover 2,247 tracked project files; generated `target/` output and Git metadata are not project components. Fish functions, completion definitions, prompts, themes, catalogs and fixtures are analyzed as data/behavior families rather than pretending each file is an independent native backend.

## Recommended architecture

Introduce a small set of platform services before porting call sites:

- `platform::process`: spawn, replace-process emulation, process groups, waits, exit status, termination.
- `platform::io`: owned native handles, pipes, duplication, inheritance, readiness, blocking mode.
- `platform::terminal`: console/VT mode, input events, size, foreground-job policy.
- `platform::fs`: path encoding, directory iteration, identity, permissions, links, atomic replacement, locking, notifications.
- `platform::identity`: user identity, home/profile directories, host name.
- `platform::locale`: locale discovery and numeric formatting.

The parser, evaluator, completion ranking, screen model, history model, and most builtins should consume these interfaces without knowing whether the backend is POSIX or Windows.

## Audit verification

Completed documentation checks:

- Rendered all 19 Markdown documents, including 41 tables, with `markdown-it-py` using CommonMark plus tables.
- Checked local Markdown/source link targets and table column counts; no missing targets or malformed table rows remain.
- Matched all 174 current runtime Rust files to direct source links in their owning analyses.
- Matched all 21 current workspace crates and the complete 2,247-file tracked project inventory to analysis ownership, with no missing or extra source paths.
- Checked the critical Windows inheritance, console-control, file-replacement and directory-notification contracts against Microsoft documentation.

These checks establish documentation structure and component coverage, not native Windows compatibility. No fish build, shell test suite, third-party CLI, console/ConPTY experiment or Windows backend scenario was executed. Each component's port-verification scenarios remain future acceptance work.