# Filesystem, paths, directories, and wildcards

## Current N4 state-filesystem overlay

`fs/windows.rs` now selects the native persistence backend: all participants lock the stable
parent `.fish_lock` with LockFileEx; transactions use checked parent/target identities,
delete-sharing opens, flushed sibling staging and checked replacement/create-new. The target
DACL is copied into protected staging security before contents are written; first creation uses
parent inheritance. Owner/SACL/integrity are not copied, and this does not promise directory
metadata or SMB-server power-loss durability. Failed saves retain pending mutations; protected
crash orphan siblings are not swept with an unsafe filename-based cleanup.

`path/windows_config.rs` chooses LocalAppData/fish/{config,data,cache}, global-exported XDG root/fish
overrides, or explicit exported HOME `.config/fish`, `.local/share/fish`, `.cache/fish` fallbacks.
Install assets remain executable-relative via `env/config_paths.rs`, not under state roots.
`native_current_exe_path` handles trusted OS image spelling (including literal extended roots)
without relaxing N1 user-path validation. See the
[N4 record](coverage.md#n4-implementation-and-evidence-record) for final combined results.

## Current N1 integration boundary

The [current module/option ledger](coverage.md#current-runtime-and-native-module-inventory)
supersedes the historical audit below. N1 is implemented and verified at source revision
`c3486d848` for the [recorded development matrix](coverage.md#n1-verified-evidence-record).

Current native owners are [`path/windows.rs`](../../src/path/windows.rs),
[`wutil/windows.rs`](../../src/wutil/windows.rs),
[`wutil/dir_iter_windows.rs`](../../src/wutil/dir_iter_windows.rs),
[`wutil/fileid.rs`](../../src/wutil/fileid.rs), the shared [`wildcard.rs`](../../src/wildcard.rs) /
[`expand.rs`](../../src/expand.rs) algorithms, environment cwd state and the filesystem builtins.
`NativeCwd` is retained beside PWD in per-process environment state; updates replace both together,
snapshots clone the Arc, and physical queries use the owned directory rather than process-global
cwd. Selected native launch, discovery, traversal and predicate callers must share that policy.
Unix `path.rs` and `wutil/dir_iter.rs` are not native backends. Native persistence is now
implemented by the N4 `fs/windows.rs` overlay above, not inferred from Unix `fs.rs`.

The source contract distinguishes drive absolute/relative, current-drive-rooted, UNC and explicit
extended drive/UNC/volume namespaces. `C:x` uses fish's cwd on C: or C:'s root otherwise, not hidden
process-global `=C:` state; a rooted path uses the cwd's drive/share root. Ordinary DOS-device,
ADS and trailing-dot/space aliases are rejected; explicit extended identities preserve literal
names, require backslashes and reject dot components. Invalid identity-bearing bytes, NUL and lone
UTF-16 surrogates must fail without replacement aliases; valid supplementary/private-use names
must retain identity. This is not a reversible arbitrary-surrogate filename codec claim.

Native opens/metadata/reparse resolution, full volume/file identity, case-sensitive-directory
policy and requested-right access probes are implemented and exercised on local NTFS. Access probes
are not Unix mode bits, read-only attributes, ACL proofs or guarantees that a later operation
cannot race. Native executable classification currently enables `.exe` only, searches explicit
PATH order while ignoring empty entries, and has no discovery cache. Additional script providers
remain N8; disabled candidates must not hide a later valid executable. Physical resolution must
separate a truly missing final name from a dangling reparse or missing intermediate directory,
and preserve link-plus-`..`. Finite ancestor revisits remain legal; cycle rejection applies to
recursive descent, with complete native identities.

The [complete N1 builtin option table](coverage.md#n1-filesystemdiscovery-options) assigns ordinary
namespace/metadata/native permissions to N1, Unix-only identity/mode/device categories to N8 and
help to N9. Active completion/highlighting UX remains N7, not implied by a shared lookup service.
The [host prerequisites](coverage.md#declared-prerequisites-and-observed-host-matrix) record the
actual Windows/NTFS/loopback-share observations without widening the supported-host matrix.
Required UNC, long-path, case-sensitive, reparse and denied-access fixtures **hard-fail if missing**.
Only the [N1 evidence record](coverage.md#n1-verified-evidence-record), filled after real debug/release
gates, applicable Unix regressions and native smoke, can establish completion.

The tested loopback SMB share supports metadata/identity/cwd but returns native error `50` for
directory case information. Actual UNC glob expansion therefore fails explicitly with status `121`
and no result aliases; local NTFS support is not a blanket SMB/FAT/ReFS support claim.
An actual volume-GUID path also resolved to the same file and supported navigation/traversal.
Renaming fixtures release only their helper process's OS cwd lock before mutation, so gates
exercise the retained fish handle, old-name replacement and real deleted-directory escape.
Owned file-input bindings (`<` / `<?`, fd 0) restore NUL-input filesystem options without global
standard-handle changes; the complete descriptor/pipe/capture runtime remains N2.

## Historical scope and evidence

The original source audit covers `fs.rs`, Unix `path.rs`, every then-existing `wutil/` child and filesystem traversal/matching in `wildcard.rs`. Expansion/completion callers are discussed in [the language analysis](02-expansion-completion-highlighting.md). Windows APIs in the historical tables are proposed equivalents, not claims about current backends or newly exercised checks. Use the current overlay and [coverage ledger](coverage.md) for implementation state.

## Historical component-by-component coverage

| Component and concrete symbols | Linux/POSIX dependencies | Native Windows equivalent or gap | Validation focus |
|---|---|---|---|
| [`fs.rs`](../../src/fs.rs): `LockedFile`, `LockingMode`, `WriteMethod`, `lock_and_load` | Opens and `flock`s the **parent directory**, shared or exclusive, before opening the data file; it does not lock the replaceable data inode. Uses Unix descriptors, `O_APPEND`/`O_CREAT` and mode `0600`. `LockedFile::new` rejects directories classified as remote. | `CreateFileW` with explicit access/sharing; `LockFileEx`/`UnlockFileEx` provide byte-range locks on a stable **file**, not an equivalent `flock` on a directory. A stable dedicated lock file is one possible new protocol, not a description of current behavior. Preserve the current lock scope across files in a directory or deliberately redesign and document it for every participant. | F4, F5; concurrent readers, appenders and replacers cannot each lock a different renamed target. |
| [`fs.rs`](../../src/fs.rs): `create_temporary_file`, `rewrite_via_temporary_file`, `PotentialUpdate`, `fsync` | Creates a random sibling temporary, updates Unix owner/group and permissions, flushes/closes before rename and holds the directory lock across replacement. Lockless fallback compares file IDs and retries but explicitly permits lost writes in the final compare/rename race. `fsync` retries interrupted calls and intentionally avoids macOS `F_FULLFSYNC`. | Exclusive-create with `CreateFileW(CREATE_NEW)`, `FlushFileBuffers`, replacement/rename APIs and compatible delete-sharing. No exact Unix `fchown` or guaranteed directory-durability mapping. Preserve `PotentialUpdate::do_save`, cleanup and callback retry behavior; do not claim the lockless fallback is transactional. | F4, F5; first creation, no-save updates, crash/failure points, stale readers, original ACL preservation and cleanup. |
| [`path.rs`](../../src/path.rs): `path_get_path`, `path_try_get_path`, `path_get_paths`, `path_check_executable`, `path_get_path_core` | Slash-containing commands bypass PATH search; `access(X_OK)` plus regular-file metadata filters candidates. Unix byte paths and errno selection preserve the first interesting lookup error. | No `X_OK` equivalent for native command discovery. Decide executable extensions, explicit names, `PATHEXT` if supported, scripts/associations, case matching and current-directory search policy with the process backend. `SearchPathW` is not a drop-in replacement because its implicit search order need not match fish's explicit PATH order. | F2; name collisions, directories named like commands, explicit relative/absolute names, empty PATH entries, denied candidate then valid candidate. |
| [`path.rs`](../../src/path.rs): `path_apply_cdpath`, `path_get_cdpath`, `path_as_implicit_cd`, `path_is_valid` | Slash-rooted absolute paths, slash-terminated working directory, Unix directory/access semantics; implicit cd recognizes selected Unix prefixes. | Native root/drive/UNC parsing and directory classification. A drive-relative `C:x` is not equivalent to `C:\x`, and `\x` needs a defined current-drive policy. No single Win32 function implements fish's CDPATH or implicit-cd policy. | F1, F2; CDPATH must not rewrite fully qualified paths; implicit cd and nonexistent/denied paths must be consistent with explicit cd. |
| [`path.rs`](../../src/path.rs): `path_make_canonical`, `paths_are_equivalent`, `path_apply_working_directory`, `append_path_component`, `paths_are_same_file` | Lexical helpers strip/collapse `/`; path equivalence is not file identity. Same-file comparison uses metadata identity. | Root-aware lexical handling plus handle identity. Do not erase UNC share boundaries, extended-path prefixes, or meaningful drive-relative forms; do not make lexical case folding synonymous with identity. | F1, F3; root preservation, trailing separators, hardlinks, junctions and case-sensitive directories. |
| [`path.rs`](../../src/path.rs): `BaseDirectory`, `make_base_directory`, `create_dir`, `path_get_config/data/cache`, `path_remoteness` | Global-exported XDG/HOME variables, Unix suffixes, `DirBuilderExt::mode(0700)`. Linux/Cygwin `statfs` magic table classifies known remote/FUSE types as `Remote`, otherwise **`Unknown`**, not automatically `Local`. NetBSD uses `statvfs`; other supported Unix systems use `MNT_LOCAL`; illumos returns `Unknown`. | Known folders and ACL-aware directory creation, while honoring chosen XDG override policy; `GetDriveTypeW` plus handle remote-protocol/volume information for remoteness. Remoteness affects locking, not just performance. Do not reuse Linux filesystem magic values or equate unknown with safe/local. | F5; local, UNC/SMB, redirected profiles, explicit XDG overrides and creation-denied errors. Directory startup policy is also in [08](08-environment-and-universal-variables.md). |
| [`wutil/mod.rs`](../../src/wutil/mod.rs): `wopendir`, `wstat`, `lwstat`, `fstat`, `waccess`, `wunlink`, `wgetcwd`, `wreadlink` | Unix OS-string conversion; `DIR*`, `access`, `AsRawFd`/`BorrowedFdFile`; metadata versus symlink metadata, current directory, unlink and link-target reads. | `FindFirstFileExW`/`FindNextFileW`, native Rust filesystem APIs or `CreateFileW` plus handle metadata, `GetCurrentDirectoryW`, `DeleteFileW`/directory removal and reparse-point handling. `GetFileAttributesExW` alone does not expose complete identity or all link semantics; opening a reparse point differs from opening its target. | F1, F3; distinguish symlink/junction/directory, broken targets, denied access and deleted cwd. |
| [`wutil/mod.rs`](../../src/wutil/mod.rs): `wrealpath`, `normalize_path`, `path_normalize_for_cd`, `wdirname`, `wbasename` | Slash/byte-oriented realpath and lexical algorithms. `wrealpath` may canonicalize the parent and append a missing final component; `path_normalize_for_cd` allows navigating out of a deleted working directory without allowing arbitrary nonexistent traversal. | Native `Path` prefix handling and `GetFinalPathNameByHandleW` for an opened target are building blocks, not equivalent algorithms. `GetFullPathNameW` is lexical and does not resolve links or verify existence. Preserve logical versus physical path semantics and the missing-last-component contract. | F1, F3; missing last vs missing intermediate component, link+`..`, root `..`, deleted cwd, UNC and extended paths. |
| [`wutil/mod.rs`](../../src/wutil/mod.rs): `unescape_bytes_and_write_to_fd`, `fish_is_pua`, `fish_iswalnum`, `wstr_offset_in` | Output performs `write` with partial-write/EINTR/EPIPE handling, `SigChecker`, Unix byte unescaping. Character classification and offset calculation have no direct Linux syscall dependency, but deliberately exclude fish-reserved/private-use codepoints. | Native output sink and cancellation/error mapping; retain byte-oriented redirected output distinct from UTF-16 path conversion. No Win32 replacement needed for string offsets/classification, but valid native private-use filename characters must not become Unix byte escapes. | F1, F6; byte output round-trip, broken pipe/cancellation, supplementary and private-use characters. |
| [`wutil/dir_iter.rs`](../../src/wutil/dir_iter.rs): `DirIter`, `DirEntry`, `DirFd`, `DirEntryType`, `do_stat`, `rewind`, `new_with_dots` | `opendir`/`readdir`/`closedir`/`rewinddir`, `dirfd`, `dirent.d_type`, `fstatat` following links, inode/device fields and mode classification including FIFO/socket/devices. BSD uses `d_fileno`; illumos lacks `d_type`. `next` reuses one entry buffer. | Enumerate native names with `FindFirstFileExW`/`FindNextFileW`/`FindClose` or native Rust iteration, with lazy target metadata and stable identity. No direct `rewinddir`/`fstatat` Win32 equivalent; reopen enumeration and handle-relative/path resolution need a race policy. Map reparse tags deliberately instead of treating every reparse point as a symlink; not every Unix file class has an ordinary Windows-directory-entry counterpart. | F3; dot-entry policy, links, unknown types, disappearing entries, iteration error vs EOF and restart. |
| [`wutil/fileid.rs`](../../src/wutil/fileid.rs): `DevInode`, `FileId::from_md`, `file_id_for_file/path/path_narrow` | Unix `MetadataExt`: device, inode, size, ctime/mtime seconds/nanoseconds; `OsStr::from_bytes`. `DevInode` detects cycles; richer `FileId` detects file changes/ABA. | `GetFileInformationByHandleEx(FileIdInfo)` returns `FILE_ID_INFO` (volume serial and 128-bit file ID); size and `FILE_BASIC_INFO` last-write/change time can enrich a generation. Creation time is not Unix ctime. Availability/stability on network or unusual volumes needs an explicit error/fallback policy; omitting identity without changing cycle detection is unsafe. | F3, F4; hardlinks, rapid replacement, equal-size updates, ID reuse and unsupported volume metadata. |
| [`wutil/errors.rs`](../../src/wutil/errors.rs): `Error::{Overflow,Empty,InvalidChar}` | None directly; numeric parse errors, not Unix errno. | Keep shared; do not substitute Win32 error codes. | F6; retain distinct failure categories. |
| [`wutil/wcstoi.rs`](../../src/wutil/wcstoi.rs): `Options`, `wcstoi`, `wcstoi_opts`, `wcstoi_partial`, `fish_wcstol_radix`, `fish_wcstoul` | None directly; Rust character parsing, radix/sign/range/consumption policies. The names describe semantics, not libc calls. | No Windows replacement needed; CRT `long` is not a suitable cross-platform width assumption. | F6; integer limits, base prefixes, negative unsigned input, whitespace and trailing characters. |
| [`wutil/wcstod.rs`](../../src/wutil/wcstod.rs): `wcstod`, `wcstod_underscores`, `parse_inf_nan`, `is_hex_float` | None directly; explicit decimal separator and shared hexadecimal parser, Rust float conversion. | Keep shared instead of changing accepted syntax to `_wcstod`/CRT locale syntax. | F6; decimal separator, exponents, inf/nan, underscore consumption and overflow. |
| [`wutil/hex_float.rs`](../../src/wutil/hex_float.rs): `parse_hex_float`, `SyntaxError` | None directly; character parsing and binary exponent/rounding logic. | No native API replacement needed. | F6; subnormals, rounding, extreme exponents and malformed hex syntax. |
| [`wutil/printf.rs`](../../src/wutil/printf.rs): `sprintf!`, `fprintf!`, `printf!`, `eprintf!` | `sprintf!` delegates to `fish_printf`; fd output macros route through `unescape_bytes_and_write_to_fd` and Unix standard-fd constants. | Keep formatter semantics; port only output handle/cancellation/encoding boundary. `WriteConsoleW` is not a replacement for arbitrary byte/file output. | F6; formatted output redirected to files/pipes and console text. |
| [`wildcard.rs`](../../src/wildcard.rs): `wildcard_match`, `wildcard_has`, `wildcard_complete`, `wildcard_complete_internal` | Pure matching has no syscall requirement. Fish has its own wildcard tokens, leading-dot rule, fuzzy/completion policy; `**` completion is intentionally not implemented by these entry points. | Keep fish matching rather than replacing it with Win32 `FindFirstFileW` pattern matching. File existence case behavior and fish's glob/fuzzy matching policy are separate decisions. | F2, F3; literal case, fuzzy case, hidden-dot files, escaped wildcards and the existing `**` completion restriction. |
| [`wildcard.rs`](../../src/wildcard.rs): `expander::WildCardExpander`, `ParentInfo`, `wildcard_expand_string`, `wildcard_test_flags_then_complete`, `file_get_desc` | Slash-segment traversal, `DirIter`, visited `HashSet<DevInode>` for recursive cycles, metadata and `access(X_OK)`, link/type descriptions. Includes a WSL-only `.dll` executable-completion exclusion and Cygwin executable-suffix handling; these are not native Windows lookup rules. | Root-aware native traversal, reparse/identity backend and executable-classification policy shared with command lookup. Preserve cancellation, completion result limits, inaccessible-branch behavior and directory suffix display. Do not transplant WSL/Cygwin exceptions as a native implementation. | F2, F3; recursive junction cycles, denied/disappearing branches, cancellation and overflow without unbounded traversal. |

## Path representation and API limits

At the historical baseline, helpers repeatedly used `wcs2zstring`, `OsStr::from_bytes`, `OsString::from_vec`, Unix `MetadataExt` and slash tests. Replacing a few libc calls was insufficient. The N1 contract now requires checked native UTF-16/Windows `OsStr` boundaries: preserve valid supplementary/private-use identities and reject lone surrogates unless a separately approved/tested reversible codec replaces rejection. Do not apply the Unix private-use byte codec to legitimate native filename characters.

The common path policy must distinguish drive-absolute (`C:\x`), drive-relative (`C:x`), rooted-current-drive (`\x`), UNC (`\\server\share\x`) and extended (`\\?\...`) paths. Win32 accepts `/` in many ordinary paths but not uniformly in extended namespaces. Fish's backslash escaping remains language syntax. Device names, alternate data streams, trailing dots/spaces and extended-length paths require explicit native semantics, not global string replacement.

For directory handles use `CreateFileW` with `FILE_FLAG_BACKUP_SEMANTICS`; `FILE_FLAG_OPEN_REPARSE_POINT` requests the link/reparse object rather than normal traversal. Neither flag supplies POSIX `openat`/`fchdir` semantics. Linux `O_PATH` and fd-relative directory operations belong to the descriptor/cwd boundary in the process analysis; their availability must not be inferred from portable-looking helpers here.

For access, `AccessCheck` evaluates a security descriptor against a token, but is not a guarantee that a subsequent open will succeed (sharing, reparse traversal and privileges also matter). An actual open is authoritative for that requested operation; it is not an interchangeable implementation of every `-r/-w/-x` query. ACLs cannot be reduced to owner/group/mode bits or read-only attributes.

## Synchronized state replacement

A Windows protocol must cover opening, stable lock identity, temporary creation, metadata/security, flushing, replacement, notifications and sharing **together**. Current directory locking serializes directory participants even across replacement. `LockFileEx` cannot simply be applied to a renamed data target, and replacing it with a per-target lock changes that scope. All native fish readers/writers must agree on the chosen stable lock object and location.

`ReplaceFileW` is relevant when a target already exists; `MoveFileExW` or supported handle-based rename facilities cover other cases, with different replace/security/sharing behavior. Do not promise Unix rename atomicity/durability for every volume or cross-volume move. Request compatible `FILE_SHARE_READ`, `FILE_SHARE_WRITE` and especially `FILE_SHARE_DELETE` where required; an uncooperative open may still prevent replacement. Map failure without silently accepting an unsaved update. This document does not prescribe new retry behavior: current rename failures are returned, not retried.

Linux/Android specialize `rewrite_via_temporary_file::update_metadata` with `clock_gettime(CLOCK_REALTIME)` and `futimens` to obtain a higher precision mtime and reduce inode-reuse ABA collisions. Windows `SetFileTime` is not, by itself, proof of a unique generation; file identity and actual volume timestamp behavior still matter. Current source also preserves Unix ownership and permissions and cleans unsaved temporaries. A Windows replacement needs an ACL/security-descriptor policy, not a numeric UID/mode imitation. `FlushFileBuffers` concerns file contents; parent-directory rename durability has no general one-line POSIX mapping.

## Windows validation scenarios

These are historical proposed scenarios, not executed results. The mandatory current gates are in [GOAL_01.md](GOAL_01.md); they supersede any historical expectation that every native UTF-16 spelling can round-trip.

- **F1 — Names and roots:** preserve valid BMP, supplementary/private-use identities and reject invalid identity bytes/NUL/lone surrogates without aliases; test spaces/backslashes, drive/UNC/extended roots, reserved names, ADS and trailing-dot/space acceptance/rejection according to the declared namespace policy. Compare lexical normalization, display and actual opens without corrupting roots.
- **F2 — Resolution and matching:** compare explicit command lookup, PATH completion, glob expansion, CDPATH and implicit cd against one agreed native policy. Include case-sensitive directories, `.exe`/script/name collisions, empty path elements and denied candidates. Never let an API's default search order add unintended current-directory execution.
- **F3 — Traversal and identity:** cover regular files, hardlinks, symlinks, broken links, junctions/mount points and unknown reparse tags; create a recursive junction cycle, delete entries during iteration, deny a branch and cancel a large `**` walk. Identity failure must not silently disable cycle protection. Verify missing-final-component canonicalization and logical vs physical `..` behavior.
- **F4 — Concurrent state:** run independent writers/readers in the same directory, mixing append and replacement as actual callers require. Exercise no-save updates, equal-size rapid rewrites and newly created targets; inject failures before/after flush and replacement. Check content consistency, metadata/security preservation, stable lock identity, export-generation rereads and temporary cleanup; distinguish documented lockless limitations from guarantees.
- **F5 — Volume/security behavior:** repeat state access on NTFS, a supported alternate local volume and SMB with differing sharing/lock support; test read-only/denied directories, redirected profiles and a reader opened without delete sharing. Verify remoteness classification changes locking intentionally and failures remain visible rather than being reported as saved state.
- **F6 — Non-filesystem utilities:** preserve numeric-parser and formatting tests' semantic expectations on Windows: integer widths/overflow, hex-float rounding, locale decimal separators, trailing text/underscore consumption and error variants. Exercise fd/handle output under partial writes, broken pipe and cancellation, separately from UTF-16 filename conversion.

## Classification

Path/metadata/iteration/identity/synchronized-state operations are foundational POSIX-backed components; Linux filesystem magic classification and high-precision mtime handling are specializations. String/numeric parsing and fish wildcard matching are portable algorithms with native path-policy dependencies at their edges. Windows requires a coherent backend and root/encoding policy, not a claim that `std::fs` or one enumeration API makes the existing Unix wrappers portable unchanged.
