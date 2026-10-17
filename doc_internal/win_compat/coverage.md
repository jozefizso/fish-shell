# Complete component coverage inventory

## Scope, state vocabulary and ownership

The **current source ledger** below records [N1 delivery 6](GOAL_01.md) on
`platform/windows/paths-discovery`, source revision `c3486d848`. It inventories native/runtime
modules, all 21 workspace crate directories, asset families, executable CLI options and builtin
subcommands/options. The [N1 evidence record](#n1-verified-evidence-record) distinguishes exercised
native behavior from future owners. Compiling a Unix-only module out does not make its command available.

States used throughout the current ledger:

- **S — supported source path:** enabled in the native registry/parser, subject to the stated
  prototype limitations. This does not assert newly executed debug/release evidence.
- **I — implemented N1 path:** enabled and exercised for the host/filesystem/feature matrix below.
  This does not enable the explicitly deferred consumers or certify other Windows hosts/volumes.
- **R — rejected:** the native entrypoint/option gate explicitly diagnoses unsupported use.
- **U — unavailable:** the service is compiled out, absent or not connected to a native entrypoint.
  A reserved builtin can be both R at dispatch and U at implementation.
- **X — permanently excluded:** faithful POSIX fork, same-PID exec and POSIX fg/bg; no replacement
  implementation or same-named external fallback is promised.

Every option in a row inherits its row's state/owner/required semantics unless separately split.
`ARG` denotes a required value and `[=ARG]` an optional value; a long spelling has no short alias
unless shown. Operands, `--`, invalid options and missing values remain part of each owner's contract.
N9 owns installed help/localization, including each actual `-h/--help` entry, not N1 or N11.
N11 reconciles evidence; it does not take implementation ownership from N1–N10.

## Current runtime and native module inventory

Paths in this table are relative to `src/`; directory rows explicitly enumerate their Rust children.
The historical file-by-file links below remain navigation, not current availability evidence.
Wiring evidence is [`lib.rs`](../../src/lib.rs), [`builtins/mod.rs`](../../src/builtins/mod.rs),
[`wutil/mod.rs`](../../src/wutil/mod.rs) and the native registry in
[`builtins/shared/misc.rs`](../../src/builtins/shared/misc.rs).

| Current source files | State and implementation owner | Required native semantics / boundary |
|---|---|---|
| `lib.rs`, `prelude.rs`, `common.rs`, `global_safety.rs`, `portable_atomic.rs`, `stdx.rs`, `panic.rs`, `operation_context.rs` | S shared/prototype subset; N1 identity boundaries, N2 runtime ownership, N3 batch consumers | Preserve shared data/algorithms, checked text boundaries, cancellation and main-thread ownership; inclusion is not complete downstream service support. |
| `ast.rs`, `tokenizer.rs`, `parse_constants.rs`, `parse_tree.rs`, `parser_keywords.rs`, `parse_util.rs`, `parser.rs`, `parse_execution.rs` | S restricted evaluator; N3 full language, N1 filesystem callers | Existing if/for/begin/boolean batch paths; functions/while/switch/substitution and other deferred forms remain rejected. N1 restores paths/globs without enabling unrelated syntax. |
| `expand.rs`, `wildcard.rs`, `complete.rs` | I N1 paths, discovery and real traversal; U active completion N7 (declarative metadata N3) | Shared fish matching, hidden/no-match rules, cancellation/result limits, directory case policy and identity-based cycles. Completion types compiling is not active editor completion. |
| `path/windows.rs` | I N1; providers beyond native `.exe` R/U N8 | Namespace-aware paths, CDPATH/implicit cd, authoritative cwd, explicit PATH order and classification shared with launch/predicates; no implicit Win32 cwd search or script fallback. No discovery cache to invalidate. |
| `path.rs` | U Unix backend on native; N1 equivalent path/discovery, N4 state roots, N8 providers | Not the selected Windows module. Known-folder/config/cache policy cannot be inferred from Unix XDG code. |
| `wutil/windows.rs`, `wutil/dir_iter_windows.rs`, `wutil/fileid.rs` | I N1 | Checked UTF-16 identity, delete-sharing opens, native error domains, physical resolution, native access probes, reparse metadata, full volume/file identity, per-directory case and handle lifetime. |
| `wutil/mod.rs`, `wutil/errors.rs`, `wutil/hex_float.rs`, `wutil/printf.rs`, `wutil/wcstod.rs`, `wutil/wcstoi.rs` | S shared numeric/output subset, I native filesystem dispatch N1; N2 full streams, N3 formatting | Keep numeric grammar/widths and byte provenance; route native filesystem operations to the Windows backend, not Unix fd/errno conventions. |
| `wutil/dir_iter.rs` | U Unix enumeration; N1 native counterpart above | `DIR*`, inode/mode and `fstatat` implementation is not native traversal support. |
| `fs.rs` | U native; N4 persistence (N1 path/identity prerequisites) | Stable locking, security, temporary replacement, flush/sharing and recovery protocol; Unix directory flock is not implemented by compiling this file out. |
| `env/mod.rs`, `env/environment.rs`, `env/var.rs`, `env/impl/mod.rs`, `env/impl/environment.rs`, `env/impl/var.rs`, `env/native.rs` | S native import/export/scoped subset; I N1 cwd integration; N3 full local/function semantics, N4 persistence | Case-sensitive fish locals, checked native case-collision/export rules and PATH lists. `NativeCwd` Arc and PWD are updated together and cloned into snapshots; physical cwd queries use the owned directory. |
| `env/config_paths.rs`, `env_universal_common.rs`, `env_dispatch.rs` | U Unix implementations; N4 | Native known-folder roots, locale/TZ/export effects, startup and durable universal state must be implemented, not silently replaced with globals. |
| `env_dispatch/windows.rs` | S limited native dispatch; N4 full variable effects | Existing read-limit dispatch is not history, locale, terminal, autoload or watcher support. |
| `exec_windows.rs`, `proc_status_windows.rs`, `proc.rs`, `job_group.rs`, `wait_handle.rs` | S sequential native foreground launch/status; I N1 resolver/cwd consumers; N2 streams, N5 supervision | Explicit application/argv/env/cwd, inherited standard endpoints and raw DWORD status (0–255 unchanged, other ordinary native exits map to 1). No POSIX pgroup/stopped-state claims. |
| `exec.rs`, `fork_exec/mod.rs`, `fork_exec/flog_safe.rs`, `fork_exec/postfork.rs`, `fork_exec/spawn.rs` | U Unix backend; fork/image replacement X | Native launch extends `exec_windows.rs`; no faithful fork or same-PID exec implementation. |
| `io_windows.rs`, `fds_windows.rs`, `null_terminated_array.rs`, `redirection.rs` | S inherited streams; I owned fd-0 input/try-input; R output/dup/close/pipeline syntax; N2 full graph | Input bindings retain real files and preserve argument versus explicitly redirected stdin precedence, including block input. N2 owns ordered multi-descriptor graphs, streaming, capture, inheritance allowlists and cancellation. Logical descriptor integers are not HANDLEs. |
| `io.rs`, `fds.rs`, `fd_monitor.rs`, `fd_readable_set.rs` | U Unix implementations; N2 native counterparts/services | No native readiness/monitor/pipe graph support inferred from the Unix code. |
| `threads/windows.rs` | S main-thread registration only; U workers/debounce N2 | Real worker, wakeup, cancellation/drain and callback-lifetime services still required. |
| `threads/mod.rs`, `threads/threads.rs`, `threads/debounce.rs` | U Unix-selected implementation; N2 | Not selected by native `lib.rs`; existing shared scheduling semantics must be retained. |
| `signal_windows.rs`, `event.rs` | S native status/event data subset; U event delivery N3/N5; native control/cancellation N5 | No fabricated Unix signal delivery; language/variable handlers N3, process/job/shell/control events N5. |
| `signal.rs`, `topic_monitor.rs`, `timer.rs` | U native services; N2 wakeups, N5 cancellation/events/accounting | Actual native completion/timing/counters; do not report unavailable measurements as zero. |
| `function.rs` | S shared metadata only; U in-memory definitions/events N3, autoload N4 | Native function execution is unavailable despite shared types compiling. |
| `autoload.rs`, `history/mod.rs`, `history/file.rs`, `history/history.rs`, `history/yaml_backend.rs`, `universal_notifier/mod.rs`, `universal_notifier/inotify.rs`, `universal_notifier/kqueue.rs`, `universal_notifier/notifyd.rs`, `universal_notifier/test_helpers.rs` | U native; N4 | Real disk discovery, state formats/locking/replacement and notifications; Unix watchers are not Windows backends. |
| `reader/mod.rs`, `reader/input.rs`, `reader/reader.rs`, `reader/iothreads.rs`, `reader/history_search.rs`, `reader/word_motion.rs` | U native; N3 noninteractive reader seam, N6 reader, N7 suggestions/search/completion | Full batch source/stdin must not wait for interactive initialization; editor has exclusive console input ownership and cancellation-safe workers. |
| `input/mod.rs`, `input/input.rs`, `input/decode.rs`, `input/binding.rs`, `key.rs`, `editable_line.rs`, `kill.rs` | U native; N6 core input/editing, N7 advanced modes | Preserve logical keys, Unicode characters and editing models, not UTF-16-unit cursor offsets or echo-only tests. |
| `terminal.rs`, `termsize.rs`, `screen.rs`, `tty_handoff.rs`, `text_face.rs`, `pager.rs` | U native; N6 transport/first reader, N7 full renderer/pager | Real console/VT modes, viewport, capability negotiation and foreground-child input donation; no POSIX fg/bg. |
| `abbrs.rs`, `highlight/mod.rs`, `highlight/highlight.rs`, `highlight/file_tester.rs` | U native; N3 declarative metadata, N7 active abbreviation/highlighting | Reuse N1 predicates; definition storage is not evaluated completion/editor parity. |
| `tinyexpr.rs`, `flog.rs`, `trace.rs`, `locale.rs`, `localization/mod.rs`, `localization/gettext.rs`, `localization/settings.rs` | S arithmetic/fixed-C-locale/diagnostic subset; N3 debug/profile/formatting, N9 locale/catalogs | Keep shared numeric parsing and real byte sinks; fixed prototype locale is not catalog discovery or working CLI debug/profile support. |
| `re.rs` | U native PCRE2 consumer; N3 | Same UTF-32 PCRE2 regex semantics, captures/replacements and error cases; Unix-only dependency registration does not provide native regex. |
| `nix.rs` | U Unix glue; N2/N5/N8 replace consumers at their owners | No blanket Win32 errno, fd, identity or signal emulation. |
| `print_help.rs` | U native help; N9 | Installed/embedded help and native viewer/error policy; existing script-based help is not available. |
| `builtins/mod.rs`, `builtins/shared/mod.rs`, `builtins/shared/error.rs`, `builtins/shared/misc.rs`; every `builtins/*.rs` and `builtins/string/*.rs` enumerated in the option ledger and historical source table | S restricted native registry, I N1 additions; R/U deferred families | Option-level states below override module inclusion. `return.rs` is compiled for exit-value parsing, not registered native `return`. |
| `bin/fish.rs`, `bin/fish/unix.rs`, `bin/fish/windows.rs`, `bin/fish_indent.rs`, `bin/fish_key_reader.rs` | S restricted fish batch CLI; R native formatter/key reader | Complete executable option ledger below; standalone tool files exist but their Windows entrypoints exit unsupported. |
| `tests/mod.rs`, `tests/prelude.rs` | U root Unix unit harness on native; each owning phase retains applicable cases | Native integration suites do not establish that all shared unit tests were discovered/executed. |

## Current workspace crate inventory

All paths are `crates/<name>`; the original source/build link table below supplies per-crate navigation.
There are 21 existing directories, not 21 native-verified crates. A workspace dependency declaration
for `localization-extraction` does not correspond to an existing crate directory and is not counted
as a delivered executable. State here concerns integration evidence, not whether a library can be
compiled in isolation.

| Crate(s), including current added sources | State / implementation owners | Required native behavior |
|---|---|---|
| `common`, `widestring` (`src/lib.rs`, `src/word_char.rs`), `util`, `wcstringutil` | S shared/prototype; I N1 identity codec; N3 full consumers | Checked OS-string/native UTF-16 versus fish text/byte provenance, Unicode/private-use handling and rejection without replacement aliases. |
| `tempfile` (including `tests/runtime.rs`) | S native create-new/temp helper source; execution evidence pending; N1 paths, N4 state/temp consumers, N8 psub, N10 distribution | Real exclusive native temp files, security/lifetime/cleanup at consumers; helper presence alone proves neither private ACLs nor passed cleanup tests. |
| `fallback`, `wgetopt`, `feature-flags` | S shared subset; N3 complete option/feature consumers, N6/N7 widths | Native boundaries without Unix-only fallback calls; complete flags and text widths at their actual consumers. |
| `printf` (all `src/fmt_fp/` and locale/tests) | S fixed-locale formatting subset; N3 full streams, N9 locale | Exact formats, errors, binary output and locale invalidation. |
| `color`, `widecharwidth` | Shared libraries present; U active native renderer; N6/N7 | Capability-aware styles and actual terminal-cell widths, not UTF-16 length. |
| `fluent`, `localization`, `gettext`, `gettext-maps`, `gettext-mo-file-parser` | S shared source/build paths; U complete native locale/catalog selection, evidence pending; N9, N10 deployment | Actual catalog lookup, fallback, precedence and packaged resources under default/minimal profiles. |
| `build-helper` (`src/lib.rs`, `src/version.rs`, `tests/host.rs`) | S native-host helper source; execution evidence pending; N9 generation, N10 build/release | Native version/build-path discovery without `/bin/sh`; execute real supported-host generation and failure propagation. |
| `build-man-pages`, `gettext-extraction`, `fluent-extraction`, `xtask` (`fluent`, `format`, `gettext`, `shellcheck` children) | U/unverified complete native tool execution, not runtime support; N9 generation, N10 supported host orchestration | Real generation/extraction/check entrypoints and outputs; declare tested external dependencies and host-only execution paths where applicable. |
| Root `fish` package, `build.rs`, `Cargo.toml`, `Cargo.lock`, `.cargo/` | Prototype native target present; N10 release/build matrix, N11 reconciliation | Rust minimum is declared 1.85; default features are `embed-manpages` and `localize-messages`. New N1 profile/toolchain/SDK results remain pending, not inherited from prototype builds. |

## Current assets and infrastructure families

The historical count table below is retained intact; no old count establishes current per-file
support. N9 must close the file-level shipped-asset ledger, including generated/embedded/extracted
forms. Earlier phases own assets required for their own working behavior now, not at N9/N11.

| Existing family | State / owner | Required native semantics |
|---|---|---|
| `etc/config.fish`, `share/config.fish`, `share/__fish_build_paths.fish.in` | U normal native startup; N4, layout N10 | Utility-free config ordering, native roots/overrides and relocatable asset discovery; `-N` intentionally bypasses configuration. |
| `share/functions/` | U as a complete native family; N3 portable definitions, N4 startup/autoload/funcsave, N6 first-prompt bindings, N7 editor helpers, N8 platform/psub/adapters, N9 remaining tools | Real fish evaluation and native services; dependency checks and truthful absence errors for optional external tools, never generic host-shell fallback. |
| `share/completions/` | U active native completion; N3 declarative metadata, N7 evaluated completion, N8 provider adapters, N9 breadth | Keep static definitions; audit dynamic commands/transitive tools and native argument/path/descriptor protocols. Windows-oriented filenames are not execution evidence. |
| `share/prompts/`, `share/themes/` | U active native prompt/rendering; N6 core prompt, N7 styles/modes, N9 remaining/VCS prompts | Utility-free first prompt; actual terminal capabilities and tested optional VCS dependencies. |
| `share/help_sections`, `doc_src/`, embedded/generated manpage resources | U installed native help; N9, package N10 | Native help/viewer/browser resolution, all executable/builtin help entries, no competing fish on PATH capturing internal dispatch. |
| `share/tools/` (Python/web/static assets), user editor/browser/clipboard/extraction helpers in functions | U native family, optional dependency behavior unverified; N6/N7 required clipboard actions, N9 user tools | Correct argv/Unicode/newlines/temp lifetime; native services or declared/tested dependencies present and absent. Python/Sphinx/gettext are not mandatory shell-startup dependencies. |
| `localization/fluent/`, `localization/po/`, generated catalogs | Native localization evidence pending; N9, N10 packaging | Actual language/environment precedence, catalog fallback and deployed data; fixed C numeric parsing remains separate. |
| `tests/windows_native.rs`, `tests/windows_native/{probe,fixture,transport,runtime,provenance,conpty}.rs` | S retained prototype regressions, rerun at N1 | The exact 15-case registry ran against both artifacts; independent runtime/provenance cases also ran. Probe is a real test helper, not another shipped CLI. |
| `tests/windows_n1.rs`, `tests/windows_n1/{native,paths,discovery,glob}.rs` | I N1; three required gates verified | Native debug/release harnesses each ran all three gates against both artifacts. Required fixture absence hard-fails rather than skips. |
| Other `tests/` checks/fixtures/interactive drivers, `benchmarks/` | Native migration/coverage unavailable as a complete family; owning phases N1–N10, N11 reconciliation/stress | Keep applicable Unix suites; translate host RUN/pty assumptions without deleting portable behavioral coverage. |
| `.github/`, `.builds/`, `docker/`, `vagrants/` | Native gate/release matrix pending; N10 CI/packaging, N11 closure; N1 owns its gate integration | Required IDs/cases must be discovered, nonempty and nonignored; label native, Unix/MSYS and cross-build evidence distinctly. |
| `build_tools/`, `cmake/`, root make/build/package metadata, `contrib/`, `osx/`, `updatecli.d/`, licenses/security/project metadata | Native release unavailable as a whole; N10, final quality N11 | Keep Unix/macOS-specific packaging separate from native x64 ZIP; supported build-host tools must actually execute. No unsigned artifact presented as signed. |

## Executable CLI option ledger

Source authority: [`bin/fish/windows.rs`](../../src/bin/fish/windows.rs) versus
[`bin/fish/unix.rs`](../../src/bin/fish/unix.rs), and
[`builtins/fish_indent.rs`](../../src/builtins/fish_indent.rs) /
[`builtins/fish_key_reader.rs`](../../src/builtins/fish_key_reader.rs). Unix option declarations
below are inventory, not Windows support. Before `-c`, native fish accepts only `-N/--no-config`
and `-c/--command`; after the first command string every remaining argument is `$argv`, even if
it resembles a flag. Combined Unix short flags and general option permutation are not implied.

| Executable / spelling or entry mode | State / owner | Required native semantics |
|---|---|---|
| fish `-c ARG`, `--command ARG` | S one command string; N3 full contract | Checked native source/argv, status, repeated commands in order; repeated `-c` is currently argv after the first, not multiple commands. |
| fish `-N`, `--no-config` | S required before command; N4 | Skip config without pretending normal startup; preserve init-command semantics/private history when N4 lands. |
| fish script file, stdin execution, option terminator `--` | R as entry modes; N3 | Native source reads, argv/filename/line/EOF and unambiguous `--` boundaries; not a successful empty shell. |
| fish `-n`, `--no-execute` | R; N3 | Parse/diagnose without startup/evaluation/argument side effects. |
| fish `-f ARG`, `--features ARG` | R; N3 | Existing feature-flag grammar, ordering and errors. |
| fish `-v`, `--version` | R; N3 | Stable real build/version output/status. |
| fish `-d ARG`, `--debug ARG` | R; N3 | Real category selection, not a recognized-but-ignored flag. |
| fish `-o ARG`, `--debug-output ARG` | R; N3 | Native diagnostic file sink and write errors without corrupting stdout/status. |
| fish `--print-debug-categories` | R; N3 | Actual available category enumeration. |
| fish `-D ARG`, `--debug-stack-frames ARG` | R; N3 | Explicit retirement/unavailability or genuinely implemented behavior; upstream ignored handling is not working stack debugging. |
| fish `-p ARG`, `--profile ARG` | R; N3 | Actual evaluation profiling and native sink errors. |
| fish `-C ARG`, `--init-command ARG` | R; N4 | Repeated init commands in command-line order after config and before execution; `-N` must not suppress them. |
| fish `-l`, `--login` | R; N4 | Native startup/login state and config order, not POSIX login-shell installation. |
| fish `-P`, `--private` | R; N4 | Private history/session persistence policy without dropping universal semantics. |
| fish `--profile-startup ARG` | R; N4 | Real startup-stage measurements and native file errors. |
| fish `--print-rusage-self` | R; N5 | Truthful native shell counters/units; unavailable POSIX fields explicit. |
| fish `-i`, `--interactive`; implicit interactivity | R/U; N6 | Actual native reader/terminal selection and mode restoration; not captured echo. |
| fish `-h`, `--help` | R; N9 | Installed native help and dependency/viewer errors. |
| fish_indent default stdin/file operands | R standalone and builtin; N3 | Reuse formatter with checked native paths, input/error/status behavior. Windows entrypoint currently exits 2 without parsing any flags. |
| fish_indent `--dump-parse-tree` (long-only; internal `P` tag is not a registered short `-P`) | R; N3 | Real parse-tree diagnostic output. |
| fish_indent `-i`, `--no-indent`; `--only-indent`; `--only-unindent` | R; N3 | Correct shared formatting transformations. |
| fish_indent `-w`, `--write` | R; N3 | Actual overwrite, denied/locked/write-error diagnostics and file lifetime. |
| fish_indent `-c`, `--check` | R; N3 | No unwanted rewrite; correct differences/status across operands. |
| fish_indent `--html`, `--ansi`, `--pygments` | R; N3 | Actual selected format/escaping/token output, not flag recognition. |
| fish_indent `-v`, `--version`; `--` | R; N3 | Real version and option/file boundary. |
| fish_indent `-h`, `--help` | R; N9 | Native formatter help matching installed tool. |
| fish_key_reader default console input | R standalone and builtin; N6 | Exclusive input ownership, key decoding, invalid/nonterminal errors and mode restoration. Windows entrypoint currently exits 2 before parsing. |
| fish_key_reader `-c`, `--continuous` | R; N6 | Actual continued key capture and exit/EOF/control policy. |
| fish_key_reader `-V`, `--verbose` | R; N6 | Real key/sequence diagnostic detail. |
| fish_key_reader `-v`, `--version`; `--` | R; N6 | Correct standalone tool dispatch/version/arguments. |
| fish_key_reader `-h`, `--help` | R; N9 | Native installed key-reader help. |

## Builtin option and subcommand ledger

Authority is the native/Unix registries and the individual parsers under
[`src/builtins/`](../../src/builtins). All currently native-registered spellings are
`[`, `cd`, `command`, `echo`, `exit`, `false`, `math`, `path`, `printf`, `pwd`,
`realpath`, `set`, `test`, `true`, `type`. Other registered Unix spellings are reserved and
rejected on native dispatch, not delegated to unrelated executables. Parser keywords have the
separate syntax disposition below. Compiled `return.rs` does not add a native `return` builtin.

### Help, flag conventions and argument-only commands

Every actual help option listed here is **R/U, N9** on native, even for an I/S command.
This includes `-h/--help` for `abbr`, `argparse`, `bg`, `bind`, `block`, `break`,
`breakpoint`, `builtin`, `cd`, `command`, `commandline`, `complete`, `contains`,
`continue`, `disown`, `emit`, `eval`, `exit`, `fg`, `fish_indent`, `fish_key_reader`,
`function`, `functions`, `history`, `jobs`, `math`, `pwd`, `random`, `read`,
`realpath`, `return`, `set`, `set_color`, `source`/`.`, `status`, `ulimit`, `wait`;
also `path -h/--help`, `path SUBCOMMAND -h/--help`, `string -h/--help` and
`string SUBCOMMAND -h/--help`. Native echo/printf additionally reject leading help-shaped
arguments through the prototype option guard; upstream echo/printf do not parse help options.
`count`, `_`, `test`/`[`, `true`/`:` and `false` have no separate help parser;
do not invent help support or reinterpret test expression strings as GNU options.

The shared required semantics for all portable rows are existing fish list/scoping/status/error
rules, native byte input/output without CRT newline rewriting, checked native identities when
needed and no implicit cmd/PowerShell execution. `--` is an option terminator only where the
actual parser recognizes it, not a new option for argument-only commands.

| Builtin / complete non-help option or operand surface | State / owner | Required native semantics |
|---|---|---|
| `echo`: `-n`, `-s`, `-e`, `-E`; text/escape operands | S prototype; N3 complete streams | No newline/no spaces, escape enable/disable and exact bytes; help guard above. |
| `exit`: optional status; shared return-value parser `--` | S prototype; N3 value/error semantics, N5 job shutdown, N6 console cleanup | Current batch exit request is not full interactive/job shutdown. |
| `true`, `false`: no parsed options | S no operands; R additional operands; N3 | Preserve success/failure; close prototype guard difference from Unix's ignored operands. `:` alias is R/U N3. |
| `printf`: FORMAT and operands; no getopt flags | S fixed-C-locale prototype; N3, locale N9 | Existing conversion/width/precision/escape grammar, binary/NUL output and numeric errors. A leading `--` is a format operand, not a claimed terminator. |
| `math`: expression operands | S argv expressions; N3 | Shared tinyexpr arithmetic/errors; full pipe/stream selection remains N2/N3. |
| `math`: `-s/--scale ARG`, `-b/--base ARG`, `-m/--scale-mode ARG` | R prototype option guard; N3 | Actual base/precision/rounding semantics; compiling parser branches does not enable flags. |
| `set`: `-l/--local`, `-g/--global`, `-x/--export`, `-u/--unexport`, `-q/--query`, `-e/--erase`; assignment/list/slice operands and `--` | S restricted prototype; N3 scope/list behavior, N1 native PATH/export/cwd boundaries | Native export collision/identity validation; complete Unix list/assignment behavior is not claimed from this subset. |
| `set`: `-f/--function`, `-n/--names`, `-L/--long`, `-S/--show`, `-a/--append`, `-p/--prepend`, `--path`, `--unpath`, `--no-event` | R; N3 (N1 native path/export boundary remains required) | Real scope, inspection, list/path-variable and event semantics, not accepted no-ops. |
| `set`: `-U/--universal` | R; N4 | Durable cross-process universal storage, never silent global assignment. |
| `argparse`: `-s/--stop-nonopt`, `-i/--ignore-unknown`, `-u/--move-unknown`, `-U/--unknown-arguments ARG`, `-n/--name ARG`, `-x/--exclusive ARG`, `-S/--strict-longopts`, `-N/--min-args ARG`, `-X/--max-args ARG`; option specifications/validation commands | R/U; N3 | Existing dynamic option grammar and evaluator validation, exact `_flag_*` scopes and argv boundaries. |
| `builtin`: `-n/--names`, `-q/--query`; execution decoration | R/U; N3 | Actual native registry/query/decorator behavior, not Unix-only names. |
| `contains`: `-i/--index`; needle/list | R/U; N3 | Exact membership/index/status and empty/Unicode values. |
| `count`: argv plus newline-terminated stdin records; no parsed options | R/U; N3 | Preserve newline count versus incomplete last record and zero status. |
| `random`: seed/range/step operands and `choice` mode; no non-help flags | R/U; N3 | Real OS entropy plus existing seeded/range/choice semantics. |
| `break`, `continue`, `return`: no non-help flags; return optional status | R/U; N3 | Loop/function scope and value/error restoration; not merely linked exit-value helpers. |
| `eval`, `emit`, `source`/`.`: no non-help flags; source filename/stdin `-` and argv, emit event/arguments, eval source operands | R/U; N3 | Same-shell evaluator/noninteractive reader, exact scope/filename/line/status; generic/variable events are not OS signals. |
| `block`: `-e/--erase`, `-l/--local`, `-g/--global` | R/U; N3 | Real language-event blocking/queued dispatch at scope boundaries. |
| `function`: `-d/--description ARG`, `-e/--on-event ARG`, `-v/--on-variable ARG`, `-a/--argument-names ARG`, `-S/--no-scope-shadowing`, `-V/--inherit-variable ARG`, `-w/--wraps ARG` | R/U; N3 | Actual in-memory definitions/scopes/events and declarative wrapping metadata; disk autoload N4, active completion N7. |
| `function`: `-s/--on-signal ARG`, `-j/--on-job-exit ARG`, `-p/--on-process-exit ARG` (including caller/self forms) | R/U; N5 | Real native lifecycle/control policy; no fictional Unix signal/pgroup handlers. |
| `functions`: `-e/--erase`, `-d/--description ARG`, `-n/--names`, `-a/--all`, `-q/--query`, `-c/--copy`, `-D/--details`, `--no-details`, `-v/--verbose`, `-H/--handlers`, `-t/--handlers-type ARG`, `--color ARG`; definition/name operands | R/U; N3 in-memory, N4 disk autoload, N5 process handlers, N7 highlighted display | Preserve function metadata/copy/tombstones/handler ownership; querying an autoloadable definition must use the real loader. |
| `complete`: `-x/--exclusive`, `-f/--no-files`, `-F/--force-files`, `-r/--require-parameter`, `-p/--path ARG`, `-c/--command ARG`, `-s/--short-option ARG`, `-l/--long-option ARG`, `-o/--old-option ARG`, `-d/--description ARG`, `-a/--arguments ARG`, `-e/--erase`, `-u/--unauthoritative`, `-A/--authoritative`, `-n/--condition ARG`, `-w/--wraps ARG`, `-k/--keep-order` | R/U; N3 declarative registration, N7 evaluated consumers | Preserve existing definitions, option aliases and registration semantics; do not equate metadata with working completion. |
| `complete`: `-C/--do-complete[=ARG]`, `--escape`, `--color ARG` | R/U; N7 | Real evaluated completion, native discovery, wrappers/cancellation/ranking and output escaping/color policy. |
| `abbr`: `-a/--add`, `-c/--command ARG`, `--position ARG`, `-r/--regex ARG`, `--set-cursor[=ARG]`, `--description[=ARG]`, `-f/--function ARG`, `--rename`, `-e/--erase`, `-q/--query`, `-s/--show`, `--list`, `-g/--global`, `-U/--universal`, `--color ARG` | R/U; N3 declarative metadata, N7 expansion/display; N4 legacy universal-state interaction | Preserve current accepted/deprecated option diagnostics (no invented short aliases for long-only spellings), regex/function expansion and cursor semantics; registration alone is not editor support. |
| `history`: `search`, `delete`, `clear`, `merge`, `save`, `clear-session`, `append`; option aliases `--search`, `--delete`, `--clear`, `--merge`, `--save` | R/U; N4 storage/session behavior, N7 active search UI | Actual private/session/cross-process history and durable merge/recovery. |
| `history`: `-p/--prefix`, `-c/--contains`, `-e/--exact`, `-t/--show-time[=ARG]`, `-n/--max ARG`, `-z/--null`, `-C/--case-sensitive`, `-R/--reverse`, `--color ARG` | R/U; N4 command behavior, N7 interactive consumer, N9 localized presentation | Real filtering/order/timestamps/byte output and storage errors. |
| `jobs`: `-c/--command`, `-g/--group`, `-l/--last`, `-p/--pid`, `-q/--quiet/--query`; targets | R/U; N5 | Real tracked jobs/PIDs/counters; group output must have declared native meaning, never fake POSIX pgroups. |
| `wait`: `-n/--any`; PID/name targets | R/U; N5 | Retained completion, cached exited targets, multiple targets, cancellation and stable identities. |
| `disown`: no non-help options; job targets | R/U; N5 | Native tracking/containment/survival policy, no stopped-job resume or fake reparenting. |
| `bg`, `fg`: no non-help options; job targets | X/R; permanent exclusions (rejection maintained by N5/N6) | No POSIX stop/continue or terminal-pgroup foreground transfer; never external fallback, thread suspension or window focus. |
| `exec` decorator: no replacement CLI flags | X/R; permanent exclusion | Reject before argument side effects or launch; spawn/wait/exit and CRT `_exec` are not same-PID exec. |
| `time` decorator: no builtin option parser | R; N5 | Actual elapsed/CPU/resource accounting for commands/pipelines/shell; no fake zero POSIX measurements. |
| `bind`: `-a/--all`, `-e/--erase`, `-f/--function-names`, `-k/--key`, `-K/--key-names`, `-L/--list-modes`, `-M/--mode ARG`, `--preset`, `-m/--sets-mode ARG`, `-s/--silent`, `--user`, `--color ARG`; key/action operands | R/U; N6 core binding dispatch, N7 full modes/actions | Same logical key identity as reader, correct preset/user tables and mode transitions; every advertised action must work. |
| `breakpoint`: no non-help flags | R/U; N6 | Real nested reader and outer scope/input/mode restoration. |
| `read`: `-a/--array/--list`, `-d/--delimiter ARG`, `-x/--export`, `-f/--function`, `-g/--global`, `-L/--line`, `-l/--local`, `-n/--nchars ARG`, `-z/--null`, `-t/--tokenize`, `--tokenize-raw`, `-u/--unexport`; variable operands | R/U; N3 noninteractive | N2 buffered input without overread or pipe seeks; delimiters/NUL/limits/EOF and assignments preserve fish semantics. |
| `read`: `-U/--universal` | R/U; N4 | Real durable universal assignment. |
| `read`: `-c/--command ARG`, `-p/--prompt ARG`, `-P/--prompt-str ARG`, `-R/--right-prompt ARG`, `-S/--shell`, `-s/--silent`; terminal use of ordinary flags | R/U; N6 | Actual interactive reader, no competing input owner and restoration after EOF/errors. |
| `commandline`: `-a/--append`, `-i/--insert`, `--insert-smart`, `-r/--replace`, `-b/--current-buffer`, `-j/--current-job`, `-p/--current-process`, `-s/--current-selection`, `-t/--current-token`, `-c/--cut-at-cursor`, `-f/--function`, `-x/--tokens-expanded`, `--tokens-raw`, `-o/--tokenize`, `-I/--input ARG`, `-C/--cursor`, `-B/--selection-start`, `-E/--selection-end`, `-L/--line`, `--column`, `--is-valid`, `--forward-jump ARG`, `--backward-jump ARG`, `--forward-jump-till ARG`, `--backward-jump-till ARG` | R/U; N6 | Real reader buffer/scopes/actions, character offsets and outside-reader errors. `--input` remains an inventoried legacy override, not a promised new public API. |
| `commandline`: `-S/--search-mode`, `-P/--paging-mode`, `--paging-full-mode`, `--search-field`, `--showing-suggestion` | R/U; N7 | Actual search/pager/suggestion state. `--paging-full-mode` is long-only in the current short-option string despite its internal `F` tag. |
| `set_color`: `-f/--foreground ARG`, `-b/--background ARG`, `--underline-color ARG`, `-o/--bold`, `-u/--underline[=ARG]`, `-i/--italics[=ARG]`, `-d/--dim`, `-r/--reverse[=ARG]`, `-s/--strikethrough[=ARG]`, `--theme ARG`, `--reset`, `-c/--print-colors`; color/reset operands | R/U; N6 basic styles, N7 full capabilities/themes | Existing shared text-face parser, real VT/console capability policy and redirected-byte behavior; no terminal-only probe on ordinary redirected output. |
| `fish_indent`, `fish_key_reader` builtin spellings | R/U; N3 formatter, N6 key reader; help N9 | All options individually inventoried in the executable ledger apply to the shared implementation too. |
| `_` (gettext): string operands, no parsed options | R/U; N9 | Real catalogs/fallback; no registered `gettext` alias. |

### N1 filesystem/discovery options

All ordinary namespace/metadata/native permission operations below are **N1**, not deferred to
N8 merely because they need Windows APIs. N8 owns Unix-only identity/mode/device categories and
additional script/platform providers. Help stays N9. Source: [`path.rs`](../../src/builtins/path.rs),
[`test.rs`](../../src/builtins/test.rs), [`cd.rs`](../../src/builtins/cd.rs),
[`pwd.rs`](../../src/builtins/pwd.rs), [`realpath.rs`](../../src/builtins/realpath.rs),
[`command.rs`](../../src/builtins/command.rs), [`type.rs`](../../src/builtins/type.rs).

| Builtin / subcommand and every non-help option | State / owner | Required native semantics |
|---|---|---|
| `cd`: no operand/HOME, CDPATH/implicit cd; `-L/--no-dereference`, `-P/--dereference` | I; N1 | Atomic logical PWD/owned physical cwd, failed-change rollback, renamed/replaced/deleted cwd, link-plus-`..`, drives/UNC; no process-global cwd mutation. |
| Shipped `cd.fish`, `prevd`, `nextd` directory history, including `cd -` | U; N4 autoload/startup, N9 helper integration | This is function-level history (`dirprev`/`dirnext`), not builtin `cd` or an OLDPWD implementation. N1 does not invent a second Windows-only history convention. |
| `pwd`: `-L/--logical`, `-P/--physical` | I; N1 | Logical spelling versus actual owned-directory name/identity; errors for unnameable/deleted physical cwd. |
| `realpath`: default physical; `-s/--no-symlinks`; `--` | I; N1 | Native lexical versus physical resolution, reparse/long/UNC handling and missing final component versus missing parent. No nonexistent `-L/-P` options invented. |
| `command`: `-a/--all`, `-q/--query/--quiet`, `-s/--search`, short-only `-v`; execution decoration | I; N1 | One ordered native provider resolver, no cwd injection, explicit-path bypass, denied/missing diagnostics and agreement with actual launch. `-v` aliases `-s`, not `--version`. |
| `type`: `-a/--all`, `-s/--short`, `-f/--no-functions`, `-t/--type`, `-p/--path`, `-P/--force-path`, `-q/--query/--quiet`, `--color ARG` | I native builtin/external lookup N1; U function/autoload/highlight portions N3/N4/N7 | Resolver and registry match actual execution. Color parses but native function-definition highlighting is compiled out; do not claim a working native highlighted function display. |
| Every `path` subcommand below: `-q/--quiet`, `-z/--null-in`, `-Z/--null-out`; source also accepts `--key ARG` through common parsing | I; N1 | Common newline/NUL/argv separation and statuses. Only `sort` consumes `--key`; accepted-but-unused `--key` elsewhere is not a working sort/key feature. |
| `path basename`: `-E/--no-extension` plus common flags | I; N1 | Root-aware basename and lexical extension removal, no file open required. |
| `path dirname`: common flags | I; N1 | Preserve drive/UNC/share/extended roots and separator rules. |
| `path extension`: common flags | I; N1 | Existing leading-dot/trailing-dot/empty-extension lexical semantics with native separators. |
| `path change-extension`: replacement operand and common flags | I; N1 | Lexical suffix change only, not native rename. |
| `path normalize`: common flags | I; N1 | Namespace-aware lexical normalization distinct from target existence/identity. |
| `path resolve`: common flags | I; N1 | Physical reparse resolution and declared missing-final policy; do not accept an absent intermediate directory as existing. |
| `path sort`: `-r/--reverse`, `-u/--unique`, `--key ARG` (`basename`, `dirname`, `path`) plus common flags | I; N1 | Existing stable natural/path sort and key/unique policy; sorting performs no filesystem metadata query. |
| `path mtime`: `-R/--relative` plus common flags | I; N1 | Real native write timestamp converted to shell epoch/units and relative age; missing/denied cases, not Windows FILETIME reported as Unix seconds. |
| `path filter`, `path is`: `-v/--invert`, `--all`, `-t/--type ARG`, short-only `-f/-d/-l`, `-p/--perm ARG`, short-only `-r/-w/-x`, plus common flags | I; N1 | Type values `file,dir,link`; permission values `read,write,exec`. Native access means requested-right open probes (directory execute means traversal, regular executable uses the shared `.exe` classification/open policy), not guessed Unix mode bits or a read-only attribute. `is` is quiet, `--all` checks all; rejection occurs before inversion. |
| `path filter/is --type` values `block,char,fifo,socket`; `--perm` values `suid,sgid,user,group` | R; N8 | Explicit unavailable native platform policy or real deliberately scoped native services; no fabricated UID/GID/modes or false success from omitted Unix branches. |
| `test`, `[`: `-d`, `-e`, `-f`, `-s`, `-h`, `-L`, `-r`, `-w`, `-x` | I; N1 | Real file/directory/size, recognized link/reparse metadata and requested-right native probes shared with discovery. `-h/-L` are predicates, not help/physical-mode flags. |
| `test`, `[`: `-ef`, `-nt`, `-ot` | I; N1 | Full volume/file identity (including hardlinks), comparable native timestamps and existing missing-file semantics; no string-only same-file comparison. |
| `test`, `[`: `-n`, `-z`, `=`, `!=`, `-eq`, `-ne`, `-gt`, `-ge`, `-lt`, `-le`, `!`, `-a`, `-o`, `(`, `)`; required closing `]` for `[` | I enabled with N1 registration; portable language owner N3 | Reuse expression parser/numeric precision/precedence and status 0/1/2; not option parsing through wgetopt. |
| `test`, `[`: `-t FD` | I native standard-stream console query; N6 full terminal context, N2 descriptor graph | Current native console endpoint/redirection truth, not POSIX isatty on a HANDLE integer. |
| `test`, `[`: `-b`, `-c`, `-p`, `-S`, `-G`, `-g`, `-k`, `-O`, `-u` | R; N8 | Explicit unsupported special-file/Unix ownership/mode diagnostics; no invented identity, device kind or set-id/sticky bits. |

### String subcommands and options

Every row is **R/U, N3**: `string` is reserved but its module and PCRE2 consumer are Unix-only.
The requirement is the existing shared fish implementation with native buffered input/output,
Unicode character/cell semantics and PCRE2-32, not Win32/.NET string/regex substitutions.
Each subcommand's leading `-h/--help` is separately R/U N9 as above. Authority:
[`string.rs`](../../src/builtins/string.rs) and each enumerated `builtins/string/` child.

| Subcommand(s) | Complete non-help flags | Required native semantics beyond common text/stream policy |
|---|---|---|
| `collect` | `-a/--allow-empty`, `-N/--no-trim-newlines` | Empty item versus no item, trailing-newline preservation and bounded capture. |
| `escape` | `-n/--no-quoted`, `--style ARG` | Fish script/url/var/regex escaping, not child-process quoting. |
| `unescape` | `-n/--no-quoted`, `--style ARG` | Existing style/invalid-sequence behavior; source `--no-quoted` is legacy ineffective, not a working native feature. |
| `join`, `join0` | `-q/--quiet`, `-n/--no-empty` | Delimiter versus NUL joining, empty fields and exact bytes. |
| `length` | `-q/--quiet`, `-V/--visible` | Characters versus display cells, never UTF-16 code-unit counts. |
| `lower`, `upper` | `-q/--quiet` | Shared Unicode case mappings, not user-locale-dependent Windows casing. |
| `match` | `-a/--all`, `-e/--entire`, `-g/--groups-only`, `-i/--ignore-case`, `-v/--invert`, `-q/--quiet`, `-r/--regex`, `-n/--index`, `-m/--max-matches ARG` | Wildcards/PCRE2 captures, named variable scopes, offsets, invalid patterns and limits. |
| `pad` | `-c/--char/--chars ARG`, `-r/--right`, `-C/--center`, `-w/--width ARG` | Real cell-width padding, invalid zero-width pad rejection. |
| `repeat` | `-n/--count ARG`, `-m/--max ARG`, `-q/--quiet`, `-N/--no-newline` | Exact repetition/limits and no-newline behavior. |
| `replace` | `-a/--all`, `-f/--filter`, `-i/--ignore-case`, `-q/--quiet`, `-r/--regex`, `-m/--max-matches ARG` | Literal/regex replacement, capture escapes and malformed replacement errors. |
| `shorten` | `-c/--char/--chars ARG`, `-m/--max ARG`, `-N/--no-newline`, `-l/--left`, `-q/--quiet` | Cell-width ellipsis/truncation, multiline and ANSI-width handling. |
| `split`, `split0` | `-q/--quiet`, `-r/--right`, `-m/--max ARG`, `-n/--no-empty`, `-f/--fields ARG`, `-a/--allow-empty` | Field/range selection, NUL empty fields and explicit substitution separation. |
| `sub` | `-l/--length ARG`, `-s/--start ARG`, `-e/--end ARG`, `-q/--quiet` | Character indexing, negatives and bounds, not UTF-16 slicing. |
| `trim` | `-c/--chars ARG`, `-l/--left`, `-r/--right`, `-q/--quiet` | Character-set/default whitespace trim, empty-result status. |

### Status subcommands/options and platform limits

All `status` rows are **R/U**: the builtin is not native-registered. The following is the complete
`StatusCmd` and option-parser surface from [`status.rs`](../../src/builtins/status.rs), not a
claim that compiling parser/proc metadata provides the command.

| Status operation / option aliases | Owner | Required native semantics |
|---|---|---|
| Default summary; `basename`/`current-basename`, `dirname`/`current-dirname`, `filename`/`current-filename`, `line-number`/`current-line-number`, `function`/`current-function`, `current-command`, `current-commandline`, `is-block`, `is-command-substitution`, `stack-trace`/`print-stack-trace` | N3 | Actual parser/source/stack context and nesting. Flags `-b/--is-block`, `-c/--is-command-substitution`, `-f/--filename/--current-filename`, `-n/--line/--line-number/--current-line-number`, `-t/--print-stack-trace`, `-L/--level ARG`. |
| `build-info`/`buildinfo`, `features`, `test-feature`, `fish-path`; `--fish-path` | N3 (N1 path boundary, N10 installed identity) | Real build/features and current executable identity, not a PATH-selected competing shell. |
| `is-login`; `-l/--is-login` | N4 | Actual native startup mode. |
| `is-interactive`, `is-interactive-read`, `is-breakpoint`; `-i/--is-interactive`, `--is-interactive-read` | N6 | Real reader/context state. |
| `is-full-job-control`, `is-interactive-job-control`, `is-no-job-control`, `job-control`; `--is-full-job-control`, `--is-interactive-job-control`, `--is-no-job-control`, `-j/--job-control ARG` (`none`, `interactive`, `full`) | N5/N6 truthful native policy; POSIX fg/bg X | Explicit native availability/difference; no successful claim of terminal-pgroup/stopped-job support. |
| `terminal`, `terminal-os`, `test-terminal-feature` | N6 capability baseline, N7 full negotiation | Actual terminal/protocol capability state and errors. |
| `get-file`, `list-files`, `language` | N9 | Installed/embedded file resources and locale/catalog precedence. |

Every `ulimit` operation below is **R/U, N8** (help N9). The complete source categories are:
`-b/--socket-buffers`, `-c/--core-size`, `-d/--data-size`, `-e/--nice`,
`-f/--file-size`, `-i/--pending-signals`, `-l/--lock-size`, `-m/--resident-set-size`,
`-n/--file-descriptor-count`, `-q/--queue-size`, `-r/--realtime-priority`,
`-s/--stack-size`, `-t/--cpu-time`, `-u/--process-count`, `-v/--virtual-memory-size`,
`-w/--swap-size`, `-y/--realtime-maxtime`, `-K/--kernel-queues`, `-P/--ptys`,
`-T/--threads`; selectors `-a/--all`, `-H/--hard`, `-S/--soft`; query and numeric,
`soft`, `hard`, `unlimited` setter operands. Each requires real enforceable native quota semantics
with scope/units/permissions/lifetime or explicit unavailable failure. Job Object limits do not
establish every POSIX soft/hard/per-user category. N5 accounting is not quota enforcement.

### Parser-owned registered spellings

`!`/`not`, `and`, `or`, `begin`/`end`, `for`, `if`/`else` are S restricted batch syntax;
their complete language/error/side-effect behavior remains N3. `while`, `switch`/`case` and
`function` are R deferred N3, with function event options owned above. `time` is R N5 and
`exec` X/R permanently. These are grammar/decorator forms, not independently executable generic
builtin functions. Shared parser helpers can compile while affected jobs remain rejected before
forbidden expansion/launch. POSIX fork has no executable CLI/builtin option to enable.

## Declared prerequisites and observed host matrix

This is the exercised development matrix, not a claim for every Windows release or filesystem.
The target is native `x86_64-pc-windows-msvc`; WSL supplies Unix regression evidence only.

| Field | Exercised value / support boundary | Evidence |
|---|---|---|
| Windows | `10.0.26300`, x64 | Native processes and all required N1 gates ran here. Other builds/ARM64 remain unverified. |
| Local filesystem | `C:` and `D:` NTFS | Required case-sensitive directories, hardlinks, symlinks, junctions, unknown reparse tags, denied ACLs and long/extended paths ran without skips. FAT/ReFS are not certified. |
| Security / link prerequisites | Non-elevated token; `DeveloperMode=1`; `LongPathsEnabled=1` | Real fixture creation, denial/restoration and assertions passed, not merely registry inspection. |
| UNC | Writable `\\localhost\D$\dev\fish-workspace\fish-n1-unc-fixture` | Metadata, identity, physical resolution and cwd gates passed. This is loopback SMB, not a remote-server matrix. |
| UNC glob capability limit | The tested SMB endpoint rejects per-directory case information with Win32 `50` | An actual `cd UNC; printf '<%s>\n' *.txt` exited `121`, emitted no stdout and retained error `50`. No guessed insensitive policy or silent no-match fallback. Local NTFS glob support does not certify SMB glob support. |
| Required UNC provisioning | `FISH_NATIVE_N1_UNC_ROOT`, otherwise a derived loopback administrative share | Required unavailable infrastructure hard-fails; it is not an ignored case. |
| Rust | `1.95.0` (`59807616e1fa2540724bfbac14d7976d7e4a3860`, LLVM `22.1.2`); Cargo `1.95.0` (`f2d3ce0bd`) | Native debug/release harnesses and executable artifacts passed. |
| Minimum verified Rust | `1.85.0` (`4d91de4e48198da2e33413efdcd9cd2cc0c46688`, LLVM `19.1.7`) | Current source built; all N1 gates then passed with this executable selected as the debug artifact. |
| Verified SDK / compiler | Windows SDK `10.0.26100.0`; MSVC tools `14.51.36231`, Visual Studio Enterprise `18.11.12306.390` | Explicit `VsDevCmd.bat -arch=x64 -host_arch=x64 -winsdk=10.0.26100.0` environment used for the minimum-Rust build. Native stable linker command independently identified these MSVC tools. Older SDKs are not certified. |
| Terminal boundary | Batch redirected streams and prototype private ConPTY console transport | W3 console/stream regressions passed against both artifacts. No interactive reader or Windows Terminal GUI/version matrix is claimed. |
| Release feature configuration | `--no-default-features`, native debug and release | Both exercised. Default embedded-manpage/localization releases are not certified by these minimal builds; N9/N10 own them. |

Identity/case-query failure is an explicit supported-volume error boundary, not permission to
disable cycle protection or lower-case names. See
[GetFileInformationByHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex)
and [FILE_CASE_SENSITIVE_INFORMATION](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_case_sensitive_information).

## N1 verified evidence record

Source revision: **`c3486d848`**, branch **`platform/windows/paths-discovery`**. Builds/gates below
used the source content committed there. They were built before the commit; their observed version
metadata is `4.9.3-148-gbc5c1f512-dirty`, not a claim that the embedded revision is `c3486d848`.
These are development artifacts, not N10 release packages.

| Evidence | Observed result |
|---|---|
| Native builds | `cargo +stable build --locked --target x86_64-pc-windows-msvc --no-default-features --bin fish --bin windows_native_probe`; same release build with `--release --bin fish`: exit 0. |
| Discovery | `cargo +stable test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n1 -- --list`: exactly `N1_PATHS`, `N1_DISCOVERY`, `N1_GLOB`; 3 tests, 0 benchmarks. |
| Required native gates | Same command with `-- --nocapture`, and again with `--release`: each 3 passed, 0 failed, 0 ignored, 0 filtered. Each harness produced exactly six PASS records: all three hyphenated IDs against debug and release executables. Twelve artifact-gate executions across the two harness profiles. |
| N1-PATHS | Real namespaces/UNC/long names, full IDs, permissions/timestamps including pre-epoch fractions, every owned filesystem option, logical/physical cd, live rename plus old-name replacement, relative metadata/globs after rename, deleted cwd and absolute/parent escape passed. NUL-input options use actual `< file` bindings and assert returned values/identity, not framing alone. |
| N1-DISCOVERY | Competing executable providers, PATH edits/replacement/cwd changes, explicit paths, ignored empty entries, case policy, disabled scripts, denied candidates and marker absence passed. |
| N1-GLOB | Shared matching and actual traversal, hidden/recursive/no-match semantics, finite ancestor revisits, per-directory case, cycles, cancellation/limits, mutation, resource release and invalid-name/reparse failures passed. |
| Native libraries | The workflow's five package suites passed 76 tests: fish 41, widestring 25, tempfile 4, common 5, util 1; none ignored. |
| Prototype regressions | `windows_native`: all 3 harness tests passed; required acceptance registry emitted all 30 debug/release PASS records. Runtime initialization and text provenance were also independently run in both harness profiles. |
| Build-helper baseline | W1 version acquisition: 3 passed; W1-HOST isolated tool-free version fallback: 1 passed. The latter tests the production version service, not compilation of a complete distribution archive. |
| Minimum toolchain | `cargo +1.85.0 rustc --locked --target x86_64-pc-windows-msvc --target-dir target/windows-msrv185 --no-default-features --bin fish -- -C debuginfo=1` in the explicit SDK environment: exit 0. All three N1 gates then passed with its executable plus the stable release artifact. |
| Applicable Unix regressions | WSL `ubuntu-dev`, Rust/Cargo `1.92.0`: `CARGO_TARGET_DIR=/tmp/fish-windows-goals-unix-target cargo test --locked --no-default-features --lib`: 181 passed, 0 failed, 0 ignored. This is Linux, not native-Windows evidence. |
| Actual Unix smoke | Ran the built fish: `cd doc_internal; path basename win_compat/GOAL_01.md; test -d win_compat; and echo directory-ok; printf '%s\n' win_compat/GOAL_0*.md`: exit 0, correct basename/directory result and all nine matching files. |
| Actual native smoke | Debug, release and minimum-Rust executables each performed central command lookup/launch, Unicode navigation, logical/physical pwd, `*.txt`, `[` predicate, drive-relative lexical normalization and NUL-delimited `path basename` through `< input.bin`: exit 0, empty stderr, correct names/status and exact NUL records. |
| CI execution boundary | Executed the actual native GOAL workflow gate body locally in PowerShell 7, changing only its evidence directory to `target/n1-native-results`: exit 0, including exact discovery/PASS-count checks. No remote GitHub run is claimed. Workflow also provisions required fixture prerequisites explicitly. |
| Local retained evidence | `target/n1-native-results/` contains native library, discovery, runtime/provenance, 30-case acceptance and both N1 harness logs. Historical GOAL.md evidence remains unchanged. |
| Volume-GUID namespace smoke | Queried the real D: volume GUID, resolved its `Cargo.toml` spelling to the same file, then navigated to that volume-GUID directory and expanded `*.toml` with native debug fish: exit 0, empty stderr, four actual TOML files. |

| Exercised artifact | SHA-256 |
|---|---|
| `target/x86_64-pc-windows-msvc/debug/fish.exe` | `45BE04296E8EC44C4261E4476CF9444A6172605E8DDF95CDD26DB415695B4F97` |
| `target/x86_64-pc-windows-msvc/release/fish.exe` | `D455DAB3E2F02EB4352BAAF42D5C722BC6AB1A5B7AC6BC0542A847DFCD155157` |
| `target/windows-msrv185/x86_64-pc-windows-msvc/debug/fish.exe` | `7C8EDD7F1D785FE8C5571A2B6CB5530C6906663520598FCB04B2C1FB52C458E5` |

To rerun the mandatory gates after building both profiles, set `FISH_NATIVE_TEST_BIN` and
`FISH_NATIVE_TEST_RELEASE_BIN` to their actual absolute paths and provision
`FISH_NATIVE_N1_UNC_ROOT` as a real writable share. Run the discovery command and both harness
commands above. Required unavailable infrastructure, missing IDs, empty execution or ignored gates
must fail. POSIX fork, same-PID exec and fg/bg remain rejected; ordinary still-deferred features
remain with the owners in the option ledger, not silently treated as successful operations.

## Historical pre-prototype audit (preserved)

The remaining sections preserve the original 2,247-file source baseline, excluding generated
`target/` artifacts, Git metadata and the analysis directory. No runtime backend was changed by
that audit. Its counts and checks are historical ownership evidence only. Prototype execution
evidence remains in [GOAL.md](GOAL.md); phase scopes remain [GOAL_01.md](GOAL_01.md) through
[GOAL_11.md](GOAL_11.md) and [ROADMAP.md](ROADMAP.md).

### Historical owning analyses

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

### Historical runtime source files

Every Rust file under `src/` in the original audit baseline is included, including module declarations and test-only helpers. Algorithmic modules need no direct Win32 substitute; their linked analyses identify transitive dependencies. Use the current runtime ledger above for newly added native modules, selected backends and source availability.

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

### Historical workspace crates

Each historical row includes all then-tracked files under the crate, including manifests, generators, build scripts and embedded unit tests. Current additions such as `build-helper/src/version.rs`, `build-helper/tests/host.rs` and `tempfile/tests/runtime.rs` are recorded above without changing old counts. Native dependencies and Windows replacements are analyzed in [workspace crates](14-workspace-crates.md).

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

### Historical assets, tools, infrastructure and metadata

All remaining files from the original tracked baseline were assigned below. Direct-child families show the original audit scope; current availability and phase ownership are in the current family ledger above.

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

### Historical coverage invariant

The original audit checked its repository listing: 174 runtime Rust files, 21 workspace crates (68 tracked crate files), and 2,005 other tracked files, totaling 2,247. Every source path had one owning analysis in that historical inventory; cross-cutting behavior could additionally be discussed elsewhere. Its local Markdown/source link checks were historical documentation checks, not a new N1 verification result.

The component documents contain future port-verification scenarios. Those scenarios are not reported as executed native-Windows tests. Documentation validation checks coverage, readable Markdown and local link targets; it does not establish runtime compatibility.
