# Complete component coverage inventory

## Scope and method

This inventory assigns every tracked project file to an owning component analysis. Runtime modules are enumerated individually; workspace crate implementation/build files are listed per crate. Shipped scripts, command completions, prompts, catalogs, fixtures and static assets are grouped by their maintained subsystem. A file count is a coverage check, not proof that every script has the same dependencies: the linked analyses distinguish pure definitions, dynamic helper execution and host-specific services.

This is the original pre-prototype 2,247-file source baseline, excluding generated `target/` artifacts, Git metadata and the analysis directory. No runtime backend was changed by that inventory audit. Subsequent native implementation/evidence is recorded in [GOAL.md](GOAL.md); [ROADMAP.md](ROADMAP.md) assigns the remaining work and requires N1 to refresh the inventory for new native modules. These historical counts are not a current native-support claim.

## Owning analyses

| Component analysis | Assigned tracked files |
|---|---:|
| [parser-and-evaluator](01-parser-and-evaluator.md) | 9 |
| [expansion-completion-highlighting](02-expansion-completion-highlighting.md) | 7 |
| [process-execution-job-control](03-process-execution-job-control.md) | 9 |
| [signals-events-waits](04-signals-events-waits.md) | 4 |
| [io-redirection-file-descriptors](05-io-redirection-file-descriptors.md) | 6 |
| [terminal-reader-screen-input](06-terminal-reader-screen-input.md) | 18 |
| [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) | 10 |
| [environment-and-universal-variables](08-environment-and-universal-variables.md) | 14 |
| [history-functions-autoload](09-history-functions-autoload.md) | 6 |
| [builtins](10-builtins.md) | 66 |
| [text-regex-localization](11-text-regex-localization.md) | 5 |
| [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) | 17 |
| [binaries-and-assets](13-binaries-and-assets.md) | 1428 |
| [workspace-crates](14-workspace-crates.md) | 68 |
| [build-packaging-documentation](15-build-packaging-documentation.md) | 253 |
| [tests-benchmarks-ci](16-tests-benchmarks-ci.md) | 327 |

## Runtime source files

Every Rust file under `src/` in the original audit baseline is included, including module declarations and test-only helpers. Algorithmic modules need no direct Win32 substitute; their linked analyses identify transitive dependencies. Consult ROADMAP.md for new native owning modules and implementation phases.

| Source file | Owning analysis |
|---|---|
| [`src/abbrs.rs`](../../src/abbrs.rs) | [history-functions-autoload](09-history-functions-autoload.md) |
| [`src/ast.rs`](../../src/ast.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/autoload.rs`](../../src/autoload.rs) | [expansion-completion-highlighting](02-expansion-completion-highlighting.md) |
| [`src/bin/fish.rs`](../../src/bin/fish.rs) | [binaries-and-assets](13-binaries-and-assets.md) |
| [`src/bin/fish_indent.rs`](../../src/bin/fish_indent.rs) | [binaries-and-assets](13-binaries-and-assets.md) |
| [`src/bin/fish_key_reader.rs`](../../src/bin/fish_key_reader.rs) | [binaries-and-assets](13-binaries-and-assets.md) |
| [`src/builtins/abbr.rs`](../../src/builtins/abbr.rs) | [builtins](10-builtins.md) |
| [`src/builtins/argparse.rs`](../../src/builtins/argparse.rs) | [builtins](10-builtins.md) |
| [`src/builtins/bg.rs`](../../src/builtins/bg.rs) | [builtins](10-builtins.md) |
| [`src/builtins/bind.rs`](../../src/builtins/bind.rs) | [builtins](10-builtins.md) |
| [`src/builtins/block.rs`](../../src/builtins/block.rs) | [builtins](10-builtins.md) |
| [`src/builtins/break.rs`](../../src/builtins/break.rs) | [builtins](10-builtins.md) |
| [`src/builtins/breakpoint.rs`](../../src/builtins/breakpoint.rs) | [builtins](10-builtins.md) |
| [`src/builtins/builtin.rs`](../../src/builtins/builtin.rs) | [builtins](10-builtins.md) |
| [`src/builtins/cd.rs`](../../src/builtins/cd.rs) | [builtins](10-builtins.md) |
| [`src/builtins/command.rs`](../../src/builtins/command.rs) | [builtins](10-builtins.md) |
| [`src/builtins/commandline.rs`](../../src/builtins/commandline.rs) | [builtins](10-builtins.md) |
| [`src/builtins/complete.rs`](../../src/builtins/complete.rs) | [builtins](10-builtins.md) |
| [`src/builtins/contains.rs`](../../src/builtins/contains.rs) | [builtins](10-builtins.md) |
| [`src/builtins/continue.rs`](../../src/builtins/continue.rs) | [builtins](10-builtins.md) |
| [`src/builtins/count.rs`](../../src/builtins/count.rs) | [builtins](10-builtins.md) |
| [`src/builtins/disown.rs`](../../src/builtins/disown.rs) | [builtins](10-builtins.md) |
| [`src/builtins/echo.rs`](../../src/builtins/echo.rs) | [builtins](10-builtins.md) |
| [`src/builtins/emit.rs`](../../src/builtins/emit.rs) | [builtins](10-builtins.md) |
| [`src/builtins/eval.rs`](../../src/builtins/eval.rs) | [builtins](10-builtins.md) |
| [`src/builtins/exit.rs`](../../src/builtins/exit.rs) | [builtins](10-builtins.md) |
| [`src/builtins/false.rs`](../../src/builtins/false.rs) | [builtins](10-builtins.md) |
| [`src/builtins/fg.rs`](../../src/builtins/fg.rs) | [builtins](10-builtins.md) |
| [`src/builtins/fish_indent.rs`](../../src/builtins/fish_indent.rs) | [builtins](10-builtins.md) |
| [`src/builtins/fish_key_reader.rs`](../../src/builtins/fish_key_reader.rs) | [builtins](10-builtins.md) |
| [`src/builtins/function.rs`](../../src/builtins/function.rs) | [builtins](10-builtins.md) |
| [`src/builtins/functions.rs`](../../src/builtins/functions.rs) | [builtins](10-builtins.md) |
| [`src/builtins/gettext.rs`](../../src/builtins/gettext.rs) | [builtins](10-builtins.md) |
| [`src/builtins/history.rs`](../../src/builtins/history.rs) | [builtins](10-builtins.md) |
| [`src/builtins/jobs.rs`](../../src/builtins/jobs.rs) | [builtins](10-builtins.md) |
| [`src/builtins/math.rs`](../../src/builtins/math.rs) | [builtins](10-builtins.md) |
| [`src/builtins/mod.rs`](../../src/builtins/mod.rs) | [builtins](10-builtins.md) |
| [`src/builtins/path.rs`](../../src/builtins/path.rs) | [builtins](10-builtins.md) |
| [`src/builtins/printf.rs`](../../src/builtins/printf.rs) | [builtins](10-builtins.md) |
| [`src/builtins/pwd.rs`](../../src/builtins/pwd.rs) | [builtins](10-builtins.md) |
| [`src/builtins/random.rs`](../../src/builtins/random.rs) | [builtins](10-builtins.md) |
| [`src/builtins/read.rs`](../../src/builtins/read.rs) | [builtins](10-builtins.md) |
| [`src/builtins/realpath.rs`](../../src/builtins/realpath.rs) | [builtins](10-builtins.md) |
| [`src/builtins/return.rs`](../../src/builtins/return.rs) | [builtins](10-builtins.md) |
| [`src/builtins/set.rs`](../../src/builtins/set.rs) | [builtins](10-builtins.md) |
| [`src/builtins/set_color.rs`](../../src/builtins/set_color.rs) | [builtins](10-builtins.md) |
| [`src/builtins/shared/error.rs`](../../src/builtins/shared/error.rs) | [builtins](10-builtins.md) |
| [`src/builtins/shared/misc.rs`](../../src/builtins/shared/misc.rs) | [builtins](10-builtins.md) |
| [`src/builtins/shared/mod.rs`](../../src/builtins/shared/mod.rs) | [builtins](10-builtins.md) |
| [`src/builtins/source.rs`](../../src/builtins/source.rs) | [builtins](10-builtins.md) |
| [`src/builtins/status.rs`](../../src/builtins/status.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string.rs`](../../src/builtins/string.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/collect.rs`](../../src/builtins/string/collect.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/escape.rs`](../../src/builtins/string/escape.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/join.rs`](../../src/builtins/string/join.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/length.rs`](../../src/builtins/string/length.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/match.rs`](../../src/builtins/string/match.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/pad.rs`](../../src/builtins/string/pad.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/repeat.rs`](../../src/builtins/string/repeat.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/replace.rs`](../../src/builtins/string/replace.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/shorten.rs`](../../src/builtins/string/shorten.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/split.rs`](../../src/builtins/string/split.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/sub.rs`](../../src/builtins/string/sub.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/test_helpers.rs`](../../src/builtins/string/test_helpers.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/transform.rs`](../../src/builtins/string/transform.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/trim.rs`](../../src/builtins/string/trim.rs) | [builtins](10-builtins.md) |
| [`src/builtins/string/unescape.rs`](../../src/builtins/string/unescape.rs) | [builtins](10-builtins.md) |
| [`src/builtins/test.rs`](../../src/builtins/test.rs) | [builtins](10-builtins.md) |
| [`src/builtins/true.rs`](../../src/builtins/true.rs) | [builtins](10-builtins.md) |
| [`src/builtins/type.rs`](../../src/builtins/type.rs) | [builtins](10-builtins.md) |
| [`src/builtins/ulimit.rs`](../../src/builtins/ulimit.rs) | [builtins](10-builtins.md) |
| [`src/builtins/wait.rs`](../../src/builtins/wait.rs) | [builtins](10-builtins.md) |
| [`src/common.rs`](../../src/common.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/complete.rs`](../../src/complete.rs) | [expansion-completion-highlighting](02-expansion-completion-highlighting.md) |
| [`src/editable_line.rs`](../../src/editable_line.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/env/config_paths.rs`](../../src/env/config_paths.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/env/environment.rs`](../../src/env/environment.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/env/impl/environment.rs`](../../src/env/impl/environment.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/env/impl/mod.rs`](../../src/env/impl/mod.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/env/impl/var.rs`](../../src/env/impl/var.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/env/mod.rs`](../../src/env/mod.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/env/var.rs`](../../src/env/var.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/env_dispatch.rs`](../../src/env_dispatch.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/env_universal_common.rs`](../../src/env_universal_common.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/event.rs`](../../src/event.rs) | [signals-events-waits](04-signals-events-waits.md) |
| [`src/exec.rs`](../../src/exec.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/expand.rs`](../../src/expand.rs) | [expansion-completion-highlighting](02-expansion-completion-highlighting.md) |
| [`src/fd_monitor.rs`](../../src/fd_monitor.rs) | [io-redirection-file-descriptors](05-io-redirection-file-descriptors.md) |
| [`src/fd_readable_set.rs`](../../src/fd_readable_set.rs) | [io-redirection-file-descriptors](05-io-redirection-file-descriptors.md) |
| [`src/fds.rs`](../../src/fds.rs) | [io-redirection-file-descriptors](05-io-redirection-file-descriptors.md) |
| [`src/flog.rs`](../../src/flog.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/fork_exec/flog_safe.rs`](../../src/fork_exec/flog_safe.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/fork_exec/mod.rs`](../../src/fork_exec/mod.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/fork_exec/postfork.rs`](../../src/fork_exec/postfork.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/fork_exec/spawn.rs`](../../src/fork_exec/spawn.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/fs.rs`](../../src/fs.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/function.rs`](../../src/function.rs) | [history-functions-autoload](09-history-functions-autoload.md) |
| [`src/global_safety.rs`](../../src/global_safety.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/highlight/file_tester.rs`](../../src/highlight/file_tester.rs) | [expansion-completion-highlighting](02-expansion-completion-highlighting.md) |
| [`src/highlight/highlight.rs`](../../src/highlight/highlight.rs) | [expansion-completion-highlighting](02-expansion-completion-highlighting.md) |
| [`src/highlight/mod.rs`](../../src/highlight/mod.rs) | [expansion-completion-highlighting](02-expansion-completion-highlighting.md) |
| [`src/history/file.rs`](../../src/history/file.rs) | [history-functions-autoload](09-history-functions-autoload.md) |
| [`src/history/history.rs`](../../src/history/history.rs) | [history-functions-autoload](09-history-functions-autoload.md) |
| [`src/history/mod.rs`](../../src/history/mod.rs) | [history-functions-autoload](09-history-functions-autoload.md) |
| [`src/history/yaml_backend.rs`](../../src/history/yaml_backend.rs) | [history-functions-autoload](09-history-functions-autoload.md) |
| [`src/input/binding.rs`](../../src/input/binding.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/input/decode.rs`](../../src/input/decode.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/input/input.rs`](../../src/input/input.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/input/mod.rs`](../../src/input/mod.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/io.rs`](../../src/io.rs) | [io-redirection-file-descriptors](05-io-redirection-file-descriptors.md) |
| [`src/job_group.rs`](../../src/job_group.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/key.rs`](../../src/key.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/kill.rs`](../../src/kill.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/lib.rs`](../../src/lib.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/locale.rs`](../../src/locale.rs) | [text-regex-localization](11-text-regex-localization.md) |
| [`src/localization/gettext.rs`](../../src/localization/gettext.rs) | [text-regex-localization](11-text-regex-localization.md) |
| [`src/localization/mod.rs`](../../src/localization/mod.rs) | [text-regex-localization](11-text-regex-localization.md) |
| [`src/localization/settings.rs`](../../src/localization/settings.rs) | [text-regex-localization](11-text-regex-localization.md) |
| [`src/nix.rs`](../../src/nix.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/null_terminated_array.rs`](../../src/null_terminated_array.rs) | [io-redirection-file-descriptors](05-io-redirection-file-descriptors.md) |
| [`src/operation_context.rs`](../../src/operation_context.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/pager.rs`](../../src/pager.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/panic.rs`](../../src/panic.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/parse_constants.rs`](../../src/parse_constants.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/parse_execution.rs`](../../src/parse_execution.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/parse_tree.rs`](../../src/parse_tree.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/parse_util.rs`](../../src/parse_util.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/parser.rs`](../../src/parser.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/parser_keywords.rs`](../../src/parser_keywords.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/path.rs`](../../src/path.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/portable_atomic.rs`](../../src/portable_atomic.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/prelude.rs`](../../src/prelude.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/print_help.rs`](../../src/print_help.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/proc.rs`](../../src/proc.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/re.rs`](../../src/re.rs) | [text-regex-localization](11-text-regex-localization.md) |
| [`src/reader/history_search.rs`](../../src/reader/history_search.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/reader/input.rs`](../../src/reader/input.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/reader/iothreads.rs`](../../src/reader/iothreads.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/reader/mod.rs`](../../src/reader/mod.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/reader/reader.rs`](../../src/reader/reader.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/reader/word_motion.rs`](../../src/reader/word_motion.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/redirection.rs`](../../src/redirection.rs) | [io-redirection-file-descriptors](05-io-redirection-file-descriptors.md) |
| [`src/screen.rs`](../../src/screen.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/signal.rs`](../../src/signal.rs) | [signals-events-waits](04-signals-events-waits.md) |
| [`src/stdx.rs`](../../src/stdx.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/terminal.rs`](../../src/terminal.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/termsize.rs`](../../src/termsize.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/tests/mod.rs`](../../src/tests/mod.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/tests/prelude.rs`](../../src/tests/prelude.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/text_face.rs`](../../src/text_face.rs) | [terminal-reader-screen-input](06-terminal-reader-screen-input.md) |
| [`src/threads/debounce.rs`](../../src/threads/debounce.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/threads/mod.rs`](../../src/threads/mod.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/threads/threads.rs`](../../src/threads/threads.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/timer.rs`](../../src/timer.rs) | [signals-events-waits](04-signals-events-waits.md) |
| [`src/tinyexpr.rs`](../../src/tinyexpr.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/tokenizer.rs`](../../src/tokenizer.rs) | [parser-and-evaluator](01-parser-and-evaluator.md) |
| [`src/topic_monitor.rs`](../../src/topic_monitor.rs) | [signals-events-waits](04-signals-events-waits.md) |
| [`src/trace.rs`](../../src/trace.rs) | [concurrency-and-runtime-support](12-concurrency-and-runtime-support.md) |
| [`src/tty_handoff.rs`](../../src/tty_handoff.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/universal_notifier/inotify.rs`](../../src/universal_notifier/inotify.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/universal_notifier/kqueue.rs`](../../src/universal_notifier/kqueue.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/universal_notifier/mod.rs`](../../src/universal_notifier/mod.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/universal_notifier/notifyd.rs`](../../src/universal_notifier/notifyd.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/universal_notifier/test_helpers.rs`](../../src/universal_notifier/test_helpers.rs) | [environment-and-universal-variables](08-environment-and-universal-variables.md) |
| [`src/wait_handle.rs`](../../src/wait_handle.rs) | [process-execution-job-control](03-process-execution-job-control.md) |
| [`src/wildcard.rs`](../../src/wildcard.rs) | [expansion-completion-highlighting](02-expansion-completion-highlighting.md) |
| [`src/wutil/dir_iter.rs`](../../src/wutil/dir_iter.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/wutil/errors.rs`](../../src/wutil/errors.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/wutil/fileid.rs`](../../src/wutil/fileid.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/wutil/hex_float.rs`](../../src/wutil/hex_float.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/wutil/mod.rs`](../../src/wutil/mod.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/wutil/printf.rs`](../../src/wutil/printf.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/wutil/wcstod.rs`](../../src/wutil/wcstod.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |
| [`src/wutil/wcstoi.rs`](../../src/wutil/wcstoi.rs) | [filesystem-paths-wildcards](07-filesystem-paths-wildcards.md) |

## Workspace crates

Each row includes all tracked files under the crate, including manifests, generators, build scripts and embedded unit tests. Native dependencies and Windows replacements are analyzed individually in [workspace crates](14-workspace-crates.md); formatting and localization internals also link to [text/localization](11-text-regex-localization.md).

| Crate | Implementation/build sources | Tracked files |
|---|---|---:|
| [`crates/build-helper`](../../crates/build-helper) | [`crates/build-helper/src/lib.rs`](../../crates/build-helper/src/lib.rs) | 2 |
| [`crates/build-man-pages`](../../crates/build-man-pages) | [`crates/build-man-pages/build.rs`](../../crates/build-man-pages/build.rs), [`crates/build-man-pages/src/lib.rs`](../../crates/build-man-pages/src/lib.rs) | 3 |
| [`crates/color`](../../crates/color) | [`crates/color/src/lib.rs`](../../crates/color/src/lib.rs) | 2 |
| [`crates/common`](../../crates/common) | [`crates/common/build.rs`](../../crates/common/build.rs), [`crates/common/src/lib.rs`](../../crates/common/src/lib.rs) | 3 |
| [`crates/fallback`](../../crates/fallback) | [`crates/fallback/build.rs`](../../crates/fallback/build.rs), [`crates/fallback/src/lib.rs`](../../crates/fallback/src/lib.rs) | 3 |
| [`crates/feature-flags`](../../crates/feature-flags) | [`crates/feature-flags/src/lib.rs`](../../crates/feature-flags/src/lib.rs) | 2 |
| [`crates/fluent`](../../crates/fluent) | [`crates/fluent/build.rs`](../../crates/fluent/build.rs), [`crates/fluent/src/lib.rs`](../../crates/fluent/src/lib.rs) | 3 |
| [`crates/fluent-extraction`](../../crates/fluent-extraction) | [`crates/fluent-extraction/build.rs`](../../crates/fluent-extraction/build.rs), [`crates/fluent-extraction/src/lib.rs`](../../crates/fluent-extraction/src/lib.rs) | 3 |
| [`crates/gettext`](../../crates/gettext) | [`crates/gettext/src/lib.rs`](../../crates/gettext/src/lib.rs) | 2 |
| [`crates/gettext-extraction`](../../crates/gettext-extraction) | [`crates/gettext-extraction/build.rs`](../../crates/gettext-extraction/build.rs), [`crates/gettext-extraction/src/lib.rs`](../../crates/gettext-extraction/src/lib.rs) | 3 |
| [`crates/gettext-maps`](../../crates/gettext-maps) | [`crates/gettext-maps/build.rs`](../../crates/gettext-maps/build.rs), [`crates/gettext-maps/src/lib.rs`](../../crates/gettext-maps/src/lib.rs) | 3 |
| [`crates/gettext-mo-file-parser`](../../crates/gettext-mo-file-parser) | [`crates/gettext-mo-file-parser/src/lib.rs`](../../crates/gettext-mo-file-parser/src/lib.rs) | 2 |
| [`crates/localization`](../../crates/localization) | [`crates/localization/src/lib.rs`](../../crates/localization/src/lib.rs) | 2 |
| [`crates/printf`](../../crates/printf) | [`crates/printf/src/arg.rs`](../../crates/printf/src/arg.rs), [`crates/printf/src/fmt_fp/decimal.rs`](../../crates/printf/src/fmt_fp/decimal.rs), [`crates/printf/src/fmt_fp/mod.rs`](../../crates/printf/src/fmt_fp/mod.rs), [`crates/printf/src/fmt_fp/tests.rs`](../../crates/printf/src/fmt_fp/tests.rs), [`crates/printf/src/lib.rs`](../../crates/printf/src/lib.rs), [`crates/printf/src/locale.rs`](../../crates/printf/src/locale.rs), [`crates/printf/src/printf_impl.rs`](../../crates/printf/src/printf_impl.rs), [`crates/printf/src/tests.rs`](../../crates/printf/src/tests.rs) | 12 |
| [`crates/tempfile`](../../crates/tempfile) | [`crates/tempfile/src/lib.rs`](../../crates/tempfile/src/lib.rs) | 2 |
| [`crates/util`](../../crates/util) | [`crates/util/src/lib.rs`](../../crates/util/src/lib.rs) | 2 |
| [`crates/wcstringutil`](../../crates/wcstringutil) | [`crates/wcstringutil/build.rs`](../../crates/wcstringutil/build.rs), [`crates/wcstringutil/src/lib.rs`](../../crates/wcstringutil/src/lib.rs) | 3 |
| [`crates/wgetopt`](../../crates/wgetopt) | [`crates/wgetopt/src/lib.rs`](../../crates/wgetopt/src/lib.rs) | 2 |
| [`crates/widecharwidth`](../../crates/widecharwidth) | [`crates/widecharwidth/src/lib.rs`](../../crates/widecharwidth/src/lib.rs), [`crates/widecharwidth/src/widechar_width.rs`](../../crates/widecharwidth/src/widechar_width.rs) | 4 |
| [`crates/widestring`](../../crates/widestring) | [`crates/widestring/src/lib.rs`](../../crates/widestring/src/lib.rs), [`crates/widestring/src/word_char.rs`](../../crates/widestring/src/word_char.rs) | 3 |
| [`crates/xtask`](../../crates/xtask) | [`crates/xtask/src/fluent.rs`](../../crates/xtask/src/fluent.rs), [`crates/xtask/src/format.rs`](../../crates/xtask/src/format.rs), [`crates/xtask/src/gettext.rs`](../../crates/xtask/src/gettext.rs), [`crates/xtask/src/lib.rs`](../../crates/xtask/src/lib.rs), [`crates/xtask/src/main.rs`](../../crates/xtask/src/main.rs), [`crates/xtask/src/shellcheck.rs`](../../crates/xtask/src/shellcheck.rs) | 7 |

## Assets, tools, infrastructure and metadata

All remaining tracked files are assigned below. Direct-child families are shown so Linux-only integration, native-Windows command definitions and portable resources do not disappear behind a root-directory label. See the owning analysis for Linux use, Windows APIs/tooling and semantic gaps.

| Source family | Tracked files | Owning analysis |
|---|---:|---|
| [`.builds/alpine.yml`](../../.builds/alpine.yml) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.builds/arch.yml`](../../.builds/arch.yml) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.builds/freebsd.yml`](../../.builds/freebsd.yml) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.builds/openbsd.yml`](../../.builds/openbsd.yml) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.cargo/config.toml`](../../.cargo/config.toml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`.clang-format`](../../.clang-format) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`.editorconfig`](../../.editorconfig) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`.gitattributes`](../../.gitattributes) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`.github/FUNDING.yml`](../../.github/FUNDING.yml) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.github/ISSUE_TEMPLATE`](../../.github/ISSUE_TEMPLATE) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.github/PULL_REQUEST_TEMPLATE.md`](../../.github/PULL_REQUEST_TEMPLATE.md) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.github/actions`](../../.github/actions) | 5 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.github/workflows`](../../.github/workflows) | 8 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`.gitignore`](../../.gitignore) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`.rustfmt.toml`](../../.rustfmt.toml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`BSDmakefile`](../../BSDmakefile) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`CHANGELOG.rst`](../../CHANGELOG.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`CMakeLists.txt`](../../CMakeLists.txt) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`CODE_OF_CONDUCT.md`](../../CODE_OF_CONDUCT.md) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`CONTRIBUTING.rst`](../../CONTRIBUTING.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`COPYING`](../../COPYING) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`Cargo.lock`](../../Cargo.lock) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`Cargo.toml`](../../Cargo.toml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`GNUmakefile`](../../GNUmakefile) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`README.rst`](../../README.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`SECURITY.md`](../../SECURITY.md) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`benchmarks/benchmarks`](../../benchmarks/benchmarks) | 14 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`benchmarks/driver.sh`](../../benchmarks/driver.sh) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`build.rs`](../../build.rs) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/check.sh`](../../build_tools/check.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/diff_profiles.fish`](../../build_tools/diff_profiles.fish) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/git_version_gen.sh`](../../build_tools/git_version_gen.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/ios_entitlements.xml`](../../build_tools/ios_entitlements.xml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/list_committers_since.fish`](../../build_tools/list_committers_since.fish) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/lsan_suppressions.txt`](../../build_tools/lsan_suppressions.txt) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/mac_notarize.sh`](../../build_tools/mac_notarize.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/make_linux_packages.sh`](../../build_tools/make_linux_packages.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/make_macos_pkg.sh`](../../build_tools/make_macos_pkg.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/make_pkg.sh`](../../build_tools/make_pkg.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/make_tarball.sh`](../../build_tools/make_tarball.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/make_vendor_tarball.sh`](../../build_tools/make_vendor_tarball.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/osx_distribution.xml`](../../build_tools/osx_distribution.xml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/osx_package_resources`](../../build_tools/osx_package_resources) | 2 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/osx_package_scripts`](../../build_tools/osx_package_scripts) | 3 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/release-notes.sh`](../../build_tools/release-notes.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/release.sh`](../../build_tools/release.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/supported_ubuntu_versions.py`](../../build_tools/supported_ubuntu_versions.py) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/update-dependencies.sh`](../../build_tools/update-dependencies.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`build_tools/version-available-in-debian.sh`](../../build_tools/version-available-in-debian.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`clippy.toml`](../../clippy.toml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`cmake/Benchmark.cmake`](../../cmake/Benchmark.cmake) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`cmake/Docs.cmake`](../../cmake/Docs.cmake) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`cmake/FindRust.cmake`](../../cmake/FindRust.cmake) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`cmake/Install.cmake`](../../cmake/Install.cmake) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`cmake/MacApp.cmake`](../../cmake/MacApp.cmake) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`cmake/PCRE2.cmake`](../../cmake/PCRE2.cmake) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`cmake/Rust.cmake`](../../cmake/Rust.cmake) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`cmake/Tests.cmake`](../../cmake/Tests.cmake) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`contrib/debian`](../../contrib/debian) | 11 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`contrib/shell.nix`](../../contrib/shell.nix) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`deny.toml`](../../deny.toml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_internal/fish-riir-plan.md`](../../doc_internal/fish-riir-plan.md) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_internal/mac-artifacts.md`](../../doc_internal/mac-artifacts.md) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_internal/rust-devel.md`](../../doc_internal/rust-devel.md) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/cmds`](../../doc_src/cmds) | 128 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/commands.rst`](../../doc_src/commands.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/completions.rst`](../../doc_src/completions.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/conf.py`](../../doc_src/conf.py) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/contributing.rst`](../../doc_src/contributing.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/design.rst`](../../doc_src/design.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/faq.rst`](../../doc_src/faq.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/fish_for_bash_users.rst`](../../doc_src/fish_for_bash_users.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/fish_indent_lexer.py`](../../doc_src/fish_indent_lexer.py) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/fish_synopsis.py`](../../doc_src/fish_synopsis.py) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/index.rst`](../../doc_src/index.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/interactive.rst`](../../doc_src/interactive.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/language.rst`](../../doc_src/language.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/license.rst`](../../doc_src/license.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/prompt.rst`](../../doc_src/prompt.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/python_docs_theme`](../../doc_src/python_docs_theme) | 8 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/relnotes.rst`](../../doc_src/relnotes.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/terminal-compatibility.rst`](../../doc_src/terminal-compatibility.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`doc_src/tutorial.rst`](../../doc_src/tutorial.rst) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`docker/alpine.Dockerfile`](../../docker/alpine.Dockerfile) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`docker/context`](../../docker/context) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`docker/docker_run_tests.sh`](../../docker/docker_run_tests.sh) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`docker/fedora.Dockerfile`](../../docker/fedora.Dockerfile) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`docker/lib.sh`](../../docker/lib.sh) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`docker/opensuse-tumbleweed.Dockerfile`](../../docker/opensuse-tumbleweed.Dockerfile) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`docker/push.sh`](../../docker/push.sh) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`docker/ubuntu-latest-lts.Dockerfile`](../../docker/ubuntu-latest-lts.Dockerfile) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`etc/config.fish`](../../etc/config.fish) | 1 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`fish.pc.in`](../../fish.pc.in) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`fish.png`](../../fish.png) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`fish.spec.in`](../../fish.spec.in) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`localization/fluent`](../../localization/fluent) | 10 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`localization/po`](../../localization/po) | 9 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`osx/CMakeMacAppInfo.plist.in`](../../osx/CMakeMacAppInfo.plist.in) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`osx/MacApp.entitlements`](../../osx/MacApp.entitlements) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`osx/fish_debug.entitlements`](../../osx/fish_debug.entitlements) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`osx/fish_term_icon.icns`](../../osx/fish_term_icon.icns) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`osx/install.sh`](../../osx/install.sh) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`osx/launch_fish.scpt`](../../osx/launch_fish.scpt) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`osx/osx_fish_launcher.m`](../../osx/osx_fish_launcher.m) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`pyproject.toml`](../../pyproject.toml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`share/__fish_build_paths.fish.in`](../../share/__fish_build_paths.fish.in) | 1 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`share/completions`](../../share/completions) | 1097 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`share/config.fish`](../../share/config.fish) | 1 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`share/functions`](../../share/functions) | 277 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`share/help_sections`](../../share/help_sections) | 1 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`share/prompts`](../../share/prompts) | 13 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`share/themes`](../../share/themes) | 25 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`share/tools`](../../share/tools) | 9 | [binaries-and-assets](13-binaries-and-assets.md) |
| [`tests/.gitignore`](../../tests/.gitignore) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/checks`](../../tests/checks) | 215 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/fish_test_helper.c`](../../tests/fish_test_helper.c) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/history_sample_bash`](../../tests/history_sample_bash) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/history_sample_corrupt1`](../../tests/history_sample_corrupt1) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/history_sample_fish_1_x`](../../tests/history_sample_fish_1_x) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/history_sample_fish_2_0`](../../tests/history_sample_fish_2_0) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/interactive.config`](../../tests/interactive.config) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/littlecheck.py`](../../tests/littlecheck.py) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/pexpect_helper.py`](../../tests/pexpect_helper.py) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/pexpects`](../../tests/pexpects) | 42 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/test_driver.py`](../../tests/test_driver.py) | 1 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`tests/test_functions`](../../tests/test_functions) | 10 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |
| [`updatecli.d/docker.yml`](../../updatecli.d/docker.yml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`updatecli.d/python.yml`](../../updatecli.d/python.yml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`updatecli.d/rust.yml`](../../updatecli.d/rust.yml) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`uv.lock`](../../uv.lock) | 1 | [build-packaging-documentation](15-build-packaging-documentation.md) |
| [`vagrants/bsds`](../../vagrants/bsds) | 7 | [tests-benchmarks-ci](16-tests-benchmarks-ci.md) |

## Coverage invariant

The inventory was checked against the repository file listing: 174 runtime Rust files, 21 workspace crates (68 tracked crate files), and 2,005 other tracked files, totaling 2,247. Every source path has exactly one owning analysis in this inventory; cross-cutting behavior can additionally be discussed elsewhere. Every local Markdown/source link is checked separately.

The component documents contain future port-verification scenarios. Those scenarios are not reported as executed native-Windows tests. Documentation validation checks coverage, readable Markdown and local link targets; it does not establish runtime compatibility.
