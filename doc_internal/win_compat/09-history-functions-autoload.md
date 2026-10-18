# History, functions, abbreviations, and autoloading

## Current N4 implementation overlay

`history/{mod,file,history,yaml_backend}.rs`, `fs/windows.rs` and `reader/batch_windows.rs` implement
native named/private/default histories and commands, now consumed by the N6 shared reader too.
Full interactive search/suggestion/session parity remains N7, not a storage-only claim.
Windows uses immutable owned byte snapshots and publishes complete flushed transactions rather
than partial appends. Existing fish YAML-compatible records, multiline/Unicode content and
real wall-clock timestamps are retained; Fish 1.x remains an existing unsupported decoder.

An initial owned session baseline survives automatic saves and watcher invalidation. Checked
explicit merge captures the resulting snapshot inside the persistence transaction before retiring
local entries. Clear-session uses a committed-generation journal to undo only acknowledged own
versions still on disk and restore the predecessor observed during their save, preserving foreign
updates and deletions even after duplicate compaction or a failed save. Same-command/same-second
records cannot uniquely identify independent authors. Directory notifications mark snapshots stale,
not automatically visible as another session's commands. Private selection precedes loading old
disk history; shutdown flushes dirty history. Active search UI remains N7.
NLS/UCRT locale and Windows historical-DST timezone handling support native history formatting;
unsupported GNU formats/IANA/POSIX TZ inputs diagnose, with full parity still N9.

Full core ordered disk/embedded autoload, path/cwd/identity invalidation, overrides/tombstones,
recursion protection and embedded provenance now work. Public `funcsave` uses native directory
creation/removal/atomic writes through `builtins/native_fs.rs` without coreutils. This does not
port external-editor `funced`, every shipped script, complete -C or editor abbreviation expansion
(N9/N7). The validator-only limit below describes historical N3, not current support.
See the [N4 record](coverage.md#n4-implementation-and-evidence-record)
for final combined results; original proposed scenarios remain historical.

## Verified N3 implementation

Native functions now execute with shared argument/inherited-variable scopes, recursion, return,
copy/erase metadata and tombstones. Generic/variable handlers and blocked/queued events run on
the main evaluator thread. In-memory abbreviation and completion declarations survive serialization
and replay with their semantic fields intact; legacy serializer line order is not rule identity.
At the N3 cutoff only embedded `_validate_int` autoloaded. General disk/embedded autoload,
public function saving and durable history/universal state were then N4 prerequisites, now delivered above.
See [N3](GOAL_03.md) and its [coverage record](coverage.md#n3-verified-evidence-record).

## Historical scope and evidence

The source families are [`history/`](../../src/history), [`function.rs`](../../src/function.rs), [`abbrs.rs`](../../src/abbrs.rs), and [`autoload.rs`](../../src/autoload.rs), including their persistence, reader and shipped-script call sites. The following are source observations and proposed native Windows mappings, not build/test results. See the [coverage ledger](coverage.md), [filesystem services](07-filesystem-paths-wildcards.md), [universal variables](08-environment-and-universal-variables.md), and [shipped assets](13-binaries-and-assets.md).

## History components

[`history/mod.rs`](../../src/history/mod.rs) declares the model, file and YAML-like codec children and reexports history symbols. Module wiring has no Linux syscall and requires no Windows replacement; the children below own the persistence boundary.

| Component and concrete source evidence | Linux/POSIX dependency, or none | Native Windows equivalent and semantic gap | Proposed validation |
|---|---|---|---|
| [In-memory history/search](../../src/history/history.rs): `History`, `HistoryImpl`, `HistoryItem`, `HistorySearch`, `PersistenceMode`, `history_id` | Deduplication, searches, pending items, timestamps, disk/memory/ephemeral policy and session naming are portable logic. File path construction uses the configured data directory and `name + "_history"`. | Keep the model, names, search order and private-session policy. Use the common native data-directory/path service, not an unrelated Windows history store. | Named/default/private histories, empty history name, newest/oldest and case-sensitive searches, duplicate command merge and ephemeral removal. |
| [On-disk codec](../../src/history/yaml_backend.rs): `decode_item_fish_2_0`, `offset_of_next_item_fish_2_0`, escaping; [writer](../../src/history/file.rs): `HistoryItem::write_to` | The fish-specific YAML-like byte format and byte offsets have no direct Linux API requirement. It is not a general YAML parser. The current writer records first/last-added timestamps and required paths. `RawHistoryFile` rejects the old 1.x format. | Preserve byte encoding/escaping and offset meaning; do not let Windows text-mode newline conversion rewrite bytes underneath offsets. UTF-16 is for native paths/API calls, not a replacement history-file format. | Existing history files, multiline commands/backslashes, non-BMP text, truncated/corrupt tail records, old 1.x rejection, and timestamp/path round trips. |
| [Read/mapping layer](../../src/history/file.rs): `MmapRegion`, `RawHistoryFile::create`, `should_mmap` | `nix::sys::mman::{mmap,mmap_anonymous,munmap}`. Non-remote history can use a file mapping; remote data directories and `ENODEV` use anonymous mapping plus `read_exact`. Thus the current fallback still depends on Unix mapping APIs. | `CreateFileMappingW`/`MapViewOfFile` plus native handle ownership; anonymous allocation or an owned byte buffer can implement the copied snapshot. Preserve lazy decode/byte offsets. Unlike Unix replacement, Windows sharing rules constrain a mapped/open target. | Empty file, unsupported mappings, remote history, file truncation/replacement while another reader holds a snapshot, mapping lifetime, and files larger than addressable storage. |
| [Concurrent storage](../../src/fs.rs): `LockedFile::new`, `lock_and_load`, `rewrite_via_temporary_file`; [history save](../../src/history/history.rs): append and vacuum paths | **The lock is on the parent directory**, not on the replaceable history data file. It uses `flock`, close-on-exec opens, `fsync`, a sibling temporary and rename. Metadata preservation uses ownership/mode; Linux/Android additionally refresh timestamps to mitigate inode reuse. Remote-directory locking is disabled; fallback rewrite detects some identity races but explicitly can lose writes. | `LockFileEx` on a stable lock file can replace directory `flock`; there is no equivalent directory byte-range lock with the same contract. `FlushFileBuffers`, `ReplaceFileW`/`MoveFileExW`, delete-sharing opens, native file IDs and ACL-aware ownership policy are needed. Coordinate the shared protocol with universal variables, rather than inventing a separate history-only transaction layer. | Two independent writers appending/vacuuming/deleting; crash during rewrite; read-only directory; target absent/present; sharing denial; stale identity; network share and lock-unavailable behavior. |
| [Pending entries/required-path hints](../../src/history/history.rs): `add_pending_with_file_detection`, `expand_and_detect_paths`, `resolve_pending` | AST inspection is portable. Background expansion/filesystem existence detection uses an environment snapshot/thread pool and defers saving; exit/exec-like commands trigger synchronous saving. | Existing Rust worker scheduling can remain, but native path/access queries must not block parser state locks or lose pending saves on exit. Required paths need the same path semantics as autosuggestions. | Submit a path-containing command then immediately exit; deleted or renamed path; drive/UNC paths; cancelled/slow network detection; pending entries absent until resolved. |
| [Output/time formatting](../../src/history/history.rs): `format_history_record`; [history shell wrapper](../../share/functions/history.fish) | Search formatting can call `libc::localtime_r` and `strftime`; output may colorize or use NUL separators. The wrapper delegates core operations to builtin history and may select a pager. | A compatible local-time/formatting service is needed; Win32 `GetDateFormatEx`/`GetTimeFormatEx` do not directly implement `strftime` format strings. Preserve NUL and multiline output through native streams; external pagers remain optional utilities. | Time-zone/DST boundaries, custom time formats, NUL output, color/pager disabled, and output broken by a consumer closing its pipe. |
| [History import and reader policy](../../src/reader/reader.rs): `import_history_if_necessary`, `should_add_to_history`; [Bash import heuristic](../../src/history/history.rs): `populate_from_bash`, `should_import_bash_history_line` | Reader imports `HISTFILE` or `~/.bash_history` through Unix open wrappers; the decoder is a conservative line heuristic, not Bash parsing. Reader chooses ephemeral/private/disk modes. | Ordinary native file reads can preserve optional import; there is no requirement for Bash to exist. Do not reinterpret PowerShell PSReadLine history as Bash format automatically. Preserve history filtering/function execution semantics. | No Bash file, explicit Unicode `HISTFILE`, unsupported Bash lines, private session, rejected history command and user history-filter function. |

### Storage constraints

- Native opens of history data that may be replaced should consistently request appropriate `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`; one incompatible reader can defeat replacement. This is necessary policy, not proof that every mapping/replacement combination works.
- Native identity should combine volume/file ID with relevant size/timestamps, not just a case-folded path or mtime. Compare a newly opened target against the old snapshot to detect replacement.
- Separate durability from concurrency: `FlushFileBuffers` is a data-flush mechanism, not a transaction or directory-metadata durability guarantee.
- A stable Windows lock file is a **proposed replacement** for current directory locking. All fish writers must agree on its location and lifetime. Do not report the current race-detection fallback as equivalent to locking.
- Roaming/network storage needs an explicit policy. Changing the default to local app data would be a port design decision, not existing history behavior.

## Functions, abbreviations, and autoloading

| Component and concrete evidence | Linux/POSIX dependency, or none | Native Windows mapping and gaps | Proposed validation |
|---|---|---|---|
| [Function registry/metadata](../../src/function.rs): `FunctionProperties`, `FunctionSet`, `load`, `copy`, `remove`, `invalidate_path` | Parsed function bodies, named/inherited variables, descriptions, scope shadowing, copies and tombstones are in-memory logic under a mutex. Removing a function also removes event handlers. Definition files are `FilenameRef` values, not inherently Unix-only paths; embedded sources also exist. Disk discovery uses `DirIter` and `.fish` suffixes. | Keep the registry and fish command identity. Store/display native filenames through the shared path/escaping services, preserving `embedded:` source provenance. Event handler execution still depends on the [event backend](04-signals-events-waits.md). | Explicit definition versus autoload, `functions --copy`, inherited variables, removal/tombstones, changed function path, filename/line-number reporting for embedded and disk scripts. |
| [Function editing/saving](../../share/functions/funced.fish), [saving](../../share/functions/funcsave.fish), [aliases](../../share/functions/alias.fish) | These are separate from the portable Rust registry. Editing uses an external editor, temporary files, checksum helper, `cat`/`cp`/`rm`/`rmdir`; saving uses `mkdir`/`rm`. `funced` already understands `embedded:` definitions. | Native file/temp operations and explicit external-editor argv launching can replace utility requirements; no Windows API can transparently supply an arbitrary user's editor. Persist `.fish` definitions with fish escaping and platform-safe filenames. | Save/edit/reload a function with spaces/Unicode in its path, missing editor, editor failure, user override of embedded function, read-only destination and name collisions. |
| [Abbreviation model](../../src/abbrs.rs): `Abbreviation`, `AbbreviationSet`, `Replacer`, `Replacement` | Literal/regex matching, position/command restrictions, last-defined precedence and cursor markers are portable logic (regex engine portability is [11](11-text-regex-localization.md)). Legacy universal provenance is model data, not a native storage API here. | Same model; do not translate replacement text into another shell's quoting language. | Literal/regex overlap, command-only/anywhere restrictions, rename/erase, multibyte/non-BMP cursor markers and last-defined precedence. |
| [Function-backed abbreviation expansion](../../src/reader/reader.rs): `exec_subshell` path when `Replacer::is_function` | Replacement can execute a fish function and external commands. This is not covered by saying that abbreviation matching is pure. | Reuse evaluator/native process and capture services; no additional Win32 abbreviation API is necessary. | Successful/failed replacer function, multiline output, unchanged status as required by reader semantics, cancellation, escaping and cursor position. |
| [Autoload resolution/cache](../../src/autoload.rs): `Autoload`, `AutoloadFileCache::check`, `resolve_command_impl`, `locate_file`, `locate_asset` | Path-list ordering, in-progress recursion protection, a 15-second freshness interval and LRU misses are portable cache logic. Disk entries compare `FileId`; paths are built with `/cmd.fish`. No filesystem-change notifier is currently required by this implementation. | Native file identity/path handling; `ReadDirectoryChangesW` could be an optimization, not an existing mandatory behavior. Keep exact function names and explicitly handle case-insensitive filesystem collisions rather than lowercasing everything. | Cached miss then create file, modified/replaced file, changed search path, recursive autoload, simultaneous case variants, and miss freshness expiry. |
| [Embedded asset loading and precedence](../../src/autoload.rs): `Asset`, `AutoloadPath::{OnDisk,Embedded}`, `perform_autoload` | `rust_embed` embeds `share` (excluding the build-path template). Custom disk paths precede embedded functions/completions; generated completions are deliberately searched **after** embedded completions. On-disk loading executes `source`; embedded bytes become a named `embedded:` parsed source. Previous statuses are restored and scripts run outside autoload locks. | Retain the existing embedded mechanism; a native distribution need not copy every default function onto disk. Native disk source/open and path classification still need porting. A blanket switch to watchers or installed-only scripts would change semantics. | Clean install with no shared-function files, user/vendor override, generated-versus-embedded completion, changed disk definition, correct source provenance and status restoration on load failure. |

## Classification and limits

History search/codec, function/abbreviation models and most cache logic are **portable**. Persistence/mappings/time formatting, filesystem discovery and execution of sourced/replacer/editor scripts are **host-backed**. The original API proposals were not tests; current N4 storage/loader implementation is distinguished from N3 evidence and remaining N7/N9 consumers by the overlays above.
