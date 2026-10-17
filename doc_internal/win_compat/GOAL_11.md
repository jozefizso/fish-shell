# GOAL_11 — Compatibility and product-quality closure

**Phase:** N11. **Status: planned / not started**.
Primary product target: `x86_64-pc-windows-msvc`; ARM64 requires its separate runtime track.
The following gates are future acceptance specifications, not existing test targets or results.
This contract expands the approved roadmap at revision `2acbbad4b`.

## Objective

Close the native product against the complete portable language, builtin/option/CLI, crate
and shipped-asset inventories, and establish measured reliability, security, performance and
accessible daily use. Release audit is necessary but cannot replace working behavior or
meaningful automated quality/stress fixtures. This phase reconciles implementation already
owned by N1–N10; it is not a catch-all owner for forgotten features or a way to call a
compiling subset complete by adding a disposition spreadsheet.

## Entry conditions and branch

- Verify [GOAL_10](GOAL_10.md) native installable ZIP, supported profiles and matrix evidence.
  Use its actual artifact, not a developer binary with extra dependencies on PATH.
- Reconcile all prior phase evidence and outstanding requirements; a prerequisite planning
  file, registry entry or original source audit is not proof of implementation.
- Obtain recorded N1/N6 baselines and adopt numeric quality budgets before optimization.
  Missing baselines/budgets are unresolved prerequisites, not grounds for post-hoc thresholds.
- Suggested branch: `platform/windows/parity-closure`; route implementation gaps to their
  owning component/phase and integrate real fixes before claiming final closure.

## Owners and reconciliation invariants

- Own final coverage ledgers, `tests/`, native test helpers/console transport, `benchmarks/`,
  native `.github/` quality/release policy and user-facing compatibility/security documentation.
- Exercise production owners in `src/`, all `crates/`, `share/`, `etc/` and packaged assets;
  do not create another shell, evaluator or wholesale platform-framework rewrite.
- Use [builtin inventory](10-builtins.md), [workspace inventory](14-workspace-crates.md),
  [assets](13-binaries-and-assets.md), [build/package](15-build-packaging-documentation.md),
  [tests/benchmarks](16-tests-benchmarks-ci.md), [coverage](coverage.md) and
  [Windows APIs](windows-api-reference.md). Original analyses provide rationale, not passing tests.
- Every original component analysis 01–16 must map to verified native/shared ownership or a
  justified semantic difference, including newly introduced native modules absent from that audit.
- Portable omissions cannot be relabeled platform-inapplicable; real optional integrations need
  tested present/absent dependencies, not a blanket Windows skip. Registration is not execution.

## Implementation delivery

1. Reconcile every builtin, subcommand, option and alias against actual behavior cases,
   including `string`, `path`, `test`, `status`, native platform options and help/localization.
   Record supported inputs/results, diagnostic/failure cases, semantic differences and owning
   implementation/tests. Fix uncovered behavior at its established owner; no successful stubs.
2. Close the executable parser ledger, not only the builtin registry. Exercise fish
   `-c/--command` (including repeated commands), script/stdin/argv/`--`, `-n/--no-execute`,
   `-f/--features`, `-v/--version`, `-d/--debug`, `-o/--debug-output`,
   `--print-debug-categories` and `-p/--profile` through real outputs/status/sinks.
   Reconcile legacy `-D/--debug-stack-frames` as explicitly retired/unavailable or genuinely
   implemented; upstream ignored behavior must not be advertised as working stack debugging.
3. Recheck `-C/--init-command`, `-l/--login`, `-N/--no-config`, `-P/--private`,
   `--profile-startup`, `--print-rusage-self` and `-i/--interactive`/implicit interactivity.
   Assert config→repeated-init-command→execution ordering; `-N` still runs init commands,
   private/no-config history rules, native accounting units and terminal/nonterminal selection.
   Include invalid options/missing arguments, parse-only side-effect suppression and stable status.
   Inventory every formatter/key-reader flag and all three binaries' `-h/--help`; retain actual
   file/stream/console semantics and matching installed help with a competing fish on PATH.
   N3/N4/N5/N6/N9 remain these behaviors' implementation owners, not deferred N11 inventions.
4. Reconcile the root package and all 21 current workspace crate directories below. Each
   row requires native runtime regression or a working supported build-host execution path,
   artifact/dependency/feature coverage and exact executed-case evidence; a host-only label
   cannot excuse runtime dependencies. Refresh the inventory if crate membership changes.

| Crate directories | Existing implementation destinations to verify |
|---|---|
| `common`, `widestring`, `util`, `tempfile`, `fallback`, `wcstringutil`, `wgetopt`, `feature-flags` | N1–N3 shared/native boundaries; complete retained regression under N11. |
| `printf` | N3 formatting/streams and N9 locale behavior. |
| `color`, `widecharwidth` | N6/N7 actual terminal rendering and character-cell behavior. |
| `fluent`, `localization`, `gettext`, `gettext-maps`, `gettext-mo-file-parser` | N9 real catalogs/runtime fallback and N10 deployment. |
| `build-helper`, `build-man-pages`, `gettext-extraction`, `fluent-extraction`, `xtask` | N9 generation/native tools and N10 supported build/release-host execution. |

5. Reconcile every shipped function/completion/prompt/theme/startup/help/Python/web asset,
   including embedded/generated/extracted forms. Retain N9 file-level dispositions and actual
   dynamic execution; do not replace them with counts or parse-only evidence. Confirm package
   resources and every original analysis destination against the released N10 artifact.
6. Migrate all applicable Unix language cases to a native direct-process harness while
   preserving behavior and the original Unix suite. Separate host RUN directives from fish
   language semantics; no hidden `/bin/sh` execution. Translate meaningful pexpect/tmux
   contracts into actual console/ConPTY input, output, editor and screen assertions.
   Audit every platform skip with reason/owner/alternate coverage; blanket skipped files,
   unexplained disabled options, undiscovered cases and empty suites block release.
7. Add sustained concurrent jobs, edits, completion, history/universal saves and notifications.
   Inject launch/I/O/state failures, cancellation, overflow, console/network disconnect,
   denied resources, invalid inputs and abrupt owned-child exits through actual OS operations.
   Synchronize race windows; verify recovery, state integrity, bounded shutdown and stable
   process/thread/handle/memory behavior after quiescence, not just a lack of exceptions.
8. Review and test interpreter injection, inherited-handle disclosure, configuration trust,
   native state/temp ACLs and cross-installation child/resource confusion. Exercise malicious
   argument/config/path fixtures, blocked effects and legitimate allowed workflows. Preserve
   disown survival and cleanup boundaries; never kill the host's enclosing Job Object.
9. Benchmark actual startup, native child launch, large pipelines/globs, history merges and
   edit/completion latency. Record hardware, Windows/build/profile, filesystem, input/tool
   inventory, antivirus/indexing context, sample method and output sink. Adopt numeric
   budgets from recorded N1/N6 baselines before optimization and retain raw measurements.
   A discarded pipe sink and rendered ConPTY are different workloads, not favorable substitutes.
10. Automate clean-install daily keyboard journeys: start, navigate, edit Unicode, complete,
    page/search history, launch/cancel tools, save settings, restart and verify state/help.
    Assert actual editor/screen/result/state observations with recorded fixture parameters.
    Add named keyboard-only and screen-reader signoff journeys on supported terminal and
    assistive-tool/version combinations, including prompt/edit/pager announcements, Unicode
    navigation and color-independent feedback. Human signoff supplements, not replaces,
    automated quality fixtures; automated screen capture does not prove screen-reader output.
11. Publish the completed semantic-difference/support ledger, known observed limitations,
    release security decisions and exact coverage/evidence. No universal UIA/accessibility
    or POSIX parity claim follows from one host. Retain explicit optional dependency,
    unsigned/signing-credential and separate ARM64/distribution boundaries from N10.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no snapshot/return-twice emulation and no post-fork backend.
  Native explicit creation and shared evaluator execution are not faithful fork semantics.
- **POSIX `exec`/`execve`:** no same-PID replacement. Keep `exec` nonzero unsupported
  before argument side effects; spawn/wait/exit and CRT `_exec` are not replacements.
- **POSIX `fg`/`bg`:** no terminal-pgroup foreground transfer or stop/continue semantics.
  Keep both unsupported; no `SuspendThread`/`ResumeThread` or window-focus substitution.
  Native `&`, `jobs`, `wait` and `disown` remain valid N5 contracts, not excluded background
  execution. Unsupported reserved commands never dispatch same-named external programs.

## Mandatory automated tests and assisted gates

| Exact gate ID | Required type | Observable setup, actions and assertions |
|---|---|---|
| **N11-COVERAGE** | release audit | Reconcile discovered and actually executed unit/integration/e2e/CLI cases with every portable feature, option/alias, crate, asset and analysis destination. Inspect native direct-process and console equivalents alongside retained Unix cases. Require nonempty executed evidence, useful failure assertions, justified skips and the published semantic-difference ledger; no unassigned/disabled supported capability. The audit consumes passing real suites and blocks on gaps; it cannot replace behavior tests with source searches or a green inventory count. |
| **N11-FAULTS** | integration/e2e | Run synchronized sustained concurrent jobs/edits/saves against real native fish/processes/files/ConPTY; inject process, pipe/I/O/state, overflow, disconnect, denied-resource and invalid-input failures. Assert output/status, durable merge integrity, responsive recovery, bounded memory/shutdown and stable handle/thread counts across repeated cycles. Verify all owned nondisowned children exit, promised disowned children survive and unrelated/host processes remain untouched; include injection and inherited-handle/state-ACL security fixtures. |
| **N11-QUALITY** | measured/manual-assisted e2e | Execute automated benchmark fixtures for all listed workloads against pre-adopted numeric budgets on recorded hosts/sinks, plus automated clean-install keyboard/editor/completion/save/restart workflows with observable screen/state/results. Record raw samples, failures and artifact/profile identity. Separately perform named terminal/screen-reader journeys for prompt/edit/pager announcements, Unicode and color-independent use, recording human observations and limits. Audit-only or manual-only completion is insufficient; no universal host/accessibility assertion. |

## Verification and evidence record

- Publish exact test names/commands as suites land; these IDs are not invented runnable Cargo
  targets. Require all three IDs, expected discovery and executed/nonignored counts; missing,
  ignored, empty or unexplained skipped mandatory cases fail the final release gate.
- Execute actual MSVC debug/release artifacts for supported default/minimal profiles and
  the N10 matrix. Keep applicable Unix language/unit/integration/doctest gates and native
  console runs; MSYS/WSL/guest results are separately labeled, not native evidence.
- Use argument-array launches and isolated user/state/temp roots, bounded waits, concurrent
  stdout/stderr drain and cancel-and-drain ownership. Reap/terminate only owned work;
  synchronize faults with explicit barriers/events rather than timing sleeps.
- Required privilege, filesystem/network, console and named accessibility infrastructure
  must be provisioned; unavailability blocks the relevant mandatory gate, never a silent skip.
- Record revision, toolchain/SDK/host/target/profile, artifact hashes, exact commands/cases,
  discovery/execution counts, suite/ledger totals, results/logs and actual dependency versions.
  Retain stress duration/load, fault schedule, resource traces, baseline/budget decisions,
  benchmark hardware/sinks/raw samples and named human signoffs separately from automated logs.
- Record actual installed runtime/daily-workflow smoke in addition to suites. Do not fabricate
  present passing tests, remote CI runs, release publication, signatures or accessibility proof.

## Definition of done and final handoff

All deliveries and gates pass with no undiscovered/ignored mandatory IDs, unresolved supported
capabilities, missing crate/asset dispositions or audit-only stand-ins for behavior.
Update all affected docs/callers/options/test registries and remove obsolete guards/placeholders.
Preserve positive prototype regressions and permanent/still-deferred rejection coverage;
replace superseded negatives with real feature-positive/failure cases as capabilities land,
without rewriting historical [GOAL](GOAL.md) evidence or freezing its old discovery counts.
Publish a release decision tied to the exact N10 artifact and completed evidence record.
No forgotten portable feature is handed to an unspecified later phase. ARM64 requires actual
native process/console/state/install evidence; optional MSI/MSIX/winget/signing require their
own infrastructure and trust verification, not fabricated credentials or a blocked unsigned ZIP.
The final support statement names tested native limits and deliberately excluded POSIX contracts.
