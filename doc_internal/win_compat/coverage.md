# Complete component coverage inventory

## Scope, state vocabulary and ownership

The **current source ledger** records N1 filesystem, N2 streaming/runtime, N3 batch-language
and N4 startup/persistent-state delivery on their `platform/windows/` phase branches. It inventories
native/runtime modules, all 21 workspace crate directories, assets, executable CLI options and
builtin options. Earlier evidence remains historical; the N4 evidence section records the
completed development matrix. Compiling Unix code out is not command support.

States used throughout the current ledger:

- **S — supported source path:** enabled in the native registry/parser, subject to stated boundaries.
- **I — implemented N1/N2/N3/N4 path:** enabled with the stated source and evidence boundaries.
  Passing development gates certify only the exercised hosts/fixtures, not all Windows volumes.
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
| `ast.rs`, `tokenizer.rs`, `parse_constants.rs`, `parse_tree.rs`, `parser_keywords.rs`, `parse_util.rs`, `parser.rs`, `parse_execution.rs` | I N3 batch language; N1 filesystem callers | Functions, recursion, while/switch, blocks, substitutions, scopes and generic/variable events execute through shared fish semantics; background/jobs N5, interactive N6, exec permanently excluded. |
| `expand.rs`, `wildcard.rs`, `complete.rs` | I N1 paths, discovery and real traversal; U active completion N7 (declarative metadata N3) | Shared fish matching, hidden/no-match rules, cancellation/result limits, directory case policy and identity-based cycles. Completion types compiling is not active editor completion. |
| `path/windows.rs` | I N1; providers beyond native `.exe` R/U N8 | Namespace-aware paths, CDPATH/implicit cd, authoritative cwd, explicit PATH order and classification shared with launch/predicates; no implicit Win32 cwd search or script fallback. No discovery cache to invalidate. |
| `path.rs` | U Unix backend on native; N1 equivalent path/discovery, N4 state roots, N8 providers | Not the selected Windows module. Known-folder/config/cache policy cannot be inferred from Unix XDG code. |
| `wutil/windows.rs`, `wutil/dir_iter_windows.rs`, `wutil/fileid.rs` | I N1 | Checked UTF-16 identity, delete-sharing opens, native error domains, physical resolution, native access probes, reparse metadata, full volume/file identity, per-directory case and handle lifetime. |
| `wutil/mod.rs`, `wutil/errors.rs`, `wutil/hex_float.rs`, `wutil/printf.rs`, `wutil/wcstod.rs`, `wutil/wcstoi.rs` | S shared numeric/output subset, I native filesystem dispatch N1; N2 full streams, N3 formatting | Keep numeric grammar/widths and byte provenance; route native filesystem operations to the Windows backend, not Unix fd/errno conventions. |
| `wutil/dir_iter.rs` | U Unix enumeration; N1 native counterpart above | `DIR*`, inode/mode and `fstatat` implementation is not native traversal support. |
| `fs.rs`, `fs/windows.rs` | I N4 native persistence; Unix backend remains separate | Stable parent `.fish_lock`/`LockFileEx`, identity-checked flushed sibling/create-new/`ReplaceFileW` transactions; target DACL is applied at staging creation before bytes. No false save acknowledgement, power-loss directory/SMB-server durability, owner/SACL/integrity copy or unsafe crash-orphan sweep. |
| `env/mod.rs`, `env/environment.rs`, `env/var.rs`, `env/impl/mod.rs`, `env/impl/environment.rs`, `env/impl/var.rs`, `env/native.rs`, `env/native_state.rs` | I N1 cwd, N3 scopes/import/export and N4 persistent state | Real universal values/flags and cross-process merge, inherited matching exported-global reconciliation, pending failed mutations and deferred callback replay. Effects/events run outside environment locks; private history does not disable universals. |
| `env/config_paths.rs`, `path/windows_config.rs`, `env_universal_common.rs` | I N4 native selection and state formats | Install assets are executable-relative and separate from user state. Defaults are LocalAppData/fish/{config,data,cache}; global-exported XDG roots append `fish`; explicit exported HOME falls back to `.config/fish`, `.local/share/fish`, `.cache/fish`, otherwise HOME uses FOLDERID_Profile. Existing universal formats/empty lists/raw bytes/flags persist with a 16 MiB read and serialized-snapshot limit. |
| `env_dispatch.rs`, `env_dispatch/common.rs`, `env_dispatch/windows.rs` | I N4 native variable effects; N6/N7 editor consumers deferred | Shared read-limit/trace/width hooks, actual function/completion path invalidation, history/private selection and locale/TZ effects; explicit reader-settings generation is an interface, not an editor. POSIX-spawn control diagnoses no native effect. |
| `exec_windows.rs`, `exec/windows_launch.rs`, `proc_status_windows.rs`, `proc.rs`, `job_group.rs`, `wait_handle.rs` | I N1 resolver/cwd and N2 prepared native pipeline launch/retained ownership; N5 user supervision | Explicit pinned application/argv/env/cwd, suspended creation/owned rollback, exact inheritance allowlists and retained raw DWORD completion. No POSIX pgroup/stopped-state claims. |
| `exec.rs`, `fork_exec/mod.rs`, `fork_exec/flog_safe.rs`, `fork_exec/postfork.rs`, `fork_exec/spawn.rs` | U Unix backend; fork/image replacement X | Native launch extends `exec_windows.rs`; no faithful fork or same-PID exec implementation. |
| `io_windows.rs`, `fds_windows.rs`, `io/windows_descriptors.rs`, `io/windows_pipe.rs`, `io/windows_deferred.rs`, `native_fd_bridge.rs`, `null_terminated_array.rs`, `redirection.rs` | I N2 typed graph, ordered redirects, real pipelines/capture/input ownership | Logical integers are not HANDLEs. Shared input cursor survives aliases; explicit closed builtin output discards, inherited unavailable output fails when used. Extra-fd bridge is restricted to the actual same executable identity; N3 owns function/block/substitution consumers. |
| `io_buffer.rs` | I N2 shared separated-buffer storage | Preserve explicit empties and byte provenance; checked size accounting and finite native metadata retention. Unix I/O collection remains Unix-owned. |
| `io.rs`, `fds.rs`, `fd_monitor.rs`, `fd_readable_set.rs` | U Unix implementations on native; I N2 native counterparts above | Native services do not reinterpret Unix fds, sockets or arbitrary handles as a universal readiness namespace. |
| `threads/windows.rs`, `threads/windows_workers.rs` | I N2 scalable waits/completion queue, workers/debounce | Main-thread dispatch, monotonic source identity, reset/post synchronization, cancellation/drain, retained child handles and abandoned-result suppression. |
| `threads/mod.rs`, `threads/threads.rs`, `threads/debounce.rs` | U Unix-selected implementation on native; I N2 native counterpart | Preserve shared scheduling semantics without Unix masks/atfork. |
| `signal_windows.rs`, `event.rs` | I N3 generic/variable events and blocking; N5 native control/lifecycle events | In-memory language handlers and queued dispatch work; no fabricated Unix signal delivery. Process/job/shell/control events remain N5. |
| `signal.rs`, `topic_monitor.rs`, `timer.rs` | U native services; N2 wakeups, N5 cancellation/events/accounting | Actual native completion/timing/counters; do not report unavailable measurements as zero. |
| `function.rs`, `autoload.rs` | I N3 functions and N4 full core disk/embedded autoload | Ordered paths, embedded fallback, generated-completion precedence, identity/cwd/path invalidation, override/tombstone/recursion policy and native source provenance. Loading arbitrary shipped assets does not port every helper's dependencies (N9). |
| `history/mod.rs`, `history/file.rs`, `history/history.rs`, `history/yaml_backend.rs` | I N4 native history | Existing YAML-compatible records and real timestamps; immutable owned baseline survives automatic saves/watcher invalidation. Checked explicit merge captures its transaction snapshot. A committed-generation journal lets clear-session undo only acknowledged own versions still present and restore their actual predecessors, preserving foreign updates/deletions through failed/compacted duplicates. Private selection precedes disk loading; shutdown flushes dirty history. Fish 1.x remains unsupported; same-command/same-second legacy records cannot identify authors uniquely. |
| `universal_notifier/mod.rs`, `universal_notifier/windows.rs`, `universal_notifier/windows_io.rs` | I N4 directory notification over N2 runtime | Parent/ancestor ReadDirectoryChangesW, full invalidation/rescan after lost detail (including ERROR_MORE_DATA 234), replacement/rebinding and cancel/drain ownership; real UNC fixtures, not a claimed live SMB-server disconnect experiment. Unix `inotify.rs`, `kqueue.rs`, `notifyd.rs`, `test_helpers.rs` remain platform-specific. |
| `reader_windows.rs`, `reader/mod.rs`, `reader/input.rs`, `reader/reader.rs`, `reader/iothreads.rs`, `reader/history_search.rs`, `reader/word_motion.rs` | I N3 batch seam and N4 actual batch history selection; U interactive reader N6, suggestions/search/completion UI N7 | Native current-history selection and stale-snapshot interface do not fabricate editor state; exclusive console input and cancellation-safe editor workers remain future work. |
| `input/mod.rs`, `input/input.rs`, `input/decode.rs`, `input/binding.rs`, `key.rs`, `editable_line.rs`, `kill.rs` | U native; N6 core input/editing, N7 advanced modes | Preserve logical keys, Unicode characters and editing models, not UTF-16-unit cursor offsets or echo-only tests. |
| `terminal.rs`, `termsize.rs`, `screen.rs`, `tty_handoff.rs`, `text_face.rs`, `pager.rs` | U native; N6 transport/first reader, N7 full renderer/pager | Real console/VT modes, viewport, capability negotiation and foreground-child input donation; no POSIX fg/bg. |
| `abbrs.rs`, `highlight/mod.rs`, `highlight/highlight.rs`, `highlight/file_tester.rs` | I N3 declarative metadata and highlighted batch display; N7 active editor consumers | Complete/abbr registration, serialization and erase work; function/type and formatter batch highlighting work with configured styles. No active completion or abbreviation expansion is claimed. |
| `tinyexpr.rs`, `flog.rs`, `trace.rs`, `locale.rs`, `locale/windows.rs`, `localization/mod.rs`, `localization/gettext.rs`, `localization/settings.rs` | I N3 arithmetic/debug/profile/fallback and N4 numeric/time locale; N9 full locale/catalog parity | Exported LC_ALL > category > LANG; NLS numeric locale and UCRT historical time formatting support C/UTF-8/native Windows names and declared aliases, UTC/Windows TZ IDs with historical DST. Unsupported IANA/POSIX TZ/GNU formats diagnose rather than silently substitute; full message catalogs remain N9. |
| `re.rs` | I N3 UTF-32 PCRE2 | Shared regex captures/replacements, named variables, offsets and error behavior; not Win32/.NET regex. |
| `nix.rs` | U Unix glue; N2/N5/N8 replace consumers at their owners | No blanket Win32 errno, fd, identity or signal emulation. |
| `print_help.rs` | U native help; N9 | Installed/embedded help and native viewer/error policy; existing script-based help is not available. |
| `builtins/mod.rs`, `builtins/shared/mod.rs`, `builtins/shared/error.rs`, `builtins/shared/misc.rs`, `builtins/native_fs.rs`; every builtin enumerated below | I N1/N3/N4 native registry; R/U deferred families | Option-level boundaries override inclusion. Internal `__fish_native_fs` supplies mkdir/remove/atomic-write for utility-free bootstrap and public funcsave; it is not a general external-utility compatibility layer. |
| `bin/fish.rs`, `bin/fish/unix.rs`, `bin/fish/windows.rs`, `bin/fish_indent.rs`, `bin/fish_key_reader.rs` | I N3 batch/formatter and N4 normal startup; R key reader N6 | Normal config → ordered init commands → execution, login/private and startup profiling work; syntax-only avoids config/init/state effects. Interactive N6 and installed help N9 remain rejected. |
| `tests/mod.rs`, `tests/prelude.rs` | U root Unix unit harness on native; each owning phase retains applicable cases | Native integration suites do not establish that all shared unit tests were discovered/executed. |
| `wutil/windows.rs::native_current_exe_path` | I N4 trusted OS-image boundary | Cached absolute OS-reported image path handles literal extended install spelling before ordinary N1 validation; shared by install assets/current-shell identity consumers, never a relaxation for user-supplied paths or a PATH-selected shell. |

## Current workspace crate inventory

All paths are `crates/<name>`; the original source/build link table below supplies per-crate navigation.
There are 21 existing directories, not 21 native-verified crates. A workspace dependency declaration
for `localization-extraction` does not correspond to an existing crate directory and is not counted
as a delivered executable. State here concerns integration evidence, not whether a library can be
compiled in isolation.

| Crate(s), including current added sources | State / implementation owners | Required native behavior |
|---|---|---|
| `common`, `widestring` (`src/lib.rs`, `src/word_char.rs`), `util`, `wcstringutil` | S shared/prototype; I N1 identity codec; N3 full consumers | Checked OS-string/native UTF-16 versus fish text/byte provenance, Unicode/private-use handling and rejection without replacement aliases. |
| `tempfile` (including `tests/runtime.rs`) | I native helper N1/N3 gate evidence; N4 state uses its own transaction backend, N8 psub, N10 distribution | Historical four leaf tests/two integrations remain helper evidence, not proof of private ACLs or every consumer. N4 staging security belongs to `fs/windows.rs`. |
| `fallback`, `wgetopt`, `feature-flags` | S shared subset; N3 complete option/feature consumers, N6/N7 widths | Native boundaries without Unix-only fallback calls; complete flags and text widths at their actual consumers. |
| `printf` (all `src/fmt_fp/` and locale/tests) | I N3 shared formatting and N4 root numeric-locale consumer; N9 full parity | Existing Rust formatter consumes the NLS-selected numeric locale; exact formats/errors/binary output remain shared. This is not complete catalog/GNU-format parity. |
| `color`, `widecharwidth` | Shared libraries present; U active native renderer; N6/N7 | Capability-aware styles and actual terminal-cell widths, not UTF-16 length. |
| `fluent`, `localization`, `gettext`, `gettext-maps`, `gettext-mo-file-parser` | S shared source/build paths; U complete native locale/catalog selection, evidence pending; N9, N10 deployment | Actual catalog lookup, fallback, precedence and packaged resources under default/minimal profiles. |
| `build-helper` (`src/lib.rs`, `src/version.rs`, `tests/host.rs`) | S native-host helper source; execution evidence pending; N9 generation, N10 build/release | Native version/build-path discovery without `/bin/sh`; execute real supported-host generation and failure propagation. |
| `build-man-pages`, `gettext-extraction`, `fluent-extraction`, `xtask` (`fluent`, `format`, `gettext`, `shellcheck` children) | U/unverified complete native tool execution, not runtime support; N9 generation, N10 supported host orchestration | Real generation/extraction/check entrypoints and outputs; declare tested external dependencies and host-only execution paths where applicable. |
| Root `fish` package, `build.rs`, `Cargo.toml`, `Cargo.lock`, `.cargo/` | I native development through N4; N10 release/build matrix, N11 reconciliation | Rust minimum 1.85 and stable 1.95 native no-default-features builds/executable gates passed. Default features are `embed-manpages` and `localize-messages`; their complete native deployment/certification remains N9/N10. |

## Current assets and infrastructure families

The historical count table below is retained intact; no old count establishes current per-file
support. N9 must close the file-level shipped-asset ledger, including generated/embedded/extracted
forms. Earlier phases own assets required for their own working behavior now, not at N9/N11.

| Existing family | State / owner | Required native semantics |
|---|---|---|
| `etc/config.fish`, `share/config.fish`, `share/__fish_build_paths.fish.in` | I N4 normal native bootstrap; N10 installed layout certification | Utility-free native config ordering, conf.d basename masks and distinct executable-relative assets/state roots. `-N` skips config, not init commands, and selects private history. |
| `share/functions/` | I N4 core disk/embedded autoload and public funcsave; U as complete native asset family | Native funcsave atomically writes real definitions without mkdir/rm utilities. N6 first-prompt bindings, N7 editor helpers, N8 providers/psub/adapters and N9 remaining tools retain ownership; successful autoload does not certify all scripts. |
| `share/completions/` | U active native completion; N3 declarative metadata, N7 evaluated completion, N8 provider adapters, N9 breadth | Keep static definitions; audit dynamic commands/transitive tools and native argument/path/descriptor protocols. Windows-oriented filenames are not execution evidence. |
| `share/prompts/`, `share/themes/` | U active native prompt/rendering; N6 core prompt, N7 styles/modes, N9 remaining/VCS prompts | Utility-free first prompt; actual terminal capabilities and tested optional VCS dependencies. |
| `share/help_sections`, `doc_src/`, embedded/generated manpage resources | U installed native help; N9, package N10 | Native help/viewer/browser resolution, all executable/builtin help entries, no competing fish on PATH capturing internal dispatch. |
| `share/tools/` (Python/web/static assets), user editor/browser/clipboard/extraction helpers in functions | U native family, optional dependency behavior unverified; N6/N7 required clipboard actions, N9 user tools | Correct argv/Unicode/newlines/temp lifetime; native services or declared/tested dependencies present and absent. Python/Sphinx/gettext are not mandatory shell-startup dependencies. |
| `localization/fluent/`, `localization/po/`, generated catalogs | Native localization evidence pending; N9, N10 packaging | Actual language/environment precedence, catalog fallback and deployed data; fixed C numeric parsing remains separate. |
| `tests/windows_native.rs`, `tests/windows_native/{probe,fixture,transport,runtime,provenance,conpty}.rs` | I retained prototype regressions rerun through N4 | Exact 15-case registry ran against both artifacts (30 PASS); runtime/provenance ran in both harness profiles. Probe is a test helper, not another shipped CLI. |
| `tests/windows_n1.rs`, `tests/windows_n1/{native,paths,discovery,glob}.rs` | I N1; three required gates verified | Native debug/release harnesses each ran all three gates against both artifacts. Required fixture absence hard-fails rather than skips. |
| `tests/windows_n2.rs`, `tests/windows_n3.rs`, `tests/windows_n3/{language,builtins,strings,metadata,capture,read,scripts,formatter}.rs` | I N2/N3; three gates per phase | Both harness profiles exercise both executable artifacts; N3 requires all eight families, 16 PASS-CASE records per harness. |
| `tests/windows_n4.rs`, `tests/windows_n4/{startup,history,storage,locale,notifications,universal,notification_probe,storage_probe}.rs` | I N4 verified registry | Exactly N4_STARTUP/N4_SHARING/N4_RECOVERY passed in debug and release harnesses, each exercising 11 families against both artifacts with 22 PASS-CASE records and no ignored cases. Minimum-Rust debug harness also passed against its debug executable plus stable release. |
| Other `tests/` checks/fixtures/interactive drivers, `benchmarks/` | Native migration/coverage unavailable as a complete family; owning phases N1–N10, N11 reconciliation/stress | Keep applicable Unix suites; translate host RUN/pty assumptions without deleting portable behavioral coverage. |
| `.github/`, `.builds/`, `docker/`, `vagrants/` | Native gate/release matrix pending; N10 CI/packaging, N11 closure; N1 owns its gate integration | Required IDs/cases must be discovered, nonempty and nonignored; label native, Unix/MSYS and cross-build evidence distinctly. |
| `build_tools/`, `cmake/`, root make/build/package metadata, `contrib/`, `osx/`, `updatecli.d/`, licenses/security/project metadata | Native release unavailable as a whole; N10, final quality N11 | Keep Unix/macOS-specific packaging separate from native x64 ZIP; supported build-host tools must actually execute. No unsigned artifact presented as signed. |

## Executable CLI option ledger

Source authority: [`bin/fish/windows.rs`](../../src/bin/fish/windows.rs) versus
[`bin/fish/unix.rs`](../../src/bin/fish/unix.rs), and
[`builtins/fish_indent.rs`](../../src/builtins/fish_indent.rs) /
[`builtins/fish_key_reader.rs`](../../src/builtins/fish_key_reader.rs). Unix option declarations
below inventory every spelling. Native fish uses the shared getopt grammar (including combined
short flags and repeated commands), stopping at the first operand or `--`. Normal batch startup
is enabled; `-N` skips config, keeps ordered init commands and uses private history.

| Executable / spelling or entry mode | State / owner | Required native semantics |
|---|---|---|
| fish `-c ARG`, `--command ARG` | I N3 | Repeated command strings execute in order; remaining operands supply `$argv`, with explicit `--` boundaries. |
| fish `-N`, `--no-config` | I N4 | Skips config only, not ordered init commands or universal state; selects private history. Not required for batch execution. |
| fish script file, stdin execution, option terminator `--` | I N3 | Native source reads, Unicode/space paths, UTF-8 BOM/CRLF, argv, filename/line/EOF and `--` boundaries. |
| fish `-n`, `--no-execute` | I N3/N4 | Parse/diagnose without evaluation, config/init execution or persistent-state effects. |
| fish `-f ARG`, `--features ARG` | I N3 | Shared feature-flag grammar and ordering. |
| fish `-v`, `--version` | I N3 | Real build/version output and status. |
| fish `-d ARG`, `--debug ARG` | I N3 | Real category selection. |
| fish `-o ARG`, `--debug-output ARG` | I N3 | Native diagnostic file sink and errors. |
| fish `--print-debug-categories` | I N3 | Actual available category enumeration. |
| fish `-D ARG`, `--debug-stack-frames ARG` | R N3 retirement | Explicit retired diagnostic with nonzero status; not accepted/ignored stack debugging. |
| fish `-p ARG`, `--profile ARG` | I N3 | Actual evaluation profiling and native sink errors. |
| fish `-C ARG`, `--init-command ARG` | I N4 | Repeated init commands in command-line order after config and before execution; `-N` does not suppress them. |
| fish `-l`, `--login` | I N4 | Actual native startup/login state, not POSIX login-shell installation. |
| fish `-P`, `--private` | I N4 | Private history/session persistence without dropping universal semantics. |
| fish `--profile-startup ARG` | I N4 | Real startup-stage measurements and native file errors. |
| fish `--print-rusage-self` | R; N5 | Truthful native shell counters/units; unavailable POSIX fields explicit. |
| fish `-i`, `--interactive`; implicit interactivity | R/U; N6 | Actual native reader/terminal selection and mode restoration; not captured echo. |
| fish `-h`, `--help` | R; N9 | Installed native help and dependency/viewer errors. |
| fish_indent default stdin/file operands | I standalone and builtin N3 | Shared formatter with checked native paths, input/error/status behavior. |
| fish_indent `--dump-parse-tree` (long-only; no short `-P`) | I N3 | Real parse-tree diagnostic output. |
| fish_indent `-i`, `--no-indent`; `--only-indent`; `--only-unindent` | I N3 | Shared formatting transformations. |
| fish_indent `-w`, `--write` | I N3 | Actual overwrite and file/write errors. |
| fish_indent `-c`, `--check` | I N3 | No rewrite; differences/status across operands. |
| fish_indent `--html`, `--ansi`, `--pygments` | I N3 | Selected escaping/token output; ANSI honors configured styles without requiring console capability. |
| fish_indent `-v`, `--version`; `--` | I N3 | Real version and option/file boundary. |
| fish_indent `-h`, `--help` | R; N9 | Native formatter help matching installed tool. |
| fish_key_reader default console input | R standalone and builtin; N6 | Exclusive input ownership, key decoding, invalid/nonterminal errors and mode restoration. Windows entrypoint currently exits 2 before parsing. |
| fish_key_reader `-c`, `--continuous` | R; N6 | Actual continued key capture and exit/EOF/control policy. |
| fish_key_reader `-V`, `--verbose` | R; N6 | Real key/sequence diagnostic detail. |
| fish_key_reader `-v`, `--version`; `--` | R; N6 | Correct standalone tool dispatch/version/arguments. |
| fish_key_reader `-h`, `--help` | R; N9 | Native installed key-reader help. |

## Builtin option and subcommand ledger

Authority is the native registry and individual parsers under [`src/builtins/`](../../src/builtins).
The N1 commands remain enabled; N3 additionally enables portable builtins, functions/return,
events, string/PCRE2, declarative metadata, ordinary read, status and formatter paths inventoried
below. Reserved later-phase spellings reject rather than falling through to unrelated executables.

### Help, flag conventions and argument-only commands

Every actual help option listed here is **R/U, N9** on native, even for an I/S command.
This includes `-h/--help` for `abbr`, `argparse`, `bg`, `bind`, `block`, `break`,
`breakpoint`, `builtin`, `cd`, `command`, `commandline`, `complete`, `contains`,
`continue`, `disown`, `emit`, `eval`, `exit`, `fg`, `fish_indent`, `fish_key_reader`,
`function`, `functions`, `history`, `jobs`, `math`, `pwd`, `random`, `read`,
`realpath`, `return`, `set`, `set_color`, `source`/`.`, `status`, `ulimit`, `wait`;
also `path -h/--help`, `path SUBCOMMAND -h/--help`, `string -h/--help` and
`string SUBCOMMAND -h/--help`. Echo/printf use their ordinary shared operand parsers,
not a prototype leading-help guard; upstream echo/printf do not parse help options.
`count`, `_`, `test`/`[`, `true`/`:` and `false` have no separate help parser;
do not invent help support or reinterpret test expression strings as GNU options.

The shared required semantics for all portable rows are existing fish list/scoping/status/error
rules, native byte input/output without CRT newline rewriting, checked native identities when
needed and no implicit cmd/PowerShell execution. `--` is an option terminator only where the
actual parser recognizes it, not a new option for argument-only commands.

| Builtin / complete non-help option or operand surface | State / owner | Required native semantics |
|---|---|---|
| `echo`: `-n`, `-s`, `-e`, `-E`; text/escape operands | I N3 | Exact newline/spacing/escape and byte semantics. |
| `exit`: optional status; shared return-value parser `--` | I N3 batch; N5 job shutdown, N6 console cleanup | Real batch exit/status; no interactive/job shutdown claim. |
| `true`, `false`, `:`: no parsed options | I N3 | Shared success/failure and ignored-operand behavior. |
| `printf`: FORMAT and operands; no getopt flags | I N3 conversions and N4 numeric locale; full locale parity N9 | Shared conversions, widths, precision, escapes, binary/NUL output and numeric errors; leading `--` is a format operand. |
| `math`: expression operands | I N3 | Shared arithmetic/errors and argv/stream selection. |
| `math`: `-s/--scale ARG`, `-b/--base ARG`, `-m/--scale-mode ARG` | I N3 | Shared base/precision/rounding semantics. |
| `set`: `-l/--local`, `-g/--global`, `-x/--export`, `-u/--unexport`, `-q/--query`, `-e/--erase`; assignment/list/slice operands and `--` | I N3; N1 native boundaries | Shared scopes/lists/slices/status, checked native PATH/export/cwd identities. |
| `set`: `-f/--function`, `-n/--names`, `-L/--long`, `-S/--show`, `-a/--append`, `-p/--prepend`, `--path`, `--unpath`, `--no-event` | I N3 | Function scope, inspection, list/path-variable behavior and event suppression. |
| `set`: `-U/--universal` | I N4 | Durable cross-process values/export/path flags, distinct-key merge and same-key last acknowledged writer; storage failure is nonzero with pending mutations retained. |
| `argparse`: `-s/--stop-nonopt`, `-i/--ignore-unknown`, `-u/--move-unknown`, `-U/--unknown-arguments ARG`, `-n/--name ARG`, `-x/--exclusive ARG`, `-S/--strict-longopts`, `-N/--min-args ARG`, `-X/--max-args ARG`; specifications/validation | I N3 | Shared dynamic option grammar, `_flag_*` scopes, argv boundaries and embedded `_validate_int` validation. |
| `builtin`: `-n/--names`, `-q/--query`; execution decoration | I N3 | Actual native registry/query/decorator behavior. |
| `contains`: `-i/--index`; needle/list | I N3 | Exact membership/index/status and empty/Unicode values. |
| `count`: argv plus newline-terminated stdin records; no parsed options | I N3 | Newline count versus incomplete last record and zero status. |
| `random`: seed/range/step operands and `choice` mode; no non-help flags | I N3 | OS entropy and shared seeded/range/choice semantics. |
| `break`, `continue`, `return`: no non-help flags; return optional status | I N3 | Loop/function scope and value/error restoration. |
| `eval`, `emit`, `source`/`.`: no non-help flags; filename/stdin `-`, argv, event/arguments, source operands | I N3 | Same-shell evaluator/reader, scope/filename/line/status; generic/variable events are not OS signals. |
| `block`: `-e/--erase`, `-l/--local`, `-g/--global` | I N3 | Language-event blocking and queued dispatch at scope boundaries. |
| `function`: `-d/--description ARG`, `-e/--on-event ARG`, `-v/--on-variable ARG`, `-a/--argument-names ARG`, `-S/--no-scope-shadowing`, `-V/--inherit-variable ARG`, `-w/--wraps ARG` | I N3/N4 | Definitions/scopes/events/wrapping metadata and full core disk/embedded autoload; active completion N7. |
| `function`: `-s/--on-signal ARG`, `-j/--on-job-exit ARG`, `-p/--on-process-exit ARG` (including caller/self forms) | R/U; N5 | Real native lifecycle/control policy; no fictional Unix signal/pgroup handlers. |
| `functions`: `-e/--erase`, `-d/--description ARG`, `-n/--names`, `-a/--all`, `-q/--query`, `-c/--copy`, `-D/--details`, `--no-details`, `-v/--verbose`, `-H/--handlers`, `-t/--handlers-type ARG`, `--color ARG`; names/definitions | I N3 display/metadata and N4 disk/embedded discovery; process handlers N5 | Metadata/copy/tombstones, generic/variable handlers and native definition-file provenance; no validator-only autoload restriction. |
| `complete`: `-x/--exclusive`, `-f/--no-files`, `-F/--force-files`, `-r/--require-parameter`, `-p/--path ARG`, `-c/--command ARG`, `-s/--short-option ARG`, `-l/--long-option ARG`, `-o/--old-option ARG`, `-d/--description ARG`, `-a/--arguments ARG`, `-e/--erase`, `-u/--unauthoritative`, `-A/--authoritative`, `-n/--condition ARG`, `-w/--wraps ARG`, `-k/--keep-order` | I N3 declarative metadata | Definitions, erase, wrappers and replay; authority aliases retain shared compatibility behavior, not new authority functionality. Active evaluation remains N7. |
| `complete`: `-C/--do-complete[=ARG]`, `--escape`; `--color ARG` | R/U N7 active completion; I N3 metadata display color | Evaluated completion remains rejected; metadata serialization/display color is available without an editor. |
| `abbr`: `-a/--add`, `-c/--command ARG`, `--position ARG`, `-r/--regex ARG`, `--set-cursor[=ARG]`, `--description[=ARG]`, `-f/--function ARG`, `--rename`, `-e/--erase`, `-q/--query`, `-s/--show`, `--list`, `-g/--global`, `-U/--universal`, `--color ARG` | I N3 declarative storage/display and shared deprecated-option diagnostics; active expansion N7 | Regex/function/cursor metadata serialize/replay. N4 real universals do not change deprecated abbr-option behavior or make declarations editor expansion. |
| `history`: `search`, `delete`, `clear`, `merge`, `save`, `clear-session`, `append`; aliases `--search`, `--delete`, `--clear`, `--merge`, `--save` | I N4 storage/session commands; N7 active search UI | Real private/named sessions, immutable initial baseline, checked merge and durable save/error/recovery; session clear journals acknowledged ownership and actual predecessors rather than resurrecting stale baseline records. |
| `history`: `-p/--prefix`, `-c/--contains`, `-e/--exact`, `-t/--show-time[=ARG]`, `-n/--max ARG`, `-z/--null`, `-C/--case-sensitive`, `-R/--reverse`, `--color ARG` | I N4 batch filtering/time/output; N7 UI, N9 full presentation parity | Real filtering/order/wall-clock timestamps/NUL bytes and storage errors; native time locale/TZ IDs are supported, unsupported GNU format/IANA/POSIX TZ inputs diagnosed. |
| `jobs`: `-c/--command`, `-g/--group`, `-l/--last`, `-p/--pid`, `-q/--quiet/--query`; targets | R/U; N5 | Real tracked jobs/PIDs/counters; group output must have declared native meaning, never fake POSIX pgroups. |
| `wait`: `-n/--any`; PID/name targets | R/U; N5 | Retained completion, cached exited targets, multiple targets, cancellation and stable identities. |
| `disown`: no non-help options; job targets | R/U; N5 | Native tracking/containment/survival policy, no stopped-job resume or fake reparenting. |
| `bg`, `fg`: no non-help options; job targets | X/R; permanent exclusions (rejection maintained by N5/N6) | No POSIX stop/continue or terminal-pgroup foreground transfer; never external fallback, thread suspension or window focus. |
| `exec` decorator: no replacement CLI flags | X/R; permanent exclusion | Reject before argument side effects or launch; spawn/wait/exit and CRT `_exec` are not same-PID exec. |
| `time` decorator: no builtin option parser | R; N5 | Actual elapsed/CPU/resource accounting for commands/pipelines/shell; no fake zero POSIX measurements. |
| `bind`: `-a/--all`, `-e/--erase`, `-f/--function-names`, `-k/--key`, `-K/--key-names`, `-L/--list-modes`, `-M/--mode ARG`, `--preset`, `-m/--sets-mode ARG`, `-s/--silent`, `--user`, `--color ARG`; key/action operands | R/U; N6 core binding dispatch, N7 full modes/actions | Same logical key identity as reader, correct preset/user tables and mode transitions; every advertised action must work. |
| `breakpoint`: no non-help flags | R/U; N6 | Real nested reader and outer scope/input/mode restoration. |
| `read`: `-a/--array/--list`, `-d/--delimiter ARG`, `-x/--export`, `-f/--function`, `-g/--global`, `-L/--line`, `-l/--local`, `-n/--nchars ARG`, `-z/--null`, `-t/--tokenize`, `--tokenize-raw`, `-u/--unexport`; variables | I N3 noninteractive | Buffered shared cursor without overread/pipe seeks; delimiters/NUL/limits/EOF and assignment scopes. |
| `read`: `-U/--universal` | I N4 | Buffered native input assigns real durable universals with storage-error propagation, not globals. |
| `read`: `-c/--command ARG`, `-p/--prompt ARG`, `-P/--prompt-str ARG`, `-R/--right-prompt ARG`, `-S/--shell`, `-s/--silent`; terminal use of ordinary flags | R/U; N6 | Actual interactive reader, no competing input owner and restoration after EOF/errors. |
| `commandline`: `-a/--append`, `-i/--insert`, `--insert-smart`, `-r/--replace`, `-b/--current-buffer`, `-j/--current-job`, `-p/--current-process`, `-s/--current-selection`, `-t/--current-token`, `-c/--cut-at-cursor`, `-f/--function`, `-x/--tokens-expanded`, `--tokens-raw`, `-o/--tokenize`, `-I/--input ARG`, `-C/--cursor`, `-B/--selection-start`, `-E/--selection-end`, `-L/--line`, `--column`, `--is-valid`, `--forward-jump ARG`, `--backward-jump ARG`, `--forward-jump-till ARG`, `--backward-jump-till ARG` | R/U; N6 | Real reader buffer/scopes/actions, character offsets and outside-reader errors. `--input` remains an inventoried legacy override, not a promised new public API. |
| `commandline`: `-S/--search-mode`, `-P/--paging-mode`, `--paging-full-mode`, `--search-field`, `--showing-suggestion` | R/U; N7 | Actual search/pager/suggestion state. `--paging-full-mode` is long-only in the current short-option string despite its internal `F` tag. |
| `set_color`: `-f/--foreground ARG`, `-b/--background ARG`, `--underline-color ARG`, `-o/--bold`, `-u/--underline[=ARG]`, `-i/--italics[=ARG]`, `-d/--dim`, `-r/--reverse[=ARG]`, `-s/--strikethrough[=ARG]`, `--theme ARG`, `--reset`, `-c/--print-colors`; color/reset operands | R/U; N6 basic styles, N7 full capabilities/themes | Existing shared text-face parser, real VT/console capability policy and redirected-byte behavior; no terminal-only probe on ordinary redirected output. |
| `fish_indent`, `fish_key_reader` builtin spellings | I N3 formatter; R/U N6 key reader; help N9 | Executable ledger options apply to the shared implementation. |
| `_` (gettext): string operands, no parsed options | I N3 fixed-C/minimal fallback; full catalogs N9 | Real fallback needed by `_validate_int`; no registered `gettext` alias or full locale/catalog selection claim. |
| Public `funcsave` function: `-q/--quiet`, `-d/--directory ARG`; function operands | I N4; `-h/--help` N9 | Utility-free native directory creation, atomic definition writes and removal; ordinary function serialization/restart autoload and native failures. Internal `__fish_native_fs mkdir|remove|atomic-write PATH` is the implementation service, not an external-tool alias. |

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
| Shipped `cd.fish`, `prevd`, `nextd` directory history, including `cd -` | S through N4 disk/embedded autoload; complete helper certification N9 | Function-level `dirprev`/`dirnext`, not builtin `cd` or an invented OLDPWD implementation. Available loader/state prerequisites are not a per-helper passing test claim. |
| `pwd`: `-L/--logical`, `-P/--physical` | I; N1 | Logical spelling versus actual owned-directory name/identity; errors for unnameable/deleted physical cwd. |
| `realpath`: default physical; `-s/--no-symlinks`; `--` | I; N1 | Native lexical versus physical resolution, reparse/long/UNC handling and missing final component versus missing parent. No nonexistent `-L/-P` options invented. |
| `command`: `-a/--all`, `-q/--query/--quiet`, `-s/--search`, short-only `-v`; execution decoration | I; N1 | One ordered native provider resolver, no cwd injection, explicit-path bypass, denied/missing diagnostics and agreement with actual launch. `-v` aliases `-s`, not `--version`. |
| `type`: `-a/--all`, `-s/--short`, `-f/--no-functions`, `-t/--type`, `-p/--path`, `-P/--force-path`, `-q/--query/--quiet`, `--color ARG` | I N1 lookup, N3 display and N4 disk/embedded function discovery | Registry/resolver match execution; real definitions and configured highlighted output, not active editor completion. |
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
| `test`, `[`: `-t FD` | I native logical-endpoint console query N2; N6 full terminal context | Current descriptor graph/redirection truth, not POSIX isatty on a HANDLE integer. |
| `test`, `[`: `-b`, `-c`, `-p`, `-S`, `-G`, `-g`, `-k`, `-O`, `-u` | R; N8 | Explicit unsupported special-file/Unix ownership/mode diagnostics; no invented identity, device kind or set-id/sticky bits. |

### String subcommands and options

Every row is **I, N3**, through the shared implementation and native buffered byte streams,
Unicode character/cell semantics and UTF-32 PCRE2. Reserved PUA preserves shared fish encoded-byte
representation and literal/raw-byte distinction; this is not a new Unicode counting convention.
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

`status` is native-registered. N3 parser/build and N4 login/embedded-resource operations below
are implemented. Later-owner rows retain explicit unavailable policy or truthful noninteractive
context; they do not enable job control, terminal negotiation or full installed help/catalogs.

| Status operation / option aliases | Owner | Required native semantics |
|---|---|---|
| Default summary; `basename`/`current-basename`, `dirname`/`current-dirname`, `filename`/`current-filename`, `line-number`/`current-line-number`, `function`/`current-function`, `current-command`, `current-commandline`, `is-block`, `is-command-substitution`, `stack-trace`/`print-stack-trace` | N3 | Actual parser/source/stack context and nesting. Flags `-b/--is-block`, `-c/--is-command-substitution`, `-f/--filename/--current-filename`, `-n/--line/--line-number/--current-line-number`, `-t/--print-stack-trace`, `-L/--level ARG`. |
| `build-info`/`buildinfo`, `features`, `test-feature`, `fish-path`; `--fish-path` | I N3/N4 (N1 path boundary, N10 installed identity) | Real build/features and trusted OS current-image identity, not a PATH-selected competing shell. |
| `is-login`; `-l/--is-login`; default login summary | I N4 | Actual native startup mode; summary and explicit query use the same real login state. Native job-control summary remains unavailable at N5/N6. |
| `is-interactive`, `is-interactive-read`, `is-breakpoint`; `-i/--is-interactive`, `--is-interactive-read` | N6 | Real reader/context state. |
| `is-full-job-control`, `is-interactive-job-control`, `is-no-job-control`, `job-control`; `--is-full-job-control`, `--is-interactive-job-control`, `--is-no-job-control`, `-j/--job-control ARG` (`none`, `interactive`, `full`) | N5/N6 truthful native policy; POSIX fg/bg X | Explicit native availability/difference; no successful claim of terminal-pgroup/stopped-job support. |
| `terminal`, `terminal-os`, `test-terminal-feature` | N6 capability baseline, N7 full negotiation | Actual terminal/protocol capability state and errors. |
| `get-file`, `list-files` | I N4 core embedded-resource access; N9 full installed help | Real embedded bytes/listing for bootstrap/autoload consumers; no complete help/extraction/tooling certification. |
| `language` | R/U N9 | Native message-catalog selection remains explicitly unavailable; N4 numeric/time locale is separate. |

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

`!`/`not`, `and`, `or`, `begin`/`end`, `for`, `if`/`else`, `while`, `switch`/`case`
and `function` are I N3 batch syntax, including scope/error/side-effect behavior and substitutions.
Function event options follow the split above. Background syntax/jobs and `time` remain R N5;
`exec` is X/R permanently. Rejection precedes forbidden expansion/launch. POSIX fork has no
executable CLI/builtin option to enable.

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

## N2 verified evidence record

Source revision: **`f6a8e9162`**, branch **`platform/windows/streaming-io`**. Native source
and acceptance suites are committed as one ownership cutover; documentation records the exercised
development artifacts separately.
Host, NTFS/UNC capability limits, stable Rust 1.95.0, minimum Rust 1.85.0, MSVC 14.51.36231
and explicit SDK 10.0.26100.0 are the same matrix recorded for N1. This is not ARM64,
default-feature release, other-filesystem or interactive-GUI certification.

### Runnable gates and retained behavior

```powershell
$env:FISH_NATIVE_TEST_BIN = (Resolve-Path target/x86_64-pc-windows-msvc/debug/fish.exe).Path
$env:FISH_NATIVE_TEST_RELEASE_BIN = (Resolve-Path target/x86_64-pc-windows-msvc/release/fish.exe).Path
$env:FISH_NATIVE_N1_UNC_ROOT = '\\localhost\D$\dev\fish-workspace\fish-n1-unc-fixture'
cargo +stable test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n2 -- --list
cargo +stable test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n1 --test windows_n2 -- --nocapture
cargo +stable test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_native native_required_acceptance_cases -- --exact --nocapture
```

`windows_n2` discovers exactly `N2_STREAM`, `N2_REDIRECT`, `N2_LIFETIME`, zero ignored.
Each emits its required hyphenated RUN/PASS ID against both executable artifacts.
The final minimum-executable run selected
`target/windows-msrv185/x86_64-pc-windows-msvc/debug/fish.exe` instead of the stable debug
path: N1 **3 passed**, N2 **3 passed**, zero failed/ignored/filtered, all 12 artifact PASS records.
The stable prototype registry retry passed its one selected harness test and all 30 required
debug/release case records; obsolete pipeline/output-rejection rows were removed, not re-pinned.
Permanent and still-deferred rejection coverage remains.
The actual native workflow gate body in `.github/workflows/test.yml` also passed locally:
codec/tempfile leaf suites, five library suites, runtime initialization and text provenance in
both harness profiles, exact 30-record prototype registry, and N1/N2 discovery plus execution
in debug and release harnesses. Each N1/N2 harness ran three tests, zero ignored/filtered,
and emitted all six required artifact PASS records. This is local execution, not a remote CI claim.

The minimum artifact built successfully with `cargo +1.85.0 build --locked --target
x86_64-pc-windows-msvc --no-default-features --bin fish --target-dir target/windows-msrv185`,
`RUSTFLAGS=-C debuginfo=1`, in the explicit SDK environment. Shared cleanup verification on WSL
Ubuntu/Rust 1.92.0 used `CARGO_TARGET_DIR=/tmp/fish-windows-goals-unix-target cargo test
--locked --no-default-features --lib`: **181 passed**, zero failed/ignored/filtered.

### What the N2 gates actually exercise

- Synchronized 32 MiB producer/consumer streaming while the producer remains live, simultaneous
  2 MiB stderr, binary/NUL byte identity, EOF/status, all four native/builtin directions,
  2 MiB internal→native relay→internal traffic and a 256 MiB transfer with bounded process memory.
- Real planner/open/dup/close owners, ordering/merge, append/atomic noclobber contention, null
  targets, missing/denied/invalid errors, empty versus closed stdin, explicit builtin discard
  versus unavailable inherited output, extra-fd same-image bridging and shared alias input cursors.
  Unsupported bridges fail before marker effects. Repeated real failed plans release native handles.
- More than 64 live waits, reset/post/removal races, actual handle recycling, immutable raw 259
  completion, real worker/debounce cancellation and mutex-registration rejection.
  Partial corrupt-image launch rollback, exact child inheritance and executable identity pinning
  are exercised through production owners, not mocked copies.
- Capture finishes while a synchronized descendant remains alive with the writer. Pending reads
  are cancelled/drained; completed partial bytes and explicit empty elements survive; finite
  element metadata is bounded. The harness releases/reaps its own held descendants.

Actual runtime smokes reproduced then corrected a five-second relay deadlock, imported alias
overread, empty-status panic and missing inherited stdout incorrectly returning success.
The corrected 2 MiB relay completed with exact bytes; explicit builtin stdout close discards
successfully, unused closed streams do not fail, and a real write to unavailable inherited stdout
returns 1 with no invented output. Missing input returns 1; corrupt native image launch returns 126.
Native child cwd comparisons use file identity because physical paths may carry `\\?\` spelling.

### Exercised artifact identities

All three report `4.9.3-150-g082685c20-dirty` through `fish -N -c 'printf "%s\n" $version'`.
They were built before the source commit; this embedded metadata is not rewritten as a later
commit ID. `--version` remains an N3 CLI delivery, not a supported N2 query.

| Development artifact | SHA-256 |
|---|---|
| `target/x86_64-pc-windows-msvc/debug/fish.exe` | `9b0511f0af6f91e82b41c7b6dcff7b2e7f94f4736d59aff4d2ad4a65b85b7014` |
| `target/x86_64-pc-windows-msvc/release/fish.exe` | `a51a0a3b272da77a24c75048cdf8469ef97f255787bbef99551e42a3479c04af` |
| `target/windows-msrv185/x86_64-pc-windows-msvc/debug/fish.exe` | `217ded25742c64bf14534c65f66fcef5b546c99835be8a32107563c318d18725` |

N3 inherits capture/input/deferred-output contracts, not already-enabled substitutions or functions.
N4 inherits actual workers/debounce/wakeups; N5 inherits process/registration lifetimes, not
already-enabled background jobs/control events. POSIX fork, same-PID exec and fg/bg remain excluded.

## N3 verified evidence record

Source revision: **`861a9bdc6`**, branch **`platform/windows/batch-parity`**. Historical N1/N2
records are unchanged phase evidence, including their then-deferred N3 boundaries; they do not
override this ledger. Artifacts were built before this source commit and report
`fish, version 4.9.3-152-g9b66558e8-dirty`, not an embedded `861a9bdc6`.
The actual current `.github/workflows/test.yml` native gate body (lines 261–360) ran locally,
exit 0, with explicit SDK 10.0.26100.0, MSVC 14.51.36231 and Rust 1.95.0.
This is local execution, not a remote CI or future-phase claim.

### Option/subcommand-to-family evidence map

Each mapping includes **every non-help option, short/long alias and operand/subcommand listed
in its referenced ledger row**, not merely the three aggregate gate names. The source suites
are `tests/windows_n3/<file>.rs`. Compatibility/ineffective options remain identified above;
mapping an option to a family does not turn legacy no-ops into features. Help stays N9.

| Required gate / behavioral family | Source suite | Mapped portable surface |
|---|---|---|
| `N3_LANGUAGE` / `control-functions-events` | `language.rs` | All grammar forms above; `function` N3 options; `functions` in-memory query/list/erase/copy/description/details/handlers/color; `block`, `emit`, loop control/return, local/function/global scope and variable events. Process/signal handlers excluded N5. |
| `N3_LANGUAGE` / `portable-builtins` | `builtins.rs` | Every listed `argparse`, `builtin`, `contains`, `count`, `random`, `echo`, `printf`, `math`, `set` non-universal option/operand; `true`/`false`/`:`, `exit`, portable test expressions, `type` functions/highlighted display, status parser/build/features aliases. `_` fallback and embedded `_validate_int` validation/override/tombstone; no general disk autoload. |
| `N3_LANGUAGE` / `string-regex` | `strings.rs` | All rows and aliases for `collect`, `escape`, `unescape`, `join`, `join0`, `length`, `lower`, `upper`, `match`, `pad`, `repeat`, `replace`, `shorten`, `split`, `split0`, `sub`, `trim`; UTF-32 regex captures/replacements/named scopes, streams, binary provenance and invalid inputs. |
| `N3_LANGUAGE` / `declarative-metadata` | `metadata.rs` | Every declarative `complete` option/alias, wrappers and metadata `--color`; every `abbr` declaration/query/list/show/rename/erase/display option, regex/function/cursor/command metadata and compatibility diagnostics. Serialize/erase/replay and independent storage, not active completion/expansion. |
| `N3_CAPTURE` / `capture-pipelines` | `capture.rs` | Quoted/unquoted/nested substitutions, functions/blocks in pipelines, explicit empty and NUL separation, shared cursor/redirects, statuses, bounded byte/element retention and held-descendant completion. Genuine capture fault aborts enclosing substitution by provenance; real child status 122 and later captures are not falsely classified. |
| `N3_CAPTURE` / `buffered-read` | `read.rs` | Every ordinary `read` option/alias above: arrays/lists, delimiters/line/NUL, character limits, tokenization/raw tokens, scopes/export/unexport, EOF/status/errors and shared cursor. Universal N4 and terminal/prompt modes N6 excluded. |
| `N3_SCRIPTS` / `invocation-source-profile` | `scripts.rs` | Fish `-N`, repeated `-c`, script/stdin/`--`, `-n`, `-f`, `-v`, `-d`, `-o`, `-p` and all long aliases, category enumeration and nonzero retired `-D`; `source`/`.` and `eval` scope/argv/filename/line behavior. Startup N4, resources N5, interactive N6 and help N9 excluded. |
| `N3_SCRIPTS` / `formatter` | `formatter.rs` | Standalone and builtin fish_indent stdin/files, all non-help formatting/check/write/version/terminator options, parse-tree, HTML/ANSI/Pygments; configured ANSI styles do not depend on console detection. |

Both native debug/release harnesses discovered and passed exactly `N3_LANGUAGE`, `N3_CAPTURE`,
`N3_SCRIPTS`: three tests each, zero ignored, all required IDs and **16 family/artifact
PASS-CASE records per harness** enforced by CI. N1/N2 likewise each passed three tests in each
harness. Prototype registry: 15 cases against both artifacts, **30 PASS**.
Runtime initialization and text provenance each passed one test in both harness profiles.
Native leaf/library suites passed with zero ignored: widestring **25**, tempfile **4**,
common **5**, util **1**, fish **52**; tempfile integration **2** and W2 codec filter **5** passed.
Retained Unix evidence: **184** library tests plus **11/11** portable files passed, zero skips.

Additional real debug/release isolated-root smokes executed a Unicode/space-path UTF-8-BOM/CRLF
script with functions/switch, block pipeline to `string upper`, UTF-32 named capture and Unicode
argv. Actual profiling produced 884 bytes; both standalone formatters produced correct nested
indentation. N3 exercised only `-N`, not startup, persistent state, jobs, interactive mode or help.
Minimum Rust 1.85 fish/formatter/probe builds succeeded. With minimum fish selected as the
debug artifact against stable release, N1/N2/N3 each passed three tests, zero failed/ignored/
filtered, with all 18 root artifact PASS records and all 16 N3 family PASS-CASE records.

### Reproduction commands and exercised artifacts

Use the x64 SDK developer environment and fixture policy recorded above; select SDK `10.0.26100.0`.
Native PCRE2 is built with `PCRE2_SYS_STATIC=1`. The target remains `x86_64-pc-windows-msvc`,
development profile `--no-default-features`; no default-feature release/install certification is implied.

```powershell
$env:PCRE2_SYS_STATIC = '1'
cargo +stable build --locked --target x86_64-pc-windows-msvc --no-default-features --bin fish --bin fish_indent --bin windows_native_probe
cargo +stable build --locked --target x86_64-pc-windows-msvc --no-default-features --release --bin fish --bin fish_indent
$env:FISH_NATIVE_TEST_BIN = (Resolve-Path target/x86_64-pc-windows-msvc/debug/fish.exe).Path
$env:FISH_NATIVE_TEST_RELEASE_BIN = (Resolve-Path target/x86_64-pc-windows-msvc/release/fish.exe).Path
cargo +stable test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n3 -- --list
cargo +stable test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n3 -- --nocapture
cargo +stable test --locked --target x86_64-pc-windows-msvc --no-default-features --release --test windows_n3 -- --nocapture
$env:RUSTFLAGS = '-C debuginfo=1'
cargo +1.85.0 build --locked --target x86_64-pc-windows-msvc --no-default-features --bin fish --bin fish_indent --bin windows_native_probe --target-dir target/windows-msrv185
Remove-Item Env:RUSTFLAGS
$env:FISH_NATIVE_TEST_BIN = (Resolve-Path target/windows-msrv185/x86_64-pc-windows-msvc/debug/fish.exe).Path
cargo +stable test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n1 --test windows_n2 --test windows_n3 -- --nocapture
```

The minimum-executable inherited N1 tests also require `FISH_NATIVE_N1_UNC_ROOT` and the native
filesystem fixtures documented above. CI evidence was written under `target/n3-gate-results/`
locally using the workflow's gate logic. Those development logs are not packaged shell assets.
Retained Unix checks used Ubuntu/Rust `1.92.0`, target directory
`/tmp/fish-windows-goals-unix-target`, `cargo test --locked --no-default-features --lib`, and
`python3 tests/test_driver.py --max-concurrency 4 <binary-directory>` with:
`functions`, `function-definition`, `function`, `function-capture`, `string`, `string-advanced`,
`math`, `argparse`, `read`, `invocation`, `eval` under `tests/checks/*.fish`.

| Exercised development artifact | SHA-256 |
|---|---|
| `target/x86_64-pc-windows-msvc/debug/fish.exe` | `92bff77867557cb3f4383e04b1630c9daf107f60ef2701c116cef92c963bd89c` |
| `target/x86_64-pc-windows-msvc/debug/fish_indent.exe` | `808365f060b0befa7e905887ed6270fa3f4a190031e1a2e5dfcd1b928b0f7b3d` |
| `target/x86_64-pc-windows-msvc/release/fish.exe` | `00dd614df551a35a326bb869692523ba999fb324f484d13c3c53989e7cea9f34` |
| `target/x86_64-pc-windows-msvc/release/fish_indent.exe` | `4a9b152b86cd0a7bb5277fc54080b6bb211df581c8c196a267124008fb94c006` |
| `target/windows-msrv185/x86_64-pc-windows-msvc/debug/fish.exe` | `c67549bc8b13213c127e9b649299c2f4de29e08d2786b5a62045ed2b0fa6c13c` |
| `target/windows-msrv185/x86_64-pc-windows-msvc/debug/fish_indent.exe` | `f6b42a063dba836f61696eadf352c42f669ca65261ba38b21b96d661ab809609` |

The isolated batch-context smoke also checked `status current-command`/`current-commandline`
against shared noninteractive behavior: program name `fish` and an empty interactive commandline.
It does not pretend that noninteractive evaluation supplies N6 reader command tracking.

At the historical N3 cutoff, remaining ownership included universal variables, disk autoload and
startup/history N4 (now implemented and verified in the current ledger);
background/jobs/native lifecycle and control N5; interactive reader/input N6; active completion,
abbreviation expansion, suggestions and editor UI N7; additional providers/platform categories N8;
installed help and full locale/catalog selection N9. POSIX fork, same-PID exec and fg/bg stay excluded.

## N4 implementation and evidence record

Source commit **`c52f6d98a`** on `platform/windows/persistent-state`, based on
`1bfef1539`. The recorded binaries were built from that base plus the N4 changes
subsequently committed as `c52f6d98a`; build metadata predates the commit.
Earlier GOAL/N1–N3 evidence and inventory counts remain unchanged.
Source authority is `tests/windows_n4.rs`; test names use underscores, emitted gates hyphens.

| Required gate | Required family | Source |
|---|---|---|
| `N4_STARTUP` | `configuration-autoload-save` | `tests/windows_n4/startup.rs` |
| `N4_STARTUP` | `history-modes` | `tests/windows_n4/history.rs` |
| `N4_SHARING` | `universal-values-events` | `tests/windows_n4/universal.rs` |
| `N4_SHARING` | `storage-transactions` | `tests/windows_n4/storage.rs` |
| `N4_SHARING` | `history-sessions` | `tests/windows_n4/history.rs` |
| `N4_SHARING` | `locale-timezone` | `tests/windows_n4/locale.rs` |
| `N4_RECOVERY` | `universal-formats-errors` | `tests/windows_n4/universal.rs` |
| `N4_RECOVERY` | `storage-crash-acl` | `tests/windows_n4/storage.rs` |
| `N4_RECOVERY` | `autoload-save-errors` | `tests/windows_n4/startup.rs` |
| `N4_RECOVERY` | `history-recovery` | `tests/windows_n4/history.rs` |
| `N4_RECOVERY` | `directory-notifications` | `tests/windows_n4/notifications.rs` |

Each family owns fresh startup/config/state roots. A complete harness must discover exactly
three gates, execute all 11 families against both debug/release executables and emit 22
PASS-CASE records. Both harness profiles are required; missing mandatory fixtures, including
`FISH_NATIVE_N1_UNC_ROOT`, must fail, not become ignored tests. Root names and fixture details
in the source are authoritative.

```powershell
cargo test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n4 -- --nocapture
cargo test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n4 --release -- --nocapture
```

### Final executed matrix

All commands exited zero. Native target: `x86_64-pc-windows-msvc`, Windows 10.0.26300,
MSVC 14.51.36231, Windows SDK 10.0.26100.0, Rust 1.95.0 (`59807616e`).
The minimum compiler was Rust 1.85.0 (`4d91de4e4`), with `RUSTFLAGS=-C debuginfo=1`
and `CARGO_TARGET_DIR=target/windows-msrv185`.

| Executed check | Observed result |
|---|---|
| Native debug/release builds of fish, fish_indent and native probe | Passed, locked/no-default-features |
| Actual native workflow gate body in `.github/workflows/test.yml` | Passed, including exact discovery/registry validation |
| Leaf codec/runtime tests | 5 codec and 2 tempfile runtime tests passed |
| Native libraries | widestring 25, tempfile 4, common 5, util 1, fish 60 passed; no failures/ignored |
| Initialization/provenance | Each exact test passed in both harness profiles |
| Prototype acceptance | Exact 15-case registry against both artifacts: 30 PASS |
| N1–N4 | Each phase discovered/executed exactly 3 tests in each harness profile against both artifacts; 0 ignored |
| N4 family registry | 22 PASS-CASE records in each harness, 44 total |
| Minimum-Rust build and N1–N4 debug harnesses | All 12 Cargo tests passed against minimum-compiler debug and stable release executables; 0 ignored |
| Retained Unix build/library | fish/fish_indent/fish_key_reader built; 183 library tests passed, 0 failed/ignored |
| Retained Unix selected checks | 14 passed, 1 skipped: locale-numeric requires an uninstalled comma-decimal locale |
| Clean-root actual executable smoke | Both artifacts loaded config and Unicode autoload, persisted a universal value across restart, and searched private history without creating any history file; all exits 0, stderr empty |

Minimum executable gate command (environment selects the above artifacts and mandatory UNC root):

```powershell
cargo +1.85.0 build --locked --target x86_64-pc-windows-msvc --no-default-features --bin fish --bin fish_indent --bin windows_native_probe
cargo +1.85.0 test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n1 --test windows_n2 --test windows_n3 --test windows_n4 -- --nocapture
```

Unix commands ran in WSL `ubuntu-dev`, separately from native evidence, using
`CARGO_TARGET_DIR=/tmp/fish-windows-goals-unix-target`:

```sh
cargo build --locked --no-default-features --bin fish --bin fish_indent --bin fish_key_reader
cargo test --locked --no-default-features --lib
python3 tests/test_driver.py /tmp/fish-windows-goals-unix-target/debug tests/checks/{history,set,setenv,autoload,completion-autoload-tombstone,config-paths,no-config,broken-config,init-unreadable-config-paths,default-setup-path,functions,invocation,read,locale,locale-numeric}.fish
```

Full local outputs are retained under `target/n4-gate-results/`: workflow per-target
discovery/execution logs, `final-debug.txt`, `final-release.txt`,
`final-native-workflow.txt`, `final-minimum-build.txt`, `final-minimum-n1-n4.txt`,
`final-unix-{build,lib,checks}.txt`, and `final-evidence.json` (toolchains, smoke,
artifact identities). CI uploads its corresponding `native-windows-results/`.

| Artifact, relative to target | SHA-256 |
|---|---|
| `x86_64-pc-windows-msvc/debug/fish.exe` | `76f174f1b16d50397fe8c3d92bfb5cb2c565ca889d286ad926b64c67d17cc4cd` |
| `x86_64-pc-windows-msvc/debug/fish_indent.exe` | `7b8d3b2ecbe1c2fd5a507715afcb0037f2a97eca2d7b5a9aae39d1f0fe3b7842` |
| `x86_64-pc-windows-msvc/release/fish.exe` | `5aea94b6f31517f663d4c85e5550c8fffe344a647fe6a4ad5e660a3699c746d7` |
| `x86_64-pc-windows-msvc/release/fish_indent.exe` | `015ed2c824168b22fa65d9b58b4063245d130b11a2b77ff48a35920dfaa68379` |
| `windows-msrv185/x86_64-pc-windows-msvc/debug/fish.exe` | `d9e3598baf711998f8166cb613f43d12c46df782ed9d595835a20667346bb1c8` |

These are development `--no-default-features` results, not default-feature release/install
certification (N10). They complete N4, not the eleven-phase project.

N4 delivers utility-free normal bootstrap, full core autoload, public atomic funcsave, durable
universals/history, numeric/time locale and directory notification. It does not deliver N5 jobs,
N6 terminal/editor, N7 active `complete -C`/abbreviation/search UI, N8 extra providers, N9 full
shipped assets/help/catalog/locale parity or N10 installation certification. Faithful POSIX fork,
same-PID exec and POSIX fg/bg remain permanently excluded. Native flush/replacement makes no
power-loss directory-metadata or SMB-server durability promise; notification recovery does not
claim a live SMB-server disconnect experiment.
