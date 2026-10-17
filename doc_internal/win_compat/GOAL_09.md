# GOAL_09 — Shipped assets, help, localization and user tools

**Phase:** N9. **Status: planned / not started**.
Primary native target: `x86_64-pc-windows-msvc`; ARM64 remains a separate extension.
All tests below are acceptance specifications, not existing runnable targets or passed tests.
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
