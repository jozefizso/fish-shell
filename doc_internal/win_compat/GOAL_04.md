# GOAL_04 — Configuration, autoload, history and universal variables

**Phase: N4**  
**Status: implemented and verified**  
**Primary native target:** `x86_64-pc-windows-msvc`; ARM64 is a separate extension.

Source commit **`c52f6d98a`** on `platform/windows/persistent-state`, from base
`1bfef1539`. Final development acceptance, minimum-Rust execution and retained Unix
results are recorded below and in the [evidence ledger](coverage.md#n4-implementation-and-evidence-record).
Historical [GOAL.md](GOAL.md) and N1–N3 evidence is unchanged.

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

### Delivered behavior and explicit limits

- Normal batch launch executes configuration, ordered repeated init commands and
  then the requested command/script/stdin. `-N` skips configuration but retains
  init commands and uses private history; `-P` makes history private, `-l`
  selects login startup, and startup profiling measures the actual stages.
  `-n` parses without configuration, init or persistent-state effects.
- Default writable roots are local `FOLDERID_LocalAppData/fish/{config,data,cache}`.
  A global exported XDG root selects `<root>/fish`; explicit exported HOME supplies
  `.config/fish`, `.local/share/fish` and `.cache/fish` fallbacks. Otherwise HOME
  comes from `FOLDERID_Profile`. Installation assets remain executable-relative,
  separate from writable roots. Unicode, spaces and literal extended install,
  state and HOME paths are covered by the startup fixture.
- Normal bootstrap and the public `funcsave` path require no Unix utilities.
  Full embedded/disk autoload, conf.d basename masking, function/completion path
  invalidation and native atomic function saves are implemented. This is not
  active `complete -C`, editor operation or a complete shipped-asset audit.
- State transactions use a stable parent `.fish_lock` with `LockFileEx`,
  create-new sibling staging, flushed writes, checked identities and
  `ReplaceFileW`. The existing DACL is copied **before staging bytes**.
  Failed writes do not acknowledge success or discard pending mutations.
  This protects the prior committed file during the exercised process failures;
  it does not promise power-loss directory durability or SMB-server durability.
  Owner, SACL and integrity-label copying are not provided. Protected crash
  orphans are not swept unsafely.
- Universal variables retain existing formats, empty-array/empty-value,
  Unicode/raw-byte, export and path semantics, with a 16 MiB read and serialized
  snapshot cap. Distinct keys merge; the last acknowledged writer wins a
  same-key conflict. Matching inherited exported globals are reconciled.
  Callback effects/events run outside environment locks; failed mutations
  retain deferred replay.
- History retains valid existing YAML and real wall-clock timestamps; the
  already-unsupported Fish 1 decoder is not added. Its initial owned baseline
  stays immutable across automatic saves and watcher invalidation. Explicit
  checked merge captures its snapshot inside the transaction before retiring
  local entries. A committed-generation journal lets session clear undo only
  acknowledged local versions still present on disk and restore their actual
  observed predecessors, preserving intervening foreign updates/deletions.
  Failed or compacted duplicate saves do not erase this ownership history.
  The legacy same-command/same-second format cannot uniquely identify authors.
  Private selection precedes disk-history initialization; shutdown flushes dirty
  history, including pending deletes and recoverable earlier save failures.
- Parent/ancestor `ReadDirectoryChangesW` registrations use the N2 runtime and
  cancel/drain lifecycle. Lost detail and `ERROR_MORE_DATA` (234) trigger full
  invalidation/rescan. Real UNC storage was exercised; no live SMB-server
  disconnect experiment is claimed.
- Exported `LC_ALL` takes precedence over category variables and `LANG`.
  Numeric formatting uses Windows NLS; historical time formatting uses UCRT.
  C/UTF-8/native Windows locale IDs and declared aliases, UTC and Windows TZ IDs
  with historical DST are supported. Unsupported IANA/POSIX TZ and GNU formats
  are diagnosed; full localization/assets parity belongs to N9.
- Read-limit, trace, function/completion paths and history dispatch reach real
  consumers. Explicit reader-settings generations and the history stale-state
  interface hand off activation to N6/N7, not a simulated interactive shell.

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
- Retain exact counts, Rust/SDK/toolchain, source revision and artifact/log identities.
  The final combined execution is recorded below; individual earlier runs are not
  substituted for that matrix.
- Include a real clean-root runtime smoke: normal launch loads config/autoload,
  a second shell sees a saved universal change, and a private session leaves no
  history. Smoke supplements automated gates; it does not replace them.

### Runnable target and current evidence

From the repository root, with the mandatory native fixtures available:

```powershell
cargo test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n4 -- --nocapture
cargo test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n4 --release -- --nocapture
```

The source-authoritative target is `tests/windows_n4.rs`. Cargo discovers exactly
`N4_STARTUP`, `N4_SHARING` and `N4_RECOVERY`; their emitted gate IDs use hyphens.
STARTUP has 2 families (`configuration-autoload-save`, `history-modes`), SHARING
has 4 (`universal-values-events`, `storage-transactions`, `history-sessions`,
`locale-timezone`) and RECOVERY has 5 (`universal-formats-errors`,
`storage-crash-acl`, `autoload-save-errors`, `history-recovery`,
`directory-notifications`). Each harness invocation exercises both debug and
release executables: 11 families per artifact, 22 `PASS-CASE` records total.
All three gate IDs must pass for both artifacts; none of the mandatory cases
may be ignored. `FISH_NATIVE_N1_UNC_ROOT` is required, not a skip switch.

The final actual workflow passed its exact discovery and execution checks: all
three N4 gates and 22 family/artifact pairs passed in each debug/release harness,
with zero ignored cases. Retained native N1–N3/prototype/initialization/provenance
gates and library tests passed; the root native library ran 60 tests.

Rust 1.85 built fish, fish_indent and the probe; its debug harnesses passed all
twelve N1–N4 Cargo tests against the minimum-compiler debug and stable release
executables. The Unix build and 183 library tests passed. Selected Unix checks:
14 passed, 1 skipped (`locale-numeric.fish`, required comma-decimal locale absent).
Both native artifacts passed fresh-root actual-executable config/autoload,
Unicode universal restart and private-history smoke with empty stderr and no
history file created.

Toolchains were Rust 1.95.0/1.85.0, MSVC 14.51.36231, SDK 10.0.26100.0,
target `x86_64-pc-windows-msvc`. Full commands, SHA-256 identities and retained
`target/n4-gate-results/` logs are in the [ledger](coverage.md#final-executed-matrix).
These are `--no-default-features` development results; N10 owns default-feature
release/install certification. N5–N11 are not completed by this delivery.

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
