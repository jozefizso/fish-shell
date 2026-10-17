# GOAL_04 — Configuration, autoload, history and universal variables

**Phase: N4**  
**Status: planned / not started**  
**Primary native target:** `x86_64-pc-windows-msvc`; ARM64 is a separate extension.

The acceptance cases below are specifications, not claims of existing test names,
runnable targets or passing results. This contract expands the approved roadmap at
commit `2acbbad4b`; it does not change the historical evidence in [GOAL.md](GOAL.md).

## Objective

Deliver ordinary native batch startup without requiring `-N`, predictable config
and autoload discovery, and durable shared universal-variable/history state.
Independent shells must observe acknowledged changes without corrupting files or
losing their own updates. A clean first launch must work without Unix utilities.
This supplies startup/state services to the interactive reader, not an editor.

## Entry conditions and dependencies

- Verify [GOAL_03](GOAL_03.md) has closed its batch language, source/script reader,
  CLI and portable-builtin gates; a plan file is not evidence of implementation.
- Verify inherited [GOAL_02](GOAL_02.md) services include real workers, debounce,
  wait/wakeup/completion ownership, cancellation-and-drain and retained handles.
  Directory notifications must use these services, not invent a second event loop.
- Preserve the completed prototype contract in [GOAL.md](GOAL.md) and N1–N3 native
  filesystem, encoding, evaluator, streaming and capture behavior.
- Suggested branch: `platform/windows/persistent-state`. Coordinate env/reader
  interfaces with [GOAL_05](GOAL_05.md) and [GOAL_06](GOAL_06.md).

## Owners and native invariants

- `src/env/`, `src/env/config_paths.rs`, `src/env_dispatch/windows.rs` and the
  shared dispatch layer own roots, export generations and variable-change effects.
- `src/env_universal_common.rs` and `src/universal_notifier/` own file formats,
  merge policy, notifications and registration lifetime; `src/history/` owns
  history codecs, sessions, locking, deletion and vacuum.
- `src/autoload.rs`, `src/function.rs`, function persistence and completion-path
  consumers own cache invalidation and embedded/disk precedence.
- `src/bin/fish/windows.rs`, `share/config.fish`, `etc/` and required startup
  functions own startup ordering, option handling and utility-free bootstrap.
- Follow [environment analysis](08-environment-and-universal-variables.md),
  [history/autoload analysis](09-history-functions-autoload.md),
  [filesystem analysis](07-filesystem-paths-wildcards.md) and
  [Windows API reference](windows-api-reference.md); mappings there are not tests.
- Keep fish's case-sensitive scoped variables, list semantics and existing format
  compatibility. Native case-insensitive environment export is a separate boundary.
  Do not substitute the registry for universal-variable storage.

## Implementation delivery

1. **Locate state and installation assets.** Define known-folder defaults for
   config, data/history and cache independently from executable-relative bundled
   data and admin/vendor config. Document HOME/global-exported XDG overrides,
   native absolute-path validation, precedence, missing roots and ACL failures.
   Keep `fish_variables` under its specified config root; do not confuse cache,
   history and config. State whether storage is local or roaming and its limits.
2. **Restore batch startup.** Inventory every native startup option and alias:
   `-C/--init-command`, `-N/--no-config`, `-P/--private`, `-l/--login` and
   `--profile-startup`. Execute configuration, repeated init commands in command
   line order, then requested commands/script/stdin. Preserve argv and status.
   `-N` skips configuration, not init commands, and implies private-history
   behavior. Specify universal-state behavior separately; private history does
   not authorize silently dropping legitimate universal-variable semantics.
3. **Make startup modes real.** Implement user/admin/vendor conf.d precedence and
   basename masking, login/session conditions and first-run setup. Honor
   no-execute parsing without running configuration/init side effects. Startup
   profiling must capture actual startup stages in its requested sink, diagnose
   unwritable destinations and not corrupt stdout or requested command status.
   Interactive execution of the same sequence belongs to N6, not a second policy.
4. **Port the bootstrap consumers now.** Audit every function executed by normal
   batch startup in `share/config.fish`/`etc/` and their transitive dependencies.
   Replace required coreutils or `/bin/sh` assumptions with actual native fish
   operations. Optional dependencies must be checked and diagnosed, never faked.
5. **Deliver real autoload/persistence.** Discover functions and completion files
   on disk, define embedded versus disk precedence, save functions safely, and
   invalidate positive/negative caches on search-path changes and file replacement.
   Port the public function-saving path and its required helpers now, including
   directory creation/replacement without Unix `mkdir`/`rm`; saving is not deferred
   to N9's complete asset audit. Fresh processes must execute saved definitions.
   Preserve evaluator scope/source diagnostics; malformed or unreadable files
   must not poison later loads or appear successfully loaded.
6. **Preserve the persistence model.** Retain valid old history/universal formats,
   escaped values, arrays, export flags and unknown-future-format protection.
   Define same-key conflicts, acknowledged-save points, concurrent erase/merge,
   history session switching, private isolation and deletion/vacuum behavior.
   Snapshot local pending changes without dropping later concurrent mutations.
7. **Implement native transactions and recovery.** Use same-volume temporary
   files, deliberate sharing flags, locks, ACL/inheritance rules and identity
   checks. Handle active readers, stale snapshots, denied/locked/replaced targets
   and partial writes. `ReplaceFileW` is not proof of POSIX rename or power-loss
   durability; document the exact tested crash-consistency guarantee.
8. **Own notifications through shutdown.** Watch parent directories so replacement
   remains visible. Treat notifications as invalidation hints; filter/rescan using
   native file identity, recover overflow/lost detail/parent replacement/network
   loss, and notify every reader. Publish state only after merge and invoke
   variable callbacks outside environment locks on the permitted thread.
   Re-arm and cancel/drain registrations before releasing buffers or handles.
9. **Wire every state consumer.** Export-generation changes must affect real
   child environments; function/completion paths invalidate the actual loaders;
   history selection updates the active history. Locale/TZ and read-limit hooks
   must reach real consumers with existing lock ordering. Own terminal/reader
   settings now through explicit N6/N7 interfaces, activating their tests there;
   no forgotten variable-dispatch consumer is deferred to the N11 audit.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no process snapshot/return-twice emulation and no post-fork
  backend. Reuse the evaluator and explicit native process creation.
- **POSIX `exec`/`execve`:** no same-PID image replacement. Keep the `exec`
  decorator nonzero unsupported before argument side effects; spawn/wait/exit and
  CRT `_exec` are not substitutes.
- **POSIX `fg`/`bg`:** no controlling-terminal/pgroup stop/continue contract and
  no `SuspendThread`/`ResumeThread` or window-focus substitute. Keep both commands
  unsupported with no same-named external fallback; the same fallback prohibition
  applies to reserved `exec`. Native `&`/jobs/wait/disown are N5, not excluded.

## Mandatory automated acceptance

| Required ID | Type | Observable acceptance |
|---|---|---|
| **N4-STARTUP** | e2e | Launch actual native fish under normal, `-N`, login and private modes with isolated Unicode/space roots. Assert override/known-folder policy, conf.d precedence/masking, bootstrap and autoload invalidation. Record config → repeated init → execution markers, argv/status, `-N` retaining init commands, no-execute producing no effects and profile content/errors. All startup and saving scenarios run without Unix utilities on PATH. Define and save a function through the real public saving path into a fresh Unicode/space directory, then launch another native fish process and assert execution of the persisted definition with exact arguments/output/status. Loading only prewritten fixtures is insufficient. |
| **N4-SHARING** | multiprocess integration | Synchronize independent real shells updating distinct/same keys, exporting, erasing, appending/deleting history and merging while readers stay open. Assert declared conflict/acknowledgement policy, all-reader notification, actual native child exports and restart-visible data without corruption/lost acknowledged updates. Private and `-N` history leave no entries. Change/unset supported locale/TZ settings using contrasting values where the declared policy changes actual shell formatting/time, then assert consumer results and restored defaults. Set `fish_read_limit` below/above a fixed input/capture payload and unset it; assert real limit failures/success and restored policy, not merely exported/stored values. Synchronize callbacks that read/write environment state and assert handler effects, permitted-thread dispatch, safe reentry outside environment locks and no deadlock. Full new locale/catalog coverage remains N9; the N4-supported consumer policy must be explicit. |
| **N4-RECOVERY** | integration | Use deterministic save-stage barriers to kill owned writers and inspect/reopen real state. Exercise truncated/corrupt/old/future files, restrictive ACLs, denied/locked/replaced files, overflow/rescan, missing/replaced parents and supported remote storage. Include denied and interrupted function-definition saves: assert nonzero useful diagnostics, preservation of the prior valid definition and successful execution of it in a fresh process, never partial-file success. Assert state recovery, preserved prior data, no false save success and drained handles; state precisely the crash consistency observed. |

Coverage within these gates must include empty list versus empty value, Unicode,
export-flag-only changes, equal-size replacements, session switches and malformed
startup options/missing option values. Autoload tests execute sourced functions
and evaluate completion definitions, not just check filenames. Required remote or
ACL fixtures are an explicit acceptance dependency, not silently ignored cases.

## Verification and exit evidence

- Run implemented acceptance against actual native MSVC debug and release
  artifacts and retain applicable Unix env/history/autoload/startup suites.
- Launch with native argument arrays, isolated state roots and child environments;
  do not mutate the developer's real configuration, history, locale or profiles.
- Bound waits; drain stdout/stderr concurrently; synchronize race stages with
  barriers, not sleeps. Terminate/reap only owned processes and drain watchers.
- CI must discover exactly the three required IDs above and their declared cases;
  fail missing IDs, zero cases, ignored mandatory cases and hidden fixture skips.
- When implementation lands, record exact runnable commands/test names, expected
  and executed/nonignored counts, Rust/SDK/toolchain, target, revision, results and
  artifact/log references. Do not invent future Cargo targets in this plan.
- Include a real clean-root runtime smoke: normal launch loads config/autoload,
  a second shell sees a saved universal change, and a private session leaves no
  history. Smoke supplements automated gates; it does not replace them.

## Definition of done and handoff

All deliveries and required gates pass; docs, native CLI help/option ledger,
callers and CI counts reflect the actual behavior. Preserve prototype positives,
permanent exclusions and still-deferred rejection checks; replace obsolete config,
autoload/universal/history rejections with positive and meaningful failure cases.
Remove obsolete guards/placeholders without rewriting [GOAL.md](GOAL.md) evidence.
Hand [GOAL_06](GOAL_06.md) a single startup/state lifecycle and drained notification
API; [GOAL_07](GOAL_07.md) consumes its completion/history invalidation. N5 remains
owner of user-visible asynchronous jobs. N9 covers remaining assets, not missing
bootstrap required here; N11 audits completed consumers rather than implementing them.
