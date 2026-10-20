# Workspace crates

## N10 execution/deployment destinations

These are explicit current destinations, not passing runtime claims. Native
`build_tools/native.ps1` preserves `default` and `minimal` features; extraction
features are maintenance-host operations, not covert runtime dependencies.
The following ledger accounts for all **21** direct-child crates plus root fish.
`fish-localization-extraction` is a manifest dependency name without a crate
directory, not an invented twenty-second workspace member.

| Direct-child crate | Native runtime / build-host destination |
| --- | --- |
| `common` | Linked native runtime algorithms and I/O/platform consumers; native crate/root gates, not host-only. |
| `widestring` | Linked native runtime wide-string encoding/path/stream consumers; native crate/root gates. |
| `util` | Linked native runtime shared utility/platform consumers; native crate/root gates. |
| `tempfile` | Linked runtime temporary-file users and host extraction helpers; native crate tests do not replace atomic-state/DACL gates. |
| `fallback` | Linked runtime width/case algorithms; native crate/console gates. |
| `wcstringutil` | Linked runtime token/string algorithms; native crate/root gates. |
| `wgetopt` | Linked native builtin/tool option parsing; native crate and actual three-executable gates. |
| `feature-flags` | Linked runtime syntax feature handling; native crate/parser gates. |
| `printf` | Linked native formatter; crate tests plus real installed printf/locale consumers. |
| `color` | Linked runtime color parser/conversion; crate tests plus actual native terminal consumers. |
| `widecharwidth` | Linked runtime Unicode width tables; crate tests plus Unicode console gates. |
| `fluent` | Linked native runtime bundles; default embeds multilingual FTL, minimal English; native N9 catalog and runtime gates. |
| `localization` | Linked default-profile runtime language types/catalog keys; native default-profile crate/N9 gates. |
| `gettext` | Linked default-profile Rust PHF lookup, no runtime libintl DLL; native default-profile crate/N9 gates. |
| `gettext-maps` | Host build script runs real native `msgfmt` and generates Rust maps; resulting maps are linked native runtime data; not excused as host-only. |
| `gettext-mo-file-parser` | Host generator/parser and shared parser library; native crate malformed/valid MO tests and real catalog generation. |
| `build-helper` | Native host build scripts (target-aware paths/version) plus linked runtime resource helpers; native W1/root/resource consumers. No `.sh` dependency. |
| `build-man-pages` | Native host Sphinx generation and build-order dependency; generated help is embedded/installed runtime data. Default generation is mandatory, minimal ZIP generation is explicit packaging work. |
| `gettext-extraction` | Host proc macro, invoked during supported Unix maintenance `cargo xtask gettext` extraction/check/update/new; never shipped as a runtime executable or mandatory startup dependency. |
| `fluent-extraction` | Host proc macro, invoked during supported Unix maintenance `cargo xtask fluent` source extraction/check/update; never runtime startup dependency. |
| `xtask` | Supported Unix maintenance CLI/library, all children assigned below; not advertised as a native Windows required operation. Native build/check/package duties use PowerShell directly instead. |

Root `fish` is the native runtime package and supplies **three separately built**
MSVC `.exe` entrypoints (`fish`, `fish_indent`, `fish_key_reader`). No hardlink,
basename-dispatch or duplicate-shell packaging convention substitutes for them.
There are no separately deployed runtime DLLs from these local Rust crates.
Runtime behavior remains subject to native consumer evidence; this role ledger
does not turn successful Cargo dependency resolution into a product result.

### Every xtask child and supported host

The supported maintenance host is Linux/Unix with native Cargo/Rust, fish,
GNU gettext (`msgfmt`, `msgmerge`, `msgattrib`, `msguniq`), `diff`, ShellCheck,
Sphinx/Python and Ruff/rustfmt as applicable. WSL can run these **Unix host tools**,
but their results are never native-Windows shell/console/install evidence.

- `check`: intentional Unix `build_tools/check.sh` orchestration. Native-required
  checks instead use `native.ps1 -Action Check`; they never delegate to this child.
- `format` (`--all`, selected paths, `--check`): Git file selection, installed
  `fish_indent`, Ruff, Cargo/rustfmt on the Unix host; not a shell startup dependency.
- `gettext check`, `update`, `new`: actual source-extraction proc macro output
  plus GNU gettext/diff on that host. A failed subprocess remains nonzero.
- `fluent check` (including `--from-source`), `format`, `rename`, `show-missing`,
  `update`, and every `resolve-outdated` child (`delete`, `ignore`, `needs-review`,
  `show-unresolved`): source extraction and FTL processing on the Unix host.
- `html-docs`: host-executable formatter plus Sphinx HTML generator; its existing
  Unix temporary symlink/PATH policy is intentionally host-only.
- `man-pages`: Cargo/native-host Sphinx generated man output; the native package
  uses the same shared generator/text inputs without invoking this Unix CLI.
- `shellcheck`: Unix shell-script maintenance checks; not a runtime native gate.

`.github/workflows/windows-release.yml` runs supported Unix workspace tests and
real documentation/extraction checks separately and inspects generated outputs;
native CI runs runtime crate/root suites and installed artifact consumers.
Existing Linux/macOS/MSYS release tooling remains separate. No all-workspace
Windows claim is inferred by omitting `xtask`, and no runtime crate is relabeled
host-only to avoid testing it.

## Current N4 consumer overlay

The 21-crate inventory and candidate audit below remain historical; their old unconditional-Unix
descriptions are not the current native availability ledger. N4 root `locale/windows.rs` supplies
NLS numeric locale to the existing Rust `printf` crate and explicit UCRT historical time formatting;
it does not replace the formatter or certify full Fluent/gettext catalogs (N9).
Native universal/history/public-funcsave transactions use `src/fs/windows.rs`, with checked
identity, stable parent locks and target DACLs at staging creation; generic `tempfile` crate
success is not evidence for those guarantees. Trusted current-image resolution lives in
`src/wutil/windows.rs::native_current_exe_path`, not a second crate-local path policy.

N4 development evidence uses `--no-default-features`; default-feature generator/catalog/install
certification remains N9/N10. Rust 1.85 and combined native/retained results are
recorded in the [N4 record](coverage.md#n4-implementation-and-evidence-record).
No original audit count or earlier evidence is changed by this overlay.

## Inventory and evidence boundary

[`Cargo.toml`](../../Cargo.toml) uses `members = ["crates/*"]`: there are **21 direct-child crates**, in addition to the root `fish` package and its three binaries. The table below inventories all 21, including every local build script. [`coverage.md`](coverage.md) provides the tracked-file inventory; examples of symbols below are entry points, not a claim that each crate contains only those functions.

Portability has two levels: a crate can contain OS-independent algorithms while its dependency graph still unconditionally imports Unix APIs. No compilation was performed for this audit. Statements about native Windows blockers below are **source-identified**, not observed compiler diagnostics.

## Per-crate mapping

| Crate / source and concrete evidence | Linux/POSIX dependency, including build host | Native Windows equivalent or no API needed; semantic gap | Proposed verification |
|---|---|---|---|
| **build-helper**: [`env_var`, `fish_build_dir`, `target_os`, `target_os_is_{apple,bsd,cygwin}`](../../crates/build-helper/src/lib.rs) | Unconditional Unix `OsStrExt`; `env_var` prints invalid-Unicode env bytes. Paths use native `Path`; rebuild helpers call `to_str().unwrap()`. `target_os` already uses `CARGO_CFG_TARGET_OS`, not host `cfg!`. No `build.rs`. | Replace Unix byte-only diagnostics with target-appropriate `OsStr` handling; no Win32 call needed for path joins. Distinguish host helpers from target detection and test non-Unicode/unpaired-surrogate build paths. `rebuild_if_embedded_path_changed` uses compile-time `cfg!(windows)` in the helper, which is the host when used by build scripts. | Native Windows host, Unix-host/Windows-target and Windows-host/Unix-target combinations; Unicode/path-space env variables and resource rebuild tracking. |
| **build-man-pages**: [`build.rs::build_man`](../../crates/build-man-pages/build.rs), [library](../../crates/build-man-pages/src/lib.rs) | Build script creates man output directories and invokes `sphinx-build`; library itself is empty. `FISH_BUILD_DOCS=0` skips; missing Sphinx warns unless `FISH_BUILD_DOCS=1`, which asserts; an invoked failing Sphinx is fatal. Sphinx config itself launches a `.sh` version script. | No runtime Windows API or runtime `man` replacement in this crate. Port host Sphinx/version tooling; do not call this crate an embedding implementation: embedding/help consumption is elsewhere. | Present/absent/failing Sphinx, all `FISH_BUILD_DOCS` states, clean Clippy build (which skips `build_man`), man outputs and embedded help consumers. |
| **color**: [`Color`, `Color24`, `from_wstr`, `to_term256_index`](../../crates/color/src/lib.rs) | No direct syscall in color parsing/conversion; dependencies include Unix-bearing `fish-common` and `fish-widestring`. No build script. | No Windows color parser needed. VT/console emission belongs to the terminal consumer, not this crate. | Named/special/RGB parsing, index approximation, and color output through the native terminal backend. |
| **common**: [`escape_string`, `unescape_one`, `is_console_session`, `read_blocked`, `write_loop`, foreground-group restoration](../../crates/common/src/lib.rs); [`build.rs`](../../crates/common/build.rs) | Mixed algorithms and OS mechanisms: Unix `OsStrExt`, raw fds and borrowed standard fds, nix reads/writes/`ttyname`, `_exit`, `tcgetpgrp`/`tcsetpgrp`, `getpgrp`, `signal(SIGTTOU, SIG_IGN)`. Build script declares target-derived `apple`. `timef` currently uses Rust `SystemTime` despite its historical gettimeofday comment. | Keep escaping/guards/time code. Handle-based `ReadFile`/`WriteFile` or Rust I/O traits replace fd I/O; `GetConsoleMode` can classify console handles but not reproduce `/dev/tty*` classification. No exact Windows foreground POSIX process-group restoration; integrate the terminal/process policy instead of translating signals literally. `_exit` after fork has no fork-equivalent use on native Windows. | Escaping/byte preservation, partial pipe writes/EOF/cancellation, console versus redirected handles, shutdown from normal/error paths, and host/target cfg distinction. |
| **fallback**: [`fish_wcwidth`, `fish_wcswidth`, `wcscasecmp`, `lowercase`](../../crates/fallback/src/lib.rs), [manifest](../../crates/fallback/Cargo.toml), [`build.rs`](../../crates/fallback/build.rs) | Current implementation is table-based width and Rust Unicode case comparison; no direct libc syscall is present there. Nevertheless the manifest declares **`libc` and `fish-common`**, so this is not an independently Windows-clean dependency graph. Build script declares `cygwin` via build-helper. Do not infer all compatibility functionality from the crate name or an older version. | No Win32 replacement for the current algorithms. Port/gate inherited Unix dependencies and the build-host helper; retain width overrides and full lowercase expansion. Terminal rendering still differs. | Width/case tests, emoji/private-use overrides, dependency-only native build, and ConPTY cursor-position agreement. |
| **feature-flags**: [`Features`, `feature_test`, `set_from_string`, `with_overridden_feature`](../../crates/feature-flags/src/lib.rs) | Rust flag storage and thread-local test overrides; no direct syscall. Depends on `fish-widestring`; no build script. | No Windows API needed. Syntax feature flags are not an implicit substitute for platform capability detection. | Flag parsing, defaults, unknown flags and nested/thread-local test override restoration. |
| **fluent**: [`FtlFiles`, `make_bundle`, `format_localized`, `set_language_precedence`](../../crates/fluent/src/lib.rs), [`build.rs`](../../crates/fluent/build.rs) | Embedded FTL resources and concurrent Rust Fluent bundles. Build script tracks resource changes using build-helper; runtime also depends on that helper and shared localization types. Feature-off embeds English only. | No Win32 message-catalog API needed. Preserve locale/tag mapping, fallback, typed variables and existing disabled bidi-isolation behavior. Debug/release resource availability is not identical by assumption. | Feature on/off, fallback chains, missing variables, plural/number formatting, native debug/release resource discovery. |
| **fluent-extraction**: [`fluent_extract`, `extract`](../../crates/fluent-extraction/src/lib.rs), [`build.rs`](../../crates/fluent-extraction/build.rs) | Host-executed proc macro writes unique files via `fish-tempfile` when `FISH_FLUENT_EXTRACTION_DIR` is set. Build script tracks that env variable. Even file-only use inherits the tempfile crate's unconditional nix dependency. | No target Windows API; port the **host** tempfile graph and output paths. | Concurrent expansions, disabled/enabled extraction, env changes triggering recompilation, Windows-host cross compilation. |
| **gettext**: [`gettext`, `get_available_languages`, `set_language_precedence`](../../crates/gettext/src/lib.rs) | Rust PHF catalog lookup with mutex-protected precedence, not libintl or dynamic MO lookup. Generated catalogs depend on host `msgfmt` through gettext-maps. No build script here. | No gettext DLL or Windows resource API needed. Locale selection is owned by root settings, not this crate. | Present/missing translations, precedence updates and catalog-free builds. |
| **gettext-extraction**: [`gettext_extract`, `write_po_entry_to_file`](../../crates/gettext-extraction/src/lib.rs), [`build.rs`](../../crates/gettext-extraction/build.rs) | Host proc macro writes PO fragments via exclusive tempfile creation under `FISH_GETTEXT_EXTRACTION_DIR`; build script tracks the variable. Unix dependency is inherited through tempfile, not a shell launched by the macro. | No Win32 translation API needed; native host filesystem/tooling adaptation. | Concurrent extraction and unique fragments, supported string-literal formats, disabled extraction and env rebuild tracking. |
| **gettext-maps**: [`build.rs::embed_localizations`](../../crates/gettext-maps/build.rs), [`CATALOGS` include](../../crates/gettext-maps/src/lib.rs) | Host `msgfmt -h`, format-check support detection, per-language mtime cache, MO parsing and PHF Rust generation. Missing `msgfmt` warns and generates empty catalogs. | Native `msgfmt` or a deliberate generated-input workflow; no runtime Windows API. `include!("{}")` uses raw displayed paths without Rust-string escaping, a source-identified backslash/quote hazard. Host executable discovery and cache semantics need proof. | Missing/broken/successful msgfmt, PO edits invalidating cache, generated files under paths containing spaces, backslashes and non-ASCII characters. |
| **gettext-mo-file-parser**: [`parse_mo_file`](../../crates/gettext-mo-file-parser/src/lib.rs) | Byte-slice MO binary parser; no OS dependencies in its manifest and no build script. | No Windows API needed. Input endianness and bounds checks remain format issues, not target issues. | Both endian encodings, malformed offsets/lengths and truncated headers; generator UTF-8 assumptions checked separately. |
| **localization**: [`Language`, `LocalizationLanguage`, `define_localization_language_type!`](../../crates/localization/src/lib.rs) | Types, PHF hashing/formatting and English default obtained from `fish-build-helper`; **not a locale discovery backend**. No build script. | No Windows API needed for the code; inherited build-helper dependency must be made portable. Default-language acquisition and Windows UI-locale policy belong elsewhere. | Language-key hashing/generated-code agreement and both Fluent/gettext language wrappers. |
| **printf**: [`printf_c_locale`, `sprintf!`](../../crates/printf/src/lib.rs), [`sprintf_locale`](../../crates/printf/src/printf_impl.rs), [`Locale`](../../crates/printf/src/locale.rs), [`fmt_fp`](../../crates/printf/src/fmt_fp/mod.rs), [`Arg`](../../crates/printf/src/arg.rs) | Production Rust formatter with explicit locale and Unicode width/segmentation; manifest declares libc, and [`tests.rs`](../../crates/printf/src/tests.rs) uses `libc::snprintf` as a comparison oracle. No build script or locale discovery here. | No Win32 formatting replacement. UCRT reference output can differ from musl/GNU semantics, so an oracle mismatch is not automatically a fish bug. Locale supports single-character separators and bounded grouping. | Integer/float limits, flags and precision, Unicode widths, locale grouping, `%n`, libc comparisons classified by intended contract. |
| **tempfile**: [`create_file_with_retry`, `new_file`, `new_dir`, `TempFile::drop`, `TempDir::drop`](../../crates/tempfile/src/lib.rs) | Files already use `OpenOptions::create_new(true)` and retry on existence; directory creation uses `nix::unistd::mkdtemp`. Drop cleanup calls Rust `remove_file`/`remove_dir_all`; file removal is attempted while its `File` member is still alive. No build script. | Retain standard exclusive file creation; use exclusive `create_dir`/`CreateDirectoryW` plus random retry for directories rather than a check-then-create race. Windows share/delete permissions, ACL/privacy expectations and cleanup while open are not equivalent to Unix unlink semantics. | Forced collisions, concurrent creators, cleanup while file is open, permissions/ACLs, Unicode temp paths and extraction use. |
| **util**: [`wcsfilecmp`, `wcsfilecmp_glob`, `get_time`, `get_seeded_rng`, `perror`](../../crates/util/src/lib.rs) | Sorting/time/random helpers are Rust. `perror` captures `nix::errno::Errno::last_raw` and calls `libc::strerror`; manifest depends on nix/libc. No build script. | Keep algorithms; preserve captured native error domain instead of interpreting `GetLastError()` as errno. `FormatMessageW` or Rust `io::Error` can represent Win32 errors; CRT errno remains separate. | Natural sorting, reproducible seeded RNG, pre-epoch time and errors with distinct Win32/CRT origins. |
| **wcstringutil**: [`StringFuzzyMatch`, `ifind`, `split_string_tok`, `fish_wcwidth_visible`, `strip_executable_suffix`](../../crates/wcstringutil/src/lib.rs), [`build.rs`](../../crates/wcstringutil/build.rs) | Algorithms use widestring/fallback. `.exe` stripping is **only enabled under custom `cygwin` cfg**, declared by build script; no direct syscall in matching. Inherited Unix graph remains. | No Win32 matching API needed. Native executable suffix policy needs an intentional change; do not globally case-fold filesystem names. | Ranking/tie ordering, Unicode prefix/suffix/case expansion, token splits, rendered controls and Windows suffix behavior. |
| **wgetopt**: [`WGetopter::next_opt`, `next_opt_indexed`, `WOption`, `ArgType`](../../crates/wgetopt/src/lib.rs) | Rust wide-string GNU-style option parsing; no direct syscall or build script; inherits wcstringutil/widestring dependencies. | No Windows API needed. Retain `-`/`--` fish conventions, not global slash-option parsing. | Argument permutation, optional/required args, ambiguous long options, `--`, and Unicode argv. |
| **widecharwidth**: [re-export module](../../crates/widecharwidth/src/lib.rs), [`WcWidth::from_char`, `WcLookupTable::{new,classify}`](../../crates/widecharwidth/src/widechar_width.rs) | Unicode tables/lookup only, no dependency or build-script OS calls. | No Win32 API needed; a table classifies code points, not complete terminal grapheme rendering. | BMP/non-BMP table agreement, control/unassigned/ambiguous/emoji classifications; render integration belongs to screen tests. |
| **widestring**: [`WString`/`wstr`, codecs, reserved encodings](../../crates/widestring/src/lib.rs), [`word_char`](../../crates/widestring/src/word_char.rs) | UTF-32 core is Rust; OS conversion unconditionally uses Unix `OsStrExt`/`OsStringExt`; C-string pointer conversion uses libc C types. No build script. | Retain UTF-32 and byte-stream codec; add distinct UTF-16 `encode_wide`/`from_wide` native boundaries and reversible lone-surrogate representation. Do not use replacement decoding for identity-bearing names. | Byte and UTF-16 round trips, reserved ranges, embedded NUL handling, width decoding, word classification and non-BMP data. |
| **xtask**: [`Task`, `run_checks`, `build_html_docs`](../../crates/xtask/src/main.rs), [`CommandExt`, file selection](../../crates/xtask/src/lib.rs), [manifest](../../crates/xtask/Cargo.toml) | Build-host CLI and library. Unconditional `std::os::unix::fs::symlink`, extensionless `fish_indent` location, `:` PATH concatenation and direct `check.sh` execution are separate blockers. No build script. All children are enumerated below. | Port native host orchestration; direct Rust `Command` is already widely used, so the issue is specific shell scripts/tool prerequisites/path construction, not every command invocation. `std::env::join_paths` and native executable names are candidates; avoid requiring privileged Windows symlink creation just to expose a formatter. | Run each task from Windows-host paths with spaces, with explicit/missing dependencies and without a POSIX shell; test host formatter selection in cross builds. |

## Every xtask child

The complete top-level [`Task` enum](../../crates/xtask/src/main.rs) is `Check`, `Fluent`, `Format`, `Gettext`, `HtmlDocs`, `ManPages`, `ShellCheck` (not an arbitrary sample).

- **Check** runs [`build_tools/check.sh`](../../build_tools/check.sh). This is a POSIX host pipeline even if the eventual compiler target is Windows. Port its orchestration or keep it explicitly a Unix-host task; a native acceptance job needs a native entrypoint.
- **Fluent** ([`FluentCommandArgs`, `ResolveOutdatedCommand`](../../crates/xtask/src/fluent.rs)) has six children: `Check` (consistency/source freshness), `Format` (files or stdin/stdout), `Rename` (IDs and variables), `ResolveOutdated`, `ShowMissing`, and `Update` (regenerate English and reconcile translations). `ResolveOutdated` itself has `Delete`, `Ignore`, `NeedsReview`, `ShowUnresolved`; these delete entries, remove annotations, replace annotations, or filter/display unresolved definitions respectively. Source extraction invokes Cargo with the feature/environment; checks can invoke `diff`, and temp directories inherit `mkdtemp`. Rust FTL transformations need no Win32 API. Verify each named operation, stdin/stdout mode, source extraction, annotation transitions and Windows filenames; installed `diff` is a host tooling requirement, not a Windows syscall.
- **Format** ([`format.rs`](../../crates/xtask/src/format.rs)) selects files and invokes `git`, `fish_indent`, `ruff format`, `cargo fmt`/`rustfmt`; it refuses unsafe dirty-tree formatting unless overridden. No replacement Windows formatting API is needed. Verify `--all`, explicit paths, check mode and executable discovery without changing repository-safety semantics.
- **Gettext** ([`GettextArgs`, nested `Task`](../../crates/xtask/src/gettext.rs)) has `Check`, `New`, `Update`: freshness/diff checking, creation of a catalog for a new language, and reconciliation of existing PO files. It obtains Rust extraction via Cargo and generates templates from Rust fragments and fish scripts, using `msgmerge`, `msgattrib`, `msguniq`, and `diff`; `LC_ALL=C.UTF-8` is a host-locale assumption. Verify all three children, reproducible sorting, existing-file protection for `New`, and temporary-file lifetime on Windows; this is not merely runtime message lookup.
- **HtmlDocs** creates a symlink-only temporary PATH directory for a supplied or built host `fish_indent`, then calls Sphinx. `:` joining and extensionless names are explicit problems. Verify supplied host formatter versus cross-built target formatter, all native paths and HTML syntax highlighting.
- **ManPages** builds `fish-build-man-pages` via Cargo. Verify skip/required/failing Sphinx behavior and generated section outputs, not a runtime `man` executable.
- **ShellCheck** ([`shellcheck.rs`](../../crates/xtask/src/shellcheck.rs)) discovers scripts and launches `shellcheck` directly. The scripts remain POSIX tooling; the linter itself needs no fish runtime Windows API. Verify file selection and a clearly reported unavailable tool rather than silently treating absent linting as success.

## Dependency and root-package implications

The root [manifest](../../Cargo.toml) unconditionally enables `libc`, forked `nix` features (event/fs/hostname/inotify/mman/process/resource/signal/term/user), and UTF-32 PCRE2. Existing Windows-specific `rust-embed` settings and `unix_path` build-dependency are not a native execution backend. [`build.rs::setup_paths`](../../build.rs) uses `unix_path` under build-script host `cfg(windows)` specifically to interpret Unix install paths; that is not Windows runtime path conversion. `get_version` still invokes `git_version_gen.sh`.

The manifest also declares `fish-localization-extraction` under `[workspace.dependencies]`, but there is no corresponding child in the 21-crate inventory and it is not itself a workspace member by that declaration. Do not count it as a twenty-second implemented component.

Target-gate Unix mechanisms at their owners, including build-host proc-macro/xtask dependencies, rather than assuming a root-only dependency change makes leaf crates portable. PCRE2's external native build must support MSVC and the 32-bit-code-unit library with matching architecture/CRT. This audit did not inspect/build the fetched dependency sources, so it makes no claim about an already-working MSVC link.

## Classification and limits

`widecharwidth` and the MO parser have no direct OS machinery; many other crates contain portable algorithms but inherit Unix dependencies. `common`, `build-helper`, `tempfile`, `util`, `widestring` and xtask have concrete native-boundary work. Catalog/doc generators and extraction macros require **host** portability independent of target runtime work. All checks above are proposed validation, not exercised results; no build, tests, formatters or external tools were run.
