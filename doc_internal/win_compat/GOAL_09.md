# GOAL_09 — Shipped assets, help, localization and user tools

**Phase:** N9. **Status: substantial implementation integrated; asset coverage and native host acceptance remain incomplete**.
Primary native target: `x86_64-pc-windows-msvc`; ARM64 remains a separate extension.
The three gates are runnable; individual passing cases do not close their failed aggregate.
This execution contract expands the approved roadmap at revision `2acbbad4b`.

## Objective

Deliver the whole shipped fish experience, not only a working Rust executable: functions,
completions, prompts, themes, startup assets, native help, translations and optional user tools.
A clean native installation must start/edit normally without Unix utilities, Python, Sphinx
or gettext installed. Optional tools must either work with their tested dependencies or
report those dependencies honestly. This phase enables release packaging in
[GOAL_10](GOAL_10.md); inventory/parse success alone cannot establish asset compatibility.

## Entry conditions and branch

- Verify [GOAL_07](GOAL_07.md) completion/editor/rendering behavior and
  [GOAL_08](GOAL_08.md) secure script providers, native helpers and descriptor adapters.
- Verify N4 storage/startup roots, precedence and persistent-state services; reuse them.
- Preserve the required editor/bootstrap assets already delivered by [GOAL_06](GOAL_06.md).
  They were necessary for usable interactivity and must not be postponed to this phase.
- Prerequisite plans in the checkout are not evidence of implemented prerequisites.
- Suggested branch: `platform/windows/assets-localization`; coordinate shared native
  services without a second evaluator or wholesale platform abstraction rewrite.

## Owning components and invariants

- Own every family under `share/` and `etc/`, including `share/config.fish`, embedded
  functions/completions, prompts/themes, `share/help_sections` and executable Python helpers.
- Reuse `src/autoload.rs` embedded extraction/discovery, `src/print_help.rs`,
  `src/localization/`, `src/locale.rs` and the executable/builtin help entrypoints.
- Runtime catalog owners: `crates/fluent`, `crates/localization`, `crates/gettext`,
  `crates/gettext-maps`, `crates/gettext-mo-file-parser`; include `crates/printf` locale tests.
- Generation owners: `crates/build-helper`, `crates/build-man-pages`,
  `crates/gettext-extraction`, `crates/fluent-extraction`, `crates/xtask` and `doc_src/`.
- Use [asset analysis](13-binaries-and-assets.md),
  [text/localization](11-text-regex-localization.md), [workspace crates](14-workspace-crates.md)
  and [build/docs](15-build-packaging-documentation.md) as rationale, not support evidence.
- Keep fish list/quoting semantics and native drive/UNC identities; never split drive names
  using Unix colon-list logic or confuse absolute disk paths with relative embedded assets.

## Implementation delivery

1. Refresh a file-level shipped-asset ledger, including embedded/generated/extracted data.
   Assign each asset native-tested, optional-with-tested-dependency, or genuinely
   platform-inapplicable with a concrete reason. Every applicable dynamic path needs a
   behavior case; neither extension `.fish` nor parse/load success proves portability.
   Ordinary missing portable functionality cannot be classified away as inapplicable.
2. Close startup, default prompts, bindings, vendor/generated completion precedence,
   navigation/directory/history helpers and prompt/theme selection. Preserve first-run,
   private/no-config and interactive/noninteractive semantics and native path list handling.
   Remove mandatory `mkdir`, `rm`, `cat`, `sleep` and `/bin/sh` dependencies via existing
   native services or fish logic; do not merely ignore their failed startup invocations.
3. Exercise all VCS prompts and optional external integrations with real tested dependencies
   and with those dependencies absent. Preserve status/pipeline information through prompt
   probes, native repository paths and Unicode output; core prompts must work without VCS.
   Keep portable/static completion tables and gate only genuinely unavailable dynamic probes.
4. Deliver clipboard copy/paste, browser/open actions, extraction/temp/cache helpers,
   migrations and `funcsave`/`funced`. Specify Unicode/newline behavior, editor argv,
   private temporary lifetime, denied writes and interrupted-save cleanup. Use native service
   paths rather than shell strings; user editor conventions are not interchangeable.
5. Adapt process/account/network and Windows-oriented completion facades to N8 services.
   Do not assume WMIC or `awk`/`tail` exists. Exercise generated completion execution and
   child-specific descriptor protocols, with precedence/cache invalidation and useful errors.
   Retain truthful `exec`/`fg`/`bg`/`suspend` and unavailable signal-wrapper diagnostics;
   unsupported reserved operations cannot dispatch a same-named unrelated executable.
6. Restore native locale discovery, explicit environment/`status language` precedence,
   Fluent/gettext catalogs, fallback and Unicode formatting. Keep C numeric parsing where
   language syntax requires it; localized display does not change arithmetic grammar.
   Isolate mutable locale/cache state and test invalidation without process-global races.
7. Build real default-feature and supported minimal configurations. A missing `msgfmt`
   warning producing empty maps is not a localized release: require real catalog inputs
   and verify selected translations. Feature-disabled English fallback is an explicitly
   selected profile, never a silent downgrade of requested localization or help features.
8. Port native Python interpreter discovery/version checks and user tools, including
   `share/tools/web_config/webconfig.py` and `share/tools/create_manpage_completions.py`.
   Retain/adapt existing Windows branches, choose the matching native child fish/formatter,
   and honor supported Python floors rather than assuming all Python 3 versions work.
   Python/Sphinx/gettext prerequisites belong to documented tool/build features, not startup.
9. Run actual web configuration edits with browser launching, loopback peer checks and
   authenticated operations intact. Reject unauthenticated/nonloopback requests; never
   weaken access controls for connectivity. Own redirect/temp files, listening sockets,
   worker threads and child processes across normal shutdown, browser failure and cancel.
10. Generate completions in real native foreground/background workflows. Discover explicit
    native manpage inputs, handle absent/corrupt/plain/compressed pages and native cache paths;
    do not imply Win32 applications universally install Unix manual inventories. Reuse N5
    background ownership and cleanup rather than delegating detachment to `/bin/sh`.
11. Close generation/extraction tools with real outputs: native temp paths and concurrent
    extraction, Fluent/gettext maintenance, Sphinx HTML/man generation and host formatter
    selection. Distinguish host executables from cross-built targets; record supported host
    roles for final N10 release closure, without claiming runtime crates are host-only.
12. Make `fish`, `fish_indent` and `fish_key_reader` `-h/--help`, plus builtin/function help,
    select their matching installation and version even with another fish first on PATH.
    Provide usable packaged help without `man`, `less` or `/bin/sh`; test missing help data,
    Unicode paths, section/fragment links and supported embedded/installed configurations.

## Integrated automation and observed boundaries

`tests/windows_n9.rs` now registers 89 asset families, five user-tool families, and
profile-specific locale/help consumers. The file-level ledger contains 1,426
shipped files. Its current snapshot still has 747 rows with explicit `REQUIRED:`
obligations; static option checks do not certify those dynamic paths. Nine builtin
completion rows now name their real reader/parser/dispatch/job consumers instead
of treating fish builtins as missing optional executables. Genuine Ruff and both
GnuPG constructor branches have concrete consumer owners; exact `gpg1`/`gpg2`
basenames and remaining partial-tool contexts are not silently closed.

The latest completed 89-family minimal-feature debug/release asset run is retained
at `target/n7-gate-results/n9-89-assets-and-full-gates-real-gnu-consumers.txt`.
Its static constructors report no failures; both actual inventory sections still
reject 747 required paths. The repaired command wrapper/query boundary and the
independent `core-native-git-repository-predicate-and-absence` family pass both
profiles. The legacy VCS aggregate's Git subconsumer also passes; its remaining
failures are the genuine Mercurial/Subversion requirements, not a concealed Git
failure. Genuine Git-operation, theme-event and SSH identity consumers now pass
both profiles in `n9-native-git-ssh-theme-and-quoted-command-after.txt`. Historical
Git layouts remain explicitly required; no fabricated operation markers close
them. The msgfmt constructor consumer also passes in this aggregate; its newly
exposed filename/language-mode requirements are not hidden by static-only approval.

| Gate | Passed / executed | Scope |
|---|---:|---|
| N9-ASSETS | 166 / 178 | All 89 registered families executed, zero ignored; six failed aggregates in both profiles; genuine Git operation/theme/SSH, three archive and msgfmt consumers pass |
| N9-TOOLS | 8 / 10 | Same full-suite run; live authenticated browser workflow remains blocked, including an inadvertent repeat of the stopped browser case |
| N9-LOCALE / default | 19 / 19 per toolchain | Fresh Unicode catalog/target roots in both current/MSRV `n7-*-wrapped-metadata-native-gettext-io.txt` logs; actual catalogs and embedded help |
| N9-LOCALE / minimal | 21 / 21 per toolchain | Fresh current/MSRV English fallback and installed-help runs; current locale follow-up recorded separately after its owned executable became available |

The genuine Clang family consumes selected C17/C++20 flags and a Unicode object,
links with matching first-party `lld-link` and the declared SDK, and runs the
resulting native executable. The genuine MySQL family queries an owned loopback
server, verifies its datadir before SQL mutations, compares actual schema records,
and consumes Unicode database selections with real restricted SQL credentials.
Both families passed in both profiles in the completed asset runs.

MySQL26.7 requires declared ASCII server storage on this host: independent wide
initialization rejects Unicode datadirs, and its default self-launching monitor
fails from the Unicode cwd with error267. The fixture uses the documented
[`--no-monitor` single-process mode](https://dev.mysql.com/doc/mysql-windows-excerpt/8.0/en/windows-installation.html),
keeps client/shell cwd and SQL data Unicode, and owns only its retained server.
This does not advertise Unicode filesystem support for that third-party server.
GPG agent/keyring, PostgreSQL cluster, and SVN repository fixtures likewise retain
independent wide-launch limitations and explicitly declared short ASCII service
storage. Genuine GPG crypto and SVN MIME/property readback passed both profiles.
PostgreSQL now also passes: raw Unicode SQL/connection-name argv independently
loses characters in this client build. UTF-8 SQL stdin creates real Unicode
database/role data; completion-selected names reach the genuine client through its
percent-encoded libpq URI interface. Readback checks actual database, login user,
payload and UTF-8 storage bytes. This is not raw-Unicode-argv support.

Native locale-value completion uses the existing NLS owner instead of Unix
`locale -a`; its selected-value numeric-format consumer passed both profiles.
Required option values beginning with `-` now retain value completion instead of
becoming new switches. Literal abbreviation rename, modified F3 dispatch, logical
token completion, function-name discovery and exact named-asset lookup have
current native and retained Unix evidence recorded in [GOAL_07](GOAL_07.md).
No interaction timeout was increased and no required journey was skipped.

The `command` metadata repair delegates dash-prefixed child switches only when
the decorator is actually wrapping a child; leading builtin query flags retain
their own namespace. Actual consumers select `--command` and `--no-config`
through normal/explicit-`--` contexts, execute a genuine native child, and resolve
an actual query target. Evidence is
`n9-genuine-child-nested-command-diagnostic.txt`,
`n9-builtin-nested-command-and-query-modes-after.txt` and the separate live Unix
child-consumer record `unix-command-delegation-smoke.json`.

Native ImageMagick enum/list consumers and the genuine legacy executable role
have real pixel/metadata consumers, but the full enum family is not passing.
Independent ImageMagick7.1.2-34 rejects its advertised `-stretch` with both
input-first (`unrecognized option`) and setting-first (`no images found`) ordering.
A different drawing API is not a substitute; its required consumer remains strict.
This publisher build also excludes the deprecated `magick convert` suffix.
That obsolete positive assertion and unreachable two-word command key were
removed; legacy-role coverage uses the real `convert.exe` basename dispatcher.
Its selected PNG/red/negate operation and System32 exclusion passed both profiles
in the completed 79-family run. The unrelated filesystem converter never receives
ImageMagick metadata.

Genuine Ruff0.9.10/0.17.1 consumers select generated rule/ignore arguments and
observe real Unicode source diagnostics and fixes. A same-path executable upgrade
exposes the new `--add-ignore` consumer without a cache flush or sleeps. Denied
cache writes exercise the real fallback. Genuine Ruff0.0.65 cannot generate fish
completions and emits failure diagnostics on stdout; its producer status is now
checked before that output can be sourced. True absence remains quiet, not a
fabricated namespace. The three Ruff families passed both profiles in the completed
run. The rumdl consumer likewise selects actual generated rule data and observes
real markdown fixes, but its remaining generator/context obligations stay explicit.

The four Typst families passed both profiles: completion-selected rendering
compares actual decoded RGB pages, query JSON/YAML is loaded semantically by the
real tool, an owned Unicode font directory changes font discovery with system and
embedded fonts disabled, and an owned watch child rebuilds blue pixels to green
after a real Unicode source edit. This does not close its remaining alias,
subcommand, format and option contexts. Font bytes are copied read-only from the
existing host font; no font is installed or registered.

Official Nmap Ncat7.992 and GnuPG1.4.23 payloads were extracted without executing
their installers. Both are genuine I386 Windows images, not AMD64/POSIX producers.
Download hashes/imports and capability observations are retained under
`target/native-build-tools/ncat-7.992/` and `gnupg-1.4.23/`; publisher signatures
were not verified. Ncat consumers transfer exact Unicode/NUL and CRLF bytes over
owned IPv4/IPv6 loopback sockets, consume source-address/port arguments, and
exercise duplex listen and connect-mode receive-only operation. Accepted fixture
sockets explicitly return to blocking mode before bounded transfers; Windows
inherits their listener's nonblocking mode. The genuine listen+receive-only
combination exits before accept and is not presented as a passing combination.
Hostname/TLS/proxy/transport/flavor contexts remain required.

The legacy GnuPG consumer imports a genuine modern-generated RSA2048 public key
with a Unicode/colon UID into its exclusively owned legacy home. Completion-picked
UID/key values verify and extract real signed Unicode/NUL data, change the
observed trust result, and reject tampering. Its selected legacy-only `--rfc1991`
literal packet has observably different framing from the default; no weak
encrypted mode is substituted. Default legacy colon listings merge the primary
UID into `pub`; the key-ID helper now requests documented `--fixed-list-mode`
records. The failing-before/passing-after evidence is
`n9-ncat-gpg1-key-record-before.txt` / `n9-ncat-gpg1-key-record-after.txt`.
Short ASCII service storage is declared, not Unicode-home support: independent
legacy Unicode-home access is lossy and native generation/default encryption hit
the unchanged deadline. Modern key preparation, scoped agent shutdown and
removal of only the owned UUID homes preserve the private-key boundary.

Genuine Jujutsu and Fossil subconsumers now pass independently. Jujutsu's direct
319-UTF-16-unit index path fails while its 204-unit Unicode counterpart works;
its supported fixture uses short Unicode storage, not an ASCII waiver. Fossil's
documented `--keep` opening mode preserves the actual owned payload. Darcs still
fails Unicode lock/add, and SVN still fails Unicode repository/cwd acceptance.
Their genuine supported ASCII-data profiles do not close those required failures.

Official Mercurial7.1.2 and the genuine PyPI7.2.4 launcher independently reject
Unicode repository cwd/filenames after lossy MBCS conversion, even with explicit
UTF-8 input/output settings. The shipped code deliberately enables legacy Windows
filesystem encoding. Official MSI payload reconstruction used read-only tables,
not installation; original manifests/binaries were not modified. Evidence is
under `target/native-build-tools/mercurial-official-7.1.2/`. ASCII success does not
close the required Unicode repository consumers.

The actual Edge launch rejected DevTools remote debugging despite an explicit
owned nondefault user-data directory. No alternate flags, transport, browser or
security settings were used to bypass it. The original error is retained in
`n9-tools-sandboxed-guest-launch-and-original-error.txt`; there is no successful
browser visual proof. An independent allowed lifetime smoke passed real server
shutdown/cancel/failure paths and owned-junction cleanup, including refusal of an
outside target, in `n9-web-lifetime-owned-junction-security-smoke.txt`. That does
not establish the blocked authenticated browser edit workflow.

The final SDK invocation selected the whole N9 suite rather than filtering to
`n9_assets`, inadvertently repeating the stopped browser case in both profiles.
Both again failed with the same DevTools rejection and launch timeout. These are
failed executions, not successful host acceptance or authorization to bypass the
rejection. No further browser launches will be attempted without a documented
resolution from the host owner.

The repeated-option Python consumer remains blocked by actual host capability,
not a reduced command: official hash-verified CPython3.11.9,3.12.10,3.13.9,3.13.16,
3.15.0 and the existing3.14.7 crash under unchanged `-X utf8 -X dev`, including
clean `-S -c pass`. Correct Unicode output precedes native0xC0000005/debug-allocator
corruption; one3.11 clean repetition returned0xC0000409. The3.11/3.12 payloads
are the latest published Windows binaries in those families, not their latest
source-only security releases. Provisioning and loaded-module evidence is retained under
`target/native-build-tools/cpython-3.13.16/`; ESET's `ebehmoni.dll` was
observed inside the clean process. A host-instrumentation interaction is
**inference, not proven causality**. No security tooling, policies, allocator
settings or required flags were changed to manufacture a passing fixture.

### Native optional-consumer fixture inputs

These are real publisher executables/data, not shim names or canned namespaces.
Missing required fixtures fail their registered consumers instead of skipping
them. Current retained evidence uses these roles:

| Environment input | Genuine fixture role |
|---|---|
| `FISH_N9_RUFF`, `FISH_N9_RUFF_PREVIOUS`, `FISH_N9_RUFF_LEGACY` | Current0.17.1, previous0.9.10 same-path upgrade, and0.0.65 present-but-unsupported generator |
| `FISH_N9_RUMDL` | Native0.3.0 generated-completion/markdown consumer |
| `FISH_N9_TYPST` | Native0.15.1 render/query/font/watch consumers |
| `FISH_N9_TYPST_FONT` | Existing genuine Segoe UI font copied only into an owned Unicode font directory |
| `FISH_N9_NCAT` | Native Nmap7.992 `ncat.exe`, not an `nc` flavor alias |
| `FISH_N9_GPG`, `FISH_N9_GPG_LEGACY` | Modern native `gpg.exe` with sibling `gpgconf.exe`, and publisher1.4.23 `gpg.exe`; no invented `gpg1`/`gpg2` executable |
| `FISH_N9_OPTIONAL_WORK_ROOT` | Explicit writable short ASCII service-storage parent, currently `D:/fn9`; each GnuPG fixture owns/removes only its marked UUID child |
| `FISH_N9_7Z`, `FISH_N9_7ZA`, `FISH_N9_7ZR` | Installed genuine full 7z, publisher26.04 AMD64 `7za.exe` and actual I386 `7zr.exe`; no renamed full-tool aliases; publisher digests match retained downloads, signatures remain unverified |

The strengthened legacy UID fixture contains two colons and Unicode. Its actual
completion-selected signing/verification consumer also passed both profiles in
`n9-gpg-multiple-colon-uid-boundary.txt`; no speculative user-ID decoder change
was necessary.

### Native build-host tooling

The unmodified native workspace library run failed before executing tests:
`fluent-ftl-tools` revision
[`17be1e6`](https://github.com/danielrainer/fluent-ftl-tools/commit/17be1e678558d776f2bde068eacedf5fa79bf6fb)
unconditionally used `nix::unistd::mkstemp`. Upstream and the published fork
still contain that boundary. The pinned library implementation is now retained
under `vendor/fluent-ftl-tools`, outside fish's own workspace-member inventory.
Windows allocation reuses `fish-tempfile` exclusive creation and closes the
reservation handle; Unix retains its existing `mkstemp` implementation. Upstream
library behavior and multi-file recovery files remain intact. The GPL-2.0
redistribution option and full license are retained; native package source-input
inventory includes this dependency rather than losing the local source identity.

Actual native CLI consumers found and fixed failed-replacement temporary leakage:
an owned destination opened without delete sharing rejects the rewrite, preserves
its original bytes, and leaves no temporary file. A Unicode rewrite is parsed and
rendered by real Fluent and then passes the formatter's check mode. Both permanent
consumers pass in `native-fluent-rewrite-and-denied-replacement-after.txt`.
The same two cases pass with native Rust1.85.0, including exact discovery and an
empty ignored-case listing. Native `Check` now enforces both names, discovery,
zero ignored cases and actual execution; the management script itself remains
unexecuted after the N10 authorization stop. The retained Unix maintainer CLI
also passes its real Unicode rewrite/render consumer, not just compilation:
`unix-fluent-rewrite-consumer-after.txt`.


Two independent native compiler consumers concurrently extract into an owned
Unicode directory, then a native Fluent runtime renders both extracted variable
messages. This isolated build also exposed the proc-macro dependency's missing
Syn feature, previously masked by workspace feature unification; the extraction
owner now declares that feature explicitly. Actual output is retained in
`native-fluent-concurrent-extraction-and-render-after.txt`.

Native `xtask` HTML generation also previously failed to compile its Unix-only
symlink call. It now builds a formatter for the platform executing `xtask`, uses
its actual target directory and executable suffix, and passes the canonical
host path directly to the documentation lexer. Full native Sphinx HTML generation
succeeded; generated Introduction/Tutorial pages contain actual fish syntax
highlighting. A separate Unicode token consumer preserves all text/scalar offsets,
and selecting the genuine fish main executable for the formatter role returns
its real error instead of silently losing highlighting. No browser visual proof,
installed-artifact identity or N11 performance acceptance is implied.
Complete native build output is retained in
`native-xtask-html-matching-host-formatter.txt`; the explicit formatter token,
wrong-role failure and extraction observations are recorded in
`native-fluent-host-consumer-observations.json`.

Two native Rust1.85 compiler consumers also concurrently produce real gettext
records under a Unicode directory. Actual GNU `msguniq` exposed a valid CRLF
header rejected by the fixed LF byte-prefix reader; logical line validation now
accepts either ending without changing escaped message data or copying the body.
Real native GNU filename argv then rejected Unicode PO paths. Rust now opens
those paths and transfers native file handles to `msgmerge`/`msgattrib`/`msgfmt`;
the template uses its generated ASCII basename relative to its real Unicode
temporary directory. This is not an ASCII storage waiver or a claim that the
publisher tools gained wide-argv support.

Actual read-only obsolete-row diff, byte preservation, update and post-update
check succeed. Real little/big-endian GNU MO catalogs retain Unicode and format
translations consumed by the native parser. The permanent CLI regression also
checks escaped-newline translations and runs on current/MSRV native toolchains
and retained Unix with real GNU tools. Native `Check` requires its exact case,
discovery, nonignored execution and the malformed-header library regression.
Evidence: `native-gettext-host-observations.json` and
`unix-maintainer-cli-real-gnu-after.txt`. Package/management and installed N11
acceptance remain blocked independently.

The corrected `msgfmt --check-header` completion drives real catalog compilation
and `msgunfmt` Unicode/format-string roundtrips, incompatible-format rejection and
missing-provider behavior. The repository PO validator now supplies native
stdin/stdout handles and constructs its repository parent through `path dirname`,
without invalid `..` components in a verbatim Windows path. Both debug/release
profiles pass the semantic completion consumer and actual checked-in PO validation
(four executions, zero ignored). Evidence:
`n9-msgfmt-header-completion-after.txt` and `native-po-caller-stdin-after.txt`.
Dynamic filename and language-mode completion requirements remain open.



## Permanently out-of-scope contracts

- **POSIX `fork()`:** no faithful snapshot/return-twice emulation or post-fork backend;
  shared language evaluation and explicit native children remain the implementation.
- **POSIX `exec`/`execve`:** no same-PID replacement. The `exec` decorator remains
  nonzero unsupported before argument side effects; spawn/wait/exit and CRT `_exec`
  are not equivalent implementations.
- **POSIX `fg`/`bg`:** no controlling-terminal pgroup stop/continue semantics. Keep both
  commands unsupported, without `SuspendThread`/`ResumeThread` or window-focus substitutes.
  N5 native `&`, `jobs`, `wait` and `disown` remain supported, not excluded background work.
  Unsupported reserved commands never fall through to same-named external programs.

## Mandatory automated tests

| Exact gate ID | Required type | Observable setup, actions and assertions |
|---|---|---|
| **N9-ASSETS** | integration/e2e | Reconcile every shipped file to its disposition and executable coverage. Start native fish outside the checkout on a clean native PATH; observe complete startup/default prompts/bindings and parse/load every applicable definition, then execute critical dynamic paths. Exercise navigation/history, VCS-present/absent prompts, clipboard/open/editor actions, extraction/cache/migration and process wrappers with real files/tools. Assert results, state, diagnostics and cleanup; reject parse-only coverage and untested optional dependencies. |
| **N9-TOOLS** | integration/e2e | Use actual native Python and live web service/browser automation to edit/save/reload themes and functions in isolated roots. Verify on-disk state by a new fish process, generated-completion precedence and background cleanup. Feed valid/compressed/absent/corrupt manpages; test supported/unsupported Python versions and no interpreter. Make real authenticated, invalid-auth and nonloopback requests; prove rejected edits cannot alter state. Assert sockets/threads/children/temp files are released on normal, failed and cancelled shutdown. Exercise generation/extraction outputs rather than only importing tools. |
| **N9-LOCALE** | unit/e2e | Test actual locale/catalog/formatting owners with non-English translations, missing keys/variables, explicit precedence, fallback and decimal/grouping locales while C numeric syntax stays stable. Run native debug/release default/minimal artifacts and compare requested versus actual catalog features. Run all three binary help entrypoints and builtin help with a different fish earlier on PATH; assert matching version/content, Unicode paths/help and working local sections without Unix tools. Required translations must not silently become empty-catalog success. |

## Verification and evidence record

- Add exact implementation case names/commands when suites land; these IDs do not claim
  currently available future Cargo targets. Discovery must find all three required IDs
  with expected executed/nonignored counts; missing, empty or hidden-skipped gates fail CI.
- Run actual MSVC debug/release binaries and supported feature profiles, owning unit tests,
  real filesystem/process integration and native console/ConPTY editor scenarios.
  Preserve applicable Unix suites; keep Unix, MSYS and native evidence distinct.
- Launch by argument arrays with isolated config/data/cache/temp roots and controlled PATH.
  Serialize/isolate global locale and clipboard changes; restore external clipboard state.
  Bound waits and drain stdout/stderr concurrently; cancel/drain workers and reap only owned
  children. Use barriers/events, not sleeps, to prove concurrent extraction and cleanup races.
- Required browser, Python, catalog, network-security and console fixtures must be provisioned;
  their absence blocks closure. Optional dependency absence is itself a tested behavior,
  not permission to skip the required present-dependency case.
- Record revision, toolchain/SDK/target, artifact/profile, exact commands/cases, inventory
  totals/dispositions, expected and executed counts, results/logs, dependency versions and
  generated-output references. Record real first-prompt/help/web-edit runtime smoke separately.

## Definition of done and handoff

Every delivery and required gate passes; the asset ledger has no undisposed entries or
parse-only claims. Update owning docs, executable help/options, supported dependency floors,
callers and CI registry counts. Preserve positive prototype and permanent/still-deferred
rejection tests; replace superseded asset/help/localization rejection tests with real
positive/failure coverage. Keep historical [GOAL](GOAL.md) evidence unchanged.
Remove obsolete platform guards, Unix-only mandatory helper paths and temporary scaffolds.
Hand the complete asset/catalog/help layout, tool host-role evidence and dependency/license
requirements to [GOAL_10](GOAL_10.md), not a list of forgotten portable features.
ARM64 and additional installer/signing infrastructure remain separate from this x64 scope.
