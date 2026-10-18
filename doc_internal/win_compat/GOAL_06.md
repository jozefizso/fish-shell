# GOAL_06 — First usable native interactive reader

**Phase: N6**  
**Status: implemented; required N6 gates verified for the recorded development matrix**  
**Primary native target:** `x86_64-pc-windows-msvc`; ARM64 is a separate extension.

This is an execution contract for the approved roadmap at commit `2acbbad4b`.
The acceptance fixtures are implemented and exercised below. Historical batch
prototype evidence in [GOAL.md](GOAL.md) remains unchanged.

## Objective

Deliver a real native prompt → edit → evaluate → prompt lifecycle with usable
Unicode/multiline editing, core bindings and native interactive tools. A console
child receives exclusive input ownership without fish protocol leakage; fish
resumes correctly and restores the original terminal state on orderly exit.
The editor reuses existing reader/screen algorithms, not a separate shell or REPL.

## Entry conditions and dependencies

- Verify [GOAL_04](GOAL_04.md) startup/config/history/autoload and
  [GOAL_05](GOAL_05.md) jobs/cancellation/event gates are complete.
- Verify inherited N2 asynchronous worker, debounce, wakeup, completion,
  cancel-and-drain and retained-handle services before integrating reader waits.
  N3 noninteractive script/source reading must already work without this editor.
- Preserve [GOAL.md](GOAL.md) positives, N1 filesystem/Unicode policy and N2/N3
  streaming/capture behavior. Do not route batch execution through terminal input.
- Suggested branch: `platform/windows/interactive-reader`. Define one owner for
  the process/reader handoff shared with N5; negotiate N7 worker/protocol interfaces.

## Owners and native invariants

- Native startup in `src/bin/fish/windows.rs` owns explicit `-i/--interactive`,
  implicit selection and N4 startup options when used interactively.
- `src/reader/`, `src/input/`, `src/key.rs`, `src/editable_line.rs` and
  `src/kill.rs` own input dispatch, editor state, key identity and core actions.
- `src/terminal.rs`, `src/termsize.rs`, `src/screen.rs` and
  `src/tty_handoff.rs` own output, viewport size, mode/protocol state and donation.
- `src/builtins/bind.rs`, `src/builtins/commandline.rs`, `src/builtins/read.rs`,
  `src/builtins/breakpoint.rs` and `src/builtins/fish_key_reader.rs` own their real
  native interactions. Required prompt/binding assets in `share/` land here.
- Follow [terminal/reader analysis](06-terminal-reader-screen-input.md),
  [signals/events](04-signals-events-waits.md),
  [assets analysis](13-binaries-and-assets.md) and
  [Windows API reference](windows-api-reference.md).
- Detect each input/output handle independently. ConPTY is a host/test transport,
  not POSIX tty semantics; an arbitrary pipe is not implicitly an interactive VT
  stream. Do not infer protocol support solely from a terminal application's name.

## Implementation delivery

1. **Enable real interactive startup.** Implement forced and implicit interactive
   selection, terminal/nonterminal/error policy and actual prompt/input cycles.
   Execute the N4 configuration → repeated init commands → requested execution
   sequence once, honoring `-N` without suppressing init commands, private history,
   login and startup profiles. No-execute must not run startup/evaluation effects.
   Inventory actual option aliases and failure diagnostics, not just registration.
2. **Land first-prompt assets.** Port every required prompt/default-binding helper
   and bootstrap dependency now. Clean startup and basic actions cannot require
   missing `mkdir`, coreutils, `/bin/sh`, WSL, MSYS or Cygwin. Do not defer required
   assets to N9 or expose advertised bindings whose actions silently do nothing.
3. **Choose input/output transport.** Document console-record translation or VT
   input and select one consumer path, not competing readers. Define UTF-16/UTF-8
   decoding, split surrogate/sequence handling, invalid-input policy, modifier/key
   identity, repeats, key-up filtering, dead keys/AltGr and committed IME text.
   Console output and redirected byte streams need distinct tested encoding paths.
4. **Integrate native waits.** Service input, jobs, universal-state notifications,
   query deadlines and worker completions using N2 services. Define priority,
   drain/reset and wakeup behavior so no source spins or starves. Keep parser and
   reader callbacks on their owning thread; cancel pending reads/queries and drain
   callbacks before freeing buffers. EOF, timeout and cancellation are distinct.
5. **Own modes and protocols together.** Save original Win32 input/output modes
   and unrelated bits; manage restoration with RAII on normal return, recoverable
   error, startup failure and orderly cancellation/exit. Configure echo/line/
   processed input, Quick Edit and VT flags deliberately. Restore any other native
   console setting fish changes; abrupt process kill is not an RAII guarantee.
6. **Make foreground handoff exclusive.** Before a child can read, pause/cancel
   editor input and finish its outstanding reads; deactivate fish-owned bracketed
   paste and negotiated enhanced-key/other input protocols, then donate native
   modes. After child completion, failed launch or orderly cancellation, reclaim
   modes and re-enable only the editor protocols in effect. Deactivate protocols
   on shell exit and restore original native modes. Track both states explicitly:
   a successful `SetConsoleMode` alone does not disable VT protocol envelopes.
7. **Deliver core editing and screen behavior.** Implement navigation, insertion,
   deletion, undo/redo, selection, kill/yank and core modes/bindings using shared
   models. Decode native text before editing; character boundaries are not UTF-16
   units or display cells. Multiline bracketed paste is text in an undo group,
   never implicit command execution; document pasted-newline handling. Implement
   real clipboard/selection policy, including unavailable/denied-service behavior.
8. **Handle rendering/resize capabilities.** Select output capabilities explicitly,
   use visible viewport dimensions rather than backing-buffer size, and reconcile
   resize with COLUMNS/LINES overrides. Bound cursor/protocol queries, preserve
   queued keys and repaint after child/background output. Avoid terminal control
   data in ordinary redirected output. Completion/pager refinements belong to N7.
9. **Restore interactive tools.** Implement `read`, `bind`, `commandline`, native
   `fish_key_reader` and nested `breakpoint` with real scope/context restoration.
   Inventory every key-reader flag and actual stream/console/error behavior; N9
   owns installed help, not forgotten tool options. Errors and nested readers must
   restore outer ownership/modes; tool input cannot race an active editor.
10. **Preserve native background policy.** N5 unredirected background stdin uses
    readable NUL and cannot steal ordinary typed commands; explicit redirection
    still works. Do not promise containment against deliberately reopened `CONIN$`
    or pretend foreground handoff implements terminal-pgroup job control.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no snapshot/return-twice emulation or post-fork backend;
  reuse the evaluator and explicit native process creation.
- **POSIX `exec`/`execve`:** no same-PID replacement. The `exec` decorator remains
  nonzero unsupported before argument side effects; spawn/wait/exit and CRT `_exec`
  are not replacements.
- **POSIX `fg`/`bg`:** keep both unsupported; no terminal-pgroup stop/continue,
  `SuspendThread`/`ResumeThread` or window-focus substitute. Reserved unsupported
  commands never fall through to same-named externals. Native `&`/jobs/wait/disown
  remain supported N5 workflows, not these excluded contracts.

## Mandatory automated acceptance

| Required ID | Type | Observable acceptance |
|---|---|---|
| **N6-EDITOR** | native console/ConPTY e2e | Drive actual fish through multiple typed/edited/submitted commands and assert editor state, output and status. Cover split UTF-16/non-BMP/combining text, core navigation, multiline paste without implicit execution, resize mid-edit, EOF and Ctrl-C prompt recovery. Repeat N4 startup ordering/private/login/`-N`/profile/no-execute behavior interactively and verify utility-free first prompt. |
| **N6-CHILD** | console e2e | A real console/TUI child reads actual keys/paste and changes modes. Observe fish protocols disabled before child input, no envelope leakage or competing editor reads, and functional editor/protocol restoration afterward. Cover failed launch, orderly cancellation and shell exit with native-mode snapshots. Background default input reaches EOF without stealing commands; explicit background redirection supplies its data. |
| **N6-TOOLS** | integration/e2e | Exercise actual native key reader and its flags, interactive read, binding mutations/dispatch, commandline operations and nested breakpoint scope/context. Invalid options, nonterminal/invalid input and nested errors produce defined diagnostics/status and restore outer input ownership, native modes and protocols. Observe actual console state rather than captured echo. |

Fragment terminal queries and input deterministically, synchronize child-read
ownership and compare saved/restored mode bits. Unit decoder/model tests may
supplement these gates but cannot substitute for real console/ConPTY behavior.

## Implementation and verified evidence

Source commit: **`6fcfb5520`**, branch `platform/windows/interactive-reader`.
The native entrypoint uses the existing editor, parser, screen, bindings and
tools rather than a separate REPL. `reader/batch_windows.rs` remains independent
for source/script/redirected input.

The transport owns real console handles and consumes exactly one ready
`ReadConsoleInputW` record on the main thread. Nested consumers share strict
Unicode/repeat state; there is no console input worker or competing prefetch.
Native cursor/viewport/palette APIs replace asynchronous terminal probes, so
late responses cannot enter a nested raw reader. Foreground donation owns modes
and protocol state through actual process/writer completion.

The implemented tools include standalone/builtin key-reader options, binding and
commandline operations, nested breakpoint, edited/silent/scalar-count/line reads,
and deliberately nonediting `read --null`. Required first-prompt helpers need no
Unix utilities. Native CF_UNICODETEXT clipboard errors are real failures; recursive
substitutions inherit the caller's descriptor overlay instead of losing stderr
redirection. Startup establishes global vi binding mode before config helpers.

### Runnable commands

From an x64 MSVC developer shell with SDK libraries available:

```powershell
cargo build --locked --target x86_64-pc-windows-msvc --no-default-features --bins
cargo build --locked --target x86_64-pc-windows-msvc --no-default-features --release --bins
.\target\x86_64-pc-windows-msvc\debug\fish.exe -N -i
cargo test --locked --target x86_64-pc-windows-msvc --no-default-features --test windows_n6 -- --nocapture
cargo test --locked --target x86_64-pc-windows-msvc --no-default-features --release --test windows_n6 -- --nocapture
```

The three Cargo IDs are `n6_editor`, `n6_child`, `n6_tools`. Each harness drives
both executable profiles. Real console/ConPTY, private logon/window-station and
clipboard fixtures are required and fail closed when unavailable; no host
clipboard fallback is permitted. Forced interactive startup rejects an arbitrary
redirected pipe rather than claiming it is a VT console.

### Recorded native and retained results

Host: **Windows `10.0.26300` x64**, local NTFS and the inherited writable
`\\localhost\D$` fixture; SDK **10.0.26100.0**, MSVC tools **14.51.36231**.
Native binaries and gates used **`--no-default-features`**.

| Toolchain | Native debug library | N6-EDITOR family executions | N6-CHILD family executions | N6-TOOLS family executions |
|---|---:|---:|---:|---:|
| Rust 1.95.0 | 111 passed | 32 passed | 20 passed | 28 passed |
| Rust 1.85.0 | 111 passed | 32 passed | 20 passed | 28 passed |

Family counts include both debug/release harnesses and both debug/release
artifacts: **80 executions per toolchain, zero failed or ignored**. The minimum
toolchain used its own debug **and release** binaries under `target/n6-msrv`,
not a stable-release fallback.

The existing workflow step **“Native GOAL leaf, library, initialization and
acceptance gates”** was replayed locally for both toolchains. Its leaf/library,
initialization/provenance, 30-record prototype registry and inherited N1–N5
debug/release gates passed. The initial replay commands reached their tool
deadlines; continuations executed the remaining gates, preserving exact
discovery/case assertions rather than skipping unfinished work. Logs are under
`target/n6-ci-stable/` and `target/n6-ci-msrv/`, including retained native
console traces. This is segmented local execution, **not a remote CI run**;
unchanged W1 helper steps are not newly claimed by this replay.

Retained Unix library: **183 passed, zero failed or ignored**. An actual Unix
PTY smoke typed/edited arithmetic and `read`, observed `42` and `雪🐟aZ`,
returned exit 3 and verified exact termios restoration. Owned trace:
`/tmp/fish-n6-unix-reader-zv9iknz5/trace.vt`.

### Actual native smoke and pre-parity baseline

An independent current-source ConPTY smoke, outside Cargo tests, exercised both
profiles: typed arithmetic produced `native=42`; nested `read` edited
`雪🐟ab` into `雪🐟aZ`; the next command returned status 0; `exit 3` returned 3.
It observed viewport `61 × 17`, then `COLUMNS=43` with 17 rows, then
`LINES=11` with 43 columns. Original and final console modes were:

| Console mode | Before fish | After fish |
|---|---:|---:|
| Input | 503 | 503 |
| Output | 7 | 7 |

Owned evidence is under
`target/n6-gate-results/native-reader-smoke-{profile}-{pid}-{stamp}/`.
The viewport family additionally queries an owned hidden classic console with
nonzero viewport origin, custom palette, reverse video and offscreen live cursor.

[Raw pre-N7 measurements and adopted budgets](baselines/N6.json) retain 96
release-profile samples, exact binary digests, host/protection conditions and
workload boundaries. `benchmarks/windows_reader.rs` reproduces the owned-console
workload. First-prompt and child metrics include their stated fixture/typing
costs; they are not isolated process-creation measurements or graphical/font
accessibility certification. N11 must compare matching workloads/conditions and
must not raise budgets after observing a regression.

This closes **N6**, not N7 completion/editor/rendering parity. Optional unqueried
VT capabilities retain fallback policy; native palette queries do not claim all
remote-terminal theme extensions. Default-feature compilation emitted missing
Sphinx/gettext-tool warnings and certifies neither installed help nor catalogs.
Other Windows/console/filesystem/ARM64 matrices and human accessibility signoffs
remain with their stated later owners.

## Verification and exit evidence

- Run actual native MSVC debug and release fish/tools with retained applicable
  Unix reader/input/editing tests. Record supported Windows/console-host versions.
- Use argument-array launches, isolated config/history and bounded waits with
  concurrent stdout/stderr draining and cleanup restricted to owned work.
- Use barriers for ownership/cancellation/resize races, not sleeps. Required console,
  ConPTY, key/clipboard or layout fixtures block exit when unavailable, not skip it.
- CI must discover exactly the three required IDs and all declared cases, rejecting
  missing/ignored IDs, empty runs and hidden unavailable-fixture success.
- Once implemented, record exact names/commands, expected and executed/nonignored
  counts, toolchain/SDK, revision, target, results and artifact/log references.
  Include mode snapshots, protocol traces and host conditions; no future command
  is represented here as already runnable.
- Retain a real runtime smoke of startup, editing/paste, child handoff and exit
  restoration. Record human terminal/accessibility observations separately from
  automated evidence; a screenshot is not an automated editor test.

## Definition of done and handoff

All deliveries/gates pass with updated CLI/tool/asset docs, consumers and CI counts.
Preserve prototype positives and permanent/still-deferred negatives; replace old
interactive/tool rejections with actual positive and failure coverage. Remove
obsolete guards/placeholders without changing historical [GOAL.md](GOAL.md) evidence.
Hand [GOAL_07](GOAL_07.md) one usable reader and explicit worker/protocol ownership;
[GOAL_08](GOAL_08.md) must use this lifecycle for interactive native providers.
N9 owns remaining asset breadth, not missing first-prompt actions. All mode/input
consumers are owned now; N11 is a final audit, not their implementation phase.
