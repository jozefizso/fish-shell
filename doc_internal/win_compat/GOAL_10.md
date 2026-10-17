# GOAL_10 — Native release and installation

**Phase:** N10. **Status: planned / not started**.
Primary product target: `x86_64-pc-windows-msvc`, delivered first as a relocatable x64 ZIP.
ARM64 runtime support and additional installer channels are separate tracks.
The gates below are acceptance specifications, not existing tests or claimed passing releases.
This contract expands the approved roadmap at revision `2acbbad4b`.

## Objective

Produce an ordinary-user-installable native distribution containing all three working fish
executables and their required resources, dependencies, documentation and licenses.
Users must install, relocate, upgrade and remove it outside the source checkout without
build tools, a POSIX runtime or accidental developer-machine dependencies. This establishes
the artifact used for final product closure in [GOAL_11](GOAL_11.md), not a compile-only
release claim or an implication that every installer/signature channel already exists.

## Entry conditions and branch

- Verify [GOAL_09](GOAL_09.md) complete asset, catalog, help and user-tool behavior,
  including real default-feature builds and tested optional dependency boundaries.
- Carry forward all prior runtime/console/state gate evidence and the N1 supported
  Windows/filesystem/terminal/toolchain matrix; prerequisite plans alone do not qualify.
- Select and document the embedded/installed resource layout and supported feature profiles
  before packaging, using established runtime discovery rather than developer-source paths.
- Suggested branch: `platform/windows/native-release`; one integration owner coordinates
  manifests, host tools and release automation with existing owning services.

## Owning components and release invariants

- Own `Cargo.toml`, `Cargo.lock`, `build.rs`, `.cargo/`, `CMakeLists.txt`, `cmake/`,
  native `.github/` CI/release integration and applicable `build_tools/` entrypoints.
- Coordinate `src/bin/`, `src/autoload.rs`, `src/print_help.rs`, installed `share/`/`etc/`,
  `doc_src/`, localization inputs and executable version/resources metadata.
- All 21 direct-child workspace crates plus the root fish package require an explicit
  runtime/native-tool/supported-build-host destination; no crate disappears through packaging.
- Use [build/package analysis](15-build-packaging-documentation.md),
  [workspace inventory](14-workspace-crates.md), [assets](13-binaries-and-assets.md),
  [CI/testing](16-tests-benchmarks-ci.md) and [Windows APIs](windows-api-reference.md).
  Their historical audits do not prove current runtime or release support.
- Reuse the existing evaluator, native backends and resource services; do not add a second
  shell or rewrite platform services as a prerequisite to a packaging feature.

## Implementation delivery

1. Build a relocatable x64 ZIP with `fish.exe`, `fish_indent.exe`, `fish_key_reader.exe`,
   required libraries, licenses/notices and the selected embedded/installed assets/help.
   Actual `.exe` entrypoints must dispatch correctly; Unix hardlink/basename assumptions
   or three files that all accidentally enter the shell are not acceptable.
2. Enumerate native dependencies, including CRT deployment and PCRE2-32 library width,
   architecture, static/dynamic choice and redistribution requirements. Package allowed
   dependencies or document supported system prerequisites; no developer DLL search path.
   Exercise real regex and formatted/interactive consumers, not just dependency metadata.
3. Make version/build metadata reproducible for the declared inputs, including source
   archives/no-Git builds and supported native resource metadata. Record revision, profile,
   toolchain and build inputs; explain any intentional nondeterministic fields rather than
   claiming byte reproducibility from version-string equality alone.
4. Locate executables/assets/help relative to the selected installation or deliberate native
   state roots. Relocation must not retain absolute developer source/build paths. Another
   fish or formatter earlier on PATH must not capture help, web tools or internal helpers.
5. Publish supported default and minimal feature profiles, minimum/current Rust/SDK and
   supported Windows/terminal/filesystem requirements. Exercise requested features rather
   than silently stripping localization/manpages because build prerequisites are absent.
   Optional Python/Sphinx/gettext are tool/build dependencies, never mandatory shell startup.
6. Close all workspace tools' execution roles, including `build-helper`, `build-man-pages`,
   `gettext-extraction`, `fluent-extraction` and every `xtask` child. Translation, extraction,
   documentation and check orchestration need real native entrypoints or documented working
   supported build-host roles; native-required operations cannot delegate to `.sh` files.
   Test generated outputs and failure propagation, not tool imports or task registration.
7. Preserve explicit destinations for the other 16 crates: `common`, `widestring`, `util`,
   `tempfile`, `fallback`, `wcstringutil`, `wgetopt`, `feature-flags`, `printf`, `color`,
   `widecharwidth`, `fluent`, `localization`, `gettext`, `gettext-maps` and
   `gettext-mo-file-parser`. Runtime dependencies require native behavior evidence;
   calling them host-only does not excuse missing execution. Do not count a dependency
   declaration without a crate directory as an invented twenty-second workspace crate.
8. Provide a supported native build/check/release command path with correct host/target
   distinction and feature propagation. Retain intentional Unix release-host tooling and
   Unix/MSYS product gates separately; a Windows-host Linux guest is not native evidence.
9. Specify user-local extraction/install, version selection, upgrade, relocation and removal.
   Preserve config, history, universal variables and other user state through upgrade/removal;
   never treat a package directory as permission to delete independent user data.
   Handle denied/locked/corrupt/partial operations with truthful errors and no false success.
10. Supply explicit PATH and Windows Terminal registration instructions and optional actions.
    Do not silently rewrite profiles, switch login shells, edit `/etc/shells` analogues or
    overwrite unrelated terminal entries. Test the documented user-visible command sequence.
11. Label the initial ZIP unsigned unless authorized signing infrastructure actually exists.
    Authenticode publication requires authorized credentials, protected signing execution,
    timestamp/trust verification under the declared policy and an identified signed artifact.
    Never invent a certificate, bypass trust checks or include private credentials in packages.
    Missing optional signing credentials do not block a correctly labeled unsigned x64 ZIP;
    they do block any signed-release claim. MSI/MSIX/winget remain additional distribution work.
12. Keep ARM64 on `platform/windows/arm64` after x64 feature closure. Architecture-matched
    CRT/PCRE2/dependencies and real ARM64 Windows process/console/state/install runs are
    required before advertising support; cross-compilation and x64 emulation are not proof.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no faithful snapshot/return-twice emulation and no post-fork backend.
  Installed fish continues to use shared evaluation and explicit native process creation.
- **POSIX `exec`/`execve`:** no same-PID process-image replacement. Keep `exec` nonzero
  unsupported before argument side effects; spawn/wait/exit and CRT `_exec` are not substitutes.
- **POSIX `fg`/`bg`:** no terminal-pgroup foreground transfer or stop/continue implementation.
  Keep both unsupported; no `SuspendThread`/`ResumeThread` or window-focus substitution.
  Native `&`, `jobs`, `wait` and `disown` remain supported N5 behavior, not POSIX job control.
  No unsupported reserved command may fall back to a same-named external executable.

## Mandatory automated tests

| Exact gate ID | Required type | Observable setup, actions and assertions |
|---|---|---|
| **N10-INSTALL** | clean-machine e2e | On clean native Windows restricted-user fixtures, extract/install outside checkout into spaces/Unicode paths; run real scripting, prompt/console and optional-tool boundary workflows. Remove checkout/build-tool/POSIX paths and verify actual dependency loading. Relocate, upgrade from a prior supported package and remove using documented actions; verify user data by contents and a subsequent reinstall, with unrelated files/profile/terminal entries unchanged. Include locked/denied/partial/corrupt installation and upgrade failures with truthful status and preserved user state. |
| **N10-MATRIX** | native CI | Execute debug/release supported default/minimal feature profiles with minimum/current Rust across the declared Windows/terminal/filesystem matrix. Discover/run unit, real integration, native console, doctest and applicable workspace-tool cases; execute supported host-only tools on their declared hosts and inspect generated outputs. Required fixtures and IDs cannot silently skip. Preserve separate applicable Unix/MSYS gates and accurate host/target/artifact labels. Validate native orchestration propagates an actual failing tool/test case rather than returning successful CI after a failed stage. |
| **N10-ARTIFACT** | integration | Inspect real archive manifest, hashes, dependency imports, licenses/notices, version/resources, architecture and CRT/PCRE2 requirements; extract the same artifact and execute all binary/help entrypoints with a conflicting fish earlier on PATH. Assert source-independent startup, script/formatter/key-reader functionality, selected help/catalog resources and correct isolated user-state locations. Compare declared reproducible metadata/inputs across builds. Verify unsigned labeling, or actual signed-artifact trust when optional signing prerequisites are supplied; no cross-build-only architecture claim. |

## Verification and evidence requirements

- When suites land, publish exact case names/commands for these IDs and expected discovery,
  executed and nonignored counts. No speculative future Cargo target is a runnable claim.
  CI rejects missing IDs, zero-case runs, ignored mandatory cases and unexplained matrix gaps.
- Use native MSVC debug/release artifacts and runtime smoke from the extracted release ZIP;
  archive listing, source audit, mock launcher or successful compilation alone cannot pass.
  Retain relevant Unix suites for shared-code/build-host changes without conflating targets.
- Launch by argument arrays, isolate state and PATH, drain both streams concurrently and
  bound process/console/network waits. Cleanup terminates/reaps only owned children and
  removes only test-owned install roots; synchronize locks/failure races with events, not sleeps.
- Required clean-machine/restricted-account, filesystem, terminal and tool fixtures block
  closure if absent. Optional signing/installer/ARM64 tracks must be labeled separately,
  not skipped required work or an excuse to omit any x64 portable shell feature.
- Record revision, Rust/Cargo/SDK/host/target, profile, supported matrix cells, exact commands,
  case counts/results, package checksums/artifact locations, loaded dependency paths, resource
  manifest and user-state preservation evidence. Record real install/upgrade/relocation smoke
  separately from automated gates and any authorized publication/signature evidence.

## Definition of done and handoff

Every delivery and all three gates pass for the declared x64 product and feature profiles.
All 21 crates and shipped resources have working execution/deployment destinations; no
missing runtime dependency is relabeled tooling. Update build/install/release/help docs,
callers and CI registries; remove obsolete prototype paths, guards and packaging scaffolds.
Preserve positive native regressions, permanent and still-deferred rejection checks; replace
obsolete packaging/help rejection tests with real artifact/failure cases as support lands.
Keep historical [GOAL](GOAL.md) evidence unchanged and unsigned/signed support claims precise.
Hand the exact installable ZIP, matrix/evidence records and explicit optional distribution
boundaries to [GOAL_11](GOAL_11.md). N11 audits product closure; it does not implement forgotten
release functionality or convert audit-only evidence into a completed release gate.
