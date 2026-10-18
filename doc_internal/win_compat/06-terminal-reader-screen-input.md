# Terminal, reader, screen, and input

## Current N6 native reader overlay

The native entrypoint now selects the shared interactive `reader/reader.rs` and
the separate `reader/batch_windows.rs` source/history path. This is an implemented
reader verified for its recorded matrix, not complete N7 parity;
[GOAL_06](GOAL_06.md#implementation-and-verified-evidence) records actual acceptance.

`console_windows.rs` owns independently validated, noninheritable duplicates of
the real console handles. `input/windows.rs` waits on the retained input handle
and reads exactly one ready `ReadConsoleInputW` record on the main thread.
Nested editor, `read`, key-reader and breakpoint consumers share the decoder;
there is no input worker or competing `ReadFile` console reader. Strict split
UTF-16/UTF-8, physical modifiers, committed text, repeats, AltGr and ignored
nontext records have explicit handling. `native_text.rs` keeps canonical
escaped-byte representations of valid native scalars atomic across editing and
undo; full terminal-cell/layout parity remains N7.

`tty_handoff/windows.rs` owns native modes and emitted protocol state together.
It pauses input and disables protocols before restoring cooked modes; foreground
jobs retain donation through actual process/writer completion. Reclaim enters
editor modes before enabling protocols. Original unrelated mode bits survive
normal exit and recoverable failures; abrupt process kill is not RAII.

Cursor/viewport queries use `GetConsoleScreenBufferInfo`; palette-backed
background queries use `GetConsoleScreenBufferInfoEx`, including reverse video.
Coordinates subtract the visible viewport origin; an off-viewport live cursor is
unknown position, not EOF. Native startup/recurrent queries emit no OSC/DSR/DA
probes, so late replies cannot become another reader's data. This native palette
policy does not claim every remote terminal extension or full N7 theme parity.
Physical console records do not need Kitty keyboard negotiation.

`env_dispatch/windows.rs` now applies actual reader settings/repaint effects,
and native startup establishes global `fish_bind_mode` before config functions.
`bind`, `commandline`, nested `breakpoint`, standalone/builtin `fish_key_reader`
and interactive `read` use real shared state and owned input. `read --null`
deliberately bypasses editing/bindings and preserves committed escape/paste/control
bytes, with scalar counts, limits, cancellation and shared-cursor semantics.
`clipboard_windows.rs` implements bounded strict CF_UNICODETEXT copy/paste with
real busy/denied/invalid-data errors; it is separate from the in-memory kill ring.
Named native control handlers are distinct from typed editor Ctrl-C.

## Scope and evidence

The table below is the original source audit, not a fresh build/run report. Candidate replacements and proposed scenarios remain historical unless superseded by the current-source overlay and its linked evidence. Linux-specific behavior is distinguished from the larger POSIX/terminal-protocol boundary.

## Component-by-component coverage

“None directly” means the component's algorithm has no direct Linux system-call requirement; its dependencies may still require a port.

| Component and concrete evidence | Linux/POSIX dependency | Native Windows equivalent and gap | Validation focus |
|---|---|---|---|
| [`reader/mod.rs`](../../src/reader/mod.rs): module wiring and re-export | None directly. | Keep wiring; do not confuse a shared module name with an available native backend. | Reach each child through interactive and noninteractive entry points. |
| [`reader/reader.rs`](../../src/reader/reader.rs): `Reader`, `reader_init`, `terminal_init`, `restore_term_mode`, `term_fix_shell_modes`, `term_donate` | `isatty`, `tcgetattr`/`tcsetattr`, `Termios`, `VMIN`/`VTIME`, `VINTR`, `VSUSP`, `VQUIT`, `OPOST`/`ONLCR`; process groups, Unix signal cancellation, descriptor input. It also owns noninteractive `read_ni`, not just the editor. | `GetConsoleMode`/`SetConsoleMode` for console mode state, `ReadFile` or console input APIs for transport. No exact termios or foreground-process-group equivalent. Separate editor state and script reads from mode/ownership operations; preserve nested readers, repaint, cancellation, and exit handling. | T1, T3, T5; run script input from a pipe with no console, including BOM, EOF, and interrupted reads. |
| [`reader/input.rs`](../../src/reader/input.rs): `InputEventQueuer for Reader`, `prepare_to_select`, `select_interrupted`, `uvar_change_notified`, `paste_commit` | Delayed events, `job_reap`, signal cancellation and a raw-fd completion port; paste is decoded through `bytes2wcstring` and dispatched as `__fish_paste` inside undo grouping. | Native wait/completion/control-event integration must service the same callbacks. A console-record input path must preserve paste-as-text semantics, not execute pasted newline bindings. | T2, T3, T6; paste remains one undo group while jobs and universal changes arrive. |
| [`reader/iothreads.rs`](../../src/reader/iothreads.rs): `Debouncers`, `event_signaller_read_fd` | `RawFd` and `FdEventSignaller` connect autosuggestion, highlighting, and history-pager completions to the input wait loop. Thread-pool/debounce logic is not Linux-specific. | A waitable event or native completion queue can replace fd readiness. An IOCP is not a drop-in fd: wake, drain/reset, and callback ordering must be defined. | T6; stale background results cannot overwrite newer edits or strand the reader asleep. |
| [`reader/history_search.rs`](../../src/reader/history_search.rs): `ReaderHistorySearch`, `smartcase_flags`, `SearchMode` | None directly; uses the history subsystem. | Keep smartcase, token/line/prefix modes, skips, and deletion logic shared; storage compatibility is separate. | Search non-ASCII history, delete matches, return to the original line. |
| [`reader/word_motion.rs`](../../src/reader/word_motion.rs): `MoveWordStateMachine`, `path_component_state_transition`, `is_path_component_character` | No syscall dependency, but path-component character classification is explicitly slash-oriented. | Keep fish-language word rules; deliberately define native path navigation for drive prefixes, UNC roots and backslashes (which are also fish escapes). | Move/delete words in quoted and escaped `C:\a b\c` and UNC paths without splitting surrogate-derived text. |
| [`input/mod.rs`](../../src/input/mod.rs): `binding`, `decode`, `input` | None directly. | Shared organization; all three children below must remain covered. | Build-time wiring is not a substitute for T1–T6. |
| [`input/input.rs`](../../src/input/input.rs): `InputData`, `InputEventQueue`, `InputEventQueuer`, `next_input_event`, `check_fd_readable`, `KeyEvent` | `RawFd`, byte reads, `FdReadableSet`, `pselect`/signal masks, `EINTR`, and `VINTR`. `next_input_event` prioritizes universal notification, then stdin, then background completion. `check_fd_readable` includes a WSL1 signal-starvation workaround, not native Windows support. | Wait on console input and backend events, or use overlapped pipe reads. WinSock `select` cannot wait for console/file/pipe handles. Preserve query/escape deadlines and event priority; replace the WSL-only workaround rather than treating it as a Windows backend. | T2, T3, T6; EOF, query timeout, escape timeout and cancellation remain distinct. |
| [`input/decode.rs`](../../src/input/decode.rs): `InputEventQueuerExt::on_byte_read`, `parse_escape_sequence`, `enqueue_event`, `decode_utf8` | VT/UTF-8 parsing is mostly protocol logic, but it invokes `next_input_event` while decoding and reads shell `VINTR` to interrupt queries. It is **not** an unchanged pure decoder behind a transport seam today. | Reuse parsing after disentangling waits and interrupt-character policy. `ENABLE_VIRTUAL_TERMINAL_INPUT` does not guarantee Kitty keyboard, focus, color-theme, or every xterm extension. No Windows API can manufacture terminal query responses unsupported by the host. | T2, T4; fragmented/invalid UTF-8 and VT sequences, timed-out queries and keys queued during a query. |
| [`input/binding.rs`](../../src/input/binding.rs): `BindingSet`, `match_key_event_to_key`, `ReadlineCmd`, `read_char` | Binding tables/match-quality logic have no Linux syscall requirement; dispatch invokes reader/parser and signal-aware input. | Reuse bindings; translate native key data into `KeyEvent`'s explicit modifiers, shifted/base-layout codepoints and associated text, not just a character. | T2; layout-independent shortcuts, user/preset modes, multikey bindings and unmatched input. |
| [`key.rs`](../../src/key.rs): `Key`, `Modifiers`, `canonicalize_keyed_control_char`, `parse_keys` | Startup termios `VERASE` determines backspace canonicalization. Modifier representation is logical, not a complete left/right Windows modifier model. | Native backspace/control-key policy replaces `VERASE`; do not promise lossless left/right modifiers without extending the shared model. | Backspace vs Delete, Ctrl+Space/Enter, AltGr and printed/reloaded binding names. |
| [`editable_line.rs`](../../src/editable_line.rs): `EditableLine`, `Edit`, `UndoHistory`, `apply_edit` | None directly; UTF-32 edits, colors and cursor positions. | No Win32 replacement needed. UTF-16 input must first become the shared text representation. | Undo/redo coalescing and selection across supplementary characters and multiline paste. |
| [`kill.rs`](../../src/kill.rs): `KillRing`, `kill_add`, `kill_yank_rotate` | None directly; in-memory mutex-protected ring. It is not an OS clipboard backend. | Keep ring shared; clipboard APIs are not necessary to implement these symbols. | Kill/yank rotation, replacement and empty-ring behavior. |
| [`pager.rs`](../../src/pager.rs): `Pager`, `PageRendering`, `SelectionMotion`, `set_term_size`, `render` | None directly; completion grouping, width/layout and editable search depend on shared screen/width primitives. | No console API required for the layout algorithm. Console size and cell-width differences must reach it through the terminal backend. | T4; narrow windows, filtered results, selected-cell motion and multiline descriptions. |
| [`screen.rs`](../../src/screen.rs): `Screen::write`, `check_status`, `save_status`, `do_move`, `ScreenData` | VT screen diff/layout plus fd writes, stdout/stderr metadata timestamps and shell `ONLCR` assumptions. `TERM=dumb` uses direct byte writes. | Retain VT rendering where supported; replace fd sinks and newline assumptions. There is no reliable console “other process wrote” timestamp equivalent; repaint invalidation is a separate policy. | T4, T5; last-column wrapping, newline handling, right prompts, background output and redirected dumb output. |
| [`terminal.rs`](../../src/terminal.rs): `Outputter::write_command`, `write_wstr`, `flush_to`, `TerminalCommand`, `OutputterStyleWriter` | VT sequence generation is portable protocol code; `RawFd`, `write_loop`, and Unix wide-to-byte conversion are native boundaries. Optional OSC/DCS/Kitty features depend on terminal capabilities, not Linux. | `WriteFile` for byte-stream sinks; console VT mode plus a consistent UTF-8/code-page policy for console handles. `WriteConsoleW` is only an alternative for detected console handles and cannot replace redirected byte output. | T1, T4; styles, reset, titles, OSC 133, unsupported-query timeout and redirected UTF-8. |
| [`termsize.rs`](../../src/termsize.rs): `read_termsize_from_tty`, `TermsizeContainer`, `signal_safe_termsize_invalidate_tty` | `ioctl(0, TIOCGWINSZ)` and a generation invalidated by `SIGWINCH`. `COLUMNS`/`LINES` override until a newer TTY generation (“last seen wins”). | Visible `srWindow` rectangle from `GetConsoleScreenBufferInfo`, not backing-buffer dimensions; `WINDOW_BUFFER_SIZE_EVENT` with `ENABLE_WINDOW_INPUT` for record input. A resize event is not a Unix signal; VT/pipe and remote transports need an explicit size source. | T4; override variables then resize, zero/invalid size fallback, visible window differing from buffer. |
| [`text_face.rs`](../../src/text_face.rs): `TextFace`, `TextStyling`, `ResettableStyle`, `parse_text_face_and_options` | None directly; color/style model and option parsing. | Keep shared; VT support for underline styles, underline color, italics, etc. varies and is not implied by truecolor support. | Style parsing and reset/unchanged distinction even if the host ignores an optional attribute. |
| [`tty_handoff.rs`](../../src/tty_handoff.rs): `TtyHandoff`, `initialize_tty_protocols`, `deactivate_tty_protocols`, `TtyQuirks` | `tcsetpgrp`, termios save/restore for stopped jobs, signal/fd handling; serialized VT enable/disable sequences and terminal-version quirks. | No `tcsetpgrp`, `SIGTTIN`/`SIGTTOU` or Unix stopped-job mode restoration equivalent. Preserve protocol lifecycle around child execution independently of native process ownership. `CreatePseudoConsole` is only relevant if fish hosts a PTY, not a replacement needed by every console client. | T5; terminal protocols disabled for external apps and restored on return/exit, including failed launch. |

## Native console versus VT transport

Do not infer transport from the terminal application's name. Windows Terminal normally hosts a client attached to a console through ConPTY; it does not necessarily give that client stdin/stdout pipe handles. A program directly connected to a pseudoconsole's host-side pipes, an SSH transport, a console handle, and ordinary redirected input are different cases.

Use `GetConsoleMode` independently on relevant input/output handles. `GetFileType` helps distinguish other handle kinds, but a pipe does not identify whether its contents are interactive terminal data. Do not turn arbitrary redirected input into an interactive VT terminal.

For console output, enable `ENABLE_PROCESSED_OUTPUT` and `ENABLE_VIRTUAL_TERMINAL_PROCESSING` where supported; select `DISABLE_NEWLINE_AUTO_RETURN` only with a tested wrapping/newline policy. Fish currently emits a literal newline for `TerminalCommand::CursorDown` and consults `ONLCR` in screen movement, so enabling flags alone is insufficient. Save the original modes and restore them on normal exit and recoverable error paths.

For console input, either select VT input or translate `ReadConsoleInputW` records; do not consume both competing paths. Configure line/echo/processed input deliberately: `ENABLE_PROCESSED_INPUT` changes Ctrl-C control-event behavior. Structured input must handle repeats, key-up filtering, UTF-16 pairs, dead keys, AltGr and finalized IME text. Mouse/focus records are not automatically equivalent to fish's VT event vocabulary; unsupported events need an explicit ignore or translation policy. Quick Edit changes require `ENABLE_EXTENDED_FLAGS`, and unrelated user mode bits should be preserved.

At audit time these were proposed backend boundaries. The current N6 overlay above supplies the native console translator, waits, renderer integration and nested ownership. Shared fd-shaped interfaces remain logical identifiers on Windows, not cast HANDLEs; retaining algorithms alone still does not establish complete N7 behavior.

## Semantic gaps and Windows validation scenarios

The following are acceptance scenarios for a port; none were executed for this audit.

- **T1 — Attachment and modes:** launch from Windows Terminal, a classic conhost console, native SSH/VT transport, and fully redirected stdin/stdout. Check console detection per handle, UTF-8 output, no code-page-dependent mojibake, and exact restoration of saved console mode bits after exit and startup failure. Pipe reads must not call console-only APIs.
- **T2 — Input fidelity:** compare legacy VT, supported enhanced-key protocols, and any chosen record backend. Exercise Esc vs Alt prefix at timing boundaries; fragmented sequences; Ctrl+C/Space/Enter; AltGr on a non-US layout; repeat counts; supplementary Unicode; dead keys and IME; focus and bracketed multiline paste. No pasted newline may execute a command implicitly.
- **T3 — Cancellation and EOF:** cancel both a pending terminal query and a long expansion; deliver a native console control event while a key is queued; close the input handle. Distinguish cancellation, hangup/EOF and timeout without relying on `EINTR` or Unix signals.
- **T4 — Rendering and resizing:** vary viewport and buffer separately, rapidly resize with a multiline prompt/pager active, then override `COLUMNS`/`LINES`. Exercise CR/LF, last-column soft wrapping, wide/ambiguous/combining/emoji text, unsupported styles and unanswered cursor/color queries. Cursor-position responses measure position; they are not a general per-character width correction mechanism.
- **T5 — Child handoff:** run a native console application that changes modes and writes without a final newline; return after normal exit, failed launch and cancellation. Fish must not race the foreground child for input, and must restore its protocol state. Background reads need a defined policy because Windows does not enforce Unix terminal job control. Out-of-band output must not corrupt the prompt merely because no TTY timestamp exists. [`has_working_tty_timestamps`](../../src/common.rs) already returns false for Windows/Cygwin and WSL1; this guard alone is not native support.
- **T6 — Multiplexing:** while the reader is blocked, complete highlighting/autosuggestions/history paging and replace `fish_variables` from another process. Verify wakeup/drain semantics, query deadlines and the intended uvar/input/background priority without lost wakeups or permanently signaled busy loops.

## Classification

Editing, search, binding tables, pager layout and style values are mostly portable algorithms. Input waiting, console mode management, size/resize, output encoding/sinks, signal cancellation and terminal ownership are POSIX-backed and need native integration. WSL-specific fixes are Linux-compatibility workarounds, not native Windows implementations. Terminal-protocol support must be negotiated or allowed to time out independently of the OS backend.
