# GOAL_06 — First usable native interactive reader

**Phase: N6**  
**Status: planned / not started**  
**Primary native target:** `x86_64-pc-windows-msvc`; ARM64 is a separate extension.

This is an execution contract for the approved roadmap at commit `2acbbad4b`.
Acceptance cases are future specifications, not existing/runnable/passing tests.
The completed batch prototype evidence in [GOAL.md](GOAL.md) remains unchanged.

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
