# GOAL_07 — Interactive feature parity

**Phase: N7**  
**Status: implementation integrated / verification in progress; not complete**  
**Primary native target:** `x86_64-pc-windows-msvc`; ARM64 is a separate extension.

This contract expands the approved roadmap at commit `2acbbad4b`. The acceptance
IDs below remain mandatory; registered tests are not evidence that they passed.
Historical prototype evidence remains in [GOAL.md](GOAL.md).

## Objective

Deliver real completion, highlighting, suggestions, history search, abbreviations,
pager and default/vi/hybrid editing behavior on the usable N6 reader. Features
must remain responsive under asynchronous work, Unicode rendering and background
output; stale results cannot overwrite the user's newer command line.
Completion must discover commands that actual native execution can run, not a
separate permissive resolver or an invented Unix environment.

## Entry conditions and dependencies

- Verify [GOAL_06](GOAL_06.md) editor, tool, console/protocol restoration and
  exclusive child-input ownership gates have closed.
- Verify inherited [GOAL_02](GOAL_02.md) worker/debounce/wakeup/completion,
  cancellation-and-drain and retained-handle services remain operational. N7
  consumes real asynchronous services; it cannot use synchronous no-op stand-ins.
- Verify [GOAL_04](GOAL_04.md) actual history, autoload, config and variable
  invalidation and [GOAL_05](GOAL_05.md) jobs/events/cancellation behavior.
- Suggested branch: `platform/windows/interactive-parity`. N7 may proceed alongside
  [GOAL_08](GOAL_08.md); provider interface changes need one integration owner.
  N8 is not an extra entry dependency, but its providers must later use the same
  resolver policy and extend these tests when they become supported.

## Owners and native invariants

- `src/complete.rs`, `src/highlight/`, `src/reader/iothreads.rs` and the reader's
  autosuggestion/history workers own evaluated completion, validation and publication.
- `src/reader/`, `src/abbrs.rs`, `src/pager.rs`, `src/editable_line.rs`,
  `src/input/binding.rs` and required `share/functions/` binding helpers own
  search, expansions, navigation, editing modes and action state.
- `src/screen.rs`, `src/terminal.rs`, `src/termsize.rs`, `src/text_face.rs` and
  text-width/key/protocol consumers own observed layout and capability negotiation.
- Completion autoload, wrapper/condition handling and command/path discovery must
  use the existing N1 resolver and N4 invalidation. N8 extends that central policy,
  not a second command-discovery implementation inside the editor.
- Follow [completion/highlighting analysis](02-expansion-completion-highlighting.md),
  [reader/terminal analysis](06-terminal-reader-screen-input.md),
  [history/autoload analysis](09-history-functions-autoload.md) and
  [Windows API reference](windows-api-reference.md).
- Reuse the evaluator, completion/ranking algorithms, editor and pager models.
  Keep fish characters, native UTF-16 units and terminal display cells distinct;
  no blanket platform guard may silently hide portable interactive behavior.

## Implementation workflow

The advisor-reviewed execution order below replaces broad, unverified feature
fanout. It changes the implementation process, not the nine deliveries, acceptance
requirements, native ownership rules or permanently excluded POSIX contracts.

### 1. Restore the ordinary console baseline

One integration owner maintains `platform/windows/interactive-parity`. Preserve
the logical topic worktrees, integrated correctness fixes and existing regressions;
pause further dependent feature edits until ordinary input works.

Review the producer, decoding, binding lookup and consuming reader dispatch for
the first observed failure. Correct its cause rather than increasing timeouts.
Use a bounded, owned-console journey: a benign F2 insertion, Unicode cursor/edit
actions, builtin output, another usable prompt, explicit exit status and exact
console-mode restoration. Stop at the first missing receipt.

**Delivery:** that journey and the relevant inherited N6 reader cases pass against
the changed executable. A library-only pass does not satisfy this delivery.

### 2. Finish small functional slices

Complete one end-to-end slice before starting the next dependent slice:

- Completion: real discovery, evaluation, insertion, cursor/escaping, acceptance
  and execution; then conditions, wrappers, autoload and invalidation.
- Editing: history/suggestions, abbreviations, pager and default/vi/hybrid actions,
  including required helpers, undo, selections, errors and mode transitions.
- Rendering: Unicode/layout, resize/background repaint, capability policy and
  fallback, separating model state from observed console cells/cursor.

Every slice preserves bounded asynchronous work, exact publication checks,
owning-thread callbacks, cancellation/drain and exclusive child input ownership.
These are continuous invariants, not a later integration phase.

**Delivery:** applicable owning-unit and real integration/console cases pass for
the slice, with artifact identity and observed results retained.

### 3. Use correctness tests at the owning layer

Use ordinary unit tests for decoder boundaries, partial input, layout, ranking,
editing state and generations. Use actual evaluator/filesystem/history integration
and owned console fixtures for consumer-visible behavior. Keep minimum decoder
robustness local and bounded; retain required real-console fallback coverage and
existing regressions. Do not expand the work into new vulnerability-reproduction
campaigns. Review ownership, bounds and consuming dispatch directly.

Use deterministic barriers, isolated state, concurrent output draining and cleanup
limited to test-owned work. Do not substitute sleeps, captured echo, source-text
assertions, mock completions or repeated stress attempts for functional evidence.

### 4. Verify and commit coherent slices

Before a slice commit, review the consuming call sites and run its applicable
native debug/release and retained Unix verification. Preserve incomplete work in
its worktree; do not commit a known failing aggregate as completed N7.

Before phase closure, run all three mandatory N7 gates and their declared families,
required inherited gates, current/MSRV native artifacts and harnesses, retained
Unix checks and the independent runtime smoke. Missing fixtures, ignored mandatory
cases, zero discovery and unavailable required evidence block closure. Record the
source revision or dirty patch identity, toolchains, artifact profiles, host/
protocol conditions and exact executed/nonignored counts. Old logs do not prove
newer code.

### Provider-policy stop rule

On a provider policy rejection, stop the affected request and record the error and
blocked delivery once. Do not repeatedly retry it, rephrase it to obtain the same
blocked output, switch providers to circumvent the rejection or change access/
safety settings. Continue only independent, allowed ordinary engineering work.
Resume affected work only after documented provider resolution or clarification;
otherwise keep the requirement explicitly blocked, without deleting its required
test or claiming completion. This workflow cannot guarantee that future requests
will avoid moderation errors.

### Current evidence and next action

The latest four-cell native x64 MSVC sweep passed all 84 registered families in
both debug and release, with zero ignored. It includes required-option values
beginning with `-`, logical-token completion, quoted substitutions, callable
aliases, resource publication, abbreviation rename, description matching,
modified-F3 query ownership, function-discovery lock scope and missing wrapped-target metadata.

| Toolchain / features | N7-COMPLETE | N7-EDIT | N7-RENDER | Command log under `target/n7-gate-results/` |
| --- | ---: | ---: | ---: | --- |
| 1.95 / minimal | 40/40 | 98/98 | 30/30 | `n7-current-minimal-wrapped-metadata-native-gettext-io.txt` |
| 1.95 / default | 40/40 | 98/98 | 30/30 | `n7-current-default-wrapped-metadata-native-gettext-io.txt` |
| 1.85 / minimal | 40/40 | 98/98 | 30/30 | `n7-msrv-minimal-wrapped-metadata-native-gettext-io.txt` |
| 1.85 / default | 40/40 | 98/98 | 30/30 | `n7-msrv-default-wrapped-metadata-native-gettext-io.txt` |

These are dirty-worktree snapshots based on `cdc211188`, not a final release/source
identity. Later optional-producer repairs require their own verification; they
are not retroactively included in these images.

Native state synchronization queues external notifications for the existing
owning-thread delayed-event dispatcher. An asynchronous variable handler can no
longer interrupt initial binding autoload inside the first `fish_prompt` event
and leave the reader with only partial presets. Actual input-function dispatch
inside a binding helper does not refresh asynchronous work against temporary
helper locals before consuming the displayed suggestion.

Evaluator epochs do not span blocking input consumption. Job/universal callbacks
have their own epochs; a typed-cancel reset is consumed once across nested
evaluators. Backlog/framing yields preserve the current worker generation.
Both greater-than-4096-key backlogs pass with suffix recovery. Silent background
completion invalidates cached cells even when a child restores the cursor.

Each of those four cells also executes all 22 workspace library targets:
256 passed and four pre-existing exhaustive printf cases remained ignored.
The root native library contributes 151 passing cases with zero ignored.
These are development checks, not a waiver of unexecuted exhaustive coverage.
The two default cells start from fresh Unicode target roots, generate actual
gettext catalogs, and pass 19 locale/help consumers each. Both minimal cells pass 21;
the current/minimal locale launch initially met an in-use owned N9 test executable
at link time and passed after its owner exited, recorded separately in
`n9-current-minimal-locale-after-owned-test-image-unlock.txt`. No process was killed
or link failure suppressed.

Retained Unix Rust1.92 library verification passed 206 cases; all 16 selected
checks and actual pexpect journeys passed at unchanged deadlines and normal
concurrency, zero skipped. The combined log is
`unix-library-interactions-and-inherited-metadata-after.txt`; it also records an
actual completion-selected GNU SHA256 consumer with the wrapped MD5 executable
absent and an independently matched Unicode payload digest.

Earlier retained-host runs exposed first-prompt, nested-reader and suggestion
timeouts. Actual syscall and startup profiles traced whole-asset traversal while
looking up one named theme. Named theme/prompt lookup now uses direct asset
retrieval. Live-file Unix debug function discovery uses a metadata-only function
index instead of traversing unrelated completions/themes/tools; it still discovers
live source files. The function-definition mutex no longer spans filesystem
enumeration. Script loading remains with the original full-data asset owner.

| Single named-asset profile observation | Before (microseconds) | After (microseconds) |
|---|---:|---:|
| Interactive batch startup | 3250505 | 6511 |
| Actual reader startup | 3309901 | 5982 |

Profiles are retained as `before-exact-lookup-unix-startup-*.profile` and
`unix-startup-*.profile`. These diagnose the functional deadline failure; they are
not adopted N11 performance baselines, budgets or release acceptance.

The inbox ConPTY host independently reports a one-cell nonspacing policy.
Layout probes an inactive private buffer without painting or consuming input;
the modern zero-cell host branch remains unexercised. Relative
`CARGO_TARGET_DIR` default-feature builds share workspace-rooted help/gettext
outputs across build scripts.

Inherited current-toolchain/minimal N1–N6 gates passed in both development profiles.
The baseline registry also passed with the admitted N8 batch/PowerShell providers
and permanent unsupported boundaries. Full refreshed traces are
`n1-inherited-scoped-function-index-and-exact-assets.txt` through
`n6-inherited-scoped-function-index-and-exact-assets.txt`, plus
`native-inherited-scoped-function-index-and-exact-assets.txt`.

A separate actual native PTY journey exercised normal startup, abbreviation
expansion, Unicode/spaced filename insertion, middle-token programmable completion
and history recall. Separate input frames proved default Ctrl-C clears an
unaccepted command without its file effect; its observed `clear-commandline`
binding preserves status zero rather than inventing a native control receipt.
The actual transcript, image SHA-256 and filesystem observations are retained in
`target/n7-gate-results/n7-live-reader-normal-startup-logical-token.json`.
Enter/Ctrl-J were remapped only for those separate clearing frames; F12 executed
the observation command.

This is not N7 closure. Final-source/release identity and the independent final
installed-artifact journey remain blocked by the N10 management authorization
rejection. The development-image journey is not a replacement for that acceptance.

## Implementation delivery

1. **Complete against real native state.** Restore dynamic and programmable
   completion, evaluated conditions, wrapper chains, argument/path/command discovery
   and completion autoload. Use actual cwd/PATH/PATHEXT and native path/quoting rules
   from the central resolver. Preserve empty/space/Unicode arguments and insertion
   escaping. A suggested command must obey the same execution policy as launch;
   unsupported script providers must not be advertised as already executable.
2. **Own completion evaluation and caches.** Evaluate shell conditions/functions
   using the real evaluator and proper scope, cancellation and main-thread parser
   ownership. Invalidate positive/negative results when files/search paths,
   definitions or applicable environment state change. Define wrapper cycles,
   condition failure, malformed definitions and inaccessible paths without poisoning
   subsequent completions or corrupting the command line.
3. **Restore suggestions and history interaction.** Use real N4 history for
   autosuggestions, line/token/prefix search, navigation, deletion and returning to
   the original edit. Honor private/session isolation and history switching; do not
   leak another session's private entries or block editing on disk synchronization.
   Preserve smartcase and Unicode behavior through actual reader actions.
4. **Deliver abbreviations and editing actions.** Implement function and regex
   abbreviations with correct replacement/cursor semantics, error behavior and
   scope. Restore commandline actions, highlighting and indentation using real
   syntax/current filesystem state. Edits, undo groups, selections, kill/yank and
   multiline behavior must remain consistent when expansions or suggestions apply.
5. **Restore pager and all required bindings.** Support grouping/filtering/search,
   descriptions, selection and navigation, narrow/multiline layouts and acceptance
   into the command line. Deliver default/emacs, vi and hybrid bindings and mode
   transitions together with every required helper asset now. Inventory each
   advertised action and callback so none remains a silent no-op until N9/N11.
6. **Make asynchronous publication race-safe.** Bound queued/running work and
   debounce expensive requests through N2 services. Associate completion,
   highlighting, suggestions and history results with command-line/context
   generations. Recheck generation on publication; obsolete work cannot overwrite
   newer text, cursor, mode, cwd or history. Wake the blocked reader through the
   established completion path and reset/drain without lost wakeups or busy loops.
   Cancel/drain on reader destruction, nested-reader changes and shell shutdown.
7. **Preserve input ownership under work.** Background computation cannot read the
   console or run parser callbacks on worker threads. Deliver callbacks on the
   owning thread without starving input, universal notifications or job events.
   Foreground child transitions retain N6 exclusive input, mode/protocol restoration
   and N5 background-NUL policy, including during outstanding completion work.
8. **Render characters as observed cells.** Preserve combining/emoji/wide/ambiguous
   text boundaries, selections, cursor placement, truncation and pager layout.
   Reconcile viewport resize, width/color variable changes and asynchronous output
   with repaint, right prompts and wrap behavior. Record host/font/width-policy
   conditions; neither UTF-16 length nor cursor queries provide a universal cell
   width oracle. Keep model and observed screen assertions separate and explicit.
9. **Negotiate terminal capabilities.** Gate colors/styles, enhanced-key protocols,
   clipboard/selection and optional features on actual capabilities and user policy.
   Bound queries, handle fragmented/unsupported/malformed replies and preserve user
   keys received during negotiation. Define denied/unavailable clipboard behavior;
   do not advertise unsupported actions as successful. N6 protocol disable/restore
   rules still apply around children and exit. Ordinary redirected output must not
   accidentally contain terminal control data or block on terminal-only queries.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no snapshot/return-twice emulation or post-fork backend;
  reuse existing evaluation and explicit native child creation.
- **POSIX `exec`/`execve`:** no same-PID image replacement. Keep the `exec`
  decorator nonzero unsupported before argument side effects; spawn/wait/exit and
  CRT `_exec` are not substitutes.
- **POSIX `fg`/`bg`:** keep both commands unsupported; no terminal-pgroup
  stop/continue or `SuspendThread`/`ResumeThread`/window-focus substitution. No
  same-named external fallback for reserved unsupported commands. Native `&`,
  jobs, wait and disown remain supported N5 behavior, not these exclusions.

## Mandatory automated acceptance

| Required ID | Type | Observable acceptance |
|---|---|---|
| **N7-COMPLETE** | unit/console e2e | Actual completion owners and native fish complete real files/native commands, with evaluated conditions, wrappers, quoting/escaping and autoload. Observe inserted text/cursor and execute representative accepted results. Deterministic barriers make old work finish after a newer edit; assert stale-result rejection, cache invalidation, bounded cancellation and reader wakeup. |
| **N7-EDIT** | console e2e | Actual input drives real history/private-session search, suggestions, regex/function abbreviations, pager navigation/filter/acceptance, vi/emacs/hybrid transitions and commandline actions. Assert exact text/cursor/selection/mode/undo state and command results, including malformed definitions, callback errors and private isolation. |
| **N7-RENDER** | unit/console e2e | Owning layout/width units plus observed console/ConPTY output cover combining/emoji/wide text, resize, color/style changes, background output and terminal negotiation. Assert exact editor state and observed screen/cursor with recorded host/font/protocol conditions. Check unsupported/fragmented query fallback and ordinary redirected output without unsolicited control data. |

Coverage includes invalid definitions/options, inaccessible/changed files, partial
or cancelled work, missing optional capabilities and reader teardown with pending
callbacks. Actual evaluator/filesystem/console behavior is required: source-text
assertions, mock completions and captured echo do not establish feature parity.

## Verification and exit evidence

- Run actual native MSVC debug and release artifacts and applicable retained Unix
  completion/highlighting/abbr/history/pager/input/screen suites.
- Use native argument arrays, isolated state and bounded process/query waits;
  concurrently drain stdout/stderr and terminate/reap only test-owned work.
- Synchronize stale-result, resize and background-output races with barriers,
  never sleep-based timing. Required console/ConPTY/layout fixtures block phase
  exit if absent; capability fallback must be tested, not confused with a skip.
- CI discovers exactly the three mandatory IDs and their declared cases; missing
  IDs, zero runs and ignored mandatory cases fail the gate.
- When suites land, record exact implementation commands/test names, discovery and
  executed/nonignored counts, revision, target, Rust/SDK/toolchain, host/font/
  protocol settings, results and artifacts. No future Cargo target is asserted here.
- Retain actual runtime smoke exercising completion acceptance, search, abbreviation,
  mode changes, resize and return from a child. Human terminal/accessibility findings
  are separate evidence, not a substitute for automated console/screen assertions.

## Definition of done and handoff

All deliveries and gates pass; every advertised feature/binding consumer and
required helper has an implementation, updated docs/callers and maintained counts.
Keep prototype positives and permanent/still-deferred rejection checks. Replace
superseded interactive-feature rejections with positive/failure coverage and remove
obsolete guards/placeholders, without rewriting historical GOAL evidence.
Hand [GOAL_09](GOAL_09.md) delivered interactive assets and capability contracts;
N9 closes the remaining shipped-asset breadth after N8, not missing required
bindings here. N8 providers extend shared completion/launch policy together.
N11 audits finished consumers; no forgotten renderer, worker or binding lands there.
