# GOAL_01 — Native filesystem, cwd, discovery and globs

**Phase:** N1. **Status: implemented and verified for the recorded development matrix.**
The three mandatory gates are implemented, discoverable and passed in native debug/release harnesses.
See [source revision, artifacts, counts, commands and volume limitations](coverage.md#n1-verified-evidence-record).
The primary target is native `x86_64-pc-windows-msvc`; ARM64 requires a separate architecture track.

## Objective and completion boundary

Make native fish navigate, identify, discover and expand real Windows filesystem objects consistently.
A user must be able to change directories, resolve names, find native executables through PATH and
expand filesystem globs without lossy names, accidental current-directory execution or invented Unix metadata.
Completion enables [N2 streaming I/O](GOAL_02.md) to resolve redirects and child cwd through one native policy.
This is a filesystem/discovery delivery, not script-provider dispatch or complete interactive completion.

## Entry conditions and dependencies

- Verify the completed proof-of-concept contract and evidence in [GOAL.md](GOAL.md) against the integration revision.
  Its presence in a checkout is not proof that its prerequisites are implemented there.
- Preserve actual native foreground launch, checked text boundaries, environment export and inherited stream behavior.
- Work on suggested branch `platform/windows/paths-discovery`, with one integration owner for shared path consumers.
- Before closing N1, record supported Windows versions, filesystems, terminal hosts, minimum Rust/Windows SDK
  and release feature configurations. Do not infer a supported-host matrix from ConPTY availability alone.
- Use approved roadmap revision `2acbbad4b` as the planning baseline; historical audit counts remain historical.

## Owning components and native invariants

Owners are `src/path/windows.rs`, `src/wutil/`, `src/fs.rs`, `src/env/`, `src/expand.rs`, `src/wildcard.rs`
and filesystem portions of `cd`, `pwd`, `path`, `realpath`, `test`, `command` and `type`.
Checked identity codecs and every consumer of executable classification share this boundary.
See [filesystem/path analysis](07-filesystem-paths-wildcards.md),
[expansion/completion/highlighting analysis](02-expansion-completion-highlighting.md),
[environment analysis](08-environment-and-universal-variables.md), [builtin analysis](10-builtins.md)
and [Windows API reference](windows-api-reference.md).

- Keep fish source quoting and case-sensitive local variables; Windows path syntax is not another command language.
- Keep fish text/byte provenance distinct from native UTF-16 identities. Reject NUL, invalid identity-bearing bytes
  and lone UTF-16 surrogates unless a separately approved and tested reversible native-name codec replaces rejection.
  Valid supplementary/private-use characters must not become Unix byte escapes or replacement aliases.
- Lexical equivalence, actual file identity and executable classification are different operations.
  Never lowercase a whole path or treat a normalized spelling as proof that two objects are identical.
- Prefer Rust filesystem APIs when they express the contract; use Win32 metadata/reparse APIs where required.
  Return native errors with their domain intact, not guessed errno, UID/GID or mode bits.

## Implementation delivery

1. **Specify and implement namespace-aware resolution.**
   - Cover relative paths, drive roots, drive-absolute and drive-relative paths, rooted-current-drive paths,
     UNC shares, long/extended paths and supported reparse points.
   - Document resolution of `C:x` separately from `C:\x`, current-drive roots, root `..`, trailing separators
     and extended namespace restrictions. Do not blindly prepend `\\?\` or erase UNC share boundaries.
   - State behavior for device-like names, alternate data streams, trailing dots/spaces and unsupported namespaces.
     Explicit failure must preserve identity; shell virtual devices must not hijack ordinary filesystem names.
   - Separate lexical normalization from opening/resolving targets; preserve missing-final-component behavior
     without accepting a missing intermediate directory as an existing path.
2. **Make cwd authoritative and race-safe.**
   - Implement logical and physical `cd`/`pwd`, CDPATH and the existing implicit-cd policy consistently.
     Fully qualified paths must not be rewritten by CDPATH.
   - Resolve relative opens, expansion and child launch against the same fish cwd and update `$PWD` coherently.
   - Define renamed/deleted cwd behavior, link-plus-`..` traversal, failed `cd` rollback and escape from deleted cwd.
   - Avoid process-global cwd races; isolate/serialize unavoidable mutation and never call a directory handle `fchdir`.
3. **Unify discovery and executable classification.**
   - Establish explicit PATH order, empty-entry/current-directory policy, explicit-path bypass, extension/provider
     precedence, case matching and diagnostics. Do not inherit a Win32 API's implicit cwd search order.
   - Share the policy between launch, `command`, `type`, executable predicates, highlighting and completion consumers;
     future editor integration consumes this service rather than adding another search algorithm.
   - Enable only native executable providers now. A disabled `.cmd`/`.bat`/PowerShell candidate must neither execute
     nor hide a later valid `.exe`. [N8](GOAL_08.md) adds providers without replacing these search rules.
   - Define cache keys and invalidation for PATH/export edits, cwd changes and filesystem replacement.
     Lookups and actual launches must agree, including denied/missing candidates and directories named like commands.
4. **Restore traversal, metadata and glob semantics.**
   - Reuse fish wildcard matching, hidden-file rules, recursive traversal and no-match behavior; Win32 wildcard
     enumeration is not a substitute for fish matching or portable expansion behavior.
   - Respect per-directory case policy and distinguish symlinks, junctions, broken links and unknown reparse tags.
   - Use real volume/file identity for same-file queries and recursive cycle detection, including hardlinks.
     Identity failures must not silently disable loop protection; document supported-volume failure behavior.
   - Preserve cancellation and expansion/result limits, inaccessible-branch behavior and cleanup of enumeration handles.
     Changing entries during traversal must not cause stale-resource use or unbounded traversal.
5. **Preserve environment and identity boundaries.**
   - Retain PATH list import/export and case-collision rules for native environment export without collapsing fish variables.
   - Implement filesystem permission/identity predicates with actual native meaning or explicit unavailability.
     An ACL query, file attribute and successful open are not interchangeable permission proofs.
   - Keep platform-specific options visibly assigned to [N8](GOAL_08.md); ordinary portable filesystem operations stay here.
6. **Refresh coverage and ownership documentation.**
   - Update the [coverage ledger](coverage.md) for current native modules, crates and assets.
   - Inventory every unsupported builtin/subcommand/option and every executable CLI option in fish, fish_indent
     and fish_key_reader; name an owning phase, required native semantics and current supported/rejected state.
   - Keep old counts as historical evidence, not a current coverage proof. N1 owns the ledger, not implementing
     N3 execution/formatter options, N4 startup flags, N5 accounting, N6 interactive/key-reader flags or N9 help.

## Permanently out-of-scope contracts

- **POSIX `fork()`:** no faithful snapshot/return-twice implementation, fork emulation or post-fork Windows backend.
  Reuse the evaluator and create children explicitly; never return fake fork success.
- **POSIX `exec`/`execve`:** no same-PID process-image replacement. Keep the `exec` decorator nonzero unsupported
  before argument side effects or child launch. Spawn/wait/exit and CRT `_exec` are not replacements.
- **POSIX `fg`/`bg`:** no terminal-pgroup foreground transfer, stop/continue or controlling-terminal semantics.
  Keep these reserved commands unsupported, with no same-named external fallback; do not substitute
  SuspendThread/ResumeThread or window focus. Native `&`, jobs, wait and disown belong to [N5](GOAL_05.md)
  and are not these excluded contracts. Preserve reserved-command rejection without external fallback for `exec` too.

## Mandatory automated tests

Exactly these three required gate IDs must be discoverable and nonignored; subcases expand their coverage.

| ID | Type | Setup, actions and observable assertions |
|---|---|---|
| **N1-PATHS** | Integration/e2e | Create real Unicode/space trees, drive roots and drive-relative fixtures, a supported UNC share and long/extended paths. Resolve/open intended objects and compare identity rather than one slash spelling. Exercise logical/physical cd, CDPATH, link-plus-`..`, hardlinks, junction cycles, case-sensitive directories, renamed/deleted cwd, missing leaf versus missing parent and access denied. Run actual native fish for every N1-owned `path`, `realpath` and `test` option: cover same-file `test -ef`, timestamp comparisons, `path mtime` native-to-shell time conversion, logical/physical and no-symlink realpath options, missing/denied inputs and the declared native permission/unavailability policy. Map assertions to the option ledger so unsupported dispatch or unported Unix metadata cannot pass on service-only tests. Assert exact output/status, cwd/PWD and child cwd agreement, failure rollback, bounded cycle handling and native errors. |
| **N1-DISCOVERY** | E2e | Launch actual native fish with isolated PATH/environment and marker executables in competing directories/extensions. Assert PATH/extension precedence, explicit-path behavior, no accidental cwd search, disabled script providers, case policy, export changes and cache invalidation after replacement/cwd changes. Compare command/type/predicate lookup with the actual executed marker; a rejected candidate's marker must remain absent, including an earlier disabled script before a valid exe. |
| **N1-GLOB** | Unit/integration | Exercise actual matching/codec owners and real directory traversal for recursive/hidden globs, no-match policy, escaping, Unicode/private-use/reserved names, per-directory case, link loops and inaccessible branches. Synchronize cancellation during a large walk and enforce expansion limits. Assert exact results/status, released resources and no replacement aliases for invalid bytes, NUL, lone surrogates or selected unsupported native names. |

Unit assertions must exercise production parsing/matching/codec owners; integration must use actual filesystem objects.
E2e assertions must run native fish and real executable fixtures, not copied lookup logic or source-text checks.
Required UNC, long-path, case-sensitive, reparse and denied-access fixtures must be provisioned explicitly;
unavailable required infrastructure blocks the phase instead of silently skipping a subcase.

## Verification and execution requirements

- Run actual native MSVC debug and release artifacts, retain applicable Unix suites for changed shared code,
  and record a real fish navigation/discovery/glob smoke in addition to the automated gates.
- Launch with argument arrays, isolated cwd/environment/user-state roots and no host-shell interpolation.
  Serialize process-global mutations or move them into isolated children.
- Bound every wait, concurrently drain child stdout/stderr and terminate/reap only owned work on failure.
  Coordinate deletion, replacement and cancellation races with barriers/events, not sleeps.
- When implementing the suites, record exact test names/commands, toolchain/SDK, target, revision, expected
  discovery and executed/nonignored counts, results and debug/release artifact references. No future command is claimed here.
- CI must fail on a missing required ID, empty run, hidden skip or unmet required fixture, not only test failure.
- Preserve positive prototype regressions and permanent/still-deferred rejection coverage. Replace obsolete path/glob
  rejection cases with meaningful positive and failure tests as support lands; update CI registry/counts accordingly.
  Do not alter the historical evidence in GOAL.md.

## Definition of done and handoff

Every delivery above and all three gates pass with recorded native debug/release evidence and applicable Unix results.
Coverage/CLI ownership documentation and affected callers are updated; obsolete path guards and placeholder paths are removed.
All permanent exclusions remain enforced before forbidden side effects; no unsupported service reports fake success.
Hand [N2](GOAL_02.md) one stable path/cwd/open/classification contract, identity/error policy and supported-system matrix.
[N3](GOAL_03.md) reuses these predicates and quoting rules; [N4](GOAL_04.md) owns persistent state directories/protocols,
[N7](GOAL_07.md) owns active completion/highlighting UX, and [N8](GOAL_08.md) owns additional script/platform providers.
Those boundaries must not defer any N1 navigation, native discovery or glob behavior required above.
